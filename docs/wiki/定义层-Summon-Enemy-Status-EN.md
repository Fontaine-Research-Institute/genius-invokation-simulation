# Definitions: Summon / Enemy / Status

> [[定义层-Summon-Enemy-Status|中文]] · English

*Scope: `crates/definitions/src/families/{summon,enemy,status}/` — summons (definition + summon zone), enemies (definition only), statuses (definition + identities + arcane edict container; bearing lives in the character family).*

---

## 1. Responsibility

Three families without a type tree: **summons** (definition + summon zone), **enemies** (definition only), **statuses** (definition + state identities + arcane edict container). Status **bearing is not in this family**: character statuses live in `CharacterState.states`, combat statuses in `CombatStatusZone`.

## 2. Key concepts and types

### 2.1 Summons (`summon/`)

- **`SummonDef<Tags>`**
  - File: `summon/def.rs`
  - Notes: `id` / `name` / `description` / `effect_kind: SummonEffectKind` / `effect_points: Nat` / `default_uses: Option<Nat>` / `tags`; `entry(summon) -> SummonEntry` is a pure transfer
- **`SummonEffectKind`**
  - File: `summon/def.rs`
  - Notes: Minimal set: `Damage` / `Heal` / `Status` / `Other` (no structured data field; inferred, to be reviewed)
- **`SummonEntry`**
  - File: `summon/zones/mod.rs`
  - Notes: `summon` (entity) / `card` (card id) / `uses: Option<Nat>` / `points: Option<Nat>` / `effect_kind: Option<SummonEffectKind>`
- **`SummonZone`**
  - File: `summon/zones/mod.rs`
  - Notes: `entries` (order = summon order) + `stats`; `SUMMON_CAP = 4`

Summon-zone rules:

- Cap 4: when full, **no new summon can be created**; but a **same-name summon still merges first** (merging adds no slot, so the cap is not violated; to be reviewed).
- **Same-name merge**: uses and effect points take the **maximum**, effect kind takes the **newest**, and the **original position is kept** (plan §4.4.15).
- **Usage decrements per trigger** (D12): `tick_with_triggers` decrements by 1 each trigger; at 0 the summon is **discarded immediately** (reusing `rules::ops::lifetime`).
- Timings: `SummonTiming::{Summoned, Merged, Triggered, Depleted, Discarded}` (D185); `execute_summon` executes pending effects.
- Pending: concrete execution actions (`SummonAction` / `SummonMeasure`) belong to a later summon-execution block; this block only provides measurements + the formal effect kind.

### 2.2 Enemies (`enemy/`)

- `id` / `name` / `description: Localized`: no top-level description in the data → `Localized::EMPTY` (D56).
- `max_hp: Nat`: definition data (source of the cap).
- `skills: &'static [&'static SkillDef]`: reuses character skill definitions (D121 / D162).
- `tags`: element / faction / weapon subclass / other weapons (D55).
- **real source: §3.3** (`crates/definitions/src/families/enemy/def.rs`).

- Enemy cards have **no zone and no operations** in this round: definition carrier only; runtime bearing and concrete enemies are pending (code marks `待接入`; 留空.md has no dedicated entry, recommend adding one).
- The data has a `maxenergy` field but this block does not collect it (energy caps belong to the energy block, pending).
- Descriptions are not assembled from skill text (that would be a second copy of the data).

### 2.3 Statuses (`status/`)

- **`StatusType`**
  - File: `status/mod.rs`
  - Notes: `CharacterStatus` / `CombatStatus`; **both types share one status id region** (D51) and are told apart by type
- **`StatusDef<Tags>`**
  - File: `status/def.rs`
  - Notes: `id` / `status_type` / `text: StateText` / `default_layers: Option<Nat>` (None = boolean) / `duration: Option<Nat>` / `tags`; `form()` is derived from `default_layers`
- **Formal identities (5)**
  - File: `status/known.rs`
  - Notes: 203 combat shield / 134 character shield / 106 frozen / 116 dendro core / 117 catalyzing field; each identity holds a `&'static StatusDef` (pointer-sized, **not a ZST**: ZST static addresses can coincide, D200)
- **`ArcaneEdict` + `ARCANE_EDICT`**
  - File: `status/arcane_edict.rs`
  - Notes: Arcane edict (Arcane Edict): **no status card row → no fabricated StatusDef**; only a state identity (boolean) plus a per-player usage container

Status types and bearing (D51 / D105 / D201):

```text
character status -> CharacterState.states (shared StateList; position CHARACTER_STATE_POS = 0)
combat status    -> CombatStatusZone (attached to the active character; transfers on switch; duration -1 per End Phase)
the status family has no zone: StatusDef is definition data, identity = static pointer to it
```

- Statuses share **one common list** `StateList` (D104 / D105): `StateEntry { state, pos, layers }`; booleans use `layers = None` (presence means "has it").
- **No third status type**: the data's "character equipment" (276 entries) conflicts with D51 → not modelled (D201, to be reviewed).
- **Tags**: `tags/status/` is empty in this round; status definitions only import already-defined tags (shield / dendro construct), the rest are `()`.

### 2.4 Key state identities (cross-file)

- **Frozen**
  - Identity: `FROZEN`
  - Form: boolean (duration 1, to be reviewed)
  - Data / basis: status 106; S6.6 "cannot use skills"
  - Definition: `families/status/known.rs`
- **Stun**
  - Identity: `STUN`
  - Form: boolean
  - Data / basis: Stunned + data 100001
  - Definition: `families/character/skill_flow/forbidden.rs` (shared identity)
- **Petrified**
  - Identity: `PETRIFIED`
  - Form: boolean
  - Data / basis: data 116033 / 176033 (two entries, same effect)
  - Definition: same
- **Satiety**
  - Identity: `SATIATED`
  - Form: boolean
  - Data / basis: data 303300; S8.1
  - Definition: `families/character/satiated.rs` (shared identity)
- **Shield (combat / character)**
  - Identity: `SHIELD_COMBAT` / `SHIELD_CHARACTER`
  - Form: layered
  - Data / basis: status 203 / 134
  - Definition: `families/status/known.rs`
- **Dendro core / catalyzing field**
  - Identity: `DENDRO_CORE` / `CATALYZING_FIELD`
  - Form: layered (usage)
  - Data / basis: status 116 / 117
  - Definition: `families/status/known.rs`
- **Damage negation**
  - Identity: `DAMAGE_NEGATION`
  - Form: layered
  - Data / basis: rulebook special entry; no standalone card
  - Definition: `families/character/damage_chain/statuses.rs` (shared identity)
- **Damage immunity**
  - Identity: `DAMAGE_IMMUNITY`
  - Form: boolean
  - Data / basis: Piercing DMG + card text "immune to all DMG"; no standalone card
  - Definition: same (shared identity)

Bridge table (`families/character/damage_chain/statuses.rs`):

- `STATUS_ENTITIES: [StatusEntity; 8]` = 106 / 203 / 116 / 117 / 134 + 100001 stun / 116033 petrified / 303300 satiety.
- `status_of_card(CardId) -> Option<&'static dyn StateIdentity>`; `same_identity` / `identity_in` compare only `identity_address()` (D57 / D58 / D200).
- Sets: `SHIELD_STATUSES` / `REDUCTION_STATUSES` / `IMMUNITY_STATUSES`; the six hand-card statuses have `hand_status_identity` / `hand_status_of_identity` (see [[定义层-Action-Card-EN]]).

### 2.5 Status duration (D13 vs the rulebook)

- Rulebook "Duration (Rounds)": **each End Phase** -1, discarded immediately when depleted ("Each End Phase" in English).
- `rules/ops/lifetime.rs` follows the **rulebook** (`ticks_in_end_phase() = true`, `duration_after_end_phase`) and its comments explicitly note that this supersedes D13's older wording ("decrement at round start").
- Both `CombatStatusZone::tick_end_phase_with_triggers` and the support-zone equivalent tick at the End Phase.

> [!IMPORTANT]
> Usage is not duration: usage decrements **per trigger** (D12); duration decrements **per End Phase** (rulebook; D13's older wording is superseded).
> Never merge the two fields, and never implement "usage -1 at action phase start" (the reference implementation's old bug, called out by D12).

## 3. Public entry points

> [!NOTE]
> Code below is copied from the current repository (including use, derive, doc comments and other boilerplate). Over-long function bodies are truncated to the signature + first body lines with an explicit note; everything else is the full item.

### 3.1 Summon definitions


First, the definition of `SummonEffectKind`; its declaration follows.
文件: crates/definitions/src/families/summon/def.rs

```rust
/// 召唤物的**效果类型**（最小集合：伤害 / 治疗 / 状态 / 其它）。
///
/// 区域层（`zones/mod.rs`）不解释这个枚举，只把它当「效果类型标识」参与同名合并
/// （取最新，分工计划 §4.4.15）；具体效果由召唤物定义 / 执行动作解释。
///
/// `待复核`：数据里没有效果类型字段，本枚举是**反推**的最小集合；
/// 元素伤害的「元素」不在这里（那是伤害类型，归角色族 / 伤害块），本枚举只分「做什么」。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SummonEffectKind {
    /// 造成伤害（效果量 = 伤害点数）。
    Damage,
    /// 治疗（效果量 = 治疗点数）。
    Heal,
    /// 赋予 / 附属状态（效果量 = 状态层数 / 可用次数，含义由状态定义解释）。
    Status,
    /// 其它（加伤 / 减伤 / 特殊效果等没有可计量点数的效果；效果量通常为 `Nat::ZERO`）。
    Other,
}
```

The snippet above gives the public shape of `SummonEffectKind`; its implementation lives in the same file.


First, the definition of `SummonDef`; its declaration follows.
文件: crates/definitions/src/families/summon/def.rs

```rust
/// 一个**召唤物**的定义（纯数据）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
#[derive(Clone, Copy, Debug)]
pub struct SummonDef<Tags> {
    /// 召唤物卡的卡 id（召唤物牌族 id 区域内唯一）。
    pub id: CardId,
    /// 4 语名字（本块只填中文，D120）。
    pub name: Localized,
    /// 4 语描述（本块只填中文，D120）。
    pub description: Localized,
    /// 效果类型（最小集合，见 [`SummonEffectKind`]）。
    pub effect_kind: SummonEffectKind,
    /// 效果量：伤害点数 / 治疗点数 / 状态层数等；`Nat::ZERO` = 无可计量效果量。
    pub effect_points: Nat,
    /// 默认可用次数：`None` = 没有可用次数（触发不递减，D12 / 分工计划「三项改可选」）。
    pub default_uses: Option<Nat>,
    /// 标签集合（D54 / D55）。
    pub tags: Tags,
}
```

The snippet above gives the public shape of `SummonDef`; its implementation lives in the same file.


First, the definition of `entry`; its declaration follows.
文件: crates/definitions/src/families/summon/def.rs

```rust
    /// 用这份定义造召唤物区条目（实体 + 卡 id + 可用次数 + 效果量 + 效果类型）。
    ///
    /// 纯搬运：不改定义、不查任何表；同名合并口径在区域层（D12 / 分工计划 §4.4.15）。
    pub fn entry(&self, summon: EntityId) -> SummonEntry {
        SummonEntry::new(summon, self.id)
            .with_uses(self.default_uses)
            .with_points(Some(self.effect_points))
            .with_effect_kind(Some(self.effect_kind))
    }
```

The snippet above gives the public shape of `entry`; its implementation lives in the same file.

### 3.2 Summon zone


First, the definition of `SUMMON_CAP`; its declaration follows.
文件: crates/definitions/src/families/summon/zones/mod.rs

```rust
/// 一方召唤物区上限：4（规则书·游戏区域 IV）。
pub const SUMMON_CAP: Nat = Nat::new(4);
```

The snippet above gives the public shape of `SUMMON_CAP`; its implementation lives in the same file.


First, the definition of `SummonEntry`; its declaration follows.
文件: crates/definitions/src/families/summon/zones/mod.rs

```rust
/// 一个召唤物：召唤物实体 + 召唤物卡 id + 计量（可用次数 / 效果量 / 效果类型标识）。
///
/// - `uses`：可用次数（`None` = 没有可用次数，触发不递减 —— 分工计划「三项改可选」）；
/// - `points`：效果量（伤害点数 / 重复次数…，含义由执行动作解释，`None` = 无计量）；
/// - `effect_kind`：效果类型（`SummonEffectKind`，由召唤物定义给出；区域层不解释）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SummonEntry {
    /// 召唤物实体号。
    pub summon: EntityId,
    /// 召唤物卡定义 id。
    pub card: CardId,
    /// 可用次数（None = 无可用次数）。
    pub uses: Option<Nat>,
    /// 效果量（含义由执行动作解释）。
    pub points: Option<Nat>,
    /// 效果类型（正式枚举；区域层不解释）。
    pub effect_kind: Option<SummonEffectKind>,
}
```

The snippet above gives the public shape of `SummonEntry`; its implementation lives in the same file.


First, the definition of `SummonZone`; its declaration follows.
文件: crates/definitions/src/families/summon/zones/mod.rs

```rust
/// **召唤物区**：最多 4 个召唤物，按召唤先后排列。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的区域」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct SummonZone {
    /// 召唤物条目（顺序 = 召唤先后）。
    entries: Vec<SummonEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: SummonZoneStats,
}
```

The snippet above gives the public shape of `SummonZone`; its implementation lives in the same file.


First, the definition of `summon_with_triggers`; its declaration follows.
文件: crates/definitions/src/families/summon/zones/mod.rs

```rust
    /// **召唤 / 生成**（规则书·游戏区域 IV）+ 派发「生成 / 合并」。
    ///
    /// - 同名（同卡 id）已存在：**合并**（使用次数取最大、效果量取最大、效果类型取最新），
    ///   不新增占位、不改变原位置；`outcome = Merged`；
    /// - 未满：占一个新位置；`outcome = Summoned`；
    /// - 已满且没有同名：**无法生成**，状态不变；`outcome = RejectedFull`（不派发）。
    pub fn summon_with_triggers(
        &self,
        entry: SummonEntry,
        triggers: &SummonTriggers,
    ) -> SummonResolution {
        let mut zone = self.clone();
        let mut merged: Option<(EntityId, Option<Nat>)> = None;
    // ...（其余实现从略；见 crates/definitions/src/families/summon/zones/mod.rs）
```

The snippet above gives the public shape of `summon_with_triggers`; its implementation lives in the same file.


First, the definition of `tick_with_triggers`; its declaration follows.
文件: crates/definitions/src/families/summon/zones/mod.rs

```rust
    /// **触发一次**（规则书·特殊词条 可用次数）：可用次数 -1；耗尽则立刻弃置。
    ///
    /// - 有可用次数且减到 0：移出 + 派发 `Depleted`；
    /// - 有可用次数且还有剩余：写回剩余次数 + 派发 `Triggered`（`amount` = 剩余次数）；
    /// - **没有可用次数**：不递减、不移除，只派发 `Triggered`（`amount` = 0）。
    ///
    /// 失败（返回 None、不改状态）：这个召唤物不在区里。
    pub fn tick_with_triggers(
        &self,
        summon: EntityId,
        triggers: &SummonTriggers,
    ) -> Option<SummonResolution> {
        let position = self
            .entries
    // ...（其余实现从略；见 crates/definitions/src/families/summon/zones/mod.rs）
```

The snippet above gives the public shape of `tick_with_triggers`; its implementation lives in the same file.

### 3.3 Enemy cards


First, the definition of `EnemyDef`; its declaration follows.
文件: crates/definitions/src/families/enemy/def.rs

```rust
/// 一张**敌人牌**的定义（纯数据）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；与 `CharacterDef` 同一做法）。
#[derive(Clone, Copy, Debug)]
pub struct EnemyDef<Tags> {
    /// 敌人牌卡 id（敌人牌族 id 区域内唯一）。
    pub id: CardId,
    /// 敌人名（本块只填中文，D120）。
    pub name: Localized,
    /// 敌人牌描述（数据没有顶层描述 -> `Localized::EMPTY`，见模块文档）。
    pub description: Localized,
    /// 生命上限（定义数据；运行期当前值不在本层）。
    pub max_hp: Nat,
    /// 技能列表（复用 [`SkillDef`]；顺序 = 数据里的技能顺序）。
    pub skills: &'static [&'static SkillDef],
    /// 标签集合（D54 / D55）。
    pub tags: Tags,
}
```

The snippet above gives the public shape of `EnemyDef`; its implementation lives in the same file.


First, the definition of `new`; its declaration follows.
文件: crates/definitions/src/families/enemy/def.rs

```rust
    /// 建一张敌人牌定义（常量 / 测试用）。
    pub const fn new(
        id: CardId,
        name: Localized,
        description: Localized,
        max_hp: Nat,
        skills: &'static [&'static SkillDef],
        tags: Tags,
    ) -> EnemyDef<Tags> {
        EnemyDef {
            id,
            name,
            description,
            max_hp,
            skills,
            tags,
        }
    }
```

The snippet above gives the public shape of `new`; its implementation lives in the same file.


First, the definition of `has_skills`; its declaration follows.
文件: crates/definitions/src/families/enemy/def.rs

```rust
    /// 是否有技能（数据里每个敌人都有技能，但定义层不假设至少一个）。
    pub fn has_skills(&self) -> bool {
        !self.skills.is_empty()
    }
```

The snippet above gives the public shape of `has_skills`; its implementation lives in the same file.

### 3.4 Status definitions


First, the definition of `StatusType`; its declaration follows.
文件: crates/definitions/src/families/status/mod.rs

```rust
/// 状态的类型（L2）：角色状态 / 出战状态。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StatusType {
    /// 角色状态：附在**某个角色**身上（跨回合，跟随角色）。
    CharacterStatus,
    /// 出战状态：附在**某方场上**（不跟随具体角色）。
    CombatStatus,
}
```

The snippet above gives the public shape of `StatusType`; its implementation lives in the same file.


First, the definition of `StatusDef`; its declaration follows.
文件: crates/definitions/src/families/status/def.rs

```rust
/// 一张**状态牌**的定义（纯数据；身份由 `known.rs` 的具体类型给）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
/// 本类型**不实现** `StateIdentity`：身份是「指向静态实例的指针」（D95 / D96），
/// 定义只是那份静态实例里的一份数据。
#[derive(Clone, Copy, Debug)]
pub struct StatusDef<Tags> {
    /// 状态牌的卡 id（状态牌族 id 区域内唯一）。
    ///
    /// 没有独立卡牌的共享身份（如「伤害抵消」）**不建** `StatusDef`，见
    /// `damage_chain::statuses`（那是「效果共享身份」，不是某张卡）。
    pub id: CardId,
    /// 状态类型（角色状态 / 出战状态，D51）。
    pub status_type: StatusType,
    /// 4 语名字 + 4 语描述（**唯一一份**；身份经它取，不另存）。
    pub text: StateText,
    /// 默认层数：`None` = 布尔形态；`Some(n)` = 层数形态默认 n 层。
    pub default_layers: Option<Nat>,
    /// 持续回合数：`None` = 不按回合清减；`Some(n)` = 回合开始清减 n（D13）。
    pub duration: Option<Nat>,
    /// 标签集合（D54：标签是零大小类型；集合形态本轮未定）。
    pub tags: Tags,
}
```

The snippet above gives the public shape of `StatusDef`; its implementation lives in the same file.


First, the definition of `new`; its declaration follows.
文件: crates/definitions/src/families/status/def.rs

```rust
    /// 建一份状态定义（常量 / 测试用；6 个参数，不再拆构造器）。
    pub const fn new(
        id: CardId,
        status_type: StatusType,
        text: StateText,
        default_layers: Option<Nat>,
        duration: Option<Nat>,
        tags: Tags,
    ) -> StatusDef<Tags> {
        StatusDef {
            id,
            status_type,
            text,
            default_layers,
            duration,
            tags,
        }
    }
```

The snippet above gives the public shape of `new`; its implementation lives in the same file.


First, the definition of `form`; its declaration follows.
文件: crates/definitions/src/families/status/def.rs

```rust
    /// 状态的形态（D104）：`default_layers` 为 `None` -> 布尔；`Some` -> 层数。
    ///
    /// 形态由「默认层数」这一份数据推导，不另存第二份（D64 精神）。
    pub const fn form(&self) -> StateForm {
        match self.default_layers {
            None => StateForm::Boolean,
            Some(_) => StateForm::Layered,
        }
    }
```

The snippet above gives the public shape of `form`; its implementation lives in the same file.


First, the definition of `status_identity`; its declaration follows.
文件: crates/definitions/src/families/status/known.rs

```rust
/// 声明一个状态的「身份类型 + 身份静态实例」。
///
/// 身份实例 = 指向该状态静态定义的一份指针（`&'static StatusDef<Tags>`）；地址即身份
/// （D95 / D96），文本从定义里取（只有一份，D110）。
macro_rules! status_identity {
    (
        $(#[$meta:meta])*
    // ...（其余实现从略；见 crates/definitions/src/families/status/known.rs）
```

The snippet above gives the public shape of `status_identity`; its implementation lives in the same file.


First, the definition of `SHIELD_COMBAT_DEF`; its declaration follows.
文件: crates/definitions/src/families/status/known.rs

```rust
/// 出战状态「护盾」（id 203）的定义。
///
/// 数据：`shield.json`；名字「护盾」，`statustypetext` = 阵营出战状态，
/// 描述「为我方出战角色提供1点护盾。（可叠加，没有上限）」，标签 `GCG_TAG_SHEILD`。
pub static SHIELD_COMBAT_DEF: StatusDef<Shield> = StatusDef::new(
    CardId::new(203),
    StatusType::CombatStatus,
    StateText {
        name: Localized::from_parts(Some("护盾"), None, None, None),
        description: Localized::from_parts(
            Some("为我方出战角色提供1点护盾。（可叠加，没有上限）"),
            None,
            None,
            None,
        ),
    },
    // 卡面「提供1点护盾」；待复核：数据没有结构化层数字段。
    Some(Nat::new(1)),
    None,
    Shield,
);
```

The snippet above gives the public shape of `SHIELD_COMBAT_DEF`; its implementation lives in the same file.


First, the definition of `FROZEN_DEF`; its declaration follows.
文件: crates/definitions/src/families/status/known.rs

```rust
/// 角色状态「冻结」（id 106）的定义。
///
/// 数据：`frozen.json`；名字「冻结」，`statustypetext` = 角色状态，
/// 描述「角色无法使用技能。（持续到回合结束）\n角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。」；
/// 标签 `GCG_TAG_FORBIDDEN_ATTACK`（标签类型还没有 -> `()`，待接入）。
pub static FROZEN_DEF: StatusDef<()> = StatusDef::new(
    CardId::new(106),
    StatusType::CharacterStatus,
    StateText {
        name: Localized::from_parts(Some("冻结"), None, None, None),
        description: Localized::from_parts(
            Some("角色无法使用技能。（持续到回合结束）\n角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。"),
            None,
            None,
            None,
        ),
    },
    // 布尔形态（D104：在列表里即代表「有」）。
    None,
    // 数据只写「持续到回合结束」，按 1 回合计（D13 回合开始清减）；待复核。
    Some(Nat::new(1)),
    (),
);
```

The snippet above gives the public shape of `FROZEN_DEF`; its implementation lives in the same file.

### 3.5 Status identity bridge


First, the definition of `StatusEntity`; its declaration follows.
文件: crates/definitions/src/families/character/damage_chain/statuses.rs

```rust
/// 一条「卡 id -> 状态身份」登记。
#[derive(Clone, Copy, Debug)]
pub struct StatusEntity {
    /// 状态卡的卡 id（状态牌族 id 区域）。
    pub card: CardId,
    /// 状态身份（类型化静态实例指针，D95 / D96）。
    pub identity: &'static dyn StateIdentity,
}
```

The snippet above gives the public shape of `StatusEntity`; its implementation lives in the same file.


First, the definition of `STATUS_ENTITIES`; its declaration follows.
文件: crates/definitions/src/families/character/damage_chain/statuses.rs

```rust
/// 反应会附属 / 生成的状态**静态登记表**（顺序 = 规则书条目顺序，不参与判定）。
///
/// 后三项不是反应产物，而是**关键字 / 卡牌会按卡 id 附属**的状态（R5 / L1）：
/// 眩晕 100001、石化 116033、饱腹 303300 —— 登记后 `keywords::attach_status`
/// 用同一张表就能查到身份（状态块落地前先由本族提供共享身份）。
pub const STATUS_ENTITIES: [StatusEntity; 8] = [
    StatusEntity::new(entities::FROZEN, &FROZEN),
    StatusEntity::new(entities::SHIELD, &SHIELD_COMBAT),
    StatusEntity::new(entities::DENDRO_CORE, &DENDRO_CORE),
    StatusEntity::new(entities::CATALYZING_FIELD, &CATALYZING_FIELD),
    StatusEntity::new(CardId::new(134), &SHIELD_CHARACTER),
    StatusEntity::new(STUN_CARD, &STUN),
    StatusEntity::new(PETRIFIED_CARD, &PETRIFIED),
    StatusEntity::new(SATIATED_CARD, &SATIATED),
];
```

The snippet above gives the public shape of `STATUS_ENTITIES`; its implementation lives in the same file.


First, the definition of `status_of_card`; its declaration follows.
文件: crates/definitions/src/families/character/damage_chain/statuses.rs

```rust
/// 按卡 id 查状态身份（没登记 -> `None`）。
pub fn status_of_card(card: CardId) -> Option<&'static dyn StateIdentity> {
    STATUS_ENTITIES
        .iter()
        .find(|entry| entry.card == card)
        .map(|entry| entry.identity)
}
```

The snippet above gives the public shape of `status_of_card`; its implementation lives in the same file.


First, the definition of `same_identity`; its declaration follows.
文件: crates/definitions/src/families/character/damage_chain/statuses.rs

```rust
/// 两个状态身份是否**同一份身份**（比较 `identity_address()` = 指向定义数据的地址；见模块文档）。
pub fn same_identity(left: &'static dyn StateIdentity, right: &'static dyn StateIdentity) -> bool {
    crate::families::action_card::status::same_state_identity(left, right)
}
```

The snippet above gives the public shape of `same_identity`; its implementation lives in the same file.


First, the definition of `identity_in`; its declaration follows.
文件: crates/definitions/src/families/character/damage_chain/statuses.rs

```rust
/// 这条身份是否在给定的身份集合里（按地址比较）。
pub fn identity_in(
    identity: &'static dyn StateIdentity,
    set: &[&'static dyn StateIdentity],
) -> bool {
    set.iter()
        .copied()
        .any(|candidate| same_identity(candidate, identity))
}
```

The snippet above gives the public shape of `identity_in`; its implementation lives in the same file.


First, the definition of `SHIELD_STATUSES`; its declaration follows.
文件: crates/definitions/src/families/character/damage_chain/statuses.rs

```rust
/// 算作**护盾**的全部状态身份（出战状态 203 + 角色状态 134）。
///
/// 两种写法（「为我方出战角色提供 N 点护盾」/「每层抵消 1 点此角色受到的伤害」）在规则上
/// 都是护盾，所以伤害修正链把两处承载的点数**相加**后一起抵消。
pub const SHIELD_STATUSES: &[&'static dyn StateIdentity] = &[&SHIELD_COMBAT, &SHIELD_CHARACTER];
```

The snippet above gives the public shape of `SHIELD_STATUSES`; its implementation lives in the same file.

### 3.6 Rules-ops interfaces referenced by this page

> [!NOTE]
> Real source of the keyword-layer / rules-ops functions referenced by this page (signatures and doc comments copied as-is).


First, the definition of `USES_PER_TRIGGER`; its declaration follows.
文件: crates/definitions/src/rules/ops/lifetime.rs

```rust
/// 每次触发消耗的可用次数（规则书：1 次）。
pub const USES_PER_TRIGGER: Nat = Nat::new(1);
```

The snippet above gives the public shape of `USES_PER_TRIGGER`; its implementation lives in the same file.


First, the definition of `uses_after_trigger`; its declaration follows.
文件: crates/definitions/src/rules/ops/lifetime.rs

```rust
/// 触发一次后的可用次数（减到 0 为止；调用方随后弃置）。
pub fn uses_after_trigger(uses: Nat) -> Nat {
    nsub!(uses, USES_PER_TRIGGER)
}
```

The snippet above gives the public shape of `uses_after_trigger`; its implementation lives in the same file.


First, the definition of `is_depleted`; its declaration follows.
文件: crates/definitions/src/rules/ops/lifetime.rs

```rust
/// 是否耗尽（= 0 -> 立刻弃置）。
pub fn is_depleted(value: Nat) -> bool {
    value == Nat::ZERO
}
```

The snippet above gives the public shape of `is_depleted`; its implementation lives in the same file.


First, the definition of `DURATION_PER_END_PHASE`; its declaration follows.
文件: crates/definitions/src/rules/ops/lifetime.rs

```rust
/// 每个结束阶段扣减的持续回合（规则书：1）。
pub const DURATION_PER_END_PHASE: Nat = Nat::new(1);
```

The snippet above gives the public shape of `DURATION_PER_END_PHASE`; its implementation lives in the same file.


First, the definition of `duration_after_end_phase`; its declaration follows.
文件: crates/definitions/src/rules/ops/lifetime.rs

```rust
/// 经过一个结束阶段后的持续回合（减到 0 为止）。
pub fn duration_after_end_phase(duration: Nat) -> Nat {
    nsub!(duration, DURATION_PER_END_PHASE)
}
```

The snippet above gives the public shape of `duration_after_end_phase`; its implementation lives in the same file.


First, the definition of `ticks_in_end_phase`; its declaration follows.
文件: crates/definitions/src/rules/ops/lifetime.rs

```rust
/// 持续回合的递减发生在**结束阶段**（不是回合开始、也不是行动阶段开始）。
///
/// 规则书原文：「每个结束阶段，都会使『持续回合』-1」；英文 "Each End Phase"。
/// 注意：这与决策日志 D13 的旧写法（回合开始递减）冲突，已按规则书为准改口径。
pub fn ticks_in_end_phase() -> bool {
    true
}
```

The snippet above gives the public shape of `ticks_in_end_phase`; its implementation lives in the same file.

## 4. Invariants and prohibitions

- **Usage is not duration** (D12 / rulebook "Duration (Rounds)").
- **Summon cap 4**: no new slot when full; a same-name merge adds no slot and **keeps the original position**; uses / points take the max, effect kind the newest (plan §4.4.15).
- **Statuses have one common list** (D104 / D105): no second container / numbering table / per-zone split.
- **Identity = the definition data address** (D57 / D58 / D200): compare with `same_identity`, never with `&dyn` fat pointers or ZST addresses.
- **The framework does not interpret status semantics** (D108): `StateList` / `CombatStatusZone` store identity + layers + duration only.
- **Only two status types** (D51 / D201): no "character equipment" third type.
- **Shared identities never fabricate a card id / StatusDef** (arcane edict, damage negation, damage immunity): identities without card data get static text only.
- **Enemies: definition only, no zone** in this round; do not copy character-zone logic to enemies.
- Pure functions, state unchanged on failure; numbers are `Nat` / `Fraction`; zero `f64`, zero `HashMap`.

> [!WARNING]
> Numbering statuses / building per-zone tables / using ZST self-addresses as identity breaks status merging and reaction matching (D42 / D105 / D200).

> [!NOTE]
> Satiety / stun / petrified are currently **shared identities** provided by the character family; the formal `StatusDef` belongs to the status block and only one of the two may exist (留空.md #79).

## 5. Basis (rulebook and D-numbers)

- Summon cap 4 / cannot create when full / settle in entry order: Rulebook "Game Zones IV"
- Usage decrements per trigger, discard when depleted: Rulebook special entry "Usage"; D12
- Duration -1 per End Phase: Rulebook special entry "Duration (Rounds)"; code follows the rulebook, D13's older wording is superseded
- Same-name summon merge: Plan §4.4.15 (Lead ruling)
- Two status types share one id region: D51; rulebook "Game Zones II"
- Shared status list / boolean presence / pointer identity: D95 / D96 / D104 / D105 / D108 / D110 / D200
- One arcane edict per player: Rulebook special entry "Arcane Legend" + Arcane Edict; D264; 留空.md #68 / #98 / #99
- Satiety / frozen / stun / petrified: S8.1 / S6.6 / Stunned / Technique / S8.4; 留空.md #31 / #79 / #80 / #82
- Enemy cards: data `tcgenemycards`; runtime bearing not designed (pending)

### 5.1 Documentation vs code mismatches (for review)

> [!NOTE]
> The current code and the rulebook win over the documents below.

1. `决策日志.md` **D13** says "duration decrements at round start", while the rulebook says "each End Phase"; `rules/ops/lifetime.rs` follows the rulebook and notes the supersession — the decision log should get a correction entry.
2. `结构.md` implies `families/status/` owns status zones; actually bearing is in the character family (`CharacterState.states` / `CombatStatusZone`), and `families/status/` only has definitions and identities.
3. The formal status definitions for stun / petrified / satiety are not yet in `families/status/known.rs`; the character family provides shared identities for now (known debt, 留空.md #79).

## 6. Common misuse

> [!WARNING]
> Most of the patterns below do not fail at the type level, but they change settlement results.

1. **Implementing "usage -1" at action-phase start**: the reference implementation's old bug (named by D12); usage is per trigger, duration is per End Phase.
2. **Creating a new slot for a same-name summon**: merge instead (max uses / points, newest kind, same position).
3. **Numbering statuses / per-zone tables / a second list**: violates D42 / D105.
4. **Using ZST self-addresses as identity**: different ZST statics may share an address → broken merging / reaction matching (D200); always use `identity_address()`.
5. **Treating the arcane edict as a status card**: no card id, no `StatusDef`; decide with `ArcaneEdict::can_play`.
6. **Creating another identity for satiety / stun / petrified**: they are shared identities now; the formal definition belongs to the status block and only one may exist (留空.md #79).
7. **Treating the status type as a third "character equipment" type**: present in data, deliberately not modelled (D201).
8. **Copying the character zone for enemies**: enemy runtime bearing is not designed yet.

## 7. Tests

- `summon/def.rs`: `summon_def_carries_id_text_kind_points_uses_and_tags`, `entry_carries_the_def_measurements_into_the_zone_entry`
- `summon/zones/mod.rs`: `cap_is_four_and_fifth_is_rejected_without_state_change`, `same_card_merges_uses_points_and_keeps_kind_newest`, `merge_keeps_position_and_works_when_full`, `tick_decrements_uses_and_discards_at_zero`
- `enemy/def.rs`: `enemy_def_carries_id_name_description_hp_tags_and_skills`, `enemy_def_reuses_skill_defs_in_data_order`
- `status/def.rs`: `status_def_carries_id_type_text_layers_duration_and_tags`, `form_is_derived_from_default_layers_only`
- `status/known.rs`: `known_status_defs_use_the_data_ids_and_types`, `identities_are_distinct_static_instances_with_one_text_copy`
- `status/arcane_edict.rs`: `fresh_container_allows_arcane_and_records_play`, `consumed_edict_blocks_arcane_with_no_edict`
- `families/character/damage_chain/statuses.rs`: `keyword_and_card_statuses_are_registered_by_their_data_ids` (bridge entries 100001 / 116033 / 303300)
- `families/character/satiated.rs`: `satiated_identity_comes_from_the_status_data`, `food_effect_grants_satiated_once_and_blocks_food_targeting`

## 8. Pending and to be verified

- **Enemy runtime bearing / energy cap**: code marks `待接入`; 留空.md has no dedicated entry (recommend adding one).
- **Formal `StatusDef` for satiety / stun / petrified**: shared identities now; #79 requires choosing one of the two (no coexistence); wording differences #80; the two petrified data entries share one identity #82.
- **Concrete ward identities** (cryo/electro/hydro/pyro wards, surge/thunder/flame wards, conditional immunity of "Weakness Exposed"): #31.
- **Third status type "character equipment"**: not modelled, to be reviewed #23.
- **Hand-card status identities and descriptions**: #10 / #106 / #115.
- **Arcane edict**: container owner / check vs play-condition order / whether it shows as a status: #68 / #98 / #99.
- **Stack caps for summons / hand-card statuses** (`stack_cap` / `stack_limit`): #70 / #71 / #72.
- **Elemental transmutation**: #108 / #118.
- **Duration vs D13**: code follows the rulebook (End Phase); D13's older wording is superseded — the decision log should get a correction entry.

## Summary

Of the summon, enemy and status families, only summons and statuses have carriers; enemies have definitions only in this round.
Summon cap is 4, same-name merging keeps the position and takes the maximum; usage decrements per trigger while duration decrements per End Phase.
A status identity is a pointer to definition data; semantic interpretation belongs to the damage chain and flow layers, not here.
---
Last verified: 2026-10-19 (code: `crates/definitions/src/rules/ops/lifetime.rs::duration_after_end_phase`)
