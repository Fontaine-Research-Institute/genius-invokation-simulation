# 定义层-Rules

> 中文 · [[定义层-Rules-EN|English]]

*本页范围: L1 牌族 / L2 类型 / L3 标签统一规则表 (`RuleRow` + `BaseRule`) 与 `rules/ops/` 纯函数的组织方式.*

---

## 1. 职责

把三级 (牌族 / 类型 / 标签) 的**基础操作**统一成一张静态规则表; 每行只声明机制, 阶段, 筛选角色与依据, 具体操作是 `rules/ops/` 下的纯函数.本层不持有状态, 不新建容器 (D20) .

## 2. 关键概念与类型

| 概念 | 文件 | 说明 |
|---|---|---|
| `Layer` / `RuleKind` / `Stage` / `QueryRole` / `RuleOp` | `crates/definitions/src/rules/mod.rs` | 层级; 机制 vs 仅分类; 生效阶段; 被谁当筛选条件; 用到哪组基础操作 |
| `RuleRow` | 同上 | 三级同一形状: `layer/kind/ops/stages/query_roles/evidence/basis` + 可选数据 (槽位 / 数字 / 开局钩子 / 伤害角色 / 查询策略等)  |
| `BaseRule` | 同上 | 统一访问器, 唯一实现 = `RuleRow`; 消费者只依赖 trait |
| `Evidence` / `NumRule` / `OpeningSpec` / `DamageRole` / `RuleName` | 同上 | 卡 id + 一句话卡文; 规则数字 (Nat) ; 开局钩子; 护盾 / 减伤角色; 行名从 generated 或唯一映射取 |
| 三级表 | `families.rs` / `types.rs` / `tags_action.rs` / `tags_character.rs` / `tags_summon_enemy.rs` | 各自 `pub static ROWS: &[RuleRow]`; `rows()` 按牌族 -> 类型 -> 标签拼接 |
| 基础操作 | `crates/definitions/src/rules/ops/*.rs` | 14 个纯函数模块: `equip` / `support` / `damage` / `lifetime` / `opening` / `order` / `query` / `resonance` / `adventure` / `simulanka` / `arkhe` / `stage` / `food` / `arcane` |

## 3. 公开入口

| 入口 | 文件 | 说明 |
|---|---|---|
| `rows()` / `all_rows()` | `rules/mod.rs` | 遍历全表 (牌族 -> 类型 -> 标签)  |
| `row_of_tag` / `row_of_family` / `row_of_action_card_type` / `row_of_status_type` | 同上 | 按标签 / 牌族 / 行动牌类型 / 状态类型查行 |
| `family_name(family)` | 同上 | 牌族显示名的唯一映射 |
| `trait BaseRule` | 同上 | `layer/name/kind/ops/stages/query_roles/evidence/basis/tag/slots/nums/opening/support_cap/damage/query_policy` |
| `rules::ops::*` | `rules/ops/` | 与 `RuleOp` 一一对应的纯函数模块 |

### 源码 

> [!NOTE]
> 以下为规则表与 ops 的真实源码摘录 (签名 + 文档注释; ops 函数体从略, 完整实现见各文件) .

**`crates/definitions/src/rules/mod.rs(Layer / RuleKind / Stage / QueryRole / RuleOp)`**

```rust

/// 规则所属层级（L1 / L2 / L3，同一张表混装）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// L1：牌族（行动牌 / 角色 / 召唤物 / 状态 / 敌人）。
    Family,
    /// L2：类型（支援 / 装备 / 事件 / 角色状态 / 出战状态）。
    CardType,
    /// L3：标签（数据里真实出现的 52 个，本表实现 51 个，排除自走牌）。
    Tag,
}


/// 规则种类。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleKind {
    /// 有基础操作 / 规则（槽位、上限、开局、伤害、查询策略…）。
    Mechanism,
    /// **仅分类、无操作**：只是数据里的一个分类标签（可能被当查询条件）。
    Classification,
}


/// 规则在哪些阶段生效（同一行可多个）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// 开局（起手 / 首发 / 元素幻变的「行动阶段开始时」）。
    Opening,
    /// 投掷阶段（元素幻变：每轮固定 2+2）。
    RollPhase,
    /// 行动阶段（打出 / 装备 / 区域进出 / 料理等）。
    ActionPhase,
    /// 伤害结算（护盾吸收 / 减伤扣减）。
    Damage,
    /// 查询 / 检索（不可发现 / 弃用）。
    Query,
    /// 建局前（组牌门槛：元素共鸣 / 国家共鸣）。
    DeckBuilding,
}

///
/// 纯分类标签也会列出一到两个查询角色 —— 这就是它们**唯一的用途**，不代表有操作。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum QueryRole {
    /// 角色元素（用于元素共鸣组牌、元素幻变队伍条件）。
    TeamElement,
    /// 元素共鸣：牌组至少 2 名同元素角色。
    ResonanceElement,
    /// 武器类型门（「『X』角色才能装备」）。
    WeaponTypeGate,
    /// 牌种条件（「从 3 张『场地』/『伙伴』/『道具』中挑选 1 张」类效果）。
    CardKindGate,
    /// 势力组牌门槛（「牌组包含至少 2 个『愚人众』/『魔物』角色」）。
    CampDeckGate,
    /// 元素幻变队伍条件（恰含两种元素）。
    TransmutationCondition,
    /// 敌人分类（技能 / 效果按此筛选）。
    EnemyClass,
    /// 召唤物分类（效果按此筛选）。
    SummonClass,
    /// 始基力:荒性形态 / 始基力:芒性形态（荒芒湮灭效应）。
    Arkhe,
}

/// 加这个字段是为了让「规则行 -> 纯函数模块」的对应关系写在数据里，
/// 消费者可以按操作反查（如找所有参与伤害结算的行），而不是在代码里散落 if 特例。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RuleOp {
    /// 开局流程（ops::opening；元素幻变 / 起手 / 首发）。
    Opening,
    /// 装备槽与互斥 / 替换（ops::equip）。
    Equip,
    /// 支援区上限（ops::support）。
    Support,
    /// 料理与饱腹（ops::food）。
    Food,
    /// 秘传与秘传揭令（ops::arcane）。
    Arcane,
    /// 行动权：战斗行动 / 快速行动（ops::stage）。
    Timing,
    /// 护盾 / 减伤（ops::damage）。
    Damage,
    /// 查询策略：不可发现 / 弃用（ops::query）。
    Query,
    /// 元素 / 国家共鸣组牌门槛（ops::resonance）。
    Resonance,
    /// 冒险动作（ops::adventure）。
    Adventure,
    /// 希穆兰卡集合 / 随机生成范围（ops::simulanka）。
    Simulanka,
    /// 始基力:荒性形态 / 始基力:芒性形态对立（ops::arkhe）。
    Arkhe,
    /// 可用次数 / 持续回合（ops::lifetime；规则书·特殊词条）。
    Lifetime,
    /// 结算顺序（ops::order；规则书·其他规则·结算顺序）。
    Order,
}
```

**`crates/definitions/src/rules/mod.rs(RuleRow 与 BaseRule)`**

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

/// 统一访问器：三级共用这一组签名（唯一实现 = RuleRow）。
///
/// 消费者（区域块 / 测试 / 卡牌）只依赖这个 trait，不直接依赖行结构。
pub trait BaseRule {
    /// 所属层级。
    fn layer(&self) -> Layer;
    /// 行名：统一从既有数据源取（标签 / 类型取自 generated，牌族走唯一映射）。
    fn name(&self) -> RuleName;

    /// 行名的中文（日志 / 测试便利方法）。
    fn name_zh(&self) -> &'static str {
        self.name().zh()
    }
    /// 机制 / 仅分类。
    fn kind(&self) -> RuleKind;
    /// 本行用到的基础操作。
    fn ops(&self) -> &'static [RuleOp];
    /// 生效阶段。
    fn stages(&self) -> &'static [Stage];
    /// 作为筛选条件被谁引用。
    fn query_roles(&self) -> &'static [QueryRole];
    /// 依据卡。
    fn evidence(&self) -> &'static [Evidence];
    /// 规格 / 决策依据。
    fn basis(&self) -> &'static [&'static str];
    /// 标签静态数据（非标签行 -> None）。
    fn tag(&self) -> Option<&'static TagText>;
    /// 槽位规则。
    fn slots(&self) -> &'static [SlotRule];
    /// 规则数字。
    fn nums(&self) -> &'static [NumRule];
    /// 开局钩子。
    fn opening(&self) -> Option<OpeningSpec>;
    /// 支援区上限。
    fn support_cap(&self) -> Option<Nat>;
    /// 伤害结算角色。
    fn damage(&self) -> Option<DamageRole>;
    /// 查询策略。
    fn query_policy(&self) -> Option<QueryPolicy>;
    /// 是不是「仅分类、无操作」。
    fn is_classification_only(&self) -> bool {
        self.kind() == RuleKind::Classification
    }
}
```
#### ops 函数签名

**`crates/definitions/src/rules/ops/adventure.rs`**

```rust
/// 冒险动作（纯函数）：`existing_spot` = 己方是否已有冒险地点。
pub fn resolve(existing_spot: bool) -> AdventureOutcome
```

**`crates/definitions/src/rules/ops/arcane.rs`**

```rust
/// 是否「所有秘传都在起始手牌中」：牌组里的秘传数不超过起始手牌数时成立（规则书第 2 条）。
pub fn all_arcane_guaranteed(arcane_in_deck: Nat, opening_hand: Nat) -> bool

/// 起始手牌中出现的秘传张数：等于秘传总数；超过起始手牌数时取起始手牌数（随机抽）。
pub fn arcane_in_opening_hand(arcane_in_deck: Nat, opening_hand: Nat) -> Nat

/// 秘传：起手一定在手上（用于把「秘传」标签转成起手抓牌谓词）。
pub fn guaranteed_in_opening_hand() -> bool

/// 打出判定：played_this_game = 本局该牌手已打出的秘传张数，
/// edict_available = 秘传揭令是否仍存在。
pub fn can_play(played_this_game: Nat, edict_available: bool) -> ArcanePlay

/// 打出后是否消耗秘传揭令（关键字 #70：会消耗）。
pub fn consumes_edict() -> bool
```

**`crates/definitions/src/rules/ops/arkhe.rs`**

```rust
/// 相反极（荒 <-> 芒；关键字 #87 / #88）。
pub fn opposite(arkhe: Arkhe) -> Arkhe

/// 受到 `incoming` 极的伤害后，会不会发生荒芒湮灭效应（自身极必须是 incoming 的相反极）。
pub fn collides_with(self_arkhe: Arkhe, incoming: Arkhe) -> bool
```

**`crates/definitions/src/rules/ops/damage.rs`**

```rust
/// 伤害修正（纯函数）：先护盾吸收，再减伤扣减；穿透伤害两者都不吃。
///
/// - `shield`：当前护盾点数（可叠加后的总量）；
/// - `reductions`：本次伤害适用的各条减伤数值（按调用方给定的顺序相加；顺序不影响总和）。
pub fn mitigate(damage: Nat, ty: DamageType, shield: Nat, reductions: &[Nat]) -> Mitigation
```

**`crates/definitions/src/rules/ops/equip.rs`**

```rust
/// 某个武器子类的槽位规则：每角色 1 件，且角色必须是该武器类型。
///
/// 依据：武器卡面「『X』角色才能装备。角色最多装备 1 件『武器』」
/// （弓 311204 / 单手剑 311503 / 法器 311104 / 双手剑 311306 / 长柄武器 311402）。
pub const fn weapon_subclass_slot(weapon_type: WeaponType) -> SlotRule

    /// 用武器类型构造。
    pub const fn with_weapon(weapon: WeaponType) -> CharacterEquip

/// 装备判定（纯函数，不改任何状态）。
///
/// - `occupied`：该角色当前已占用的槽位（每槽位最多出现一次；调用方按需给）；
/// - `same_kind_present`：天赋专用 —— 该角色是否已装备**同种**（同卡 id / 同名）天赋。
pub fn check_equip(
    card: &EquipCard,
    character: &CharacterEquip,
    occupied: &[SlotKind],
    same_kind_present: bool,
) -> EquipOutcome
```

**`crates/definitions/src/rules/ops/food.rs`**

```rust
/// 该角色本回合已食用 eaten_this_round 次时的食用判定。
pub fn eat(eaten_this_round: Nat) -> EatOutcome

/// 处于饱腹状态的角色**不能成为**其他料理事件牌的目标（规则书第 2 条）。
pub fn can_be_targeted_by_food(satiated: bool) -> bool

/// 处于饱腹状态的角色**不会受到**其他料理事件牌的效果影响（规则书第 2 条）。
pub fn can_be_affected_by_food(satiated: bool) -> bool

/// 本回合已通过料理复苏 revived_this_round 个角色时，还能不能复苏（规则书第 4 条）。
pub fn can_revive(revived_this_round: Nat) -> bool
```

**`crates/definitions/src/rules/ops/lifetime.rs`**

```rust
/// 触发一次后的可用次数（减到 0 为止；调用方随后弃置）。
pub fn uses_after_trigger(uses: Nat) -> Nat

/// 是否耗尽（= 0 -> 立刻弃置）。
pub fn is_depleted(value: Nat) -> bool

/// 经过一个结束阶段后的持续回合（减到 0 为止）。
pub fn duration_after_end_phase(duration: Nat) -> Nat

/// 持续回合的递减发生在**结束阶段**（不是回合开始、也不是行动阶段开始）。
///
/// 规则书原文：「每个结束阶段，都会使『持续回合』-1」；英文 "Each End Phase"。
/// 注意：这与决策日志 D13 的旧写法（回合开始递减）冲突，已按规则书为准改口径。
pub fn ticks_in_end_phase() -> bool
```

**`crates/definitions/src/rules/ops/opening.rs`**

```rust
/// 游戏准备的执行顺序。
pub fn pregame_steps() -> [PregameStep; 3]

    /// 用两种元素构造（不从 `new` 起名：本类型没有「默认值」，不实现 Default）。
    pub const fn from_elements(elements: [Element; 2]) -> ElementalTransmutation

    /// 条件（关键字 #23）：队伍元素集合**恰等于** {X, Y}。
    ///
    /// 「恰等于」= 两种元素各至少 1 名、且没有第三种元素的角色。
    pub fn team_matches(&self, team: &[Element]) -> bool

    /// 投掷阶段（每轮，牌已在场）：总是投出 2 个 X + 2 个 Y。
    pub fn roll_phase_dice(&self) -> [(Element, Nat); 2]

    /// 开局步骤（首次、第 1 轮行动阶段开始时；顺序即结算顺序）。
    pub fn opening_steps(&self) -> [OpeningStep; 5]

    /// 开局瞬间的骰子转化：把 4 颗骰子转化为该两元素各 2 颗（总数不变）。
    ///
    /// 实现复用 `resources::dice::DicePool::convert`（确定性扣减，D144；选哪颗骰的分支由上层做）：
    /// 先把 4 颗**任意来源**的骰子转成第一种元素，再把其中 2 颗转成第二种元素。
    /// **为什么分两步**：`convert` 一次只能给一个目标；两步都写「任意来源」会把刚转出来的
    /// 骰子当成第二轮的来源（`take_any` 先扣具体色），净效果就不是 2+2 —— 第二步显式指定来源。
    /// 骰子不足 4 颗 -> `None`（不改状态）。
    /// 待实测：转化固定 2+2（Lead 裁定 3）、只处理牌库中的牌（裁定 4）。
    pub fn convert_opening_dice(&self, pool: &DicePool) -> Option<DicePool>

    /// 投掷阶段（每轮）的固定骰：与开局转化同一套运算（「总是投出 2+2」⇔「4 颗骰转成 2+2」）。
    pub fn apply_roll_phase(&self, pool: &DicePool) -> Option<DicePool>

    /// 触发（第三行）：弃置此牌，并从 2 张指定「祝佑」中挑 1 张加入手牌。
    ///
    /// 候选具体是哪两张由**具体卡**给出（规则层不硬编码具体卡），这里只给操作形状。
    pub fn on_reaction_triggered(&self) -> TransmutationTrigger

/// 是不是第 1 轮（「首次生效」的判据；Lead 裁定 2）。
///
/// 轮次从 1 起算；第 1 轮投掷阶段牌还在牌库，所以开局那套只在第 1 轮的行动阶段开始时执行。
pub fn is_opening_round(round: Nat) -> bool
```

**`crates/definitions/src/rules/ops/order.rs`**

```rust
/// 同一时间点、同一牌手内：first 是否先于 second 结算（规则书第 2 条）。
///
/// 同一类别 -> false（同类按进场先后，规则书第 3 条，见 same_source_uses_entry_order）。
pub fn resolves_before(first: EffectSource, second: EffectSource) -> bool

/// 同一牌手、同一类别：按**进场先后**结算（规则书第 3 条；进场顺序由区域状态持有）。
pub fn same_source_uses_entry_order() -> bool
```

**`crates/definitions/src/rules/ops/resonance.rs`**

```rust
/// 统计队伍里某元素的角色数量。
pub fn characters_of_element(team: &[Element], element: Element) -> Nat

/// 元素共鸣组牌门槛：队伍里该元素角色数 >= 2。
pub fn element_resonance_allows(team: &[Element], element: Element) -> bool

/// 国家共鸣组牌门槛：同国家角色数 >= 2（国家标签只做查询条件，不建枚举）。
pub fn country_resonance_allows(same_country_count: Nat) -> bool
```

**`crates/definitions/src/rules/ops/simulanka.rs`**

```rust
/// 召唤物种类数。
pub fn summon_kind_count() -> Nat

/// 随机生成的取值范围（具体集合由调用方按标签查询得到）。
pub fn random_pick_scope() -> RandomPickScope
```

**`crates/definitions/src/rules/ops/stage.rs`**

```rust
/// 这次行动结算后是否轮到对方（规则书：战斗行动 -> 轮到对方；快速行动 -> 继续）。
pub fn passes_turn(timing: Timing) -> bool

/// 按规则书给「行动种类」定行动权（使用技能 / 切换角色 / 宣布结束 = 战斗行动）。
pub fn timing_of_action(kind: ActionKind) -> Timing

/// 这次行动是不是战斗行动。
pub fn is_combat_action(kind: ActionKind) -> bool

/// 打出一张**行动牌**的行动权：默认快速行动；带「战斗行动」标签时改为战斗行动
/// （规则书·特殊词条 战斗行动（行动牌））。
pub fn timing_of_card_play(has_combat_action_tag: bool) -> Timing

/// 下一个阶段（结束阶段之后回到投掷阶段）。
pub fn next_phase(phase: RoundPhase) -> RoundPhase
```

**`crates/definitions/src/rules/ops/support.rs`**

```rust
/// 进场判定（纯函数）。
pub fn support_enter(occupied: Nat) -> SupportEnter

/// 离场操作形状（具体移除由支援区落点实现）。
pub fn support_exit() -> SupportExit
```


## 4. 不变量与禁止

> [!IMPORTANT]
> 本层不持有状态, 不新建容器: 区域状态归各区块, ops 只做"判定 + 结果" (D20) .
> 每行必须有 `evidence` 或 `basis`; 机制行必须声明 `ops`, 仅分类行不得声明操作.

- 标签行覆盖除"自走牌"外的全部标签; 自走牌不建行, 不实现.
- 数字一律 `Nat`; 本层不做概率运算, 随机只给范围 / 集合, 抽样走引擎.
- 行名不另存: 标签 / 类型取自 generated 文本指针; 牌族名只走 `family_name`.
- 新机制先查既有 `RuleOp` / 纯函数模块, 不要绕开表散落特例.

## 5. 依据 (规则书与 D 编号) 

> [!NOTE]
> 依据优先级: 规则书 > 权威关键字 > 卡面文本; 不读参考实现 (D122) .
> 规则书与旧决策冲突时以规则书为准 (如持续回合已改为结束阶段) .

| 编号 | 结论 |
|---|---|
| D20 | 框架只给接口; 判定 / 结果用纯函数, 不持有状态 |
| D122 | `rules/ops/` 的组织方式与依据来源 |
| D108 | 本层不解释具体状态语义, 只声明机制 |
| D240 | 具体卡牌效果不在本层 (在 cards crate)  |

## 6. 常见误用

> [!WARNING]
> 绕过规则表写特例会同时破坏"可遍历 / 可测试"两个性质, 测试会钉死表覆盖度.

1. 在 ops 里新建区域 / 缓存状态: 违反 D20.
2. 直接 match `RuleRow` 字段写特例: 应使用 `BaseRule` 访问器.
3. 给"仅分类"标签声明 `ops`: 测试失败; 分类标签只有 `query_roles`.
4. 新增标签忘了在 `tags_*/ROWS` 加行: `tag_rows_cover_every_tag_except_automaton` 会抓出.
5. 拿 `evidence` 的卡 id 做运算: 它只用于审计 / 展示.

## 7. 测试

| 文件 | 测试 |
|---|---|
| `rules/mod.rs` | `table_covers_all_three_layers`, `tag_rows_cover_every_tag_except_automaton`, `every_row_has_evidence_or_basis`, `classification_rows_have_no_operations`, `mechanism_rows_declare_operations`, `every_rule_op_is_declared_by_some_row`, `lookups_work_for_families_and_types`, `numeric_rules_are_positive`, `query_policy_rows_match_query_module`, `opening_hooks_are_attached_to_expected_rows` |
| `rules/ops/*.rs` | 如 `equip.rs::matching_weapon_attaches_to_empty_slot`, `support.rs::support_zone_cap_is_four`, `lifetime.rs::duration_ticks_at_end_phase_only`, `order.rs::five_sources_are_in_rulebook_order`, `damage.rs::shield_absorbs_then_reduction_subtracts` |

## 8. 待接入与待实测

- `留空.md#15`: 三级统一层仍有需状态块的接线项 (结算顺序调度, 结束阶段扣持续回合 / 可用次数, 起手换牌, 首发选择, 击倒 / 免于击倒 / 转换形态, 支援满 4 分支, 召唤物满 4, 元素幻变时序) .
- `留空.md#63`: 自行巧局 (蓄力 / 额外行动 / 等级突破) 不并入常规流程, 本阶段不做.
- `留空.md#66`: F7 时机调用点 / F10 结束阶段顺序两处待复核.
- `留空.md#70` / `#71`: Z15 超限处理方式, 召唤物"层数 vs 可用次数"口径待实测.

---

最后核对: 2026-10-19 (代码: crates/definitions/src/rules/mod.rs:RuleRow) 