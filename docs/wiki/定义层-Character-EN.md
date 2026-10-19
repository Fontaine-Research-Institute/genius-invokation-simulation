# Definitions: Character

> [[定义层-Character|中文]] · English

*Scope: `crates/definitions/src/families/character/` — definition carriers, runtime state, the three zones, the damage chain, the skill flow, timings, and form / growth / satiety.*

---

## 1. Responsibility

The character card family: **definition carriers** (`CharacterDef` / `SkillDef`) + **runtime state** (`CharacterState`) + **three zones** (character zone / equipment zone / combat status zone) + **damage chain** + **skill flow** + timings / form / growth / satiety. No high-level semantic wrappers (D133), no "boolean energy gain" (D138), no concrete cards (D240).

## 2. Key concepts and types

### 2.1 Definition carriers

| Type | File | Notes |
|---|---|---|
| `CharacterDef<Tags>` | `definition.rs` | `id` / `name` / `tags` (generic tag set) / `max_hp` / `max_energy` / `skills: &'static [&'static SkillDef]`; caps are **definition data**, runtime current values live in `CharacterState` |
| `SkillDef` / `SkillCost` | `skill.rs` | `id` (= character id × 10 + index) / text / `kind: SkillKind` / `cost: SkillCost { dice: &'static [CostComponent], energy: Nat }` / `effect: SkillEffect = fn()` (**placeholder signature**, D162) |
| `SkillKind` / `SpecialNormalAttack` / `DamageType` | `types.rs` | Four skill kinds: `NormalAttack(Option<SpecialNormalAttack>)` (`None` = plain normal attack / `Some(Charged)` = charged / `Some(Plunging)` = plunging) / `ElementalSkill` / `ElementalBurst` / `Passive`; damage types: `Elemental(Element)` / `Physical` / `Piercing` |

> [!NOTE]
> Charged / plunging attacks are not separate skill kinds (D123 / D126); they still count as normal attacks (D124). To test "affects normal attacks", use `is_normal_attack`.

### 2.2 Runtime state (`character.rs`)

- `hp: Nat` / `max_hp: Nat`: `hp <= max_hp` (clamped in the constructor and every write).
- `energy: Nat` / `max_energy: Nat`: "spend all energy" for bursts is expressed by card data.
- `states: StateList`: D105 all statuses share one list; booleans use `layers=None`.
- `attachment: Attachment`: D167 boolean element set (multi-element, no stacks; not a status).
- **Verbatim source: §3.2** (`crates/definitions/src/families/character/character.rs`).

- Pure functions: `hp_add / hp_sub / hp_set`, `energy_add / energy_sub / energy_set`, `from_def`; all saturating `Nat`, inputs untouched.
- **Does not do** "energy gain (boolean)" or "HP changed = defeated" semantics (D138 / D133); triggered operations live in `trigger.rs`.

### 2.3 The three zones (D42: zone = field; sides = two instances)

| Zone | File | Key rules |
|---|---|---|
| Character zone `CharacterZone` | `zones/field.rs` | `CHARACTER_SLOTS = 3`; `slots` order = entry order (switching does not reorder); **the active character is an independent pointer** (`active()` / `active_entity()`), not slot 0; timings `Entered / Left / SwitchOut / SwitchIn / ActiveChanged` |
| Equipment zone `EquipmentZone` | `zones/equipment.rs` | **One per on-field character**; slots `SlotKind { Weapon, Artifact, Talent, Technique }`; exclusivity / weapon-type gate reuse `rules::ops::equip` (D16); occupied slot → forced replace and discard the old equipment; talents overwrite by identical card; timings `Equipped / Discarded` |
| Combat status zone `CombatStatusZone` | `zones/combat_status.rs` | Attached to the active character (`attached_to`); switching **only changes attached_to, entries stay put**; re-attaching the same status adds layers and keeps the old duration; End Phase `tick_end_phase_with_triggers`; timings `Attached / Transferred / Removed / DurationTicked / Depleted` |

### 2.4 Damage chain (`damage_chain/`)

`DamageChainEnv` (`chain.rs`) borrows read-only pointers only:

```text
combat_status / character_states   // carriers read for shields, reductions, immunity (both are read)
shields / reductions / immunities  // status identity sets (default statuses::*_STATUSES)
modifiers: DamageModifiers         // AmountOp four-stage modifiers + rounding direction
type_override: Option<DamageType>  // type override (physical -> element etc.; before reaction check, 留空.md #110/#120)
```

**Settlement order**: immunity (zero it out; piercing ignores it) → bonuses → multiplication / division → shields → reductions → HP loss (only yields `to_hp`; does not subtract HP).

| Stage | Implementation | Notes |
|---|---|---|
| 0 Immunity | `statuses::IMMUNITY_STATUSES` + `step_immunity` | "immune to all damage" → short-circuits into "base → immune → HP loss 0"; **piercing ignores immunity** (K5) |
| 1 Bonuses | `bonus::DamageBonus` + `step_bonus` | generic increases + reaction bonus + elemental mastery (`MASTERY_DAMAGE_PER_POINT` to be measured); piercing skips the whole stage |
| 2 Multiplication / division | `bonus::{AmountOp, MulMode, MultiplierZone, Rounding}` + `step_scale` | Four-stage model (plan §4.4.5): in-zone add/subtract → standalone multipliers + multiplier zone (multipliers added) → multiply all divisors into one denominator, divide and round → out-of-zone add/subtract; division by zero panics (D44) |
| 3 Shields | `rules::ops::damage::mitigate` + `step_shield` | combat status 203 + character status 134 points are summed; piercing ignores |
| 4 Reductions | same + `step_reduction` | "damage negation" entries are summed and applied once; piercing ignores |
| 5 HP loss | `step_to_hp` | the result is handed to `damage()` (attachment / reaction check / triggers unchanged) |

- Insert-style settlement (D196): `schedule()` returns `ScheduledChain` (a `timeline::Resolution` frame + cursor); supports `insert` of sub-settlements and `snapshot()` for debugging.
- Result: `ChainStage` (Base / Bypassed / Boosted / Immune / Scaled / Shielded / Reduced / ToHp), `ChainOutcome` (`bonus_total` / `damage_after_bonus` / `after_scale` / `shield_used` / `reduction_total` / `to_hp` / `bypassed` / `immune` / `immunity_available` / `scale` / `stages`).
- Records (D221 / D223 / D224 / D225): `settle_records` / `damage_with_chain_recorded` write `DamageChainRecord` (Hit / TypeOverride / Increase / Mastery / ReactionBonus / BonusTotal / PiercingBypass / Immune / Scale / Shield / Reduction / ToHp / Reaction), **with the real values at the computation site**, never derived from before/after diffs; disabling the `records` feature costs nothing.

**Low-level damage entry** (`damage.rs`): `damage()` = HP loss + elemental attachment + reaction check / consuming participating elements + trigger registration (D164 / D167); `damage_with_triggers` additionally dispatches "HP changed"; `damage_with_chain` (and `_recorded`) runs the damage chain first and then calls `damage()`. Attachment and reaction registration live in `attachment.rs` (`Attachment` is a boolean set; `ReactionRegistry` supports registration by kind / by element pair / any-callback).

### 2.5 Skill flow (`skill_flow/`)

| Module | Contents |
|---|---|
| `mod.rs` | `SkillUseContext` / `SkillUsePolicy` / `can_use` / `pay_skill_cost` / `skill_energy_gain` / `use_skill` / `use_skill_with_plunging` / `SkillUse { skill, effective_kind, dice, character, uses, before, after }` |
| `attack_form.rs` | "Counts as" rulings of S8.9 / S8.10: `AttackForm{Normal,Charged,Plunging}`, `AttackFormInputs{dice_total,plunging_pending}`, `is_even`, `effective_normal_attack`, `PlungingMark` |
| `forbidden.rs` | R5 query "which skills does a status forbid": `SKILL_FORBIDDING_STATUSES` (frozen / stun / petrified), `ForbiddenUses`, `ForbiddenState`, `forbidden_by` / `skill_allowed` / `technique_allowed` |
| `technique.rs` | L4 technique flow: `SkillUseClass` + `UseFlow`, `TechniqueContext`, `can_use_technique` / `use_technique`, `TechniqueUse`, `dispatch_technique` (**always empty**) |
| `usage.rs` | `SkillUses` (per-skill per-round limit + used count, `engine::stats::Counter`; **a rules state, not under stats/**, D198) |
| `prepared.rs` | Prepared skills (K53): `PreparedSkills` (begin / complete / interrupt / tick_action_round / pending_action); blocks active skill selection |
| `timing.rs` | `dispatch_skill`: dispatches `SkillTiming::SkillUsed` across the three scopes "any → a kind → a specific skill", merges, returns pending effects (D186) |

**`can_use` check order** (fixed; returns the first matching `CannotUse`):

```text
1 PassiveSkill            passives cannot be actively used (policy can enable)
2 ForbiddenByStatus       frozen / petrified / stun (R5; S6.6 / K55 / S8.4)
3 Preparing / PreparedSkillReady   K53 prepared-skill blocking (policy can disable)
4 UsageLimit              per-round usage count
5 NotEnoughEnergy         energy (a burst's charge_cost = its energy cap)
6 NotEnoughDice           dice (can_afford is a "possibly enough" upper bound)
```

**`use_skill` order**: `can_use` → dispatch "before skill use" → pay (dice + energy) → dispatch "after skill use" → record usage + grant energy (normal attack / elemental skill +1); **it does not execute the skill effect body** (`SkillDef::effect` is still a placeholder `fn()`). `effective_kind` is the post-ruling effective form (charged / plunging), for observation and later dispatch only.

### 2.6 Timings and triggers (`trigger.rs`)

`CharacterTiming` is the key type of the character family's **single** registry (D185 / D198):

```text
HpChanged        // HP changed (damage / heal / raw add or subtract)
Healed           // after healing
Defeated         // defeated (HP 0 and settlement finished, D194)
AvoidedDefeat    // "not considered defeated" (marker only, no healing)
SkillUsed { phase: Before|After, scope: Any|Kind(SkillKind)|Skill(SkillId) }
FormTransformed  // form transformed (dispatched by form.rs)
```

A trigger entry = "timing key + condition fn pointer + effect fn pointer + ordering tag" (D186); `dispatch_character` only orders and returns pending effects, `execute_character` actually executes; `CharacterFrames = timeline::Resolution<CharacterEffect>` (insert-style settlement). **Neither techniques nor prepared skills dispatch `SkillUsed`** (K58 / K53).

### 2.7 Form / growth / satiety

| Module | Notes |
|---|---|
| `form.rs` | `FormDef` (name / element / tag pointer slice / skill list) / `Form` / `FormState` (base + current; `transform_to` / `restore` are pure); `FormTriggers` + `transform_with_triggers` dispatch `FormTransformed`; **`CharacterState` has no form field yet** (留空.md #32 / #22) |
| `growth.rs` | `GrowthAttribute{Attack,MaxHp,MaxEnergy}` + `GrowthDirection` + `Growth` / `GrowthList`; `effective_max_hp` / `actual_max_hp(definition value)`; definition value immutable, actual = definition + growth; on defeat `reset_max_hp()` (D238) |
| `satiated.rs` | Identity `SATIATED` (data 303300; boolean); `CharacterState::{satiated, with_satiated, clear_satiated, food_targetable}`; `clear_at_round_end` / `clear_on_defeat` (S8.1 clause 3) |
| `energy.rs` | `energy_gain(base, modifiers)`: gain modifiers (D193: no "energy changed" timing key) |

### 2.8 Relation to statuses

- Character statuses = `CharacterState.states` (shared `StateList`, position `CHARACTER_STATE_POS = 0`); shield / reduction / immunity / frozen / stun / petrified / satiety identities are provided or bridged by `damage_chain::statuses` (see [[定义层-Summon-Enemy-Status-EN]]).
- Combat statuses = `CombatStatusZone` (attached to the active character); equipment = `EquipmentZone` (one per character).

## 3. Public entry points

> [!NOTE]
> Code below is copied verbatim from the current repository (including `use`, `derive`, doc comments and other boilerplate). Over-long function bodies are truncated to the signature + first body lines with an explicit note; everything else is the full item.

### 3.1 Definition carriers

**`CharacterDef`** — `crates/definitions/src/families/character/definition.rs`

```rust
/// 一张**角色牌**的定义（D162）。
///
/// 泛型参数 `Tags` 是**标签集合**：标签集合形态本轮未定（见 `lib.rs`），
/// 样板用**类型级零大小元组**（如 `(Hydro, Claymore, NodKrai)`）——
/// 标签就是类型本身（D54），组合起来仍然零大小、无分配、编译期可断言。
///
/// 上限口径（用户裁决）：`max_hp` / `max_energy` 是**定义数据**（上限来源）；
/// 运行期当前值在 `CharacterState`（用 [crate::families::character::character::CharacterState::from_def]
/// 按本定义建初始状态）。
///
/// TODO(待定)：标签集合形态（元组 / Vec / 位图）正式确定后改这一处。
#[derive(Clone, Copy, Debug)]
pub struct CharacterDef<Tags> {
    /// 卡牌 id（角色牌族 id 区域内唯一）。
    pub id: CardId,
    /// 角色名（样板只填中文，D120）。
    pub name: Localized,
    /// 标签集合。
    pub tags: Tags,
    /// 生命上限。
    pub max_hp: Nat,
    /// 充能上限。
    pub max_energy: Nat,
    /// 技能列表（顺序 = 使用 / 展示顺序：普攻、战技、爆发、被动）。
    pub skills: &'static [&'static SkillDef],
}
```

**`SkillDef`** — `crates/definitions/src/families/character/skill.rs`

```rust
/// 一个技能的**定义**（D121 / D162）。
///
/// 只存「识别信息 + 文案 + 种类 + 消耗 + 效果函数」；
/// 技能种类复用 `types::SkillKind`（D121 / D123–D126）。
///
/// TODO(P3)：`description` 将来应从 `generated` 按（牌族, 卡 id, 技能 id）取，
/// 而不是把中文写进卡牌代码（结构.md §4.4 规则 6；当前 D120 / D162 样板例外）。
#[derive(Clone, Copy, Debug)]
pub struct SkillDef {
    /// 技能 id（角色 id × 10 + 序号，见 `base::id`）。
    pub id: SkillId,
    /// 技能名（样板只填中文，D120）。
    pub name: Localized,
    /// 技能完整中文文案（样板只填中文，D120）。
    pub description: Localized,
    /// 技能种类。
    pub kind: SkillKind,
    /// 消耗。
    pub cost: SkillCost,
    /// 效果体（占位签名，见 [SkillEffect]）。
    pub effect: SkillEffect,
}
```

**`SkillCost`** — `crates/definitions/src/families/character/skill.rs`

```rust
/// 技能消耗（D121 / D162）。
///
/// - `dice`：元素骰费用，复用 `resources::dice::CostComponent`（D142 / D161：
///   `SpecificElement(元素, n)` = 指定元素 n 个；`AnyColor(n)` = 任意 n 个；`SameColor(n)` = 同色 n 个）；
/// - `energy`：充能费用，`Nat::ZERO` = 不要充能。
///
/// 本类型**只描述「要花什么」**；支付 / 扣减在「被花的东西」那边（骰池、角色充能，D121 / D133）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SkillCost {
    /// 元素骰费用（按 CostComponent 逐项扣）。
    pub dice: &'static [CostComponent],
    /// 充能费用（`ZERO` = 不消耗充能）。
    pub energy: Nat,
}
```

**`SkillKind`** — `crates/definitions/src/families/character/types.rs`

```rust
/// 技能的 4 个种类（D121）。
///
/// 普通攻击是「一类」而不是单个值：它的**形态**由 `SpecialNormalAttack` 表达
/// （D123 / D124 / D125 / D126）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SkillKind {
    /// 普通攻击。
    ///
    /// - `None` = 普通攻击本体（默认形态，D125）；
    /// - `Some(形态)` = 特殊普通攻击（重击 / 下落攻击，D126）。
    ///
    /// 两种都属于普通攻击类：对普通攻击生效的效果对特殊形态同样生效（D124），
    /// 判断一律用 `SkillKind::is_normal_attack`。
    NormalAttack(Option<SpecialNormalAttack>),
    /// 元素战技。
    ElementalSkill,
    /// 元素爆发。
    ElementalBurst,
    /// 被动技能。
    Passive,
}
```

**`SpecialNormalAttack`** — `crates/definitions/src/families/character/types.rs`

```rust
/// 普通攻击的**特殊形态**（D123 / D126）。
///
/// 重击 / 下落攻击**不是**独立的技能种类（D123），只是普通攻击的一种形态，
/// 所以没有 `SkillKind::ChargedAttack` 这类变体；它们仍然属于普通攻击类（D124）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SpecialNormalAttack {
    /// 重击。
    Charged,
    /// 下落攻击。
    Plunging,
}
```

**`DamageType`** — `crates/definitions/src/families/character/types.rs`

```rust
/// 伤害类型（D121）：元素 / 物理 / 穿透。
///
/// 本文件只标类型，**不做任何结算**（D133）：加减乘除、减免、穿透不吃减免等
/// 由伤害结算侧处理。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DamageType {
    /// 元素伤害：带具体元素（伤害的元素属性）。
    Elemental(Element),
    /// 物理伤害。
    Physical,
    /// 穿透伤害。
    Piercing,
}
```

### 3.2 Runtime state

**`CharacterState`** — `crates/definitions/src/families/character/character.rs`

```rust
/// 一个角色的状态：生命 / 充能（当前值 + 上限）+ 状态列表 + 元素附着。
///
/// 容器没有身份（D42）：它就是状态里的一个字段；「这是谁」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34），这里不出现敌我枚举或区号。
///
/// 不变式（本类型维护）：
/// 1. `hp <= max_hp`；`energy <= max_energy`（构造与写操作都钳住）；
/// 2. 生命 / 充能的**上限本身**只能通过构造给出（本块不提供「改上限」操作，
///    上限变更属于卡牌效果 / 后续层）。
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct CharacterState {
    /// 当前生命。
    pub hp: Nat,
    /// 生命上限。
    pub max_hp: Nat,
    /// 当前充能。
    pub energy: Nat,
    /// 充能上限。
    pub max_energy: Nat,
    /// 这个角色身上的**状态**（D105 / D112 的通用列表；本块只放字段，不解释状态语义）。
    pub states: StateList,
    /// 元素附着（D130；不是状态，见 attachment.rs）。
    pub attachment: Attachment,
}
```

**`new`** — `crates/definitions/src/families/character/character.rs`

```rust
    /// 建一个角色状态：生命 / 充能先各自钳到自己的上限，状态列表与附着为空。
    ///
    /// 参数顺序 = (当前生命, 生命上限, 当前充能, 充能上限)。
    pub fn new(hp: Nat, max_hp: Nat, energy: Nat, max_energy: Nat) -> CharacterState {
        CharacterState {
            hp: at_most(hp, max_hp),
            max_hp,
            energy: at_most(energy, max_energy),
            max_energy,
            states: StateList::new(),
            attachment: Attachment::none(),
        }
    }
```

**`from_def`** — `crates/definitions/src/families/character/character.rs`

```rust
    /// 按角色牌定义建**初始状态**（用户裁决的上限口径）：
    ///
    /// 生命 = 生命上限、充能 = 0；状态列表 / 元素附着 / 准备技能为空。
    /// 上限是**定义数据**（~CharacterDef::max_hp~ / ~max_energy~），运行期当前值在这里。
    pub fn from_def<Tags>(def: &CharacterDef<Tags>) -> CharacterState {
        CharacterState {
            hp: def.max_hp,
            max_hp: def.max_hp,
            energy: Nat::ZERO,
            max_energy: def.max_energy,
            states: StateList::new(),
            attachment: Attachment::none(),
        }
    }
```

**`hp_add`** — `crates/definitions/src/families/character/character.rs`

```rust
    /// 生命**加**：加到上限为止（不会超过 `max_hp`）。返回新状态，不改入参。
    ///
    /// 只做数值：本方法**不代表**治疗 / 回复这类语义，调用方自己组合。
    pub fn hp_add(&self, amount: Nat) -> CharacterState {
        let mut next = self.clone();
        next.hp = at_most(nadd!(self.hp, amount), self.max_hp);
        next
    }
```

**`hp_sub`** — `crates/definitions/src/families/character/character.rs`

```rust
    /// 生命**减**：减到 0 为止（饱和减法，不为负）。返回新状态，不改入参。
    ///
    /// 只做数值：本方法**不代表**受到伤害；生命到 0 也**不代表**倒下
    /// （倒下 / 免于击倒属于后续统一包装层，D8 / D133）。
    pub fn hp_sub(&self, amount: Nat) -> CharacterState {
        let mut next = self.clone();
        next.hp = nsub!(self.hp, amount);
        next
    }
```

**`energy_add`** — `crates/definitions/src/families/character/character.rs`

```rust
    /// 充能**加**：加到上限为止（不会超过 `max_energy`）。返回新状态，不改入参。
    ///
    /// 只做数值：**不提供**「本次是否获得充能」这类布尔封装（D138），
    /// 是否真的获得 / 溢出多少由调用方自己读前后值判断。
    pub fn energy_add(&self, amount: Nat) -> CharacterState {
        let mut next = self.clone();
        next.energy = at_most(nadd!(self.energy, amount), self.max_energy);
        next
    }
```

### 3.3 The three zones

**`CharacterZone`** — `crates/definitions/src/families/character/zones/field.rs`

```rust
/// **角色区**：最多 3 个槽位 + 当前出战角色指针 + 统计。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的角色区」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct CharacterZone {
    /// 槽位（顺序 = 进入角色区的先后；切换出战不重排）。
    slots: Vec<CharacterSlot>,
    /// 当前出战角色（没有角色 / 全部被击倒 -> None）。
    active: Option<EntityId>,
    /// 角色区统计（D187；feature 关闭时零大小、空操作）。
    stats: CharacterZoneStats,
}
```

**`active_entity`** — `crates/definitions/src/families/character/zones/field.rs`

```rust
    /// 当前出战角色实体号（没有 -> None）。
    pub fn active_entity(&self) -> Option<EntityId> {
        self.active
    }
```

**`switch_active_with_triggers`** — `crates/definitions/src/families/character/zones/field.rs`

```rust
    /// **切换角色**（战斗行动）：把后台角色 `to` 切为出战角色，原出战角色变后台。
    ///
    /// 派发顺序（`结算顺序.md` 未规定，暂定）：`SwitchOut`（原出战）-> `SwitchIn`（新出战）
    /// -> `ActiveChanged`（新出战）。待复核。
    ///
    /// **不做**骰子费用支付（规则书·战斗行动【切换角色】「支付 1 个任意类型的元素骰」）：
    /// 支付归骰池（D121 / D133 / 分工计划 §4.4.15），本层只做区域与时机。
    ///
    /// 失败（返回 None、不改状态）：没有出战角色 / `to` 不在角色区 / `to` 已经是出战角色。
    pub fn switch_active_with_triggers(
        &self,
        to: EntityId,
        triggers: &CharacterZoneTriggers,
    ) -> Option<CharacterZoneResolution> {
        let from = self.active?;
        if from == to || !self.contains(to) {
    // ...（其余实现从略；见 crates/definitions/src/families/character/zones/field.rs）
```

**`EquipmentZone`** — `crates/definitions/src/families/character/zones/equipment.rs`

```rust
/// **装备栏**：某个场上角色的装备列表。
///
/// 容器没有身份（D42 / D182）：它就是「该角色的装备区」这个变量；`owner` 让事件 / 查询知道
/// 「这是谁的装备栏」，不需要外部再维护「角色 -> 装备区」表。
#[derive(Clone, Debug)]
pub struct EquipmentZone {
    /// 这个装备栏属于哪个角色。
    owner: EntityId,
    /// 已装备的牌（顺序 = 挂上的先后）。
    entries: Vec<EquipmentEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: EquipmentZoneStats,
}
```

**`slot_entry`** — `crates/definitions/src/families/character/zones/equipment.rs`

```rust
    /// 某个槽位的第一件装备（只读指针；空槽 -> None）。
    pub fn slot_entry(&self, slot: SlotKind) -> Option<&EquipmentEntry> {
        self.entries.iter().find(|entry| entry.slot == slot)
    }
```

**`equip_with_triggers`** — `crates/definitions/src/families/character/zones/equipment.rs`

```rust
    /// **挂装备**（规则书·游戏区域 II(1)）+ 派发「装备 / 弃置」。
    ///
    /// 判定复用 `rules::ops::equip::check_equip`：
    /// - 武器：角色武器类型必须匹配（`required_weapon`）；「其他武器」角色装不了武器；
    /// - 圣遗物 / 特技：每角色 1 件，同槽强制替换；
    /// - 天赋：不限张数，同卡 id 同种覆盖。
    ///
    /// 返回的结果里 `outcome` 是 `Rejected` 时**状态不变**、不派发。
    pub fn equip_with_triggers(
        &self,
        entry: EquipmentEntry,
        required_weapon: Option<WeaponType>,
        character: &CharacterEquip,
        triggers: &EquipmentTriggers,
    ) -> EquipmentResolution {
        let occupied: Vec<SlotKind> = self
            .entries
    // ...（其余实现从略；见 crates/definitions/src/families/character/zones/equipment.rs）
```

**`CombatStatusZone`** — `crates/definitions/src/families/character/zones/combat_status.rs`

```rust
/// **出战状态区**：挂在某个出战角色上的状态列表。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的区域」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct CombatStatusZone {
    /// 当前挂载的出战角色（没有出战角色 -> None）。
    attached_to: Option<EntityId>,
    /// 出战状态条目（顺序 = 进场先后）。
    entries: Vec<CombatStatusEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: CombatStatusStats,
}
```

**`attach_with_triggers`** — `crates/definitions/src/families/character/zones/combat_status.rs`

```rust
    /// **进入出战状态区**（生成 / 附着）+ 派发「附着」。
    ///
    /// - `active`：这些状态挂在哪个出战角色上（同时更新 `attached_to`）；
    /// - 同名状态已存在：层数**相加**（同 `StateList` 口径），持续回合**保留原值**（待复核）；
    /// - 形态归一：布尔类 `layers = None`；层数类缺省按 1 层。
    pub fn attach_with_triggers(
        &self,
        active: EntityId,
        status: &'static dyn StateIdentity,
        layers: Option<Nat>,
        duration: Option<Nat>,
        triggers: &CombatStatusTriggers,
    ) -> CombatStatusResolution {
        let normalized = normalize_layers(status, layers);
        let mut zone = self.clone();
    // ...（其余实现从略；见 crates/definitions/src/families/character/zones/combat_status.rs）
```

**`tick_end_phase_with_triggers`** — `crates/definitions/src/families/character/zones/combat_status.rs`

```rust
    /// **结束阶段**：所有带持续回合的出战状态 -1；耗尽的**立刻弃置**。
    ///
    /// 递减 / 耗尽判定复用 `rules::ops::lifetime`（`duration_after_end_phase` / `is_depleted`）。
    /// 没有持续回合的条目不动、不派发。待执行按条目顺序（= 进场先后）给出：
    /// 未耗尽的发 `DurationTicked`（`amount` = 剩余回合），耗尽的发 `Depleted` 并移出。
    pub fn tick_end_phase_with_triggers(
        &self,
        triggers: &CombatStatusTriggers,
    ) -> CombatStatusResolution {
        // 先拷一份条目再遍历：统计要 &mut，借不到 &zone.entries。
        let entries = self.entries.clone();
    // ...（其余实现从略；见 crates/definitions/src/families/character/zones/combat_status.rs）
```

### 3.4 Damage chain

**`MulMode`** — `crates/definitions/src/families/character/damage_chain/bonus.rs`

```rust
/// 乘法怎么算（分工计划 §4.4.5）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MulMode {
    /// 独立乘法：单独乘一次。
    Standalone,
    /// 加入统一倍数区：区内乘数**相加**后统一乘一次。
    Zone,
}
```

**`Rounding`** — `crates/definitions/src/families/character/damage_chain/bonus.rs`

```rust
/// 取整方向（分工计划 §4.4.5：具体值**暂不决定**，由调用方给；默认 `Down`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    /// 向上取整（ceil）。
    Up,
    /// 向下取整（floor；整数除法的自然结果）。
    Down,
}
```

**`AmountOp`** — `crates/definitions/src/families/character/damage_chain/bonus.rs`

```rust
/// 一条**数值修正**（分工计划 §4.4.5 的 `AmountOp`）：只允许「运算 + 数值」，
/// 不允许把代码（闭包 / trait 对象 / 回调）注入结算（分工计划 §4.4.5 的硬规矩）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AmountOp {
    /// 加：可声明是否计入乘算区（`true` = 乘除之前，会被乘数放大）。
    Add {
        /// 加多少。
        value: Nat,
        /// 是否计入乘算区（`true` = 第 1 段；`false` = 第 4 段）。
        in_mul_zone: bool,
    },
    /// 乘：必须声明是独立乘法还是进统一倍数区。
    Mul {
        /// 乘数（`2` = 翻倍）。
        value: Nat,
        /// 独立乘法 / 倍数区。
        mode: MulMode,
    },
    /// 除：所有除数相乘成一个分母后一起除。
    Div {
        /// 除数（**不能为 0**，否则结算时 panic，D44）。
        value: Nat,
    },
    /// 减（伤害减免 / 伤害降低）：同样可声明是否计入乘算区；用 `Nat` 饱和减法，不会为负。
    Reduce {
        /// 减多少。
        value: Nat,
        /// 是否计入乘算区（`true` = 第 1 段；`false` = 第 4 段）。
        in_mul_zone: bool,
    },
}
```

**`MultiplierZone`** — `crates/definitions/src/families/character/damage_chain/bonus.rs`

```rust
/// 统一的**倍数区**（分工计划 §4.4.5：只有一个，挂在角色 types 上）。
///
/// - `mul_sum` = 所有进倍数区的乘数**值**之和（单个「翻倍」= 2）；
/// - `is_empty()`（没有乘数）时 [`MultiplierZone::apply`] **原样返回**（否则会乘成 0）；
/// - 多个倍数叠加时规则书未定义 -> `待实测`（见模块文档）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct MultiplierZone {
    /// 所有进倍数区的乘数之和（`0` = 区内没有乘数）。
    pub mul_sum: Nat,
    /// 取整方向（`Option`：未定 -> 用调用方 / 默认 `Down`）。
    pub rounding: Option<Rounding>,
}
```

**`DamageModifiers`** — `crates/definitions/src/families/character/damage_chain/bonus.rs`

```rust
/// 一次伤害的**乘算 / 除算输入**（第 2 / 3 / 4 段的修正列表 + 取整方向）。
///
/// 与 [`DamageBonus`] 分开：`DamageBonus` 是「加伤」的既有入口（签名不改），本类型只加
/// 「乘 / 除 / 区外加减」；调用方按需用 [`DamageChainEnv::with_modifiers`] 挂进伤修链。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DamageModifiers<'a> {
    /// 数值修正列表（顺序**不影响**结果：同类运算可交换；只用于观测与将来的保序扩展）。
    pub ops: &'a [AmountOp],
    /// 取整方向（`None` -> `Down`）。
    pub rounding: Option<Rounding>,
}
```

**`scale`** — `crates/definitions/src/families/character/damage_chain/bonus.rs`

```rust
    /// **完整结算四段运算**：输入 = 「基础伤害 + 加伤」（第 1 段的加法部分），
    /// 返回每一步的中间值 + 最终结果。
    ///
    /// 顺序（分工计划 §4.4.5）：
    /// 1. 加减（`in_mul_zone = true`）；
    /// 2. 乘（独立各乘一次；倍数区相加后统一乘一次）；
    /// 3. 除（所有除数相乘成一个分母后一起除，再按取整方向取整）；
    /// 4. 加减（`in_mul_zone = false`）。
    pub fn scale(&self, after_flat_bonus: Nat) -> ScaledDamage {
        // 1) 计入乘算区的加减。
        let (in_zone_added, in_zone_reduced) = self.in_zone_delta();
    // ...（其余实现从略；见 crates/definitions/src/families/character/damage_chain/bonus.rs）
```

**`DamageChainEnv`** — `crates/definitions/src/families/character/damage_chain/chain.rs`

```rust
/// 修正链从「区」读数据的**只读依赖**（D183：只借不拥有、不拷贝、不重造承载）。
///
/// 生命周期都是 `&'a`：区由状态层的两个具名实例（我方 / 敌方）持有。
#[derive(Debug)]
pub struct DamageChainEnv<'a> {
    /// 出战状态区（只读指针）：护盾 / 减伤的**出战状态**承载。
    pub combat_status: &'a CombatStatusZone,
    /// 目标角色的状态列表（只读指针）：护盾 / 减伤的**角色状态**承载。
    pub character_states: &'a StateList,
    /// 算作**护盾**的状态身份集合（默认 [`statuses::SHIELD_STATUSES`]）。
    pub shields: &'a [&'static dyn StateIdentity],
    /// 算作**减伤**的状态身份集合（默认 [`statuses::REDUCTION_STATUSES`]）。
    pub reductions: &'a [&'static dyn StateIdentity],
    /// 算作**伤害免疫**的状态身份集合（默认 [`statuses::IMMUNITY_STATUSES`]）。
    ///
    /// 命中 = 本次伤害归 0（不结算）；**穿透伤害不吃免疫**（K5）。
    pub immunities: &'a [&'static dyn StateIdentity],
    /// 乘算 / 除算 / 区外加减（分工计划 §4.4.5；默认 [`DamageModifiers::NONE`]）。
    pub modifiers: DamageModifiers<'a>,
    /// **伤害类型覆盖**（卡面「把造成的物理伤害变为水元素伤害」类效果；默认 None = 不覆盖）。
    ///
    /// 作用点 = **进入计算前**：覆盖后的类型用于穿透 / 免疫 / 乘算等全部数值判定；
    /// 是否覆盖后**再判反应**由调用方决定（[crate::families::character::damage::damage_with_chain]
    /// 会用同一个 effective type 做反应预测与判定，所以覆盖后元素伤害正常引发反应）。
    pub type_override: Option<DamageType>,
}
```

**`with_type_override`** — `crates/definitions/src/families/character/damage_chain/chain.rs`

```rust
    /// **覆盖本次伤害的类型**（默认 None = 不覆盖；None 时与改前逐字节等价）。
    ///
    /// 依据：天赋 / 状态卡面「使角色造成的物理伤害变为水元素伤害」类效果；规则书没有
    /// 「伤害类型转换」词条 —— 口径 = 覆盖后完全按新类型结算（含反应判定、穿透 / 免疫）；
    /// 覆盖发生在**进入链之前**，所以记录里能看到原始类型与新类型两条。
    pub fn with_type_override(mut self, ty: Option<DamageType>) -> Self {
        self.type_override = ty;
        self
    }
```

**`effective_type`** — `crates/definitions/src/families/character/damage_chain/chain.rs`

```rust
    /// 本次伤害**实际生效**的类型（有覆盖用覆盖值，否则用原始值）。
    pub fn effective_type(&self, original: DamageType) -> DamageType {
        self.type_override.unwrap_or(original)
    }
```

**`settle`** — `crates/definitions/src/families/character/damage_chain/chain.rs`

```rust
/// **跑一次伤修链**（= `schedule(...).run()`；最常用的入口，不记录）。
pub fn settle(
    input: DamageInput,
    env: &DamageChainEnv<'_>,
    bonus: &DamageBonus<'_>,
) -> ChainOutcome {
    schedule(input, env, bonus).run()
}
```

### 3.5 Skill flow

**`CannotUse`** — `crates/definitions/src/families/character/skill_flow/mod.rs`

```rust
/// 不能使用技能的原因。
///
/// 判定顺序固定（[`can_use`] 文档里写明），同一个局面只会返回**第一个**命中的原因，
/// 便于调用方 / 测试稳定断言。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CannotUse {
    /// 被动技能不可主动使用（[`SkillUsePolicy::allow_passive`] 为 false）。
    PassiveSkill,
    /// 该角色有技能**正在准备**：本次行动被跳过（K53）。
    Preparing {
        /// 正在准备的技能。
        skill: SkillId,
    },
    /// 该角色有技能**已完成准备**：将直接使用它，不能再选别的技能（K53）。
    PreparedSkillReady {
        /// 已完成准备的技能。
        skill: SkillId,
    },
    /// 本回合使用次数已达上限（「每回合 N 次」）。
    UsageLimit {
        /// 达到上限的技能。
        skill: SkillId,
        /// 每回合上限。
        limit: Nat,
    },
    /// 充能不够（元素爆发的充能费用）。
    NotEnoughEnergy {
        /// 技能要求的充能。
        required: Nat,
        /// 当前充能。
        available: Nat,
    },
    /// 骰子不够付。
    ///
    /// 注意：`DicePool::can_afford` 是「**可能**就行」（折叠组按可能颜色计入上界），
    /// 所以 [`can_use`] 通过后 [`pay_skill_cost`] **仍可能**返回这个失败（折叠组需要先显式展开）。
    NotEnoughDice,
    /// 角色被状态**禁止使用技能**（冻结 / 石化 / 眩晕；S6.6 / K55 / S8.4）。
    ///
    /// 查询实现在 [`forbidden`]；规则书原文「角色无法使用技能」。
    ForbiddenByStatus {
        /// 命中的禁用状态（身份 + 被禁用的入口）。
        forbidden: ForbiddenState,
    },
}
```

**`SkillUseContext`** — `crates/definitions/src/families/character/skill_flow/mod.rs`

```rust
/// 一次「使用技能」判定的**只读局面上下文**（D42：容器没有身份，就是个值）。
///
/// 字段都是只读借用：判定 / 支付都**不改入参**（失败时什么都不改）。
#[derive(Clone, Copy, Debug)]
pub struct SkillUseContext<'a> {
    /// 要使用的技能定义。
    pub skill: &'a SkillDef,
    /// 使用者的状态（出战角色；充能在这里）。
    pub character: &'a CharacterState,
    /// 我方骰池。
    pub dice: &'a DicePool,
    /// 每个技能的每回合次数（区自持）。
    pub uses: &'a SkillUses,
    /// 该角色身上「准备技能」的槽（区自持；没有就传空容器）。
    pub prepared: &'a PreparedSkills,
    /// 策略开关。
    pub policy: SkillUsePolicy,
    /// 「获得充能时获得量 +N」类修正（`energy::energy_gain` 的 modifiers；没有就传空切片）。
    pub energy_gain_bonus: &'a [Nat],
}
```

**`can_use`** — `crates/definitions/src/families/character/skill_flow/mod.rs`

```rust
/// **能否使用**这个技能（只读；成功 -> `Ok(())`，失败 -> 第一个命中的原因）。
///
/// 判定顺序（固定）：
/// 1. **[`CannotUse::PassiveSkill`]**：被动技能不可主动使用（策略可关）；
/// 2. **[`CannotUse::ForbiddenByStatus`]**：角色被状态禁止使用技能（冻结 / 石化 / 眩晕；
///    S6.6 / K55 / S8.4）；查询在 [`forbidden`]，**不受策略开关影响**；
/// 3. **[`CannotUse::Preparing`] / [`CannotUse::PreparedSkillReady`]**：K53 准备技能阻塞
///    （策略可关）；判定用 `PreparedSkills::pending_action`；
/// 4. **[`CannotUse::UsageLimit`]**：本回合次数（`SkillUses`）；
/// 5. **[`CannotUse::NotEnoughEnergy`]**：充能（`SkillCost::energy`；元素爆发的卡面数据里它等于
///    充能上限，即规则书「花费所有充能」的编码方式）；
/// 6. **[`CannotUse::NotEnoughDice`]**：骰子（`DicePool::can_afford`，可能就行）。
///
/// **不检查**（见模块文档「不做 / 待接入」）：出战角色是谁、行动权 / 阶段、
/// 目标是否存在、费用修正链。
pub fn can_use(context: &SkillUseContext<'_>) -> Result<(), CannotUse> {
    // 1) 技能种类：被动不可主动使用。
    if matches!(context.skill.kind, SkillKind::Passive) && !context.policy.allow_passive {
    // ...（其余实现从略；见 crates/definitions/src/families/character/skill_flow/mod.rs）
```

**`pay_skill_cost`** — `crates/definitions/src/families/character/skill_flow/mod.rs`

```rust
/// **支付技能费用**：骰子（复用 `DicePool::pay`）+ 充能（`CharacterState::energy_sub`）。
///
/// **纯函数**：成功返回新状态；失败返回 `Err`，**入参一个字节都不改**。
///
/// - 充能先判（`SkillCost::energy`；为 0 的技能不扣）；
/// - 骰子按 `resources::dice` 的支付语义：① 指定元素骰 -> ② 同色（颜色由 `same_color` 给）->
///   ③ 任意骰；折叠组未显式展开时不能用于 ① / ②（见 `DicePool::pay` 文档）；
/// - 规则书「元素爆发花费所有充能」由**卡面数据**表达（爆发的 `SkillCost::energy` = 充能上限），
///   本层不写死 `max_energy`（将来若出现充能费用加减，只改费用，不改支付）。
pub fn pay_skill_cost(
    skill: &SkillDef,
    character: &CharacterState,
    dice: &DicePool,
    same_color: Option<Element>,
) -> Result<SkillPayment, CannotUse> {
    let required = skill.cost.energy;
    let available = character.energy();
    if available < required {
        return Err(CannotUse::NotEnoughEnergy {
            required,
            available,
        });
    }
    let Some(paid_dice) = dice.pay(skill.cost.dice, same_color) else {
        return Err(CannotUse::NotEnoughDice);
    };
    Ok(SkillPayment {
        dice: paid_dice,
        character: character.energy_sub(required),
    })
}
```

**`use_skill`** — `crates/definitions/src/families/character/skill_flow/mod.rs`

```rust
/// **使用技能**：把「能否使用 -> 派发使用技能时 -> 支付 -> 派发使用技能后 -> 记使用 + 给充能」
/// 按固定顺序串起来（顺序见模块文档）。
///
/// - 失败返回 `Err`，**入参一个字节都不改**（[`can_use`] / [`pay_skill_cost`] 都是纯函数，
///   派发也是只读的）；
/// - **不执行技能效果体**（`SkillDef::effect` 还是占位 `fn()`，D162 TODO）；
/// - **不执行派发结果**（D186）：`before` / `after` 只是待执行列表；
/// - **准备技能不走这里**（K53：不派发「使用技能时 / 后」；走 `prepared::PreparedSkills`）；
/// - 规则书的「普通攻击 / 元素战技 后获得 1 点充能」**已作用**在返回的 `character` 上（先于调用方执行
///   `after`）；规则书没规定它与「使用技能后」触发的先后，本层取「获得充能先」暂定。
pub fn use_skill(
    triggers: &SkillTriggers,
    context: &SkillUseContext<'_>,
    who: EntityId,
    target: Option<EntityId>,
    same_color: Option<Element>,
) -> Result<SkillUse, CannotUse> {
    // 没有「下落攻击」待消耗标记时的判定：只按「行动开始前的骰总数」判重击（S8.9）。
    use_skill_determined(
        triggers,
        context,
        who,
        target,
        same_color,
        PlungingMark::NONE,
    )
}
```

**`use_skill_with_plunging`** — `crates/definitions/src/families/character/skill_flow/mod.rs`

```rust
/// **使用技能（带下落攻击标记）**：与 [`use_skill`] 完全一致，只是把 [`PlungingMark`]
/// 纳入「生效形态」判定（S8.10：切换为出战角色后，本回合下一个战斗行动若为普通攻击 ->
/// 视为下落攻击）。
///
/// - 判定只影响返回的 [`SkillUse::effective_kind`]（观测 / 后续「普通攻击后」派发用）；
///   费用 / 充能 / 次数与 [`use_skill`] 相同（重击 / 下落攻击仍属普通攻击类，D124）；
/// - 本层**不消费**标记：战斗行动结束后由调用方调 [`PlungingMark::after_combat_action`]；
/// - 骰总数取样 = 调用瞬间（支付前）的 `context.dice`。
pub fn use_skill_with_plunging(
    triggers: &SkillTriggers,
    context: &SkillUseContext<'_>,
    who: EntityId,
    target: Option<EntityId>,
    same_color: Option<Element>,
    plunging: PlungingMark,
) -> Result<SkillUse, CannotUse> {
    use_skill_determined(triggers, context, who, target, same_color, plunging)
}
```

**`AttackForm`** — `crates/definitions/src/families/character/skill_flow/attack_form.rs`

```rust
/// 一次普通攻击**生效的形态**（判定结果的标注；`None` = 不是普通攻击）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttackForm {
    /// 普通攻击本体。
    Normal,
    /// 重击（S8.9：行动开始前骰总数为偶数）。
    Charged,
    /// 下落攻击（S8.10：切换后本回合下一个战斗行动是普通攻击）。
    Plunging,
}
```

**`AttackFormInputs`** — `crates/definitions/src/families/character/skill_flow/attack_form.rs`

```rust
/// 判定「视为重击 / 下落攻击」需要的两个输入。
///
/// 只放**判定真正用到的**两个量：骰总数（重击）与待消耗标记（下落）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttackFormInputs {
    /// 我方**行动开始前**的元素骰总数（`DicePool::total`；决定重击）。
    pub dice_total: Nat,
    /// 「本回合下一个战斗行动视为下落攻击」的待消耗标记。
    pub plunging_pending: bool,
}
```

**`effective_normal_attack`** — `crates/definitions/src/families/character/skill_flow/attack_form.rs`

```rust
/// **生效形态判定**（S8.9 / S8.10）：给「定义上的技能种类」+ 判定输入，返回本次**生效**的种类。
///
/// 规则（按顺序）：
/// 1. 不是「普通攻击本体」（`NormalAttack(None)`）-> 原样返回（战技 / 爆发 / 被动 / 已写死形态）；
/// 2. 下落标记待消耗 -> `NormalAttack(Some(Plunging))`；
/// 3. 行动开始前骰总数为偶数 -> `NormalAttack(Some(Charged))`；
/// 4. 否则 -> 原样（普通攻击本体）。
///
/// 纯函数：不改入参、不消费标记（消费由调用方在战斗行动结束后用
/// [`PlungingMark::after_combat_action`] 做）。
pub fn effective_normal_attack(kind: SkillKind, inputs: AttackFormInputs) -> SkillKind {
    // 1) 只改「普通攻击本体」：非普通攻击 / 已写死特殊形态 -> 原样。
    if !matches!(kind, SkillKind::NormalAttack(None)) {
        return kind;
    }
    // 2) 下落攻击优先（规则书未写与重击叠加 -> 待实测）。
    if inputs.plunging_pending {
        return SkillKind::plunging_attack();
    }
    // 3) 行动开始前骰总数为偶数 -> 重击。
    if is_even(inputs.dice_total) {
        return SkillKind::charged_attack();
    }
    // 4) 普通攻击本体。
    kind
}
```

**`PlungingMark`** — `crates/definitions/src/families/character/skill_flow/attack_form.rs`

```rust
/// 「切换后本回合下一个战斗行动视为下落攻击」的**待消耗标记**（S8.10）。
///
/// 状态机（全是纯值；由持有方替换字段）：
/// - 角色被切换为**出战角色** -> [`armed`](PlungingMark::armed)（置位）；
/// - 一次**战斗行动**结束后 -> [`after_combat_action`](PlungingMark::after_combat_action)（清除）；
/// - 回合结束 / 新回合开始 -> [`cleared_round`](PlungingMark::cleared_round)（清除）。
///
/// 「切换角色」本身也是战斗行动：切换后置位，**不是**立刻被这次切换消耗掉。
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct PlungingMark {
    pending: bool,
}
```

**`ForbiddenUses`** — `crates/definitions/src/families/character/skill_flow/forbidden.rs`

```rust
/// 被禁用的**主动使用入口**（哪些动作不能做）。
///
/// 规则书里「冻结 / 石化 / 眩晕」都是「角色无法使用技能」，特技也一并禁用（S8.4 特技）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ForbiddenUses {
    /// 普通攻击不能用。
    pub normal_attack: bool,
    /// 元素战技不能用。
    pub elemental_skill: bool,
    /// 元素爆发不能用。
    pub elemental_burst: bool,
    /// 特技不能用（S8.4：「也会同时无法使用『特技』」）。
    pub technique: bool,
}
```

**`forbidden_by`** — `crates/definitions/src/families/character/skill_flow/forbidden.rs`

```rust
/// 用**默认身份集合**查询：角色身上有没有导致禁用技能的状态。
///
/// 有 -> Some（状态身份 + 被禁用的入口）；没有 -> None。
pub fn forbidden_by(states: &StateList) -> Option<ForbiddenState> {
    forbidding_status(states, SKILL_FORBIDDING_STATUSES).map(|state| ForbiddenState {
        state,
        // 冻结 / 石化 / 眩晕的原文都是「无法使用技能」+ S8.4 连特技一起禁 -> 全部主动入口。
        uses: ForbiddenUses::ALL,
    })
}
```

**`skill_allowed`** — `crates/definitions/src/families/character/skill_flow/forbidden.rs`

```rust
/// 这个技能种类现在**还能不能用**（只看状态禁用；费用 / 次数 / 准备技能归 can_use）。
pub fn skill_allowed(states: &StateList, kind: SkillKind) -> bool {
    match forbidden_by(states) {
        None => true,
        Some(forbidden) => !forbidden.uses.forbids_skill_kind(kind),
    }
}
```

**`technique_allowed`** — `crates/definitions/src/families/character/skill_flow/forbidden.rs`

```rust
/// 特技现在**还能不能用**（S8.4：冻结 / 石化 / 眩晕时同时无法使用特技）。
pub fn technique_allowed(states: &StateList) -> bool {
    match forbidden_by(states) {
        None => true,
        Some(forbidden) => !forbidden.uses.technique,
    }
}
```

**`SkillUseClass`** — `crates/definitions/src/families/character/skill_flow/technique.rs`

```rust
/// **可主动使用入口的分类**：把「技能」与「特技」并列成一个判定点。
///
/// 为什么不用 SkillKind 表达特技：K58 原文「使用『特技』时不视为使用技能」，
/// 且 D123 / D126 只把重击 / 下落攻击作为普通攻击的形态 —— 特技不是技能种类，
/// 所以单独用一个分类枚举承载「流程语义」。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SkillUseClass {
    /// 普通攻击（含重击 / 下落攻击形态）。
    NormalAttack,
    /// 元素战技。
    ElementalSkill,
    /// 元素爆发。
    ElementalBurst,
    /// 被动技能（不能主动使用，只作为分类存在）。
    Passive,
    /// 特技（不是技能；战斗行动；不派发 SkillUsed；伤害不算角色造成）。
    Technique,
}
```

**`TechniqueContext`** — `crates/definitions/src/families/character/skill_flow/technique.rs`

```rust
/// 特技使用判定的**只读上下文**（D42：容器没有身份，就是个值）。
#[derive(Clone, Copy, Debug)]
pub struct TechniqueContext<'a> {
    /// 使用特技的角色实体。
    pub who: EntityId,
    /// 该角色的状态（技能禁用查询读这里）。
    pub character: &'a CharacterState,
    /// 该角色的装备栏（特技槽在这里）。
    pub equipment: &'a EquipmentZone,
    /// 该角色是不是**出战角色**（S8.4 第 1 条：出战角色装备特技才能使用）。
    pub is_active: bool,
}
```

**`use_technique`** — `crates/definitions/src/families/character/skill_flow/technique.rs`

```rust
/// **使用特技**：判定通过后返回结果（不支付 / 不派发 / 不执行效果）。
///
/// 依据：S8.4 / K58。调用方拿到 TechniqueUse 后：
/// 1. 按战斗行动结算（Timing::Combat）；
/// 2. **不要**派发「使用技能时 / 后」（dispatch_technique 恒为空）；
/// 3. 效果造成的伤害按「非角色造成」处理。
pub fn use_technique(context: &TechniqueContext<'_>) -> Result<TechniqueUse, CannotUseTechnique> {
    can_use_technique(context)?;
    let entry = context
        .equipment
        .slot_entry(SlotKind::Technique)
        .expect("can_use_technique 已确认特技存在");
    Ok(TechniqueUse {
        who: context.who,
        equip: entry.equip,
        technique: entry.card,
        class: SkillUseClass::Technique,
        timing: technique_timing(),
    })
}
```

**`SkillUses`** — `crates/definitions/src/families/character/skill_flow/usage.rs`

```rust
/// 每个技能的**每回合使用计数**（区自持的普通值，D182）。
///
/// - 没有登记的技能 = **不限次数**；
/// - 上限由卡牌 / 状态自己登记（[`SkillUses::with_limit`]）—— 本层不预设「谁每回合几次」；
/// - 「每回合」的清零由持有方在回合开始调用 [`SkillUses::reset_round`]。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SkillUses {
    entries: Vec<SkillUseEntry>,
}
```

**`with_limit`** — `crates/definitions/src/families/character/skill_flow/usage.rs`

```rust
    /// 登记 / 覆盖一个技能的每回合上限（**纯函数**：返回新容器）。
    pub fn with_limit(&self, skill: SkillId, limit: Nat) -> SkillUses {
        let mut next = self.clone();
        match next.entries.iter_mut().find(|entry| entry.skill == skill) {
            Some(entry) => entry.limit = Some(limit),
            None => next.entries.push(SkillUseEntry {
                skill,
                limit: Some(limit),
                used: Counter::ZERO,
            }),
        }
        next
    }
```

**`recorded`** — `crates/definitions/src/families/character/skill_flow/usage.rs`

```rust
    /// 记一次使用（**纯函数**：返回「已用 +1」的新容器，不改入参）。
    ///
    /// 没登记过的技能也会被记进容器（上限 `None`），便于查询「本回合用过几次」。
    pub fn recorded(&self, skill: SkillId) -> SkillUses {
        let mut next = self.clone();
        match next.entries.iter_mut().find(|entry| entry.skill == skill) {
            Some(entry) => {
                entry.used.add(Nat::new(1));
            }
            None => next.entries.push(SkillUseEntry {
                skill,
                limit: None,
                used: Counter::new(Nat::new(1)),
            }),
        }
        next
    }
```

**`PreparedSkills`** — `crates/definitions/src/families/character/skill_flow/prepared.rs`

```rust
/// 一个角色身上的**准备技能槽**（D182：普通值类型，由区自己持有；没有全局状态）。
///
/// 可以有**多个**槽（卡面「分别准备技能：长延涤流和终幕涤流」），顺序 = 开始准备的先后。
/// 同一个技能允许重复准备（按顺序逐个别完成 / 打断）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreparedSkills {
    slots: Vec<PreparedSkill>,
}
```

**`begin`** — `crates/definitions/src/families/character/skill_flow/prepared.rs`

```rust
    /// **开始准备**：追加一个准备槽，`rounds` = 还需几个行动轮（`ZERO` = 立刻完成准备）。
    ///
    /// 纯函数：返回新容器，不改入参。只应由「出战角色」调用（K53；本层不校验，见模块文档）。
    pub fn begin(&self, skill: SkillId, rounds: Nat) -> PreparedSkills {
        let mut next = self.clone();
        next.slots.push(PreparedSkill {
            skill,
            rounds_left: rounds,
        });
        next
    }
```

**`pending_action`** — `crates/definitions/src/families/character/skill_flow/prepared.rs`

```rust
    /// 「轮到该角色行动时」应该做什么（K53）。
    ///
    /// 判定顺序：**已完成准备的先直接用**（K53「如果技能已经完成准备，角色将会在此时直接使用此技能」）；
    /// 没有就绪的、但有准备中的 -> 跳过行动；都没有 -> `None`（正常行动）。
    pub fn pending_action(&self) -> Option<PreparedAction> {
        if let Some(skill) = self.ready_skill() {
            return Some(PreparedAction::Use { skill });
        }
        self.slots
            .iter()
            .find(|slot| !slot.is_ready())
            .map(|slot| PreparedAction::Skip { skill: slot.skill })
    }
```

**`dispatch_skill`** — `crates/definitions/src/families/character/skill_flow/timing.rs`

```rust
/// **一次技能使用的有序派发**（D186：只排序不执行）。
///
/// 步骤：
/// 1. 按 [`skill_timings`] 三档作用域各派发一次（`timeline::dispatch`，条件 + 时机匹配；
///    单侧内部已按来源类别 / 进场先后排序）；
/// 2. 三趟结果按**结算顺序**归并（规则书第 2 / 3 条）；同来源类别、同进场序的并列项保持
///    「任意 -> 某一类 -> 具体技能」的趟序（暂定、待复核）。
///
/// 只读 + 不执行：不调用任何效果函数指针，也不改注册表与事件。
#[must_use]
pub fn dispatch_skill(
    triggers: &SkillTriggers,
    phase: SkillPhase,
    skill: &SkillDef,
    event: &Event,
) -> Vec<SkillPending> {
    let mut ordered: Vec<SkillPending> = Vec::new();
    for timing in skill_timings(phase, skill) {
        ordered.extend(
            dispatch(triggers, timing, event)
                .into_iter()
                .map(|item| item.into_queued()),
        );
    }
    ordered.sort_by(by_resolution_order);
    ordered
}
```

### 3.6 Timings / form / growth / satiety

**`CharacterTiming`** — `crates/definitions/src/families/character/trigger.rs`

```rust
/// 角色族的**时机键**（D185：由角色族自己定义；通用层不认识它）。
///
/// 每个值都是「角色族的一类事件」，派发时与登记项里的时机键做相等比较；
/// `SkillUsed` 额外带「阶段 × 作用域」数据（卡面措辞的三档粒度），其余四个是零大小变体。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CharacterTiming {
    /// 生命值变动（受伤 / 治疗 / 直接加减血都算）。
    ///
    /// 依据：规则书·其他规则 击倒「当角色的生命值被降为0，则在技能或效果结算完成后，角色就会被
    /// 『击倒』」；特殊词条 护盾「受到伤害时，将会消耗『护盾值』来抵消伤害」；
    /// 卡面「受到伤害时」「受到伤害后」「治疗后」。
    ///
    /// 方向（掉血 / 回血）由**派发入口**决定：伤害走 `damage_with_triggers`、治疗走
    /// `heal_with_triggers`、裸加减血走 `hp_sub_with_triggers` / `hp_add_with_triggers`。
    HpChanged,
    /// 治疗后（治疗结算完成）。
    ///
    /// 依据：规则书未单列条目；卡面「治疗后 / 受治疗后」。
    Healed,
    /// 被击倒时（生命值 0 且**技能 / 效果结算完成后**）。
    ///
    /// 依据：规则书·其他规则 击倒「当角色的生命值被降为0，则在技能或效果结算完成后，角色就会被
    /// 『击倒』。此时，角色身上所附属的装备和状态都会被弃置，充能会被清空」。
    Defeated,
    /// 免于被击倒（**当前不视作被击倒**；不含治疗 —— 治疗是另一个操作，Lead 拍板）。
    ///
    /// 依据：规则书·其他规则 免于被击倒「此时，角色不视为被『击倒』过（因此，角色所附属的装备和
    /// 状态不会被弃置，充能也不会被清空）」。
    /// 用户口径：「免于击倒就是当前不视作被击倒，但由于血量不 >1 还会继续尝试被击倒，所以会治疗一点」。
    /// 所以本时机**只标记「当前不视作被击倒」**：不内置治疗、不派发 `Healed`；
    /// 卡牌效果自己组合 `heal_with_triggers`（例如补到 1 点血）。
    AvoidedDefeat,
    /// **使用技能时 / 后**（卡面「使用技能时」「使用技能后」；规则书·进行行动 战斗行动【使用技能】）。
    ///
    /// - `phase`：使用技能时（技能效果结算之前，费用修正类效果的时点）/ 使用技能后（技能效果结算完成后）；
    /// - `scope`：任意技能 / 某一类技能 / 某个具体技能（卡面措辞的三档粒度）。
    ///
    /// 为什么带数据而不是拆成几个零大小变体：卡面同时存在「使用技能后」「使用元素爆发后」
    /// 「使用 `$[S…]` 后」，抽象成「阶段 × 作用域」后**一个变体**就能表达；派发时按三档作用域
    /// 各查一次，再按结算顺序归并（`families::character::skill_flow::timing::dispatch_skill`）。
    ///
    /// **特技（K58）与准备技能（K53）的使用不触发这个时机**：
    /// 特殊词条 特技「角色使用『特技』时不视为使用技能，无法触发『使用技能后』……」；
    /// 权威关键字 K53 准备技能「此类需要准备的技能，无法触发『使用技能后』……」。
    SkillUsed {
        /// 阶段（使用技能时 / 后）。
        phase: SkillPhase,
        /// 作用域（任意 / 某一类 / 某个具体技能）。
        scope: SkillScope,
    },
    /// **转换形态**（规则书·其他规则 转换形态；权威关键字 K1013 / K1014 / K1012）。
    ///
    /// 依据：规则书·其他规则 转换形态「部分角色可以通过一些效果『转换形态』。转换为其他形态后，
    /// 角色的名称、元素类型、标签、技能等都有可能发生改变」；
    /// 权威关键字 K1013「『焚尽的炽炎魔女』」（女士 永久转换；元素 冰 -> 火；只能用转换后技能）、
    /// K1014「『失能形态』」（不能用原本技能）、K1012「汲取对应元素的力量」（按汲取元素获得技能）。
    ///
    /// 派发入口在 `families::character::form`：`transform_with_triggers` /
    /// `restore_with_triggers`（**转换完成后**派发一次；事件带转换后的元素类型）。
    /// 形态效果要读写当前形态，所以登记在 `form::FormTriggers`
    /// （效果签名 `fn(&mut FormState, &Event, EntityId)`），与 `CharacterTriggers` 分开
    /// （待接入：统一「角色状态 + 形态」上下文）。
    FormTransformed,
}
```

**`dispatch_character`** — `crates/definitions/src/families/character/trigger.rs`

```rust
/// 主动派发：从角色族注册表里取出「这个时机、条件成立」的登记项，按结算顺序排成**待执行**。
///
/// 排序 / 条件 / 稳定顺序全部复用 `timeline::dispatch`（规则书·其他规则·结算顺序）；
/// 本函数只把借用的 `Pending` 转成 owned（事件按值带走），**不执行效果**。
pub fn dispatch_character(
    triggers: &CharacterTriggers,
    timing: CharacterTiming,
    event: &Event,
) -> Vec<CharacterPending> {
    dispatch(triggers, timing, event)
        .into_iter()
        .map(|item| item.into_queued())
        .collect()
}
```

**`FormState`** — `crates/definitions/src/families/character/form.rs`

```rust
/// 一个角色的**形态状态**：基础形态 + 当前形态（对应「角色当前形态字段」）。
///
/// 基础形态 = 角色原本那张角色卡的形态；当前形态 = 转换之后的形态（未转换时两者相同）。
///
/// 纯函数风格：`transform_to` / `restore` 都吃 `&self`、返回新状态、不改入参。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FormState {
    /// 基础形态（角色原本的角色卡）。
    base: Form,
    /// 当前形态（转换后；未转换时 = `base`）。
    current: Form,
}
```

**`transform_to`** — `crates/definitions/src/families/character/form.rs`

```rust
    /// **转换到某个形态**（纯函数：返回新状态，不改入参）。
    ///
    /// 转换**是否允许**由效果层判断（效果持有目标形态）；本函数只做状态替换。
    /// 转换后名称 / 元素 / 标签 / 技能的所有读接口都走新形态。
    pub const fn transform_to(self, form: Form) -> FormState {
        FormState {
            base: self.base,
            current: form,
        }
    }
```

**`GrowthList`** — `crates/definitions/src/families/character/growth.rs`

```rust
/// 一组**成长条目**（叠加容器；纯函数风格）。
///
/// 容器没有身份（D42）：它就是某个角色 / 某个状态里的一个字段；「这段成长是谁给的」
/// 由持有它的东西（装备 / 天赋 / 场地）表达，这里不登记来源。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GrowthList {
    /// 成长条目（顺序 = 加入顺序；合计与顺序无关）。
    entries: Vec<Growth>,
}
```

**`effective_max_hp`** — `crates/definitions/src/families/character/growth.rs`

```rust
    /// 读**成长后的生命上限**（基础生命上限 + 生命上限成长）。
    pub fn effective_max_hp(&self, base: Nat) -> Nat {
        self.effective(base, GrowthAttribute::MaxHp)
    }
```

**`actual_max_hp`** — `crates/definitions/src/families/character/growth.rs`

```rust
    /// 读**实际最大生命值上限**（= 定义值 + 生命上限成长；D238）。
    ///
    /// `definition_max_hp` 传**定义值**（`CharacterDef::max_hp`，不可变）；
    /// 结果 = **实际值**（可变）。**选择器 / 出现值只能用实际值**，定义值选择器禁用。
    /// 角色被击倒后由 `reset_max_hp()` 把成长清掉，本函数自然回到定义值。
    pub fn actual_max_hp(&self, definition_max_hp: Nat) -> Nat {
        self.effective_max_hp(definition_max_hp)
    }
```

**`Satiated`** — `crates/definitions/src/families/character/satiated.rs`

```rust
/// 「饱腹」状态身份（D95 / D96：静态实例指针即身份）。
#[derive(Debug)]
pub struct Satiated(&'static StateText);

impl Satiated {
    /// 把自身当**身份指针**用（&'static dyn StateIdentity）。
    pub fn identity(&'static self) -> &'static dyn StateIdentity {
        self
    }
}
```

**`with_satiated`** — `crates/definitions/src/families/character/satiated.rs`

```rust
    /// **料理生效后**：让角色进入饱腹（规则书 S8.1 第 1 条）。
    ///
    /// - 布尔形态：重复调用幂等（不会叠层）；
    /// - 纯函数：返回新状态，不改入参。
    pub fn with_satiated(&self) -> CharacterState {
        let mut next = self.clone();
        next.states.add(CHARACTER_STATE_POS, &SATIATED, None);
        next
    }
```

### 3.7 Keyword and rules-ops interfaces referenced by this page

> [!NOTE]
> Real source of the keyword-layer / rules-ops functions referenced by this page (signatures and doc comments copied verbatim).

**`DamageArgs`** — `crates/definitions/src/keywords/op.rs`

```rust
/// **造成伤害**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DamageArgs {
    /// 目标选择器（敌方出战角色 / 所有敌方后台角色 / 自己…）。
    pub targets: TargetSelector,
    /// 基础伤害数值。
    pub amount: Nat,
    /// 伤害类型（七元素 / 物理 / 穿透；枚举本身就是「元素 + 物理 + 穿透」）。
    pub ty: DamageType,
    /// 伤害来源类别（普通攻击 / 元素战技 / 元素爆发 / 状态 / 召唤物 / 反应…）。
    pub source_kind: DamageSourceKind,
    /// 目标选取策略。
    pub pick: TargetPick,
    /// **乘算 / 除算修饰**（分工计划 §4.4.5 的 `AmountOp` 列表 + 取整方向）。
    ///
    /// - 默认 [`DamageModifiers::NONE`]（不乘不算）；
    /// - 卡面「此次伤害翻倍 / +N / 减半」这类**本段伤害自己的**数值修正写在这里
    ///   （状态按层数给的倍率由效果方换算成 `AmountOp` 再放进来）；
    /// - 穿透伤害**整段跳过**（规则书·特殊词条 穿透伤害）。
    pub modifiers: DamageModifiers<'static>,
}
```

**`with_modifiers`** — `crates/definitions/src/keywords/op.rs`

```rust
    /// 补上乘算 / 除算修饰（链式）。
    pub const fn with_modifiers(mut self, modifiers: DamageModifiers<'static>) -> DamageArgs {
        self.modifiers = modifiers;
        self
    }
```

**`Step`** — `crates/definitions/src/keywords/step.rs`

```rust
/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}
```

**`execute_step_me`** — `crates/definitions/src/keywords/step.rs`

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

**`ChainStage`** — `crates/definitions/src/families/character/damage_chain/chain.rs`

```rust
/// 修正链的**一段**（观测用；顺序就是执行顺序，`结算顺序.md` §三 第 1 行）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChainStage {
    /// 基础伤害（链的起点）。
    Base {
        /// 基础伤害。
        amount: Nat,
    },
    /// 穿透伤害：不吃加伤（规则书·特殊词条 穿透伤害）。
    Bypassed {
        /// 原样通过的数值。
        amount: Nat,
    },
    /// 加伤之后的伤害值。
    Boosted {
        /// 本次加上的点数。
        added: Nat,
        /// 加伤后的伤害值。
        total: Nat,
    },
    /// **伤害免疫**：本次伤害归 0（不结算；穿透不吃免疫）。
    Immune {
        /// 被免疫掉的基础伤害。
        ignored: Nat,
    },
    /// **乘算 / 除算之后**的伤害值（分工计划 §4.4.5 第 2 / 3 / 4 段）。
    Scaled {
        /// 乘数合计（独立乘法的积 × 倍数区和；没有 -> 1）。
        multiplier: Nat,
        /// 除数的乘积（没有 -> 1）。
        divisor: Nat,
        /// 乘除之后的伤害值。
        total: Nat,
    },
    /// 护盾抵消之后。
    Shielded {
        /// 被护盾吸掉的点数。
        absorbed: Nat,
        /// 抵消后剩余的伤害值。
        remaining: Nat,
    },
    /// 减伤扣减之后。
    Reduced {
        /// 被减伤扣掉的点数。
        removed: Nat,
    },
    /// 最终扣到生命值的伤害。
    ToHp {
        /// 扣血数值（`<=` 前面各段的结果；不会为负）。
        amount: Nat,
    },
}
```

**`mitigate`** — `crates/definitions/src/rules/ops/damage.rs`

```rust
/// 伤害修正（纯函数）：先护盾吸收，再减伤扣减；穿透伤害两者都不吃。
///
/// - `shield`：当前护盾点数（可叠加后的总量）；
/// - `reductions`：本次伤害适用的各条减伤数值（按调用方给定的顺序相加；顺序不影响总和）。
pub fn mitigate(damage: Nat, ty: DamageType, shield: Nat, reductions: &[Nat]) -> Mitigation {
    // 穿透：不参与任何抵消（关键字 #4）。
    if ty == DamageType::Piercing {
    // ...（其余实现从略；见 crates/definitions/src/rules/ops/damage.rs）
```

**`uses_after_trigger`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 触发一次后的可用次数（减到 0 为止；调用方随后弃置）。
pub fn uses_after_trigger(uses: Nat) -> Nat {
    nsub!(uses, USES_PER_TRIGGER)
}
```

**`is_depleted`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 是否耗尽（= 0 -> 立刻弃置）。
pub fn is_depleted(value: Nat) -> bool {
    value == Nat::ZERO
}
```

**`duration_after_end_phase`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 经过一个结束阶段后的持续回合（减到 0 为止）。
pub fn duration_after_end_phase(duration: Nat) -> Nat {
    nsub!(duration, DURATION_PER_END_PHASE)
}
```

**`can_be_targeted_by_food`** — `crates/definitions/src/rules/ops/food.rs`

```rust
/// 处于饱腹状态的角色**不能成为**其他料理事件牌的目标（规则书第 2 条）。
pub fn can_be_targeted_by_food(satiated: bool) -> bool {
    !satiated
}
```

## 4. Invariants and prohibitions

- HP / energy never exceed their caps; caps come only from the definition or an explicit effect.
- Character-zone slot order = entry order; switching the active character **does not reorder** slots; the active character is an independent pointer.
- The damage chain **never changes `damage()` semantics**: HP loss / attachment / reaction check / consuming participating elements / trigger registration remain that one implementation.
- **Piercing damage takes no bonuses at all**: immunity / bonuses / multiplication / shields / reductions are all skipped (rulebook "Piercing DMG" + K5).
- A technique is not a skill: no `SkillUsed` dispatch (`dispatch_technique` is always empty), its damage is not dealt by the character, and it is a combat action (S8.4 / K58).
- Frozen / petrified / stun forbid skills and techniques, queried uniformly through `can_use` / `can_use_technique` (S6.6 / K55 / S8.4).
- Prepared skills do not go through `use_skill` (K53: neither "before" nor "after skill use" triggers).
- Stats do not participate in position equality; skill usage counts are rules state (D198).
- Pure functions, inputs untouched on failure; numbers are `Nat`; zero `f64`, zero `HashMap`.

> [!IMPORTANT]
> Immunity short-circuits first, and type override happens before the reaction check (留空.md #25 / #110 / #120).
> When the order is not decided by the rulebook, do not invent one; record it in 留空.md.

## 5. Basis (rulebook and D-numbers)

| Topic | Basis |
|---|---|
| Character zone: 3 slots / active character / switching (pay 1 die) | Rulebook "Game Zones I(3)(4)(5)" + combat action [Switch Character]; payment lives in flow |
| Equipment zone: per character 1 weapon / 1 artifact / 1 technique / any talents | Rulebook "Game Zones II(1)"; D16 |
| Combat statuses transfer on switch, -1 per End Phase | Rulebook "Game Zones II(3)" + special entry "Duration (Rounds)" |
| Damage attaches elements and triggers reactions | D164 / D167 |
| Damage-chain order | Rulebook special entries + `结算顺序.md` §3 + plan §4.4.5; 留空.md #17 / #25–#28 |
| Defeat / avoid-defeat / healing split apart | D193 / D194 / D195 |
| Skill timing keys and three scopes | D198; rulebook "Take Action: Use Skill" + card wording |
| Charged / plunging "counts as" | S8.9 / S8.10; D123–D126; 留空.md #29 / #30 |
| Technique flow | S8.4 + K58; 留空.md #85–#87 / #101 |
| Satiety and food | S8.1 + data 303300; D17; 留空.md #78–#81 |
| Form transformation / growth | Rulebook "Other Rules: Form Transformation" + K1013/K1014; K64/K65; D238 |

### 5.1 Documentation vs code mismatches (for review)

> [!NOTE]
> The items below were found in this review; the current code wins and the old docs / decision log need updates.

1. `结构.md` §4.3 draws the skill flow as `families/character/skills/{prepared,transform,defeat,attack_kind}.rs`; it actually is `families/character/skill_flow/` + `form.rs` + `trigger.rs` + `damage_chain/`.
2. `结构.md` lists `character/zones/{status.rs, skill.rs, equipment.rs}`, which does not match: the real files are `zones/{field.rs, equipment.rs, combat_status.rs}`; character statuses use the shared `StateList`.
3. `决策日志.md` **D165** says `families/character/prepared.rs` is cancelled, but code keeps `skill_flow/prepared.rs` (`can_use` blocks skills while preparing); the Lead must decide which one wins.
4. `留空.md` item #32 says `flow/skill_use.rs` does not call `use_skill_with_plunging` yet, but flow already has `use_skill_me_with_plunging`; that entry is stale.

## 6. Common misuse

> [!WARNING]
> The following patterns change settlement semantics or bypass the unified entry points.

1. **Treating charged / plunging as separate skill kinds**: they are just `NormalAttack(Some(...))` (D123 / D126); use `is_normal_attack` to test "affects normal attacks".
2. **Checking frozen / stun yourself**: use `skill_flow::forbidden`; do not build another status-name table.
3. **Treating `can_use` as full action validation**: it does not check the active character, action rights / phase, target legality, or the cost-modifier chain.
4. **Routing techniques through `use_skill`**: a technique is not a skill; use `skill_flow::technique` (end-to-end: `flow::use_technique_me/opp`).
5. **Passing a second status list to `DamageChainEnv` / building your own shield carrier**: the chain only reads `CombatStatusZone` + `CharacterState.states` (D183).
6. **Forgetting `type_override`**: for "physical damage becomes Hydro damage" you must use `with_type_override`, otherwise the reaction is judged on the original type.
7. **Modelling satiety as layers / hand-writing its text**: satiety is a boolean shared identity (data 303300); replacing the definition must be coordinated with the status block (留空.md #79).
8. **Only setting HP to 0 on defeat**: also go through `trigger::defeat_with_triggers` / upper-layer cleanup; the actual cap resets per D238.

## 7. Tests

| File | Representative tests |
|---|---|
| `character.rs` | `new_clamps_current_values_to_their_maxima`, `hp_sub_saturates_at_zero`, `numeric_operations_are_pure` |
| `types.rs` | `four_skill_kinds_are_covered`, `special_forms_still_count_as_normal_attack`, `physical_and_piercing_are_not_elemental` |
| `definition.rs` / `skill.rs` | `definition_carries_skill_list_in_order`, `skill_def_carries_identity_kind_cost_and_effect` |
| `zones/field.rs` / `zones/equipment.rs` / `zones/combat_status.rs` | `switch_dispatches_out_then_in_then_active_changed`, `occupied_slot_replaces_and_discards_old_equipment`, `transfer_moves_attachment_and_keeps_entries` |
| `damage_chain/chain.rs` / `bonus.rs` / `record.rs` | `immunity_short_circuits_the_chain_and_deals_zero`, `multipliers_apply_between_bonus_and_shield`, `type_override_rewrites_the_type_before_the_chain` |
| `damage.rs` | `damage_with_chain_shield_reduces_the_damage_that_lands`, `damage_with_chain_is_pure_on_the_character_state` |
| `skill_flow/*` | `forbidden_status_blocks_skill_use_and_changes_nothing`, `even_dice_total_turns_normal_attack_into_charged`, `technique_dispatches_no_skill_used_trigger` |
| `form.rs` / `growth.rs` / `satiated.rs` | `transform_switches_name_element_tags_and_skills`, `reset_max_hp_returns_actual_cap_to_definition`, `food_effect_grants_satiated_once_and_blocks_food_targeting` |

## 8. Pending and to be verified

- **Damage multipliers / immunity**: immunity-before-bonus order, whether attachment / reactions happen under immunity, multiplier-zone stacking, rounding direction, `DamageBonus` vs multiplication order — 留空.md #25–#28; concrete ward identities belong to the status block #31; card "double damage" is declared at the card layer #32.
- **Charged / plunging**: plunging vs charged priority #29; plunging-mark retention #30; mark holding and flow cleanup #59–#61 / #77; whether "declare round end" clears the mark #95.
- **Techniques**: cost not specified by the rulebook #85; whether it consumes per-round usage / the dispatch filter for "after the character triggers an elemental reaction" #86; end-to-end already exists in `flow/technique.rs`, details #101.
- **Status-forbidden skills**: check ordering (only affects error-code priority) #83; frozen and friends are cleared in state/flows #84; the behaviour change (`CannotUse::ForbiddenByStatus`) is automatically applied by flow #88.
- **Satiety**: which End Phase step clears it #78; formal `StatusDef` vs the shared identity, only one may exist #79; data text vs rulebook wording #80; scheduling belongs to cards + flow #81.
- **Form / type conversion**: `CharacterState` has no form field and the event cannot carry form identity #22; `DamageChainEnv::type_override` is wired (#120), which cards use which conversion belongs to cards.
- **Enemy cards**: `EnemyDef` lacks an energy cap and has no runtime zone (留空.md #15).

---
Last verified: 2026-10-19 (code: `crates/definitions/src/families/character/skill_flow/mod.rs::can_use`)
