# 定义层-Reactions

> 中文 · [[定义层-Reactions-EN|English]]

*本页范围: 14 个元素反应身份, 规则 / 效果表, `table::resolve`, 以及纯函数反应效果与 `EffectOp` 请求.*

---

## 1. 职责

14 个元素反应身份 + 效果表 + 纯函数效果: 输入反应事件与最小局面摘要, 输出"本伤害 +N"与带精确概率的底层操作清单; 不自己结算, 不采样, 不碰附着 (附着消耗由伤害入口按 D167 处理) .插入式结算见 [[定义层-Timeline]], 记录契约见 [[定义层-Records]].

## 2. 关键概念与类型

| 概念 | 文件 | 说明 |
|---|---|---|
| `ReactionIdentity` | `crates/definitions/src/families/character/attachment.rs` | 反应身份 trait; 比较用 `identity_address()`, 不能比 `&dyn` 地址 |
| 14 个身份 | `crates/definitions/src/reactions/*.rs` | 每文件一个 ZST + 静态实例: 融化 / 蒸发 / 超载 / 超导 / 感电 / 冻结 / 扩散 / 结晶 / 燃烧 / 绽放 / 原激化 / 月感电 / 月绽放 / 月结晶 |
| `CONDITIONS` | 各反应文件 | `[ReactionCondition; N]`: 同一对元素的两种先后各一条 (扩散 / 结晶 8 条)  |
| `table` | `reactions/table.rs` | `base_rules` / `rules` / `EFFECTS` / `effect_of` / `resolve` / `is_lunar` / `rule_matches` |
| 效果类型 | `reactions/effect.rs` | `ReactionEffectResult { damage_bonus, branches }`, `ReactionBranch { ops, probability: Fraction }`, `EffectContext`, `Side` |
| `EffectOp` | 同上 | 底层操作请求: `Damage` / `Status` / `Summon` / `ForceSwitch` / `AddCardToHand` / `HandCardStatus` |
| `entities` | `reactions/entities.rs` | 反应生成 / 附属的实体 id (冻结 106, 护盾 203, 草原核 116, 激化领域 117, 燃烧烈焰 115, 雷暴云 205, 月笼 212, 月笼协奏 211)  |

## 3. 公开入口

| 入口 | 文件 | 说明 |
|---|---|---|
| `base_rules() -> Vec<ReactionRule>` | `reactions/table.rs` | 11 条基础反应规则 |
| `rules(overrides: LunarOverrides) -> Vec<ReactionRule>` | 同上 | 启用的月反应规则在前 (覆写基础反应)  |
| `effect_table() -> &'static [EffectEntry]` | 同上 | 14 条效果登记项 |
| `effect_of(reaction)` | 同上 | 按身份数据地址查效果函数指针 |
| `resolve(event, context) -> Option<ReactionEffectResult>` | 同上 | 对一次已判定反应求效果 |
| `is_lunar(reaction)` / `rule_matches(rule, attachment, attack)` | 同上 | 是否月反应; 规则条件是否命中 |

### 源码 

> [!NOTE]
> 以下为反应层的真实源码摘录 (类型 / trait / 表函数签名 + 文档注释; 函数体从略) .

**`crates/definitions/src/families/character/attachment.rs(ReactionIdentity 与身份比较)`**

```rust
/// 反应身份（与 `StateIdentity` 同款，D95 / D96）：具体反应模块定义自己的零大小类型 +
/// 一份静态实例；运行期身份 = 指向它的**那份数据**的地址（D57 / D58 口径）。
///
/// **本块不实现任何具体反应**（超载 / 感电 / 结晶 … 都定义在别的块）；
/// 注册表按**身份地址**比较，不建编号表、不用字符串匹配。
pub trait ReactionIdentity: 'static + std::fmt::Debug {
    /// 可读名字（调试 / 日志用；**不参与身份判断**）。
    fn name(&self) -> &'static str;

    /// **身份地址** = 指向该反应**定义数据**的指针（D57 / D58 口径）。
    ///
    /// 为什么不用 `&dyn ReactionIdentity` 的地址：零大小（ZST）静态实例**没有独立地址**
    /// （本仓库实测 rustc 1.98.1 下不同 ZST 静态实例地址相同）；反应身份目前都是 ZST
    /// （`pub struct Melt;` + `static MELT: Melt`），直接比 `&dyn` 会把所有反应判成同一个。
    ///
    /// 默认实现 = `name()` 返回的 `&'static str` 的**数据地址**：本仓库每个反应模块的名字
    /// 字面量各是一份独立静态数据，所以稳定且互不相同。将来反应有自己的定义数据
    /// （如 `ReactionDef`）时，实现应覆盖本方法返回那份定义的地址。
    fn identity_address(&self) -> *const () {
        self.name().as_ptr() as *const ()
    }
}

/// 两个反应身份是否是**同一份身份**（比较 `identity_address()`；D57 / D58）。
///
/// **不要**直接比 `&dyn ReactionIdentity`（`std::ptr::addr_eq`）：ZST 身份地址可能相同。
pub fn same_reaction_identity(
    left: &'static dyn ReactionIdentity,
    right: &'static dyn ReactionIdentity,
) -> bool {
    std::ptr::eq(left.identity_address(), right.identity_address())
}
```

**`crates/definitions/src/reactions/effect.rs(HandCardStatus / EffectOp / 分支与结果类型)`**

*注释中的关键字代码已替换为名称, 代码行未改*

```rust
/// 反应赋予**手牌**的状态（关键字，不是卡牌）。
///
/// 依据：规则书·元素反应·效果 月感电「赋予敌方随机1张手牌[电击]」、
/// 月绽放「赋予我方随机1张手牌[费用降低]」；关键字表 `tcgkeywords_4lang.json`
/// 关键字「电击」/「费用降低」（手牌状态，具体状态块待接入）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HandCardStatus {
    /// 电击：月感电的雷暴云入场 / 可用次数增加时赋予敌方手牌。
    ElectricShock,
    /// 费用增加。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_COST_INCREASE。
    CostIncrease,
    /// 费用降低：月绽放赋予我方手牌。
    CostReduction,
    /// 赋能。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_EMPOWERMENT。
    Empowerment,
    /// 不可调和。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_UNTUNABLE。
    Untunable,
    /// 无效化。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_INVALIDATION。
    Invalidation,
}

pub enum EffectOp {
    /// 造成伤害（交给已有伤害入口；本模块不结算增减伤 / 护盾）。
    Damage {
        /// 伤害数值。
        amount: Nat,
        /// 伤害类型（元素 / 物理 / 穿透）。
        ty: DamageType,
        /// 目标范围。
        target: TargetScope,
    },
    /// 附属 / 叠加状态（交给状态区；**待接入**）。
    Status {
        /// 状态牌 id（状态牌族）。
        card: CardId,
        /// 挂载位置（角色状态 / 出战状态）。
        target: StatusTarget,
        /// 叠加层数（规则书「可用次数」不是层数，单独放 `uses`）。
        stacks: Nat,
        /// 可用次数（`None` = 该状态没有可用次数概念）。
        uses: Option<Nat>,
        /// 叠加上限（`None` = 不设上限；结晶的「最多2点」放这里）。
        stack_cap: Option<Nat>,
    },
    /// 生成 / 叠加召唤物（交给召唤物区；**待接入**）。
    Summon {
        /// 召唤物牌 id（召唤物牌族）。
        card: CardId,
        /// 生成在哪一方。
        side: Side,
        /// 初始可用次数。
        uses: Nat,
        /// 叠加上限（`None` = 无限叠加）。
        stack_limit: Option<Nat>,
    },
    /// 强制切换到下一个角色（交给角色区；**待接入**）。
    ForceSwitch {
        /// 被强制切换的角色范围（超载 = 承载反应的角色）。
        target: TargetScope,
    },
    /// 生成一张牌加入手牌（交给手牌区；**待接入**）。
    AddCardToHand {
        /// 行动牌 id（行动牌牌族）。
        card: CardId,
        /// 加入哪一方的手牌。
        side: Side,
    },
    /// 给**随机一张**手牌赋予状态（分支已按 D190 展开；交给行动牌状态区；**待接入**）。
    HandCardStatus {
        /// 要赋予的手牌状态。
        status: HandCardStatus,
        /// 哪一方的手牌。
        side: Side,
        /// 手牌下标（`0..手牌数`；下标 -> 具体牌的映射由手牌区只读指针给出，D183）。
        index: Nat,
    },
}

/// 一条**已展开**的分支：底层操作清单 + 该分支的精确概率（D190，不采样）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReactionBranch {
    /// 本分支要执行的底层操作（按规则书条文顺序；可能为空 = 纯加伤 / 无额外效果）。
    pub ops: Vec<EffectOp>,
    /// 该分支的精确概率（`ExactNat128` 分子分母，零浮点）。
    pub probability: Fraction,
}

/// 一次反应效果结算的**纯函数结果**。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReactionEffectResult {
    /// 「本伤害 +N」（确定性；所有分支相同）。
    pub damage_bonus: Nat,
    /// 分支列表。确定性效果 = 恰好一支、概率 1；随机效果 = 每个可能结果一支、概率和为 1。
    pub branches: Vec<ReactionBranch>,
}
```

**`crates/definitions/src/reactions/effect.rs(ReactionEffectFn 别名)`**

```rust
/// 效果函数指针（D186：登记的是**执行函数指针**，不是效果数据 / trait object / 闭包）。
///
/// 参数：
/// - `&ReactionEvent`：谁触发 / 作用在谁身上 / 命中的已覆盖元素 / 本次攻击元素；
/// - `&EffectContext`：本次效果需要的最小局面摘要（是否出战角色、双方手牌数）。
pub type ReactionEffectFn =
    fn(&crate::families::character::attachment::ReactionEvent, &EffectContext) -> ReactionEffectResult;
```

**`crates/definitions/src/reactions/table.rs(EffectEntry 与公开函数签名)`**

```rust
/// 一条**效果登记项**：反应身份 + 执行函数指针（D186：登记函数指针，不登记效果数据）。
#[derive(Clone, Copy, Debug)]
pub struct EffectEntry {
    /// 反应身份（与规则表里的身份是同一份静态实例，按**指针**比较）。
    pub reaction: &'static dyn ReactionIdentity,
    /// 效果函数指针（纯函数，不执行、不采样）。
    pub effect: ReactionEffectFn,
}

/// 基础反应规则表（顺序 = 规则书条目顺序；元素对互不重叠，顺序不影响判定）。
///
/// 交给 `damage::DamageEnv { rules: &base_rules(), .. }`；
/// 需要月反应覆写时改用 [`rules`]。
pub fn base_rules() -> Vec<ReactionRule>

    /// 有没有启用任何月反应。
    pub const fn any(self) -> bool

/// **本次伤害使用的规则表**：启用的月反应规则在前，基础规则在后。
///
/// - 返回 `Vec`：切片要活到 `DamageEnv` 用完（`DamageEnv.rules` 是借用）；
/// - 月反应与基础反应同元素对 -> 先命中月反应（覆写：原效果完全不执行，D128 / D129）；
/// - 未启用的月反应不出现 -> 对应元素对走基础反应。
pub fn rules(overrides: LunarOverrides) -> Vec<ReactionRule>

    /// 一条登记项。
    pub const fn new(
        reaction: &'static dyn ReactionIdentity,
        effect: ReactionEffectFn,
    ) -> EffectEntry

/// 全部效果登记项（只读切片）。
pub fn effect_table() -> &'static [EffectEntry]

/// 按**反应身份数据地址**查效果函数指针（没登记 -> `None`）。
///
/// 身份 = `ReactionIdentity::identity_address()`（D57 / D58；ZST 身份没有独立地址）：不建编号表、不用字符串匹配。
pub fn effect_of(reaction: &'static dyn ReactionIdentity) -> Option<ReactionEffectFn>

/// 对一次已判定成功的反应求效果（纯函数；没登记 -> `None`）。
///
/// 调用时机：`damage` 返回 `reaction: Some(..)` 之后（插入式结算：附着 -> 判定反应 ->
/// 反应效果 -> 继续原步骤，见 `结算顺序.md` §三）。
pub fn resolve(
    event: &ReactionEvent,
    context: &EffectContext,
) -> Option<ReactionEffectResult>

/// 这条反应是不是月反应（月感电 / 月绽放 / 月结晶）。
pub fn is_lunar(reaction: &'static dyn ReactionIdentity) -> bool

/// 这条规则的条件是否命中（`damage` 判定用同一口径；这里给测试 / 调用方查表用）。
pub fn rule_matches(
    rule: &ReactionRule,
    attachment: &crate::families::character::attachment::Attachment,
    attack: crate::base::Element,
) -> Option<crate::base::Element>
```

**`crates/definitions/src/reactions/melt.rs(反应身份示例: 融化)`**

```rust
/// 反应身份（指针身份：具体反应模块的零大小类型 + 一份静态实例，D95 / D96）。
#[derive(Debug)]
pub struct Melt;

impl ReactionIdentity for Melt {
    fn name(&self) -> &'static str {
        "融化"
    }
}

/// 全局唯一的反应身份实例。
pub static MELT: Melt = Melt;

/// 触发条件（D132：同一对元素**两种先后各一条**）。
pub static CONDITIONS: [ReactionCondition; 2] =
```

## 4. 不变量与禁止

> [!IMPORTANT]
> 效果函数是纯函数: 不执行, 不采样, 不持有状态; 随机效果在观测点展开成"具体分支 + 精确概率" (D190) .
> 本模块不碰附着 (消耗按 D167 由伤害入口完成) , 也不把 `damage_bonus` 加进 `damage()` (D184 缺口) .

- 随机效果展开是临时的, 不写进状态 (D191) .
- 月反应覆写是反应部分级替换: 启用时月反应规则排在基础规则前, 原效果完全不执行 (D14 / D128 / D129) ; 不要加角色级启用字段.
- 身份比较一律 `same_reaction_identity` / `identity_address`.

## 5. 依据 (规则书与 D 编号) 

> [!NOTE]
> 行为依据只读规则书 (元素反应·规则 / 效果) + 卡面 + 权威关键字; 顺序依据 `结算顺序.md` §三.

| 反应 | 元素对 (已覆盖 / 本次攻击)  | 效果摘要 |
|---|---|---|
| 融化 | 冰/火 (任一先后)  | 本伤害 +2 |
| 蒸发 | 水/火 | 本伤害 +2 |
| 超载 | 雷/火 | 本伤害 +2; 目标为出战角色时强制切换下一个角色 |
| 超导 | 雷/冰 | 本伤害 +1; 目标以外所有敌方角色 1 点穿透伤害 |
| 感电 | 雷/水 | 本伤害 +1; 目标以外所有敌方角色 1 点穿透伤害 |
| 冻结 | 冰/水 | 本伤害 +1; 附属[冻结] (无法使用技能, 持续到回合结束; 受火/物理 +2 并移除)  |
| 扩散 | 风 + 冰/水/火/雷 | 目标以外所有敌方角色 1 点该元素伤害 |
| 结晶 | 岩 + 冰/水/火/雷 | 本伤害 +1; 我方出战角色 1 点护盾 (可叠加, 最多 2 点)  |
| 燃烧 / 绽放 / 原激化 | 火/草, 水/草, 雷/草 | 本伤害 +1; 分别生成燃烧烈焰 / 草原核 / 激化领域 |
| 月感电 / 月绽放 / 月结晶 | 水/雷, 水/草, 水/岩 | 月反应 (见 `reactions/lunar_*.rs`)  |

- D132: 反应触发条件 (同对元素两种先后) ; D167: 附着消耗; D184: 伤害修正链 / 反应效果执行仍由调用方接.
- D190 / D191: 观测展开 + 临时性; D14 / D128 / D129: 月反应覆写; 关键字"月感电"/"月绽放"/"月结晶".

## 6. 常见误用

> [!WARNING]
> 在反应效果里直接扣血 / 加状态会绕过统一执行器, 并让记录与结算不一致.

1. 在效果函数里直接扣血 / 加状态: 应产出 `EffectOp`, 由执行器转调已有底层入口.
2. 期望 `damage()` 自动加 `damage_bonus` 或执行 `branches`: 当前调用方必须在伤害前加值, 伤害后执行分支.
3. 随机效果采样后返回单结果: 违反 D190, 应返回所有分支 + 精确概率.
4. 把月反应当独立元素对的新反应: 它们与基础反应同元素对, 靠规则表顺序覆写.
5. 用 `std::ptr` 比反应身份: ZST 地址可重合, 应用 `same_reaction_identity`.

## 7. 测试

| 文件 | 测试 |
|---|---|
| 各反应文件 | `melt.rs::melt_adds_two_damage_with_no_extra_ops`, `crystallize.rs::crystallize_adds_one_and_shields_the_source_active_character`, `frozen.rs::frozen_adds_one_and_applies_frozen_status_to_the_reacted_character`, `lunar_charged.rs::lunar_charged_expands_random_hand_into_uniform_branches` |
| `reactions/mod.rs` | `melt_rule_plugs_into_the_existing_damage_entry`, `damage_entry_does_not_apply_the_bonus_yet` |
| `reactions/table.rs` / `entities.rs` | `EXPECTED_BONUS` 清单 (14 身份 + 加伤值) ; `entity_ids_are_the_rulebook_entities` |

## 8. 待接入与待实测

- `留空.md#52`: 反应效果执行时机 = 步边界 (D204) , 规则书顺序在同一步有后续 op 时可能不同 (D259) .
- `留空.md#53`: 反应效果是否应递归引发第二层反应未定.
- `留空.md#54` / `#55`: 非伤害附着的消耗口径, 风/岩不可附着形态是否判反应.
- `留空.md#56` / `#57` / `#58`: trace / record 路径不结算反应, pending 先后, 写回时手牌费用表同步.
- `留空.md#16`: 反应身份目前用名字字符串数据地址, 计划改成每反应一份静态定义数据.
- 结晶护盾"最多 2 点"与状态 203'没有上限'冲突暂按封顶 2 点 (`crystallize::SHIELD_STACK_CAP`) , 待实测.

---

最后核对: 2026-10-19 (代码: crates/definitions/src/reactions/table.rs:resolve) 