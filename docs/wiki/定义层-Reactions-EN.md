# Definitions Layer: Reactions

> [[定义层-Reactions|中文]] · English

*Scope: the 14 elemental reaction identities, the rule / effect tables, `table::resolve`, and pure-function reaction effects with `EffectOp` requests.*

---


## 1. Responsibility

Fourteen elemental reaction identities + an effect table + pure-function effects: given a reaction event and a minimal position summary, output "this damage +N" and a list of low-level operations with exact probabilities. They do not settle anything themselves, do not sample, and do not touch attachments (handled by the damage entry point per D167). See inserted settlements in [[定义层-Timeline-EN]] and the record contract in [[定义层-Records-EN]].

## 2. Key concepts and types

- ---: ---; ---
- `ReactionIdentity`: Reaction identity trait; compare with `identity_address()`, never `&dyn` addresses (文件: `crates/definitions/src/families/character/attachment.rs`)
- The 14 identities: One ZST + static instance per file: Melt / Vaporize / Overloaded / Superconduct / Electro-Charged / Frozen / Swirl / Crystallize / Burning / Bloom / Quicken / Lunar-Charged / Lunar-Bloom / Lunar-Crystallize (文件: `crates/definitions/src/reactions/*.rs`)
- `CONDITIONS`: each reaction file; `[ReactionCondition; N]`: one entry per order of the same element pair (8 for Swirl / Crystallize)
- `table`: `base_rules` / `rules` / `EFFECTS` / `effect_of` / `resolve` / `is_lunar` / `rule_matches` (文件: `reactions/table.rs`)
- Effect types: `ReactionEffectResult { damage_bonus, branches }`, `ReactionBranch { ops, probability: Fraction }`, `EffectContext`, `Side` (文件: `reactions/effect.rs`)
- `EffectOp`: same; Low-level requests: `Damage` / `Status` / `Summon` / `ForceSwitch` / `AddCardToHand` / `HandCardStatus`
- `entities`: Entity ids generated / attached by reactions (Frozen 106, Shield 203, Dendro Core 116, Catalyzing Field 117, Burning Flame 115, Thundercloud 205, Moondrift 212, Moondrift Harmony 211) (文件: `reactions/entities.rs`)

## 3. Public entry points

- ---: ---; ---
- `base_rules() -> Vec<ReactionRule>`: The 11 base reaction rules (文件: `reactions/table.rs`)
- `rules(overrides: LunarOverrides) -> Vec<ReactionRule>`: same; Enabled lunar rules first (they override base reactions)
- `effect_table() -> &'static [EffectEntry]`: same; 14 effect entries
- `effect_of(reaction)`: same; Look up the effect function pointer by identity data address
- `resolve(event, context) -> Option<ReactionEffectResult>`: same; Compute the effect of an already determined reaction
- `is_lunar(reaction)` / `rule_matches(rule, attachment, attack)`: same; Whether it is lunar; whether a rule condition matches

### Source

> [!NOTE]
> Real reaction-layer source excerpts (types / trait / table function signatures + doc comments; bodies omitted).

The interfaces are listed per file; the file path is shown above each code block.

File: `crates/definitions/src/families/character/attachment.rs`

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

The file's current public interface is shown above; bodies are omitted.

File: `crates/definitions/src/reactions/effect.rs`

*Keyword codes in comments have been replaced with names; code lines are unchanged.*

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

The file's current public interface is shown above; bodies are omitted.

File: `crates/definitions/src/reactions/effect.rs`

```rust
/// 效果函数指针（D186：登记的是**执行函数指针**，不是效果数据 / trait object / 闭包）。
///
/// 参数：
/// - `&ReactionEvent`：谁触发 / 作用在谁身上 / 命中的已覆盖元素 / 本次攻击元素；
/// - `&EffectContext`：本次效果需要的最小局面摘要（是否出战角色、双方手牌数）。
pub type ReactionEffectFn =
    fn(&crate::families::character::attachment::ReactionEvent, &EffectContext) -> ReactionEffectResult;
```

The file's current public interface is shown above; bodies are omitted.

File: `crates/definitions/src/reactions/table.rs`

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

The file's current public interface is shown above; bodies are omitted.

File: `crates/definitions/src/reactions/melt.rs`

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

The file's current public interface is shown above; bodies are omitted.

## 4. Invariants and prohibitions

> [!IMPORTANT]
> Effect functions are pure: they never execute, sample, or hold state; random effects are expanded at the observation point into "concrete branches + exact probabilities" (D190).
> This module does not touch attachments (D167, done by the damage entry point) and does not add `damage_bonus` into `damage()` (D184 gap).

- Random expansion is temporary and never written into state (D191).
- Lunar overrides are reaction-level replacement: enabled lunar rules precede base rules, so the original effect is not executed at all (D14 / D128 / D129); no character-level enable flags.
- Identity comparison always uses `same_reaction_identity` / `identity_address`.

## 5. Basis (rulebook and D-numbers)

> [!NOTE]
> Behavior evidence comes only from the rulebook (elemental reactions · rules / effects) + card text + authoritative keywords; order follows `结算顺序.md` §3.

- ---: ---; ---
- Melt: Cryo/Pyro (either order); This damage +2
- Vaporize: Hydro/Pyro; This damage +2
- Overloaded: Electro/Pyro; This damage +2; force-switch to the next character if the target is active
- Superconduct: Electro/Cryo; This damage +1; 1 piercing DMG to all enemy characters other than the target
- Electro-Charged: Electro/Hydro; This damage +1; 1 piercing DMG to all enemy characters other than the target
- Frozen: Cryo/Hydro; This damage +1; attach [Frozen] (cannot use skills until end of round; removed with +2 damage when hit by Pyro or Physical DMG)
- Swirl: Anemo + Cryo/Hydro/Pyro/Electro; 1 DMG of that element to all enemy characters other than the target
- Crystallize: Geo + Cryo/Hydro/Pyro/Electro; This damage +1; 1 shield on our active character (stackable, up to 2)
- Burning / Bloom / Quicken: Pyro/Dendro, Hydro/Dendro, Electro/Dendro; This damage +1; create Burning Flame / Dendro Core / Catalyzing Field respectively
- Lunar-Charged / Lunar-Bloom / Lunar-Crystallize: Hydro/Electro, Hydro/Dendro, Hydro/Geo; Lunar reactions (see `reactions/lunar_*.rs`)

- D132: reaction trigger conditions; D167: attachment consumption; D184: the damage-modifier chain / reaction execution is still wired by the caller.
- D190 / D191: observation expansion + temporariness; D14 / D128 / D129: lunar overrides; keywords "Lunar-Charged" / "Lunar-Bloom" / "Lunar-Crystallize".

## 6. Common misuse

> [!WARNING]
> Deducting HP / attaching statuses directly in a reaction effect bypasses the unified executor and desynchronizes records from settlement.

1. Deducting HP / attaching statuses in the effect function: produce `EffectOp` and let the executor call the existing entries.
2. Expecting `damage()` to add `damage_bonus` or execute `branches`: the caller must add the bonus before damage and execute branches after.
3. Sampling a random effect and returning one result: violates D190; return all branches with exact probabilities.
4. Treating lunar reactions as new element pairs: they share the base pairs and override through rule-table order.
5. Comparing reaction identities with `std::ptr`: ZST addresses collide; use `same_reaction_identity`.

## 7. Tests

- Each reaction file: `melt.rs::melt_adds_two_damage_with_no_extra_ops`, `crystallize.rs::crystallize_adds_one_and_shields_the_source_active_character`, `frozen.rs::frozen_adds_one_and_applies_frozen_status_to_the_reacted_character`, `lunar_charged.rs::lunar_charged_expands_random_hand_into_uniform_branches`
- `reactions/mod.rs`: `melt_rule_plugs_into_the_existing_damage_entry`, `damage_entry_does_not_apply_the_bonus_yet`
- `reactions/table.rs` / `entities.rs`: `EXPECTED_BONUS` table (14 identities + bonus values); `entity_ids_are_the_rulebook_entities`

## 8. Not yet wired and to be verified

- `留空.md#52`: reaction effects execute at step boundaries (D204); the rulebook order may differ when the same step has later ops (D259).
- `留空.md#53`: whether reaction effects should recursively trigger a second layer is undecided.
- `留空.md#54` / `#55`: non-damage attachment consumption, and whether non-attachable Anemo/Geo forms trigger reactions.
- `留空.md#56` / `#57` / `#58`: trace / record paths do not settle reactions, pending order, and hand-card cost table synchronization on write-back.
- `留空.md#16`: reaction identities currently use the name-string data address; the plan is one static definition per reaction.
- Crystallize's "up to 2" shield conflicts with status 203's "no upper limit"; currently capped at 2 (`crystallize::SHIELD_STACK_CAP`), pending in-game verification.

## Summary

The 14 reaction identities are determined by element pairs, and effect functions are pure, returning this-damage bonuses plus low-level operation branches. Random effects expand at the observation point into branches with exact probabilities; they neither sample nor write back state. table::resolve only returns effects; attachment consumption and actual execution belong to the damage entry point and executor.

---



Last verified: 2026-10-19 (code: crates/definitions/src/reactions/table.rs:resolve)