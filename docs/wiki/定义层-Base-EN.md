# Definitions Layer: Base

> [[定义层-Base|中文]] · English

*Scope: the stateless, behavior-free base data of the definitions layer — elements, card families, ids, and the player perspective — plus the shared convention that a state "identity" is a data address.*

---

## 1. Responsibility

Provide **behavior-free, state-free** base data: `Element`, `CardFamily` (= id region), `CardId` / `SkillId`, and `Perspective`; plus the shared convention that state identities **use pointers as identity** (implementation: `families/action_card/status/state.rs`, not the `base/` directory).

## 2. Key concepts and types

| Concept | File | Notes |
|---|---|---|
| `Element` | `crates/definitions/src/base/element.rs` | Seven elements in fixed order Pyro / Hydro / Electro / Dendro / Cryo / Anemo / Geo; no numeric or priority semantics; `ELEMENTS` is the only complete array |
| `CardFamily` | `crates/definitions/src/base/family.rs` | ActionCard / Character / Summon / Status / Enemy; **the family defines the id region**, so ids are not guaranteed unique across families |
| `CardId` / `SkillId` | `crates/definitions/src/base/id.rs` | `#[repr(transparent)]` newtypes over `Nat`; `CardId` is unique only within its family; `SkillId` is derived from the character id (×10 + index) |
| `Perspective` | `crates/definitions/src/base/perspective.rs` | "Our side / opponent" are two named relative-side instances (`Me` / `Opp`), not numbers; `other()` flips |
| `StateIdentity` / `identity_address` | `crates/definitions/src/families/action_card/status/state.rs` | Identity = **the address of the definition data**, not the address of `&dyn` |

### Why identity_address does not compare &dyn addresses

Static instances of zero-sized types (ZSTs) in Rust have no distinct address; in this repository (rustc 1.98.1) different ZST static instances can share an address. Comparing different zero-sized identities as `&dyn StateIdentity` by pointer would treat them as the same identity (D57 / D58).

Every identity therefore owns exactly one definition data item (`StateText` / `StatusDef` / reaction identity data). `identity_address` returns the `StateText` address that `text()` points to by default; comparison always goes through `same_state_identity`.

## 3. Public entry points

| Entry | File | Notes |
|---|---|---|
| `CardId::new(v: u32) -> CardId` | `base/id.rs` | `const`; ids always use newtypes, never bare integers |
| `SkillId::new(v: u32) -> SkillId` | `base/id.rs` | `const`; skill ids derive from character ids |
| `Perspective::other(self) -> Perspective` | `base/perspective.rs` | `const`; paired entry points bind the perspective |
| `StateIdentity::{text, form, identity_address, name}` | `families/action_card/status/state.rs` | Identity trait; `name` defaults to Chinese |
| `same_state_identity(left, right)` | same | Compares `identity_address`; the only correct comparison |

Example references (type + file + note): `ELEMENTS: [Element; 7]` in `base/element.rs`; the state trait in `families/action_card/status/state.rs`.

### Source

> [!NOTE]
> The excerpts below are copied from the current repository (signatures and doc comments; bodies omitted where not needed — see the file for the full implementation).

**`crates/definitions/src/base/element.rs`**

```rust

/// 元素（7 种）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Element {
    /// 火。
    Pyro,
    /// 水。
    Hydro,
    /// 雷。
    Electro,
    /// 草。
    Dendro,
    /// 冰。
    Cryo,
    /// 风。
    Anemo,
    /// 岩。
    Geo,
}
```

**`crates/definitions/src/base/family.rs`**

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

**`crates/definitions/src/base/id.rs`**

```rust
/// 卡牌 id：某个牌族 id 空间内的编号（跨牌族不保证唯一）。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CardId(pub Nat);

impl CardId {
    /// **const 构造**（用于写常量）。
    pub const fn new(v: u32) -> CardId {
        CardId(Nat::new(v))
    }
}
```

**`crates/definitions/src/base/perspective.rs`**

```rust
/// 卡面写法里不出现它（写的是 `SideScope::Own` / `SideScope::Opponent`，
/// 由本绑定在解析时换成具体的一侧）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Perspective {
    /// 我方（`DefState::me`）是视角方。
    Me,
    /// 敌方（`DefState::opp`）是视角方。
    Opp,
}
```

**`crates/definitions/src/families/action_card/status/state.rs`**

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

/// 状态身份的**身份地址**（= 指向其定义数据的指针；折叠键 / 比较都用它）。
pub fn state_address(state: &'static dyn StateIdentity) -> *const () {
    state.identity_address()
}

/// 两个状态身份是否是**同一份身份**（比较 `identity_address()`；D57 / D58）。
///
/// **不要**直接比 `&dyn StateIdentity`（`std::ptr::addr_eq`）：ZST 静态实例的地址可能相同。
pub fn same_state_identity(
    left: &'static dyn StateIdentity,
    right: &'static dyn StateIdentity,
) -> bool {
    std::ptr::eq(left.identity_address(), right.identity_address())
}
```
## 4. Invariants and prohibitions

> [!IMPORTANT]
> Id uniqueness is per family: always look up / store with `(CardFamily, CardId)`; game logic contains no "name → card" lookup.
> Compare identities only with `same_state_identity` (which compares `identity_address`); never introduce TypeId, string names, or numeric codes.

- Variant order in `Element` / `CardFamily` carries no rule semantics; never use `as usize` as a priority.
- Do not define tags in `base`: in `tags/`, a tag is one file / one zero-sized type (see [[定义层-Tags-EN]]).
- Ids are always `Nat` / newtypes; constants go through `CardId::new` and friends.

## 5. Basis (rulebook and D-numbers)

> [!NOTE]
> Evidence priority: rulebook > authoritative keywords > card text (D122). All numbers below are in `决策日志.md`.

| Number | Conclusion |
|---|---|
| D51 | Character status / combat status share one id region of the status family |
| D53 / D54 | Tags are zero-sized types in the independent top-level `tags/` directory, one file per tag |
| D29 / D34 | enemy/ally are named relative-side instances, not numbers |
| D57 / D58 | Identity = the address of the definition data |
| D67 | The runtime identity of a tag = `&TAGS.<field>` |

## 6. Common misuse

> [!WARNING]
> The first two patterns conflate different identities or different families; tests pin both.

1. Using `let key = card_id` as a globally unique key — different families may reuse the same number; include the family.
2. Comparing state identity with `std::ptr::eq(&dyn_a as *const dyn, &dyn_b as *const dyn)` — ZST addresses collide; use `same_state_identity`.
3. Iterating `Perspective` as a 0/1 index — it expresses a relative side; paired `*_me` / `*_opp` entry points bind it.

## 7. Tests

| File | Tests |
|---|---|
| `base/element.rs` | `element_enum_is_complete_and_ordered`, `elements_are_all_distinct` |
| `base/family.rs` | `card_family_has_all_five_id_regions`, `card_family_is_not_a_numeric_encoding` |
| `base/id.rs` | `id_is_transparent_over_nat`, `uniqueness_is_per_family_not_global`, `skill_id_is_a_nat_newtype` |
| `base/perspective.rs` | `other_flips_between_the_two_named_sides` |
| `families/action_card/status/state.rs` | different ZST identities may share an address and must be distinguished via `identity_address` |

## 8. Not yet wired and to be verified

- `留空.md#16`: reaction identities are ZSTs and `identity_address` defaults to the name-string data address; the plan is one static `ReactionDef` per reaction (affects the 14 reaction files).
- `留空.md#6`: the initial deck / game-setup flow is not implemented (`cards_in_initial_deck` only accepts an externally supplied `&[CardId]`).

---

Last verified: 2026-10-19 (code: crates/definitions/src/base/id.rs:CardId)