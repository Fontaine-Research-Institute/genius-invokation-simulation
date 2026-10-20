# Definitions: Families

> [[定义层-Families|中文]] · English

*Scope: an overview of the five card families under `crates/definitions/src/families/` — which zones each owns and where its extension points are.*

---

## 1. Responsibility

Organize the **five card families (L1 `CardFamily`)** into "type definitions + zone state + zone operations". **Concrete cards are not in this crate**: per D240 they live in `gi-genius-invokation-cards` (`new/crates/cards/`).

## 2. Key concepts and types

### 2.1 The three-level model (D51 / D53–D55)

- **L1**
  - Concept: Card family
  - Definition site: `CardFamily` in `src/base/family.rs`
  - Notes: `ActionCard` / `Summon` / `Character` / `Enemy` / `Status`; **the family decides the id region**, ids are not guaranteed unique across families
- **L2**
  - Concept: Type
  - Definition site: **Only action cards and statuses have one**: `ActionCardType` in `src/families/action_card/mod.rs`, `StatusType` in `src/families/status/mod.rs`
  - Notes: Character / summon / enemy have no type; pinned by an exhaustive match in `src/families/mod.rs`
- **L3**
  - Concept: Tag
  - Definition site: Top-level `src/tags/` (sibling of families, D53)
  - Notes: Each tag = one **zero-sized type + one static `TagText`** (D54 / D57 / D58 / D64); no "tag enum", no numeric ids

### 2.2 The five families at a glance

- **ActionCard**
  - Definition carriers: `ActionCardDef` (type / text / base cost / play condition D217)
  - Zones / containers owned: Hand `HandState`, pile `PileState`, support zone `SupportZone`, hand-card states `HandCardStates`, current cost `CurrentCost`
  - Extension points: New card: cards crate; type-level behaviour: `rules/` rows + `flow/`
  - Files: `src/families/action_card/{def.rs, zones/, areas/}`
- **Character**
  - Definition carriers: `CharacterDef` + `SkillDef` + `SkillKind`
  - Zones / containers owned: Character zone `CharacterZone` (3 slots + active pointer), per-character `EquipmentZone`, combat status zone `CombatStatusZone`, `CharacterState` (HP / energy / statuses / attachment)
  - Extension points: New skill: `SkillDef`; new timing: `CharacterTiming`; new reaction: `ReactionRegistry`; damage modifiers: `DamageModifiers`
  - Files: `src/families/character/`
- **Summon**
  - Definition carriers: `SummonDef` + `SummonEffectKind`
  - Zones / containers owned: Summon zone `SummonZone` (cap 4 / same-name merge / usage decrement D12)
  - Extension points: New summon: cards crate; execution action pending
  - Files: `src/families/summon/{def.rs, zones/mod.rs}`
- **Enemy**
  - Definition carriers: `EnemyDef` (HP / skill list, reusing `SkillDef`)
  - Zones / containers owned: **No zone / no operations**
  - Extension points: Definition data only; runtime bearing not designed
  - Files: `src/families/enemy/def.rs`
- **Status**
  - Definition carriers: `StatusDef` + five formal identities in `known.rs` + `ArcaneEdict`
  - Zones / containers owned: **Statuses have no zone of their own**: character statuses in `CharacterState.states` (shared `StateList`, D105), combat statuses in `CombatStatusZone`
  - Extension points: New status: `StatusDef` + identity instance; semantics belong to the status block
  - Files: `src/families/status/{def.rs, known.rs, arcane_edict.rs}`

### 2.3 Zone conventions (D40 / D42 / D182 / D183)

- **A zone is a field (a variable) inside state**: no `AreaId`, no zone registry, no "entity → owning zone" mapping.
- Sides = **two named instances of the same container types** (`DefState.me` / `DefState.opp`); no side parameter, no 0/1 encoding (D29 / D34).
- Zones expose **read-only pointers** (`entries()` / `len()` / `stats()` etc.); every update is a **pure function** (takes `&self`, returns a new value).
- Timings: each zone defines its own timing key (e.g. `SupportTiming` / `SummonTiming` / `CharacterZoneTiming` / `CombatStatusTiming`) → `timeline::dispatch` **orders only, never executes** (D185 / D186).

> [!IMPORTANT]
> A zone is a struct field (D42): never number zones, never build an "entity → zone" table; the code that owns the field already knows where the entity is.
> Sides are two named instances (D29 / D34), never a parameter or a numeric encoding.

## 3. Public entry points

> [!NOTE]
> Code below is copied from the current repository (including use, derive, doc comments and other boilerplate). Over-long function bodies are truncated to the signature + first body lines with an explicit note; everything else is the full item.

### 3.1 Card family and type enums


First, the definition of `CardFamily`; its declaration follows.
文件: crates/definitions/src/base/family.rs

```rust
/// L1 牌族（id 区域）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CardFamily {
    /// 行动牌（有类型：支援 / 装备 / 事件）。
    ActionCard,
    /// 召唤物（无类型）。
    Summon,
    /// 角色牌（无类型）。
    Character,
    /// 敌人牌（无类型）。
    Enemy,
    /// 状态（无类型；角色状态 / 出战状态在牌族自己的 StatusType 里区分）。
    Status,
}
```

The snippet above gives the public shape of `CardFamily`; its implementation lives in the same file.


First, the definition of `ActionCardType`; its declaration follows.
文件: crates/definitions/src/families/action_card/mod.rs

```rust
/// 行动牌的类型（L2）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ActionCardType {
    /// 支援牌：打到支援区，留在场上持续生效（场地 / 伙伴 / 道具 / 护盾 / 元素幻变 …）。
    Support,
    /// 装备牌：挂到角色身上（武器 / 圣遗物 / 天赋 / 特技 …）。
    Equipment,
    /// 事件牌：结算完就离开（料理 / 秘传 / 元素共鸣 / 战斗行动 …）。
    Event,
}
```

The snippet above gives the public shape of `ActionCardType`; its implementation lives in the same file.


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

## 4. Invariants and prohibitions

- **No zone numbers** (D42); **containers do not interpret semantics** (D108 / D112); **exactly one copy of data**: text lives in `generated` (D61 / D64).
- **Pure functions first**: operations take `&self` and return new values; failure = state unchanged + `None` / `false` / empty branches, no panic (division by zero excepted, D44).
- **No central registry**: the framework only provides registration interfaces; each concrete card registers itself (D20 / D22).
- **Concrete cards never enter definitions** (D240 / D241); `cards` is not pulled into simulation by default (D243).
- Numbers are `Nat` (saturating) or `ExactNat128` (probabilities); bare `+ - * /` and bare integer types are forbidden (§8.3), as are `f64` (D159) and `HashMap`.

> [!WARNING]
> Building a registry or a stats hub under `families/` violates D20 / D240; the framework gives interfaces, each feature implements itself.
> Treating a tag as a number or comparing tags by string violates D54 / D57; compare by pointer identity (D199 / D200).

## 5. Basis (rulebook and D-numbers)

- Three zones (character / support / summon) + hand / pile / equipment attachment: Rulebook "Game Zones I–VI"; `families/*/zones/`
- Family = id region; type / tag layering: D51 / D53 / D54 / D55
- Sides stay out of the engine; two named instances: D29 / D30 / D34
- Zone = field; no zone numbers / registry: D40 / D42
- Stats sink into `stats/`, read-only pointers, trimmable: D181 / D182 / D183 / D187
- Timings defined and dispatched by each family: D184 / D185 / D186
- Record API / content type in its own file: D221 / D223 / D224 / D225
- Framework vs concrete cards split into crates: D240 / D241 / D242 / D243

### 5.1 Documentation vs code mismatches (for review)

> [!NOTE]
> This review found deviations in `结构.md`; D53 / D55 and the current code win, the structure diagram needs an update.

1. `结构.md` §4.3 draws tags under `families/<family>/types/<type>/tags/`; they actually live in top-level `src/tags/`.
2. `结构.md` plans `families/action_card/types/`, `families/character/skills/`, `zones/status.rs`, `zones/skill.rs`; none exist (type-level action-card behaviour is in `rules/` + `flow/`; skill flow is in `skill_flow/`; character statuses use the shared `StateList`).
3. `结构.md` calls the combat status zone `zones/status.rs`; the real file is `zones/combat_status.rs`.

## 6. Common misuse

> [!WARNING]
> The patterns below break three global conventions: "zone = field", "one copy of data", "no registries".

1. **Treating a tag as a number**: tags are zero-sized types + static metadata pointers (D54 / D57); never `as u32`, never compare by string.
2. **Numbering zones / building an "entity → zone" table**: violates D42.
3. **Building registries or stats hubs under `families/`**: violates D20 / D240.
4. **Referencing an id without its family**: ids can repeat across families (D51); always use `(CardFamily, CardId)`.
5. **Folding stats into position equality**: each zone's `PartialEq` explicitly excludes stats (D187).
6. **Copying text into definitions**: text comes from `generated` (D61 / D64 / D120).
7. **Confusing duration with usage**: usage decrements per trigger (D12); duration decrements per End Phase (rulebook; D13's older wording is superseded).

## 7. Tests

- `src/families/mod.rs`: `tests::only_action_card_and_status_have_types`, `tests::type_enums_belong_to_their_own_family`
- `src/families/action_card/mod.rs`: `tests::action_card_has_exactly_three_types`
- `src/families/status/mod.rs`: `tests::status_types_share_one_id_region`
- Family pages: see the "Tests" sections of [[定义层-Character-EN]] / [[定义层-Action-Card-EN]] / [[定义层-Summon-Enemy-Status-EN]]

## 8. Pending and to be verified

- **Enemy cards: no zone / no runtime bearing**: marked "pending" in code; `留空.md` has no dedicated entry yet (recommend adding one).
- **Concrete cards: only one sample** (`cards/src/cards/character`): belongs to the card-implementation phase; `留空.md #12` records that the character-card structure still needs a user decision, multi-version loading is `留空.md #24` (D248 / D249 / D251).
- **Card version-range loading** is not implemented: `留空.md #24`.
- **No `types/` subdirectories for action cards / statuses**: see §5.1.
- Per-zone pending items (duration timing, support order, summon merge semantics, …) are listed on the family pages and reference `留空.md #67` / `#93` / `#94`.

## Summary

This page indexes the five card families: each has definition carriers plus its own zones, and concrete cards live in the cards layer.
Families compose through the keyword layer, rules ops and the flow layer; they do not call each other.
New families follow the zone-as-field convention, with no zone numbers and no entity-to-zone map.
---
Last verified: 2026-10-19 (code: `crates/definitions/src/families/mod.rs::tests::only_action_card_and_status_have_types`)
