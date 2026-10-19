# Crate-Definitions（游戏定义层）

> 中文 · [[Crate-Definitions-EN|English]]

*本页范围：`definitions` crate 的职责、目录与概念、公开入口与 feature、不变量、依据、测试规模与待接入项。*

---

## 1. 职责

承载「除具体卡牌以外的全部游戏定义」：牌族 / 类型 / 标签、被花资源（骰池）、三级统一基础操作、状态与串联流程、关键字与「一步」、触发调度、元素反应、统一记录、编译期源码嵌入、版本同步，以及 `build.rs` 生成的数据与富文本渲染。

- 目录：`new/crates/definitions/`
- package / lib：`gi-genius-invokation-definitions` / `gi_genius_invokation_definitions`
- 依赖：`engine`（机制原语）、`richtext`（文本解析）；默认零第三方依赖
- 不含具体卡牌（D240 / D241）：具体卡牌在 `cards`，经 `simulation` 门面可选接入（D243）

## 2. 关键概念与类型

分层（D51 / D53 / D54）：base（L1 牌族 + id + 元素 + 视角）→ families（L2 类型枚举 + 定义载体 + 区域 + 操作）→ tags（L3 标签，顶层目录）。

| 目录 | 一句话 |
|---|---|
| `src/base/` | `CardFamily`（id 区域）+ `CardId` / `SkillId` / `Element` / `Perspective` |
| `src/families/` | 每个牌族的框架：类型枚举 + 定义载体 + 区域状态 + 底层操作（行动牌：支援区 / 手牌卡状态 / 当前费用 / 牌库 / 查询；角色：伤害 / 伤害修正链 / 技能流程 / 形态与成长 / 装备栏 / 状态区；召唤物：定义 + 召唤物区；状态：类型枚举 + 定义；敌人：定义） |
| `src/tags/` | L3 标签：顶层目录，按「牌族 / 类型」分组；一个标签一个文件、一个零大小类型（D54），共 52 个 |
| `src/resources/` | 被花的东西：`dice/` 骰池 —— 纯计数（不是实体，D47）、三块表示（D49）、生成 / 支付 / 转化 / 查询 / 折叠展开 |
| `src/rules/` | 三级统一基础操作：`RuleRow` 静态规则表 + `ops/` 纯函数（槽位 / 上限 / 开局 / 料理 / 伤害 / 查询 / 结算顺序；排除自走牌） |
| `src/timeline/` | 触发 / 事件调度：泛型注册表（时机键 + 条件函数指针 + 执行函数指针，D186）+ 有序派发 + 结算帧（D196） |
| `src/reactions/` | 14 个元素反应身份（11 基础 + 3 月反应）+ 效果函数指针表 + 效果请求（`EffectOp`） |
| `src/records/` | 统一记录器 `Record<T>` / `Recorder<T>` / `ShowRecord`（D222–D225） |
| `src/source/` | 编译期源码嵌入：`card_source!` / `module_source!`、`SourceFile` / `SourceOwner`、use 依赖解析（D210–D216 / D242） |
| `src/version/` | `GameVersion` + `card_game_version!`（D244–D246） |
| `src/generated/` | `build.rs` 生成的数据表 + 访问器 + `Localized`（D56）+ 富文本 `render`（解析由 richtext 提供） |
| `src/state/` | `DefState`（`me` / `opp` + `round` + `first_player`）/ `PlayerDefs` + 终局判定 + `state::flows`（开局 / 切换 / 击倒 / 复苏 / 结束阶段 / 回合开始 / 下落标记） |
| `src/flow/` | 端到端动作入口：使用技能 / 打牌 / 切换角色 / 元素调和 / 特技 / 回合骨架 / `pending` 执行器 / `recorded` 包装 / 钩子 |
| `src/keywords/` | 关键字函数 + 一步 `Step`（D204）+ 目标选择器（含「自己」，D205 / D206）+ 反应子结算 + `trace` / `record` |

**数据文件**：`generated/data/{terms,keywords,descriptions}.json`（入库）；`build.rs` 编译期生成静态表到 `$OUT_DIR/generated_data.rs`，运行期不读文件；标签数据是 `generated::TAGS.<字段>` 字段访问（D67）。

## 3. 公开入口

模块全部 `pub`：`base` / `families` / `flow` / `generated` / `keywords` / `reactions` / `records` / `resources` / `rules` / `source` / `state` / `tags` / `timeline` / `version`。crate 根不做 `pub use`；对外统一经 `simulation` 门面；crate 根导出三个宏：

| 入口 | 用途 |
|---|---|
| `card_source!(path: ["character", "aino"])` | 卡牌文件源码自声明（自动绑定模块的 `pub const ID`，D215） |
| `module_source!(path: [...], name: MODULE_NAME)` | 非卡牌模块源码自声明 + 多语名字 |
| `card_game_version!(6, 2)` | 声明「本卡已同步到的最新版本」（D244–D246） |

**主要公开类型 / 入口**

| 模块 | 入口示例 |
|---|---|
| `state` | `DefState::new / from_players`、`DefState::{me, opp, round, first_player}`、`state::flows::*`、`state::terminal::{settlement_outcome, Outcome, ROUND_LIMIT}` |
| `flow` | `use_skill_me/opp`、`play_card_*`（含 `_with_timing` / `_food` / `_arcane`）、`switch_character_*`、`tuning_*`、`use_technique_*`、`turn::*`、`pending::{execute_pending, execute_frames}`、`recorded::*`、`fold_states` |
| `keywords` | `Step`、`execute_step_me/opp`、`execute_steps_*`、关键字函数（`cause_damage` / `heal` / `apply_status` / `summon` / `draw_cards` / `gain_dice` …）、`TargetSelector` |
| `timeline` | `TriggerRegistry` / `Trigger` / `dispatch` / `resolve` / `Resolution`（帧 + `insert` / `snapshot`） |
| `records` | `Recorder<T>` / `Record<T>` / `ShowRecord` |
| `rules` | `rows()` / `RuleRow` / `ops::*` 纯函数 |
| `resources::dice` | `DicePool` / `DieFace` / `CostComponent` / `DiceGenSpec` / `can_afford` / `pay` / `all_dice` |
| `source` | `SourceFile` / `SourceOwner` / `source_files()` / `source_of(path)` |
| `version` | `GameVersion`；`version::Nat` 再导出 engine 的 `Nat` |

**Cargo feature**

| feature | 默认 | 含义 |
|---|---|---|
| `embed-source` | 是 | 编译期嵌入各文件源码（D213）；关闭时 `SOURCE.content` 为空、汇总表为空 |
| `records` | 是 | 统一记录器（D223 / D224）；关闭时 `Recorder` 零大小、`push` 空操作 |
| `stats` | 否 | 各块 `stats/` 统计（D187）；关闭时统计变空操作 |
| `rules` | 否 | 打开 `engine/rules` 并编译 `GameRules` 适配（`state/terminal.rs`，D24） |

### 3.1 真实源码（逐字摘自当前仓库）

**源码文件 `SourceFile`**

`new/crates/definitions/src/source/mod.rs:149-162`

```rust
/// 一份**编译期嵌入的完整源代码**（D213 / D214 / D215 / D216）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceFile {
    /// 归属（卡牌 id / 模块多语名字）。
    pub owner: SourceOwner,
    /// 相对 crate 根的路径（段用 / 拼接，层级原样保留）。
    pub path: &'static str,
    /// 宏里写的路径分段（原样保留）。
    pub segments: &'static [&'static str],
    /// 本文件全文（include_str! 原文嵌入；不做宏展开 / 解析）。
    pub content: &'static str,
    /// 编译期解析出的依赖表（D214 / D216；用 [SourceFile::dependencies] 取结构化视图）。
    pub dependencies: ParsedDependencies,
}
```

**卡牌源码声明宏 `card_source!`**

`new/crates/definitions/src/source/mod.rs:270-307`

```rust
/// 卡牌文件宏（D214 / D215 / D216 / D242）。
///
/// 用法（在卡牌 crate 里）：gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);
///
/// - 具体卡牌在 gi-genius-invokation-cards crate（D240 / D241）；本宏是**通用定义**，留在本 crate；
/// - 路径是**分段字符串列表**（每段一个 "..."，内部用 / 拼接）；
/// - 拼出 src/cards/<segments...>/mod.rs；include_str! 以**调用方 crate** 的
///   CARGO_MANIFEST_DIR 为根（从 cards crate 调用时读到 crates/cards/src/cards/...），
///   path 字段仍是 src/cards/...（结构不变）；
/// - **不写 id**：展开直接引用本模块的 pub const ID: CardId。本模块没有 ID
///   （或类型不是 CardId）时，展开里的 const _: CardId = ID; 会**编译报错**，
///   提示「本模块需定义 pub const ID: CardId」，不会静默绑定错 id。
#[cfg(feature = "embed-source")]
#[macro_export]
macro_rules! card_source {
    (path: [$($segment:literal),+ $(,)?] $(,)?) => {
        /// 本文件的归属 + 编译期嵌入的原文 + 解析出的依赖（D213 / D214 / D215 / D216）。
        pub static SOURCE: $crate::source::SourceFile = {
            // D215：宏里不写 id —— 绑定本模块的 pub const ID: CardId；
            // 本模块缺 ID 或类型不对时这里编译报错（不静默）。
            // 本模块需定义 pub const ID: CardId。
            const _: $crate::base::CardId = ID;
            const CONTENT: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/",
                concat!("src/cards", $("/", $segment),+, "/mod.rs")
            ));
            $crate::source::SourceFile {
                // D215：id 自动绑定到本模块的 ID（调用处解析，不重复写卡 id）。
                owner: $crate::source::SourceOwner::Card(ID),
                path: concat!("src/cards", $("/", $segment),+, "/mod.rs"),
                segments: &[$($segment),+],
                content: CONTENT,
                dependencies: $crate::source::parse_dependencies(CONTENT),
            }
        };
    };
}
```

**模块源码声明宏 `module_source!`**

`new/crates/definitions/src/source/mod.rs:329-356`

```rust
/// 非卡牌模块文件宏（D216）。
///
/// 用法：crate::module_source!(path: ["families", "character", "damage"], name: MODULE_NAME);
///
/// - 路径是**分段字符串列表**，拼出 src/<segments...>.rs（模块文件）；
/// - name 是 Localized 表达式（模块自己的多语名字），归属 = SourceOwner::Module(&name)；
///   建议用模块里的 static MODULE_NAME: Localized，这样引用地址稳定（'static）。
#[cfg(feature = "embed-source")]
#[macro_export]
macro_rules! module_source {
    (path: [$($segment:literal),+ $(,)?], name: $name:expr $(,)?) => {
        /// 本模块的归属 + 编译期嵌入的原文 + 解析出的依赖（D216）。
        pub static SOURCE: $crate::source::SourceFile = {
            const CONTENT: &str = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/",
                concat!("src", $("/", $segment),+, ".rs")
            ));
            $crate::source::SourceFile {
                owner: $crate::source::SourceOwner::Module(&$name),
                path: concat!("src", $("/", $segment),+, ".rs"),
                segments: &[$($segment),+],
                content: CONTENT,
                dependencies: $crate::source::parse_dependencies(CONTENT),
            }
        };
    };
}
```

**版本声明宏 `card_game_version!`**

`new/crates/definitions/src/version/macros.rs:5-28`

```rust
/// 声明「本卡 / 本模块已同步（核对）到的最新游戏版本」（D245 / D246）。
///
/// 用法：crate::card_game_version!(6, 2); 生成
/// pub const GAME_VERSION: GameVersion = GameVersion::new(Nat::new(6), Nat::new(2));
///
/// 语义（D245 / D246）：
/// - 这个值 = 本卡已同步（核对）到的最新游戏版本；
/// - 「已同步到最新版本」的唯一表示方式 = 把宏里的版本号改成最新版本号；
/// - 不要求卡牌内容有任何改动：新版本没调整这张卡，同样算同步（未调整也算同步）；
/// - 卡牌实现与版本数值解耦：数值只表示「核对过，且该卡在该版本未调整」。
///
/// 升级流程（D246）：**改数字 -> 不动卡牌内容**。
/// 例：最新版变成 6.3 时，把 crate::card_game_version!(6, 2); 改成
/// crate::card_game_version!(6, 3);，卡牌实现保持原样即算已同步。
#[macro_export]
macro_rules! card_game_version {
    ($major:expr, $minor:expr $(,)?) => {
        /// 本卡 / 本模块已同步（核对）到的最新游戏版本（D245 / D246；由 card_game_version! 生成）。
        pub const GAME_VERSION: $crate::version::GameVersion = $crate::version::GameVersion::new(
            $crate::version::Nat::new($major),
            $crate::version::Nat::new($minor),
        );
    };
}
```

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

**`DefState::me`（只读）**

`new/crates/definitions/src/state/def_state.rs:111-114`

```rust
    /// 我方的全部区（**只读指针**，D183）。
    pub fn me(&self) -> &PlayerDefs {
        &self.me
    }
```

**`DefState::opp`（只读）**

`new/crates/definitions/src/state/def_state.rs:116-119`

```rust
    /// 敌方的全部区（**只读指针**，D183）。
    pub fn opp(&self) -> &PlayerDefs {
        &self.opp
    }
```

**`DefState::round`（只读）**

`new/crates/definitions/src/state/def_state.rs:121-126`

```rust
    /// 当前回合数（0 = 还没开始第一回合；1 = 第 1 回合）。
    ///
    /// 推进点只有一个：回合开始流程 `crate::state::flows::round_begin::begin_round`。
    pub fn round(&self) -> Nat {
        self.round
    }
```

**`DefState::first_player`（只读）**

`new/crates/definitions/src/state/def_state.rs:136-141`

```rust
    /// **本回合的先手牌手**（[`None`] = 还没裁定；只读指针，D183）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】+ 其他规则·结算顺序第 1 条。
    pub fn first_player(&self) -> Option<Perspective> {
        self.first_player
    }
```

**端到端入口 `use_skill_me`**

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

**端到端入口 `use_skill_opp`**

`new/crates/definitions/src/flow/skill_use.rs:477-486`

```rust
/// **使用技能**（敌方使用；与 use_skill_me 共用一份实现，D35）。
pub fn use_skill_opp(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
) -> Result<SkillOutcome, SkillUseError> {
    // 标记的持有区在根状态（PlayerDefs::plunging）：旧入口读使用方当前标记。
    use_skill_opp_with_plunging(state, env, request, settlement, state.opp().plunging())
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

**执行一步 `execute_step_me`**

`new/crates/definitions/src/keywords/step.rs:116-123`

```rust
/// **执行一步**（我方视角）。
///
/// 只调用关键字函数：**不做任何 insert / execute_pending**（D204）；返回的待执行由调用方
/// 在步边界执行。
pub fn execute_step_me(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_step(state, step, &scoped)
}
```

**顺序执行多步 `execute_steps_me`**

`new/crates/definitions/src/keywords/step.rs:173-177`

```rust
/// **顺序执行多步**（我方视角）：每步之间执行上一步派发出来的待执行（插入只在步边界，D204）。
pub fn execute_steps_me(state: &DefState, steps: &[Step], env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_steps(state, steps, &scoped)
}
```

**触发登记项 `Trigger`**

`new/crates/definitions/src/timeline/trigger.rs:44-67`

```rust
/// 一条**触发登记项**（纯数据；`K` / `E` 都是 `Copy` 时整体 `Copy`）。
///
/// 字段与依据：
/// - `timing`：时机键（家族定义）；
/// - `owner`：这条触发挂在哪个实体上（「谁」）；
/// - `source_kind`：结算顺序第 2 条的来源类别；
/// - `entry`：进场先后（结算顺序第 3 条）；
/// - `condition`：条件函数指针（D186）；
/// - `effect`：执行函数指针（D186；派发只把它放进有序列表，不执行）。
#[derive(Clone, Copy, Debug)]
pub struct Trigger<K, E = Effect> {
    /// 时机键（由牌族 / 类型定义）。
    pub timing: K,
    /// 这条触发挂在谁身上（实体号；派发结果原样带回）。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条；由区域在登记时给出）。
    pub entry: Nat,
    /// 条件函数指针（D186）。
    pub condition: Condition,
    /// 执行函数指针（D186）。
    pub effect: E,
}
```

**单侧派发 `dispatch`**

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

**双方派发 `resolve`**

`new/crates/definitions/src/timeline/dispatch.rs:98-118`

```rust
/// **双方合并派发**（`结算顺序.md` §一 第 1 条「当前牌手优先」）：**当前牌手**的注册表先，对方的后。
///
/// 两个注册表是两个具名实例（D34：不传 `Side` 枚举、不用编号）：调用方把「当前行动权所在那一侧」
/// 放 `current`，另一侧放 `other`；两侧内部各自按 [`dispatch`] 的规则 2 / 3 排序。
///
/// 只读 + 不执行效果。
#[must_use]
pub fn resolve<'e, K, E>(
    current: &TriggerRegistry<K, E>,
    other: &TriggerRegistry<K, E>,
    timing: K,
    event: &'e Event,
) -> Vec<Pending<'e, E>>
where
    K: Copy + PartialEq,
    E: Copy,
{
    let mut ordered = dispatch(current, timing, event);
    ordered.extend(dispatch(other, timing, event));
    ordered
}
```

**结算帧 `Resolution`**

`new/crates/definitions/src/timeline/suspend.rs:125-133`

```rust
/// **结算帧栈**：插入式结算的当前状态（D196）。
///
/// - 最后一帧 = 当前帧；前面的帧是「被插入点保留的父结算」；
/// - [`Resolution::insert`] 在当前位置压入子结算帧；子帧做完自动弹回父帧的 `cursor`；
/// - 中断 = 保留这个值（帧与 `cursor` 都在）；确实被击倒 = [`Resolution::discard`]。
#[derive(Clone, Debug)]
pub struct Resolution<E> {
    frames: Vec<Frame<E>>,
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

**只读快照 `Resolution::snapshot`**

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

**记录条目 `Record<T>`**

`new/crates/definitions/src/records/mod.rs:34-41`

```rust
/// 一条记录：**属于哪一步** + 任意内容（内容类型由使用方定义）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record<T> {
    /// 步号（0 起；由写入方给）。
    pub step: Nat,
    /// 记录内容（类型由写入方定义）。
    pub content: T,
}
```

**显示由内容类型定义 `ShowRecord`**

`new/crates/definitions/src/records/mod.rs:50-54`

```rust
/// **内容类型自己定义如何显示**（记录器不预设种类 / 字段）。
pub trait ShowRecord {
    /// 把这条内容写成文本（零依赖；由内容类型决定格式）。
    fn show(&self, out: &mut String);
}
```

**记录器 `Recorder<T>`**

`new/crates/definitions/src/records/mod.rs:56-63`

```rust
/// **记录器**：泛型容器，按写入顺序保存 Record。
///
/// 唯一写入入口 [Recorder::push]；对外只读指针 [Recorder::entries]。
#[cfg(feature = "records")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recorder<T> {
    entries: Vec<Record<T>>,
}
```

**唯一写入入口 `Recorder::push`**

`new/crates/definitions/src/records/mod.rs:88-91`

```rust
    /// **唯一写入入口**：把一条内容记到某一步。
    pub fn push(&mut self, step: Nat, content: T) {
        self.entries.push(Record::new(step, content));
    }
```

**骰池 `DicePool`**

`new/crates/definitions/src/resources/dice/mod.rs:351-362`

```rust
/// 骰池 = 三块（D49）：万能 / 折叠的同色组 / 已展开的具体色。
///
/// 字段私有：对外只有一个读取入口 all_dice（D143），返回固定快照（D145）。
#[derive(Clone, PartialEq, Debug)]
pub struct DicePool {
    /// 万能骰个数。
    omni: Nat,
    /// 颜色未知的同色组（规范序：按 source 再按 size）。
    folded: Vec<FoldedGroup>,
    /// 已展开的具体颜色计数（按 Element 序，无 0 项）。
    known: Vec<(Element, Nat)>,
}
```

**够不够付 `can_afford`**

`new/crates/definitions/src/resources/dice/mod.rs:649-698`

```rust
    /// 够不够付（**可能就行**，D144）：折叠组按「可能是什么色」计入上界。
    ///
    /// 注意：pay 要求折叠组先展开，所以 can_afford = true 不保证 pay 直接成功。
    pub fn can_afford(&self, cost: &[CostComponent]) -> bool {
        validate_cost(cost);
        let mut demand = [Nat::ZERO; 7];
        let mut any = Nat::ZERO;
        let mut same_color_counts: Vec<Nat> = Vec::new();
        for component in cost {
            match *component {
                CostComponent::SpecificElement(element, count) => {
                    let slot = element_slot(element).get() as usize; // bare-int-ok: 容器下标
                    demand[slot] = nadd!(demand[slot], count);
                }
                CostComponent::AnyColor(count) => any = nadd!(any, count),
                CostComponent::SameColor(count) => same_color_counts.push(count),
            }
        }
        for count in same_color_counts {
            let mut best_slot = Nat::ZERO;
            let mut best_delta: Option<Nat> = None;
            for (slot, element) in ELEMENTS.iter().enumerate() {
                let known = known_count(&self.known, *element);
                let current = demand[slot];
                let before = shortfall(current, known);
                let after = shortfall(nadd!(current, count), known);
                let delta = nsub!(after, before);
                if best_delta.is_none_or(|best| delta < best) {
                    best_delta = Some(delta);
                    best_slot = Nat::new(slot as _);
                }
            }
            let slot = best_slot.get() as usize; // bare-int-ok: 容器下标
            demand[slot] = nadd!(demand[slot], count);
        }
        let mut demand_total = any;
        for count in demand.iter() {
            demand_total = nadd!(demand_total, *count);
        }
        if demand_total > self.total() {
            return false;
        }
        let flexible = nadd!(self.omni, folded_total(&self.folded));
        let mut shortfall_total = Nat::ZERO;
        for (slot, element) in ELEMENTS.iter().enumerate() {
            let known = known_count(&self.known, *element);
            shortfall_total = nadd!(shortfall_total, shortfall(demand[slot], known));
        }
        shortfall_total <= flexible
    }
```

**确定性扣减 `pay`**

`new/crates/definitions/src/resources/dice/mod.rs:700-731`

```rust
    /// 扣减（确定性，不产生分支；D47/D144）。付不起 -> None。
    ///
    /// - 按 ① SpecificElement → ② SameColor → ③ AnyColor 顺序扣（D168/D169）；
    /// - SpecificElement：先用该色的 known，再用万能；
    /// - SameColor：付哪个颜色由调用方给（same_color）；没给 -> None；
    /// - AnyColor：按固定顺序扣「折叠组 -> 具体色（元素序）-> 万能」（不产生分支）。
    ///
    /// 折叠组未显式展开时不能用于指定色 / 同色（返回 None）。
    pub fn pay(&self, cost: &[CostComponent], same_color: Option<Element>) -> Option<DicePool> {
        validate_cost(cost);
        let mut next = self.clone();
        // ① 指定元素骰
        for component in cost {
            if let CostComponent::SpecificElement(element, count) = *component {
                next = next.pay_color(element, count)?;
            }
        }
        // ② 相同元素骰（颜色由调用方给）
        for component in cost {
            if let CostComponent::SameColor(count) = *component {
                let element = same_color?;
                next = next.pay_color(element, count)?;
            }
        }
        // ③ 任意元素骰
        for component in cost {
            if let CostComponent::AnyColor(count) = *component {
                next = next.pay_any(count)?;
            }
        }
        Some(next)
    }
```

**统一规则行 `RuleRow`**

`new/crates/definitions/src/rules/mod.rs:242-281`

```rust
/// **统一规则行**：三级共用同一形状。
///
/// 用 ..RuleRow::EMPTY 只写本行用到的字段；表在 families / types / tags_* 五个模块里。
#[derive(Clone, Copy, Debug)]
pub struct RuleRow {
    /// 所属层级。
    pub layer: Layer,
    /// 机制 / 仅分类。
    pub kind: RuleKind,
    /// 本行用到的基础操作（与 rules/ops/ 模块一一对应）。
    pub ops: &'static [RuleOp],
    /// 生效阶段。
    pub stages: &'static [Stage],
    /// 作为筛选条件被谁引用。
    pub query_roles: &'static [QueryRole],
    /// 依据卡（id + 一句话）。
    pub evidence: &'static [Evidence],
    /// 规格 / 决策依据（分工计划 / 决策日志 / 关键字条目的文字依据）。
    pub basis: &'static [&'static str],
    /// 仅标签行：标签静态数据的访问器（指针身份，D54 / D57）。
    pub tag: Option<TagData>,
    /// 仅牌族行：牌族枚举。
    pub family: Option<CardFamily>,
    /// 仅行动牌类型行：类型枚举。
    pub action_card_type: Option<ActionCardType>,
    /// 仅状态类型行：状态类型枚举。
    pub status_type: Option<StatusType>,
    /// 槽位规则（装备类）。
    pub slots: &'static [SlotRule],
    /// 规则数字（上限 / 次数 / 门槛）。
    pub nums: &'static [NumRule],
    /// 开局钩子。
    pub opening: Option<OpeningSpec>,
    /// 支援区上限（支援类型 / 支援类标签）。
    pub support_cap: Option<Nat>,
    /// 伤害结算角色（护盾 / 减伤）。
    pub damage: Option<DamageRole>,
    /// 查询策略（不可发现 / 弃用）。
    pub query_policy: Option<QueryPolicy>,
}
```

**遍历规则表 `rows`**

`new/crates/definitions/src/rules/mod.rs:434-442`

```rust
/// 遍历全表（顺序：牌族 -> 类型 -> 标签）。
pub fn rows() -> impl Iterator<Item = &'static RuleRow> {
    families::ROWS
        .iter()
        .chain(types::ROWS.iter())
        .chain(tags_action::ROWS.iter())
        .chain(tags_character::ROWS.iter())
        .chain(tags_summon_enemy::ROWS.iter())
}
```

**关键字查询 `keyword`**

`new/crates/definitions/src/generated/mod.rs:85-95`

```rust
/// 关键字查询：按 K 码二分查找（生成表按 code 升序）。
///
/// **数据来源**：generated/data/keywords.json（权威七圣召唤 keyword 数据，142 码 × 4 语名字 +
/// 说明；129 码有 4 语说明，13 码无说明——含手牌状态 3003~3008，数据没有就保持空）。
/// 名字/说明都**不是反推**的。将来换数据时**只覆盖 keywords.json 再 rebuild**，本函数与生成表结构不变。
pub fn keyword(code: KeywordCode) -> Option<&'static KeywordDef> {
    KEYWORDS
        .binary_search_by_key(&code.get(), |definition| definition.code.get())
        .ok()
        .map(|index| &KEYWORDS[index])
}
```

**全部关键字 `keywords_all`**

`new/crates/definitions/src/generated/mod.rs:97-100`

```rust
/// 全部关键字（按 K 码升序）。
pub fn keywords_all() -> &'static [KeywordDef] {
    &KEYWORDS
}
```

**游戏版本 `GameVersion`**

`new/crates/definitions/src/version/mod.rs:26-35`

```rust
/// 游戏版本号（D244）：major.minor，如 6.2。
///
/// 派生 Ord / PartialOrd 用于普通比较；构造与访问器是 const。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct GameVersion {
    /// 主版本号，如 6。
    major: Nat,
    /// 次版本号，如 2。
    minor: Nat,
}
```


**关键字函数：造成伤害**

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

**关键字函数：治疗**

`new/crates/definitions/src/keywords/keyword.rs:298-325`

```rust
/// **治疗**：对每个目标角色 hp_add 并派发「生命值变动 -> 治疗后」。
pub fn heal(state: &DefState, env: &EffectEnv<'_>, args: &HealArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    fan_out(state, env, candidates, TargetPick::All, |current, side, target| {
        let target_state = character_state(current, side, target)?;
        let resolution = heal_with_triggers(
            &target_state,
            target,
            args.amount,
            Some(env.subject()),
            env.character_triggers(side),
        );
        let next = with_character(current, side, target, resolution.state);
        let mut pending = PendingSet::new();
        for item in resolution.pending {
            push_pending(
                &mut pending,
                side,
                FlowPending::Character {
                    character: target,
                    pending: item,
                },
            );
        }
        Some((next, pending, Vec::new()))
    })
}
```

**规则 ops：装备校验**

`new/crates/definitions/src/rules/ops/equip.rs:180-214`

```rust
/// 装备判定（纯函数，不改任何状态）。
///
/// - `occupied`：该角色当前已占用的槽位（每槽位最多出现一次；调用方按需给）；
/// - `same_kind_present`：天赋专用 —— 该角色是否已装备**同种**（同卡 id / 同名）天赋。
pub fn check_equip(
    card: &EquipCard,
    character: &CharacterEquip,
    occupied: &[SlotKind],
    same_kind_present: bool,
) -> EquipOutcome {
    if let Some(required) = card.required_weapon {
        match character.weapon {
            Some(weapon) if weapon == required => {}
            Some(_) => return EquipOutcome::Reject(EquipReject::WeaponTypeMismatch),
            None => return EquipOutcome::Reject(EquipReject::OtherWeaponCannotEquip),
        }
    }
    match card.slot {
        SlotKind::Talent => {
            if same_kind_present {
                EquipOutcome::OverwriteSameKind
            } else {
                EquipOutcome::Attach
            }
        }
        SlotKind::Weapon | SlotKind::Artifact | SlotKind::Technique => {
            // 待实测：同槽已有时是强制替换（Lead 裁定 2）还是禁止打出，卡文只有「最多装备 1 件」。
            if occupied.contains(&card.slot) {
                EquipOutcome::ReplaceExisting(card.slot)
            } else {
                EquipOutcome::Attach
            }
        }
    }
}
```

**规则 ops：结算顺序判定**

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


## 4. 不变量与禁止

> [!IMPORTANT]
> 本 crate 不含具体卡牌；单向依赖（不得依赖 `cards` / `simulation`）。对外入口纯函数：吃 `&DefState`、返回新状态，入参一个字节不动。

- 零 `f64`（D159）：概率 / 权重一律 `Fraction`（`ExactNat128`）；业务数值一律 `Nat`。
- 禁止裸整型与裸算术（§8.3）：`#![deny(clippy::arithmetic_side_effects)]` + 三个扫描脚本（只提示）。
- 数据只一份：标签数据唯一在 `generated::TAGS`（D64 / D67）；源码宏与类型唯一在本 crate（D242）；`CostComponent` 唯一在 `resources::dice`（D161）。
- 敌我不编号、区 = 变量（D29 / D34 / D42）；成对入口共用一份实现。
- 一步原子（D204）：步内禁止插入；插入只能发生在步与步之间。
- 插入式结算不是栈（D196）；结算顺序唯一权威是 `结算顺序.md`，常量只在 `rules::ops::order`。
- 不确定先不物化（D190 / D191）：观测点展开成「分支 + 精确概率」，之后折叠回。
- feature 关闭不改变语义（`records` / `stats` / `embed-source`）。
- 不持有全局状态：注册表 / 记录器 / 骰池都是普通值字段。

## 5. 依据（规则书与 D 编号）

| 领域 | 依据 |
|---|---|
| 牌族 / 类型 / 标签三级 | D51 / D53 / D54 / D55；`分工计划.md` §4.4.0.0 |
| base / id / 敌我 / 区 | D29 / D34 / D36 / D37 / D41 / D42 |
| 骰池（纯计数 / 三块 / 展开 / 支付） | D46–D49、D139–D148、D168–D172 |
| 三级统一基础操作 | D176–D179；规则书·进行行动 / 游戏区域 / 特殊词条 |
| 状态与区 | D95 / D96、D101–D119、D182 / D183 |
| 伤害 / 反应 / 结算顺序 | 规则书·元素反应（规则 + 效果两篇）、特殊词条；`结算顺序.md`；D127–D136、D164–D167、D193–D198 |
| 技能 / 准备技能 / 特技 | 规则书·进行行动【使用技能】与特殊词条；D7 / D15 / D19 / D123–D126 / D165 / D166 |
| 关键字 + 一步 | D203–D209；规则书·特殊词条 |
| 触发调度 / 函数指针 | D185 / D186 |
| 记录 | D222–D225 |
| 源码嵌入 | D210–D216 / D242 |
| 版本同步 | D244–D246 |
| 生成数据 / 本地化 / 渲染 | D56 / D61–D68 / D74–D79 |
| 门面与拆分 | D240–D243 |

> [!NOTE]
> 依据优先级（`RULES.md` 第三节）：规则书 > 卡面 / 权威关键字 > 参考实现（只看方法）。规则书与旧决策冲突时以规则书为准，改不动的标 `// 待接入`。

## 6. 常见误用

> [!WARNING]
> 不要在本 crate 写具体卡牌、不要写 `f64` 或裸算术、不要给标签再造总枚举 / 编号、不要在 `timeline::dispatch` 里执行效果。

- 让 `definitions` 依赖 `cards` / `simulation`（反向依赖；cards 有守卫测试）。
- 在一步内部插入子结算（违反 D204）。
- 自己再写一份结算顺序表（唯一权威 `结算顺序.md`）。
- 在 `records` 里定义业务字段 / 种类枚举（内容类型归各模块，D225）。
- 关闭 `records` / `stats` 后让结算结果改变。
- 新增模块文件用 `module_source!` 后忘记在 `source::SOURCE_FILES` 加一行（编译能过，`source_of` 找不到）。

## 7. 测试

- `cargo test --offline -p gi-genius-invokation-definitions`：当前 917 条单元测试（lib target；doctest 0；默认 features）。
- `--features rules` 额外编译 `state/terminal.rs` 的 `GameRules` 适配测试（D24）；CI 对 workspace 使用 `--features rules,dedup,opt-all`。
- 规模分布（按测试名前缀，随开发变化）：`families::character` 304、`families::action_card` 98、`resources::dice` 45、`rules::ops` 42、`state::flows` 33、`flow::*` 约 130、`keywords::*` 约 80、`timeline::*` 约 35、`reactions::*` 约 30、`generated` 24、`state::*` 29、`records` 4、`source` 7、`version` 3、`tags` 1。
- 代表用例：

| 用例 | 覆盖点 |
|---|---|
| `state/def_state.rs::state::def_state::tests::clone_is_a_deep_snapshot` | 状态深拷贝 |
| `flow/skill_use.rs::flow::skill_use::tests::after_skill_pending_is_inserted_at_the_current_settlement_position` | 插入式结算 |
| `flow/turn.rs::flow::turn::tests::declare_round_end_records_first_declarer_and_opens_the_end_phase` | 回合骨架 |
| `keywords/step.rs::keywords::step::tests::felis_burst_is_one_atomic_step` | 一步原子 |
| `keywords/step.rs::keywords::step::tests::insertion_happens_only_between_two_steps` | 步间插入 |
| `timeline/suspend.rs::timeline::suspend::tests::inserted_sub_resolution_returns_to_the_insertion_point` | 结算帧恢复 |
| `rules/mod.rs::rules::tests::every_row_has_evidence_or_basis` | 规则表依据 |
| `resources/dice/parse.rs::resources::dice::parse::tests::content_any_slot_is_random` | 骰子生成 |
| `generated/text/tests.rs::generated::text::tests::every_generated_text_parses_strictly` | 全量文本解析 |
| `tags/mod.rs::tags::tests::every_tag_is_a_distinct_zero_sized_type` | 52 个标签全零大小且类型名互不相同 |

## 8. 待接入与待实测

> [!NOTE]
> 本 crate 规则落点最多；下表只列代表项。完整清单见 `留空.md`，以 #103 第六轮总账为准（更早条目写作时按当轮状态记，之后部分已接线，属低报）。

| 编号 | 事项 |
|---|---|
| #15 | 三级统一基础操作待接入：结算顺序实际调度、结束阶段扣持续回合 / 可用次数、起手换牌、首发选择、击倒 / 免于击倒 / 形态转换、支援满 4、召唤满 4、元素幻变时序 |
| #17–#32 | 伤害修正链顺序、结晶护盾上限、元素精通、伤害免疫与加伤先后、乘算取整、下落 vs 重击等 |
| #33–#39 | 第一回合先手、结束阶段先手口径、切换角色错误优先级、元素调和来源骰限制、不可调和语义 |
| #42–#44 | 终局判定接进 flow、`GameRules` 占位、definitions 的 `rules` feature 是否进 CI（待确认） |
| #45–#50 | F10 抓牌后的手牌费用表 / `drawn_marks`、下落标记端到端接线 |
| #51–#57 | 两套插入式结算类型并存（技术债）、反应效果执行时机、非伤害附着的消耗口径、trace 路径不结算反应等 |
| #62–#66 | 覆盖报告剩余项：L12「最近角色」、自行巧局三条（另一模式不做）、Z13 / Z14 / L9 等 |
| #67–#102 | 支援持续回合递减时机、秘传揭令归属、超限处理、饱腹解除、特技交互、复苏后出战 / 时机等 |
| #105–#122 | 收尾轮：Z6 生产写入路径、L9 幻变钩子未接进开局、伤害类型转换已补、双负语义已落地（#121） |
| 无编号 | `build.rs` 手写 JSON 解析器与 D74「改用 JSON 库」冲突；`src/lib.rs` / `src/families/mod.rs` 模块文档仍写旧范围（见 [[Architecture]] 末节） |

---

最后核对：2026-10-19（代码：`new/crates/definitions/src/lib.rs:pub mod`）
