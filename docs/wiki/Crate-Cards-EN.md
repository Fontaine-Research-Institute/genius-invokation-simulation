# Crate-Cards (Concrete Cards)

> [[Crate-Cards|中文]] · English

*Scope: the `cards` crate's responsibility, card organization and sample, public entry points, invariants, basis, tests, and pending work.*

---

## 1. Responsibility

The concrete-card layer: only "what this exact card is" — each card's own data (id / tags / HP / energy / skills / text) and effect logic. Card resources and shapes stay in `definitions` (D240 / D241). It has a one-way dependency on `definitions` / `engine`, never the reverse.

- Directory: `new/crates/cards/`
- package / lib: `gi-genius-invokation-cards` / `gi_genius_invokation_cards`

## 2. Key Concepts and Types

| Concept | Location | Notes |
|---|---|---|
| Crate root | `src/lib.rs` | `pub mod cards; pub mod source;` + `#![deny(clippy::arithmetic_side_effects)]` |
| Card tree | `src/cards/` | structure moved from `definitions/src/cards/`; only `character/aino/` so far |
| Family entry | `src/cards/character/mod.rs` | one directory per card (D163); no central registry yet |
| First sample card | `src/cards/character/aino/mod.rs` | Aino (character card 1216): `ID` / `MAX_HP = 10` / `MAX_ENERGY = 2` / tag tuple `(Hydro, Claymore, NodKrai)` / four `SkillDef` / `AINO: CharacterDef<Tags>` |
| Version sync | same file: `card_game_version!(6, 2);` | generates `GAME_VERSION: GameVersion` (D244-D246); bump the number when nothing else changes |
| Source self-declaration | same file: `card_source!(path: ["character", "aino"]);` | generates `pub static SOURCE: SourceFile` (verbatim text + compile-time use dependencies; D214-D216) |
| Card source table | `src/source/mod.rs` | `SOURCE_FILES` (hand-written list), `source_files()` / `source_of(path)` / `source_files_under(prefix)` |
| Source mechanism | `definitions/src/source/mod.rs` | `card_source!` / `module_source!`, `SourceFile` / `SourceOwner`, dependency parser stay in definitions (D242) |
| Effect bodies | same Aino file | the four `fn ..._effect()` are `todo!("missing: ...")` placeholders right now |

The tag-set representation is still undecided; the sample uses a type-level zero-sized tuple `type Tags = (Hydro, Claymore, NodKrai)` plus `const TAGS: Tags` (D54: a tag is the type itself).

## 3. Public Entry Points

| Entry | Notes |
|---|---|
| `cards::cards::character::aino::{ID, MAX_HP, MAX_ENERGY, Tags, TAGS, SKILL_*, AINO, SOURCE}` | all constants and definitions of the sample card |
| `cards::source::{SOURCE_FILES, source_files, source_of, source_files_under}` | card source table queries |
| `cards::source::{SourceOwner, DependencyKind}` | re-exports of the definitions source types |
| `definitions::card_source!` / `card_game_version!` | macros used by card files (mechanism in definitions, D242) |

> [!NOTE]
> Card definitions use `static` (stable address); `SkillDef` contains id / name / description / `kind` / `cost` / `effect` function pointer.

### 3.1 Real Source Excerpts (verbatim from the current repository)

**Card id `ID`**

`new/crates/cards/src/cards/character/aino/mod.rs:30-31`

```rust
/// 爱诺的卡牌 id（角色牌族）。
pub const ID: CardId = CardId::new(1216);
```

**Version declaration `card_game_version!`**

`new/crates/cards/src/cards/character/aino/mod.rs:42`

```rust
gi_genius_invokation_definitions::card_game_version!(6, 2);
```

**Source declaration `card_source!`**

`new/crates/cards/src/cards/character/aino/mod.rs:54`

```rust
gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);
```

**Tag tuple `Tags`**

`new/crates/cards/src/cards/character/aino/mod.rs:62-65`

```rust
/// 标签集合（**类型级零大小元组**）：水元素 / 双手剑 / 挪德卡莱。
///
/// 标签就是类型本身（D54），所以这个元组零大小、无分配。
pub type Tags = (Hydro, Claymore, NodKrai);
```

**Tag constant `TAGS`**

`new/crates/cards/src/cards/character/aino/mod.rs:67-68`

```rust
/// 标签集合的常量值（三个零大小标签；运行期不存在）。
pub const TAGS: Tags = (Hydro, Claymore, NodKrai);
```

**Skill definition `SKILL_12161`**

`new/crates/cards/src/cards/character/aino/mod.rs:138-152`

```rust
/// 技能 12161 敲打修理法（普通攻击）。
///
/// - 文案：造成 2 点物理伤害。
/// - 消耗：1 水元素骰 + 2 无色骰。
pub static SKILL_12161: SkillDef = SkillDef {
    id: SKILL_ID_NORMAL_ATTACK,
    name: zh_only("敲打修理法"),
    description: zh_only("造成2点物理伤害。"),
    kind: SkillKind::normal_attack(),
    cost: SkillCost::from_dice(&[
        CostComponent::SpecificElement(Element::Hydro, Nat::new(1)),
        CostComponent::AnyColor(Nat::new(2)),
    ]),
    effect: normal_attack_effect,
};
```

**Character card definition `AINO`**

`new/crates/cards/src/cards/character/aino/mod.rs:206-217`

```rust
/// 爱诺的角色牌定义（第一张卡样板）。
///
/// 用 `static` 而不是 `const`：定义需要**稳定地址**（将来注册表 / 状态里存引用）。
pub static AINO: CharacterDef<Tags> = CharacterDef {
    id: ID,
    name: zh_only("爱诺"),
    tags: TAGS,
    max_hp: MAX_HP,
    max_energy: MAX_ENERGY,
    // 顺序固定：普攻 → 战技 → 爆发 → 被动（与游戏内文本描述的 skills 顺序一致）。
    skills: &[&SKILL_12161, &SKILL_12162, &SKILL_12163, &SKILL_12164],
};
```

**Card source table `SOURCE_FILES`**

`new/crates/cards/src/source/mod.rs:23-27`

```rust
/// 已接入的**卡牌**源码文件清单（手写登记，按 path 字典序升序）。
///
/// 新增卡：文件里写 `gi_genius_invokation_definitions::card_source!(path: [...]);` 后，
/// 在这里加一行 `&crate::cards::<牌族>::<卡>::SOURCE`。
pub static SOURCE_FILES: &[&SourceFile] = &[&crate::cards::character::aino::SOURCE];
```

**`source_files`**

`new/crates/cards/src/source/mod.rs:29-32`

```rust
/// 全部已登记的卡牌源码文件（只读；顺序 = [SOURCE_FILES]）。
pub fn source_files() -> &'static [&'static SourceFile] {
    SOURCE_FILES
}
```

**`source_of`**

`new/crates/cards/src/source/mod.rs:34-37`

```rust
/// 按 path 精确查找（path 口径 = 宏路径分段拼出的相对 crate 根路径）。
pub fn source_of(path: &str) -> Option<&'static SourceFile> {
    SOURCE_FILES.iter().copied().find(|file| file.path == path)
}
```

**`source_files_under`**

`new/crates/cards/src/source/mod.rs:39-45`

```rust
/// 按路径前缀筛出某目录下的文件（path 口径同上；只读迭代器）。
pub fn source_files_under(prefix: &str) -> impl Iterator<Item = &'static SourceFile> + '_ {
    SOURCE_FILES
        .iter()
        .copied()
        .filter(move |file| file.path.starts_with(prefix))
}
```


## 4. Invariants and Prohibitions

> [!IMPORTANT]
> One-way dependency: `cards -> definitions / engine`; `definitions` must not depend on `cards` (guard test `source::tests::dependency_direction_is_cards_to_definitions`).

- Card layout mirrors the old definitions layout: `src/cards/<family>/[<type>/]<card>`, one directory per card (D163); source `path` still starts with `src/cards/`.
- Do not move resources/shapes here: `CardId` / `CardFamily` / types / tags / `CostComponent` / `CharacterDef` / `SkillDef` stay in `definitions` (D241).
- Every card must declare `card_game_version!`; the value is the latest version the card has been synced to; bump only the number when nothing else changed (D245 / D246).
- After source self-declaration, also add a line to `SOURCE_FILES` (a missing line compiles but is invisible to `source_of`); keep the list sorted by path.
- Text is Chinese-only for now (D120); the four-language corpus is `definitions/generated/data/descriptions.json` (test data).
- Numeric rules: `Nat` / constants; the crate denies `clippy::arithmetic_side_effects`.

> [!WARNING]
> Taking concrete card data from the reference implementation is forbidden (D122); card data is copied verbatim from in-game text descriptions.

## 5. Basis (Rulebook and D Numbers)

| Topic | Basis |
|---|---|
| Character-card structure follows the reference project; text Chinese-only | D120 |
| Reference implementation may only be read for technique, never for data | D122 |
| First card (Aino) sample structure | D162 |
| Concrete-card path and one-directory-per-card | D163 |
| Concrete cards split into their own crate with unchanged layout | D240 |
| Resources and shapes stay in definitions; only instances move | D241 |
| Source-embedding macros and mechanism stay in definitions; only `SOURCE_FILES` moves | D242 |
| simulation excludes concrete cards by default | D243 |
| Version macro semantics (only "synced to", not "latest") | D244 / D245 / D246 |
| Card multi-versioning (not implemented) | D248 (pending item #24) |
| Card data basis | rulebook / card text (file header comments) |

## 6. Common Misuse

> [!WARNING]
> Do not put resource/shape types in cards, do not let definitions depend on cards, and do not forget `SOURCE_FILES` or `card_game_version!`.

- Changing card content while syncing to a new version: for unchanged cards only the number changes (D246).
- Reimplementing rules or copying definitions settlement logic into cards.
- Copying concrete card data from the reference implementation (D122).
- Starting "multi-version cards" now: D248 is not implemented (pending item #24).
- Calling Aino's four effect bodies: they are `todo!` and panic; the card is a data sample only.

## 7. Tests

- `cargo test --offline -p gi-genius-invokation-cards`: currently 11 tests.
- Representative cases:

| Test | Coverage |
|---|---|
| `cards/src/cards/character/aino/mod.rs::cards::character::aino::tests::aino_basic_data_matches_in_game_text` | Aino base data |
| `...::aino_tags_are_three_zero_sized_tags` | zero-sized tag tuple |
| `...::aino_skills_match_in_game_text` | four skills / costs / text |
| `...::game_version_matches_declared_sync_version` | version macro |
| `...::source_embeds_this_file_verbatim` | verbatim source embedding |
| `...::source_is_registered_in_source_registry` | source table registration |
| `cards/src/source/mod.rs::source::tests::registry_lists_card_files_in_path_order` | list ordering |
| `cards/src/source/mod.rs::source::tests::dependency_direction_is_cards_to_definitions` | dependency-direction guard |

## 8. Pending Work and Field Verification

| No. | Item |
|---|---|
| #24 | Card multi-versioning (non-overlapping intervals) plus one-version-at-load-time selection (D248) are not implemented; per-card version choice with latest-by-default (D249 / D251) is pending |
| #32 | "Doubling" style effects need `DamageArgs::with_modifiers` declared in the cards layer; not wired yet |
| #92 | The data source for `has_combat_action_tag` is in the cards layer (flow only receives a bool) |
| #81 / #109 | Food (including revival food) scheduling belongs to cards + flow; definitions provides pure operations and `flow::revive` only |
| no number | Registration mechanism missing: `cards/mod.rs` states "no central registry; wait for the registration mechanism (P3)"; Aino is not registered yet |
| no number | Aino's four effect bodies are placeholders: missing damage entry / status creation / summon area / passive trigger registration are listed in the file comments |

---

Last reviewed: 2026-10-19 (code: `new/crates/cards/src/cards/character/aino/mod.rs:AINO`)
