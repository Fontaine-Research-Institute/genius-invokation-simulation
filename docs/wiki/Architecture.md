# 架构总览

> 中文 · [[Architecture-EN|English]]

*本页范围: 六个 crate 的依赖方向, 仓库目录, 一局对局的数据流, "框架只给接口"的落地方式, 以及已知文档偏差.*

---

## 1. crate 与依赖方向

```text
richtext（零依赖：只解析文本）
     ▲
     │
cards ──► definitions ──► engine（机制原语，默认零依赖）
  │            │
  │            └──► richtext
  │
simulation（门面）──► engine / definitions / richtext（+ cards 可选）
compute（算力 bin）──► 默认无依赖；按计算需要自己加
```

| crate (package 名)  | 目录 | 依赖 | 职责 |
|---|---|---|---|
| `gi-genius-invokation-engine` | `new/crates/engine` | 无 (仅 `dll` feature 引 `libloading`)  | 机制原语: 数值 / 标识 / 实体 / 区 / 随机与选择 / 精确分数 / 统计 |
| `gi-genius-invokation-richtext` | `new/crates/richtext` | 无 | 富文本解析: 原始标记文本 → 中性片段序列 |
| `gi-genius-invokation-definitions` | `new/crates/definitions` | engine, richtext | 游戏定义层 (牌族 / 类型 / 标签 / 资源 / 规则 / 状态 / 流程 / 关键字 / 调度 / 记录)  |
| `gi-genius-invokation-cards` | `new/crates/cards` | definitions, engine | 具体卡牌: 每张卡的数据 + 效果逻辑 |
| `gi-genius-invokation-simulation` | `new/crates/simulation` | 上列四项 (cards 可选)  | 对外唯一门面: 只有 re-export, 没有逻辑 |
| `gi-genius-invokation-compute` | `new/crates/compute` | 默认无 | 算力工具链: 一个计算 = 一个 bin 文件夹 |

> [!IMPORTANT]
> 依赖方向由 Cargo 强制: `definitions` 不依赖 `cards` (有守卫测试) ; `richtext` 不依赖任何业务 crate; `engine` 无上层语义.
> 外部使用者只依赖 `simulation` 门面; 跨 crate 导入统一为 `gi_genius_invokation_simulation::{engine, definitions, richtext}`.

## 2. 目录结构

```text
new/
├─ Cargo.toml              # workspace：members = crates/*
├─ rust-toolchain.toml     # stable（离线环境不写具体版本号）
├─ docs/wiki/              # 本 Wiki
├─ crates/{engine,richtext,definitions,cards,simulation,compute}/
├─ tools/                  # run-compute.py、三个扫描脚本、COMPUTE.md
└─ .github/workflows/      # ci.yml、compute.yml
```

`definitions` 内部: `base/` (L1 牌族 + id + 元素 + 视角) , `families/` (每牌族的类型枚举 + 定义载体 + 区域 + 操作) , `tags/` (L3 标签, 顶层目录) , `resources/` (骰池) , `rules/` (三级统一基础操作) , `timeline/` (触发注册 + 派发 + 结算帧) , `reactions/` (元素反应) , `records/` (记录 API) , `source/` (源码嵌入) , `version/` (版本同步) , `generated/` (生成数据 + 渲染) , `state/` (`DefState` + 串联流程) , `flow/` (端到端动作) , `keywords/` (关键字函数 + 一步) .详见 [[Crate-Definitions]].

## 3. 一局对局的数据流

| 层 | 位置 | 作用 |
|---|---|---|
| 状态 | `definitions/src/state/` | `DefState`: `me` / `opp` 两个具名实例 + `round` + `first_player`; 各区自持状态 (D42 / D182 / D183)  |
| 流程 | `definitions/src/flow/` | 吃 `&DefState` 的动作入口: 使用技能 / 打牌 / 切人 / 调和 / 特技 / 回合骨架; 返回新状态 + 待执行, 失败 `Err` |
| 效果组合 | `definitions/src/keywords/` | 关键字函数 + **一步 Step** (D204) : 步内按书写顺序, 禁止插入 |
| 调度 | `definitions/src/timeline/` | 注册表 + 有序派发 (只排序, 不执行, D186) + 结算帧 (插入式, D196)  |
| 基础操作 | `definitions/src/rules/` | 三级统一基础操作纯函数 (槽位 / 上限 / 开局 / 查询 / 结算顺序…)  |
| 反应 | `definitions/src/reactions/` | 反应身份 → 效果函数指针; 伤害修正链在 `families/character/damage_chain/` |
| 记录 | `definitions/src/records/` | `Recorder<T>`: 属于哪一步 + 任意内容 (D223 / D224)  |

**以"使用技能"为例** (`definitions/src/flow/skill_use.rs`) : 

1. 成对入口 `use_skill_me` / `use_skill_opp` (同一份实现, D29 / D34) ; 
2. 能否使用 → 支付 (骰子 / 充能) → 派发"使用技能前" (只排序) ; 
3. 执行效果体: `keywords::Step`, 步内按序调用关键字函数; 
4. **步末 flush**: 各区派发的待执行交给调用方执行 (`flow/pending.rs`) ; 反应子结算在步边界插入; 
5. 派发"使用技能后", 用 `timeline::Resolution::insert` 插到当前结算位置 (D196) ; 
6. 分支终局判定 (`state/terminal.rs`; 双负 = `BothLose`, D267) .

**文本数据流**: `generated/data/*.json` → `build.rs` (编译期生成静态表到 `OUT_DIR`) → `generated` 访问器 → `richtext::parse` (严格解析) → `generated::text::render` (取值来源 → 最终字符串) .

### 3.1 接口源码摘录
**根状态 `DefState`**

`new/crates/definitions/src/state/def_state.rs:58-82`

```rust
/// **一局对局的完整状态**：我方全部区 + 敌方全部区（两个具名实例，D34）+ 回合数。
///
/// 容器没有身份（D42）：它就是「这一局」这个变量本身，不出现对局号、不登记。
#[derive(Clone, Debug, PartialEq)]
pub struct DefState {
    /// 我方的全部区。
    me: PlayerDefs,
    /// 敌方的全部区。
    opp: PlayerDefs,
    /// 回合数（0 = 还没开始第一回合；每回合开始 +1；上限见
    /// `crate::state::terminal::ROUND_LIMIT`）。
    ///
    /// 依据：规则书·其他规则 对局回合上限「第15回合开始时，如果仍未分出胜负，则双方都会输掉
    /// 这场对局」—— 判定需要知道「现在是第几回合」，所以回合数由根状态持有。
    round: Nat,
    /// **本回合的先手牌手**（行动阶段 / 结束阶段的处理起始方；[`None`] = 还没裁定）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】「先『宣布回合结束』的牌手会成为
    /// 『先手牌手』，并在下回合的行动阶段优先行动」；其他规则·结算顺序第 1 条
    /// 「先结算当前牌手的所有卡牌效果」。
    ///
    /// 写入方：开局 / 加载流程（第一回合由外部裁定；之后由上一回合的先宣布者接任）。
    /// 读取方：引擎求解接口 `state::terminal::DefRules::to_move`（当前轮到谁行动）。
    first_player: Option<Perspective>,
}
```

**动作入口 `use_skill_me`**

`new/crates/definitions/src/flow/skill_use.rs:430-446`

```rust
/// **使用技能**（我方使用；下落标记读根状态）。
///
/// - 成功 -> 结算分支（确定性技能恰好一支）；settlement 会被**插入**「使用技能后」的子结算
///   （D196：插到当前 cursor 位置 = 步边界；调用方之后用 pending::execute_frames 执行完）；
/// - 失败 -> Err，state / settlement 一个字节都不改；
/// - 下落攻击标记（S8.10）：读 `state.me().plunging()` 判生效形态；战斗行动结束时按
///   `battle_action_end_me` 清除并写回根状态。需要显式传标记时用
///   [use_skill_me_with_plunging]。
pub fn use_skill_me(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
) -> Result<SkillOutcome, SkillUseError> {
    // 标记的持有区在根状态（PlayerDefs::plunging）：旧入口读使用方当前标记。
    use_skill_me_with_plunging(state, env, request, settlement, state.me().plunging())
}
```

**一步 `Step`**

`new/crates/definitions/src/keywords/step.rs:44-48`

```rust
/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}
```

**派发 `dispatch`**

`new/crates/definitions/src/timeline/dispatch.rs:60-96`

```rust
/// **单侧派发**：从一个牌手的注册表里取出「这个时机、条件成立」的登记项，按结算顺序排好。
///
/// 排序 = `结算顺序.md` §一 第 2~4 条：来源类别 -> 进场先后 -> 登记顺序（稳定排序）。
/// 第 1 条「当前牌手优先」不在本函数里：它由 [`resolve`] 用「两个注册表的先后」表达。
///
/// 只读 + 不执行：不调用任何效果函数指针，也不改注册表。
#[must_use]
pub fn dispatch<'e, K, E>(
    registry: &TriggerRegistry<K, E>,
    timing: K,
    event: &'e Event,
) -> Vec<Pending<'e, E>>
where
    K: Copy + PartialEq,
    E: Copy,
{
    // 1) 只留「时机相等 + 条件成立」的登记项。
    let mut matched: Vec<Trigger<K, E>> = registry
        .entries()
        .iter()
        .copied()
        .filter(|trigger| trigger.applies(timing, event))
        .collect();
    // 2) 按结算顺序排（稳定排序：完全同序时保持登记顺序）。
    matched.sort_by(by_resolution_order);
    // 3) 产出「函数指针 + 事件」的待执行列表（不执行）。
    matched
        .into_iter()
        .map(|trigger| Pending {
            effect: trigger.effect,
            event,
            owner: trigger.owner,
            source_kind: trigger.source_kind,
            entry: trigger.entry,
        })
        .collect()
}
```

**插入子结算 `Resolution::insert`**

`new/crates/definitions/src/timeline/suspend.rs:168-178`

```rust
    /// **插入子结算**：在当前帧当前位置压入新帧（父帧的 `cursor` 不动）。
    ///
    /// - 子结算执行完后**回到插入点**，继续父帧剩余步骤；
    /// - 不重排父帧已有步骤（不是 LIFO 触发栈）；
    /// - 空插入 = 不做任何事。
    pub fn insert(&mut self, steps: Vec<Queued<E>>) {
        if steps.is_empty() {
            return;
        }
        self.frames.push(Frame::new(steps));
    }
```

**结算快照 `Resolution::snapshot`**

`new/crates/definitions/src/timeline/suspend.rs:202-217`

```rust
    /// **只读快照**：当前结算嵌套状态（当前帧 / 剩余步骤 / 插入点）。
    ///
    /// 不改状态、不含函数指针；供打印 / 导出对照游戏结算顺序。
    pub fn snapshot(&self) -> ResolutionSnapshot {
        let current = match self.frames.len() {
            0 => None,
            len => Some(nsub!(Nat::new(len as _), Nat::new(1))),
        };
        let frames = self
            .frames
            .iter()
            .enumerate()
            .map(|(depth, frame)| frame.snapshot(Nat::new(depth as _)))
            .collect();
        ResolutionSnapshot { current, frames }
    }
```

**记录入口 `Recorder::push`**

`new/crates/definitions/src/records/mod.rs:88-91`

```rust
    /// **唯一写入入口**：把一条内容记到某一步。
    pub fn push(&mut self, step: Nat, content: T) {
        self.entries.push(Record::new(step, content));
    }
```

**折叠回 `fold_states`**

`new/crates/definitions/src/flow/mod.rs:130-147`

```rust
/// **折叠回**（D191）：把局面完全相同的分支合并，概率**精确相加**。
///
/// 观测点展开出的分支是临时的（D191：展开 / 回收成对）：一旦不再需要区分「怎么走到这个局面」，
/// 就把等价分支折叠回一支。比较口径 = DefState 的 PartialEq（各区自己的等价语义，
/// 统计不参与，D187）；顺序保持首次出现顺序（可复现）。
///
/// 注意：只比较**局面**，不比较待执行（Queued 没有 PartialEq）；需要连待执行一起折叠时，
/// 由调用方先执行完待执行再调本函数（skill_use / play_card 内部就是这么用的）。
pub fn fold_states(branches: Vec<(DefState, Fraction)>) -> Vec<(DefState, Fraction)> {
    let mut folded: Vec<(DefState, Fraction)> = Vec::new();
    for (state, probability) in branches {
        match folded.iter_mut().find(|(existing, _)| *existing == state) {
            Some(entry) => entry.1 = entry.1.exact_add(probability),
            None => folded.push((state, probability)),
        }
    }
    folded
}
```


**关键字函数: 造成伤害**

`new/crates/definitions/src/keywords/keyword.rs:241-292`

```rust
/// **造成伤害**：对选择器解析出的每个角色走伤修链（加伤 -> 护盾 -> 减伤 -> 扣血 / 附着 / 反应）。
///
/// 每次伤害都派发「生命值变动」待执行（不执行，步末统一做，D204）。
pub fn cause_damage(state: &DefState, env: &EffectEnv<'_>, args: &DamageArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    fan_out(state, env, candidates, args.pick, |current, side, target| {
        let target_state = character_state(current, side, target)?;
        let player = player_of(current, side);
        // 乘算 / 除算修饰随参数进来（卡面「此次伤害翻倍」等）；默认 NONE。
        let chain_env = DamageChainEnv::new(player.combat_status(), &target_state.states)
            .with_modifiers(args.modifiers);
        let chained = damage_with_chain(
            &target_state,
            env.subject(),
            target,
            args.amount,
            args.ty,
            &ChainDamageEnv {
                damage: &env.damage,
                chain: &chain_env,
                bonus: DamageBonus {
                    increases: env.increases,
                    mastery: env.mastery,
                    reaction_bonus: Nat::ZERO,
                    reacted: false,
                },
                context: env.effect_context,
                triggers: env.character_triggers(side),
            },
        );
        let next = with_character(current, side, target, chained.outcome.state);
        let mut pending = PendingSet::new();
        for item in chained.pending {
            push_pending(
                &mut pending,
                side,
                FlowPending::Character {
                    character: target,
                    pending: item,
                },
            );
        }
        // R4：伤害路径判出的反应不在这里执行（D204 步内不插）；记成反应子结算，
        // 由步执行器在步边界插入执行（做完回到原步骤）。
        let mut reactions = Vec::new();
        if let Some(hit) = chained.reaction {
            reactions.push(ReactionRequest::new(hit, env.viewer(), side, false));
        }
        Some((next, pending, reactions))
    })
}
```

**规则 ops: 结算顺序判定**

`new/crates/definitions/src/rules/ops/order.rs:38-59`

```rust
/// 同一时间点、同一牌手内：first 是否先于 second 结算（规则书第 2 条）。
///
/// 同一类别 -> false（同类按进场先后，规则书第 3 条，见 same_source_uses_entry_order）。
pub fn resolves_before(first: EffectSource, second: EffectSource) -> bool {
    // 同一类别不比先后（同类按进场先后，规则书第 3 条）。
    if first == second {
        return false;
    }
    let mut seen_first = false;
    for source in SOURCE_ORDER {
        if source == first {
            if seen_first {
                return false;
            }
            seen_first = true;
        }
        if source == second {
            return seen_first;
        }
    }
    false
}
```


## 4. "框架只给接口"的落地

> [!NOTE]
> 这条是最高原则 (D20; `new/README.md` 的"项目理念"; `分工计划.md` §8.5) , 压过一切具体设计.

| 功能 | 框架给的 (机制)  | 业务归谁 |
|---|---|---|
| 随机 / 选择 | `enumerate_random` / `enumerate_random_weighted` / `enumerate_select` + 精确分数 | 各块自己 `parse.rs` 的选择列表与去重函数 (D25 / D26)  |
| 触发调度 | `timeline::TriggerRegistry` + `dispatch` (只排序, 不执行)  | 牌族 / 类型定义时机键并主动派发; 卡牌登记效果函数指针 (D185 / D186)  |
| 结算插入 | `timeline::Resolution` 帧 + `cursor` | 效果层决定在哪插入什么 (D196)  |
| 区 / 状态 | 各区自持状态 + `with_*` 纯函数; `state` 只组合 | 各区自己 (D42 / D182 / D183)  |
| 三级基础操作 | `rules/` 静态规则表 + `rules::ops` 纯函数 | 具体卡牌按需调用; 不建中枢表 (D176–D179)  |
| 支付 | 骰池 / 充能 / 秘传各自的 `can_afford` / `pay` | 卡牌只声明费用数据 (D121 / D133 / D144)  |
| 文案 | `generated` 访问器 + `richtext` 解析 + `render` | 卡牌只写 `Localized` / 引用生成数据 (D56 / D62 / D75)  |
| 记录 | `records::Recorder` 极薄容器 | 各模块定义记录内容类型并 `push` (D222–D225)  |

> [!WARNING]
> 反例 (一律不许) : 中枢统计器, 中枢收集器, 内建行为表, 统一减费计算器, 属性统计表, 行为开关表, 内建 buff 表.

## 5. 时间与不确定性

| 主题 | 口径 | 依据 |
|---|---|---|
| 结算顺序 | 同一时机内以 `结算顺序.md` 为唯一权威; 顺序常量只在 `rules::ops::order` 一份 | D197; `结算顺序.md` |
| 插入式结算 | 帧 + `cursor`, 子结算做完回到插入点; 不是 LIFO 栈 | D196 |
| 一步原子 | 步内按书写顺序, 禁止插入; 插入只能发生在步之间 | D204 |
| 不确定性 | 不确定先不物化; 观测时展开成"分支 + 精确概率", 不再影响别处后折叠回 | D190 / D191 |
| 零浮点 | 概率 / 权重 / 随机全部整数 (`Fraction` = `ExactNat128` 分子分母; `Rng` 对外只给 `u128`)  | D158–D160 |

## 6. 已知文档偏差

> [!WARNING]
> 以下旧文档与当前代码 / 决策不一致.本 Wiki 按代码写; 旧文档待维护者更新.

| 旧文档 | 偏差 |
|---|---|
| `new/README.md` / `README.en.md` | 骰子"每颗 = 实体", 结算顺序"编译期解出", 类型树继承, dll / 两阶段加载, "core / engine / definitions 三个 crate", `Rc` 结构共享 |
| `结构.md` / `分工计划.md` | 旧目录树与旧签名: f64 概率, 旧 `DefState` 字段, base 里的 `CardType` / `CardTag`, "卡牌代码不存文案" |
| `definitions/src/lib.rs` | 模块文档仍说 resources / timeline / loader 后续加入; 实际已存在 |
| `definitions/src/families/mod.rs` | 仍说 families'只放类型枚举'; 实际还有定义载体 / 区域 / 操作 |
| `definitions/src/records/mod.rs` | 仍说 damage_chain / 各区记录"待接入"; 实际多个 `record.rs` 已用 `Recorder` |
| `definitions/build.rs` | 手写 JSON 解析器 (离线零依赖) , 与 D74'改用 JSON 库'冲突 |

## 7. 相关页面

| 页面 | 内容 |
|---|---|
| [[Conventions]] | 设计约定速查 |
| [[Crate-Engine]] / [[Crate-Richtext]] / [[Crate-Definitions]] / [[Crate-Cards]] / [[Crate-Simulation]] / [[Crate-Compute]] | 各 crate 详情 |
| [[定义层-State]] / [[定义层-Flow]] / [[定义层-Keywords]] / [[定义层-Timeline]] | 运行时各层 |

---

最后核对: 2026-10-19 (代码: `new/crates/definitions/src/lib.rs:pub mod`) 
