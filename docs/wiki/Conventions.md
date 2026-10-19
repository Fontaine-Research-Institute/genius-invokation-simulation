# 设计约定速查

> 中文 · [[Conventions-EN|English]]

*本页范围：写代码前必须对一遍的硬约定；每条给要求、为什么、落点、反例与依据。*

---

## 1. 类型定身份：零大小类型 + 指针身份

**要求**

- 标签 = 一个零大小类型 + 一个文件（D54）；标签不是枚举常量、不是字符串、不是数字。
- 标签的运行期身份 = 指向它唯一一份静态数据的指针（D57 / D58）；标签文件里的 `data()` 只返回引用，不写字面量。
- 状态 / 反应的身份比较一律用 `identity_address()`（指向定义数据的 `*const ()`），比较用 `std::ptr::eq` / `std::ptr::addr_eq`（D199 / D200 / D202）。
- 除卡牌 id 这类必须留数字的以外，一切 id 编译后消失（D41）：不引入 `TypeId`、字符串键、数字编号。

**落点**

| 内容 | 位置 |
|---|---|
| 标签类型 + `data()` | `definitions/src/tags/action_card/support/location.rs` |
| 标签数据唯一一份 | `definitions/src/generated/mod.rs` 的 `TAGS`（一个标签一个字段） |
| 状态身份 | `definitions/src/status/def.rs`、`status/state.rs` 的 `identity_address()` |
| 反应身份 | `definitions/src/reactions/mod.rs`（`ReactionIdentity::identity_address()`） |

> [!WARNING]
> 零大小静态实例的地址可能相同，直接对 ZST 做 `ptr::addr_eq` / `ptr::eq` 会把不同身份判成同一个（D200 实测）。
> 身份必须落到「定义数据」的地址。

## 2. 数据只一份

**要求**

- 每个标签恰好一份静态数据；访问一律字段访问（`TAGS.location.name`），不建数组 / 下标表 / 名字→下标映射 / `HashMap`（D64 / D67）。
- 标签文件里不许出现名字字符串字面量，只引用生成数据（D64 的校验测试盯这一点）。
- 同一份格式只允许一个定义处：`CostComponent` 归骰子模块、其它地方只再导出（D161）；源码嵌入宏与类型归 `definitions`、只有汇总表跟 crate 走（D242）。

> [!TIP]
> 需要「方便访问」时，先问：这份数据是不是已经有唯一的一份？有就从那一份走，别再抄。

## 3. 纯函数 + `Option` / `Result`

**要求**

- 对外入口一律纯函数：吃 `&DefState`、返回新 `DefState`（需要时带待执行 / 分支 + 精确概率）；失败返回 `Err` 或 `None`。
- 入参一个字节都不能改；内部可以先 `clone` 再改，那份克隆就是返回值。
- `&mut` 只允许出现在执行器内部（如 `flow::pending::execute_pending` 调各区的 `execute_*`），对外仍保持纯函数形状。
- 多分支用 `Vec<(DefState, Fraction)>`，不采样、不合并概率。

> [!IMPORTANT]
> 选型说明见 `definitions/src/state/flows/mod.rs`：搜索必须能分叉、插入式结算要能「不执行也返回」、回滚 / 对照 / 单测都要求入参不变。

## 4. 零浮点

**要求**

- `crates/` 下不出现 `f64`（D159；`grep -r "f64" new/crates` 应为 0 命中）。
- 概率 / 权重 / 概率质量一律 `Fraction`（分子 / 分母 `ExactNat128`；D152 / D153 / D158）。
- 随机源对外只给 128 位整数：`Rng::next_u128` / `Rng::next_below_u128`（D160）。
- 观测点展开成「确定分支 + 精确概率」，不采样（D190）。

> [!NOTE]
> 只有将来对外展示 / 求解器边界才允许一次性转浮点，且不在核心 crate 内（留空 #14）。

## 5. Nat 与 ExactNat

| 要求 | 说明 | 依据 |
|---|---|---|
| 业务数值默认 `Nat` = `Nat32`（饱和） | 别的宽度（`Nat8/16/64/128`）只有特意设计才用，且必须登记到 `不入库/nat-宽度统计.md` | D37 |
| 概率相关用 `ExactNat` = `ExactNat128`（精确） | 加 / 减 / 乘上溢或下溢都 panic | D153 / D156 |
| 算术宏两族共用 | `nadd!` / `nsub!` / `nmul!` / `ndiv!`，语义由类型决定 | D155 |
| 禁止裸整型与手写裸 `+ - * /` | `definitions` / `cards` 有 `#![deny(clippy::arithmetic_side_effects)]`，另有三个扫描脚本 | §8.3 |

> [!WARNING]
> 反例：`let hp: u32 = 10;`、`let n = 2 + 3;`、用饱和 `Nat` 算概率、随便引入 `Nat64` 而不登记。

## 6. 饱和运算

**要求**

- 饱和族：加 / 减 / 乘饱和，溢出停 `MAX`、减法到 `ZERO` 停，不 panic（溢出是正常业务）。
- 除法是唯一会崩的运算：除数为 0 → panic（D44；`Div` 与 `ndiv!` 一致，debug / release 相同）。
- 不提供 `div_or_zero` / `saturating_div` 之类变体（D44）。

**落点**：`engine/src/nat.rs` 的 `saturating_add/sub/mul`、`Div` 与四个宏；`Fraction::new` 分母 0 → panic。

> [!WARNING]
> 不要依赖「溢出会 panic」发现业务问题，也不要用 `max/min` 手动兜底代替饱和语义。

## 7. 一步（Step）是原子单位

**要求**

- 一步 = 一次可观察效果内的全部动作（含多目标 / 多实例 / 逐张处理 / Repeat），按书写顺序执行（D204 / D207 / D208 / D209）。
- 步内禁止插入任何外来结算（触发 / 其它效果 / 子结算都不行）。
- 触发与待执行只在步末统一派发 / 执行；`timeline::Resolution::insert` 只能用在步与步之间。
- 一步同时是显示单位和不可中断的原子结算单位。

**落点**：`definitions/src/keywords/step.rs`、`keywords/mod.rs` 的硬约束、`flow/skill_use.rs` 的步末 flush。

> [!IMPORTANT]
> 反例：在关键字函数内部直接执行 pending / 插入子结算（破坏原子性）；把「一步」理解成「一个动作」。

## 8. 效果 = 执行函数指针

**要求**

- 触发登记项 = 时机键 + `owner` + `source_kind` + `entry` + 条件函数指针 + 执行函数指针。
- 注册表是普通值（每侧一个实例、放在状态字段里），没有全局单例；不用 `TypeId`、不用字符串键（D185）。
- `dispatch` 只做时机 / 条件匹配与排序，不执行效果（D186）；执行归调用方。
- 同一张卡多实例：全部登记进同一张表，用 `owner` 在效果函数内部做实例过滤。

**落点**：`definitions/src/timeline/trigger.rs`、`timeline/mod.rs`（`dispatch` / `resolve`）；顺序常量唯一来源 `rules/ops/order.rs`。

> [!WARNING]
> 不要登记闭包 / trait object / 效果数据 enum；不要在 `dispatch` 里顺手执行效果；不要每个实例建一张注册表。

## 9. 记录 API

**要求**

- 统一记录器在 `definitions/src/records/`：`Record<T> { step: Nat, content: T }`、`Recorder<T>::push / entries / step_entries / render`。
- 记录器不预设种类 / 字段；显示由内容类型实现 `ShowRecord` 自己定义（D224）。
- 每个模块的记录内容类型单独一个文件（D225）：`keywords/record.rs`、`damage_chain/record.rs`、`zones/record.rs`、`flow/record.rs` …；`push` 调用留在业务处。
- feature `records` 默认开启；关掉时 `Recorder` 是零大小类型、`push` 空操作，结算结果不变。

**入口**：带 `_recorded` 后缀的入口（如 `flow::recorded::use_skill_me_recorded`）。

> [!WARNING]
> 不要在 `records` 里定义业务枚举 / 固定字段；不要各模块自造平行记录格式或直接 `println!`。

## 10. 敌我不编号；区是变量

**要求**

- 敌我 = 同一套结构的两个具名实例（`me` / `opp`）；一切「对某一方」的入口成对出现（`xxx_me` / `xxx_opp`），共用一份实现。
- 不传 `Side`、不用 `[T; 2]` + 下标、不用 0/1 编码。
- 区 = 状态里的字段：没有区号、没有区表、引擎不记录「实体属于哪个区」；容器只有 `Area` / `SingleArea` 两种类型。

**落点**：`definitions/src/state/def_state.rs`、`state/player.rs`；`engine/src/area.rs` / `entity.rs`。

> [!WARNING]
> 不要在 `definitions` 里新造 `Side` 枚举到处传，也不要用 `players[0]` / `players[1]` 区分敌我。

## 11. 结算顺序与插入

**要求**

- 同一时机内的顺序以工作区根 `结算顺序.md` 为唯一权威（暂定）；顺序常量只有 `rules/ops/order.rs` 一份。
- 结算是插入式不是栈：帧 + `cursor`，子结算做完回到插入点继续；插入不重排父结算（D196）。

**落点**：`definitions/src/timeline/mod.rs`、`timeline/suspend.rs`、`rules/ops/order.rs`。

> [!WARNING]
> 不要在别处再写一份「角色牌 → … → 支援牌」的顺序表，不要用 LIFO 栈实现触发队列。

## 12. 相关页面

| 页面 | 内容 |
|---|---|
| [[Architecture]] | 依赖方向、数据流、已知文档偏差 |
| [[Crate-Engine]] | 数值 / 分数 / 随机原语 |
| [[定义层-Keywords]] | 一步与关键字函数 |
| [[定义层-Timeline]] | 触发注册与插入式结算 |
| [[定义层-Records]] | 记录器实现 |

---

## 13. 真实源码摘录（逐条对应上文）

**标签类型（§1）**

`new/crates/definitions/src/tags/action_card/support/location.rs:8-10`

```rust
/// 「场地」：支援牌中的场地牌。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Location;
```

**标签数据入口 `data()`（§1 / §2）**

`new/crates/definitions/src/tags/action_card/support/location.rs:12-15`

```rust
/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.location
}
```

**状态身份 trait（§1）**

`new/crates/definitions/src/families/action_card/status/state.rs:43-78`

```rust
/// 状态身份（D95/D96/D110）：具体状态模块定义自己的零大小类型 + 一份静态实例；
/// 运行期身份 = 指向那份实例的指针；静态定义数据 = 它给出的 StateText（名字 + 描述）。
///
/// 谁定义状态谁给类型；使用处 import 那个模块、拿到静态实例后交给 StateList 的操作。
/// 本层**只解释身份 + 形态**，不解释减费 / 赋能等具体语义（D108）。
pub trait StateIdentity: 'static + std::fmt::Debug {
    /// 状态的静态定义数据（4 语名字 + 描述；只有一份，不拷贝）。
    fn text(&self) -> &'static StateText;

    /// 这个状态是布尔值还是必须带层数。
    fn form(&self) -> StateForm;

    /// **身份地址** = 指向该身份**定义数据**的指针（D57 / D58 口径）。
    ///
    /// 为什么不用 `&dyn StateIdentity` 的地址：零大小（ZST）静态实例**没有独立地址**
    /// （本仓库实测 rustc 1.98.1 下不同 ZST 静态实例地址相同），直接比 `&dyn` 会把
    /// 不同身份判成同一身份。所以身份一律取「它指向的那份数据」的地址。
    ///
    /// 默认实现 = [`StateIdentity::text`] 返回的那份 `StateText` 的地址：本仓库每个身份
    /// 都有唯一一份文本数据；将来有独立定义数据（如 `&'static StatusDef`）的实现可以覆盖，
    /// 返回定义本身的地址（同样唯一）。
    fn identity_address(&self) -> *const () {
        self.text() as *const StateText as *const ()
    }

    /// 状态的可读名字（调试 / 日志用；**不参与身份判断**）。
    ///
    /// 默认从 StateText 取中文原文；状态模块想换语言自己覆盖。
    fn name(&self) -> &'static str {
        self.text()
            .name
            .get(crate::generated::Lang::ZhCn)
            .text()
            .unwrap_or("")
    }
}
```

**身份地址 `identity_address()`（§1）**

`new/crates/definitions/src/families/action_card/status/state.rs:55-66`

```rust
    /// **身份地址** = 指向该身份**定义数据**的指针（D57 / D58 口径）。
    ///
    /// 为什么不用 `&dyn StateIdentity` 的地址：零大小（ZST）静态实例**没有独立地址**
    /// （本仓库实测 rustc 1.98.1 下不同 ZST 静态实例地址相同），直接比 `&dyn` 会把
    /// 不同身份判成同一身份。所以身份一律取「它指向的那份数据」的地址。
    ///
    /// 默认实现 = [`StateIdentity::text`] 返回的那份 `StateText` 的地址：本仓库每个身份
    /// 都有唯一一份文本数据；将来有独立定义数据（如 `&'static StatusDef`）的实现可以覆盖，
    /// 返回定义本身的地址（同样唯一）。
    fn identity_address(&self) -> *const () {
        self.text() as *const StateText as *const ()
    }
```

**业务/概率共用的算术宏（§4 / §5 / §6）**

`new/crates/engine/src/nat.rs:426-435`

```rust
/// 加法宏（**两族共用**，语义由类型决定，见 [`NatArith`]；D155）。
///
/// - `nadd!(Nat::MAX, Nat::new(1))` -> `Nat::MAX`（饱和，不 panic）；
/// - `nadd!(ExactNat::MAX, ExactNat::new(1))` -> panic（精确族不允许饱和）。
#[macro_export]
macro_rules! nadd {
    ($a:expr, $b:expr) => {
        $crate::nat::NatArith::nat_add($a, $b)
    };
}
```

**分数构造（§4）**

`new/crates/engine/src/prob.rs:77-86`

```rust
    /// 构造 `num / den`（**不约分**；需要规范化时用加 / 乘 / 除的结果）。
    ///
    /// **分母为 0 -> panic**（与引擎除法一致：除零是逻辑错误，不是饱和业务）。
    pub const fn new(num: ExactNat128, den: ExactNat128) -> Fraction {
        assert!(
            den.get() != 0,
            "除零：分数分母为 0（与引擎除法一致，这是逻辑错误）"
        );
        Fraction { num, den }
    }
```

**一步 `Step`（§7）**

`new/crates/definitions/src/keywords/step.rs:44-48`

```rust
/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}
```

**触发登记项 `Trigger`（§8）**

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

**记录条目 `Record<T>`（§9）**

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

**记录写入 `Recorder::push`（§9）**

`new/crates/definitions/src/records/mod.rs:88-91`

```rust
    /// **唯一写入入口**：把一条内容记到某一步。
    pub fn push(&mut self, step: Nat, content: T) {
        self.entries.push(Record::new(step, content));
    }
```

**费用组件 `CostComponent`（§2 / §5）**

`new/crates/definitions/src/resources/dice/mod.rs:157-168`

```rust
/// 花费 / 消耗的一位（D168：只有三类，枚举顺序 = 加费/减费优先级）。
///
/// 骰子模块只提供底层能力：can_afford（可能就行）与 pay（确定性扣减），**不做支付策略**（D147）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CostComponent {
    /// ① 指定元素骰：n 个**同一种指定元素**（只能是一种，不传多元素列表）。
    SpecificElement(Element, Nat),
    /// ② 相同元素骰：n 个**同色**（颜色在支付时再定）。
    SameColor(Nat),
    /// ③ 任意元素骰：n 个任意颜色。
    AnyColor(Nat),
}
```

**关键字函数 `cause_damage`（§8）**

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

**结算顺序常量（§11）**

`new/crates/definitions/src/rules/ops/order.rs:29-36`

```rust
/// 同一牌手内、不同来源类别的结算顺序（规则书第 2 条）。
pub const SOURCE_ORDER: [EffectSource; 5] = [
    EffectSource::CharacterCard,
    EffectSource::CharacterEquipmentOrStatus,
    EffectSource::CombatStatus,
    EffectSource::Summon,
    EffectSource::Support,
];
```

**顺序判定 `resolves_before`（§11）**

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


---

最后核对：2026-10-19（代码：`new/crates/definitions/src/keywords/step.rs:Step`）
