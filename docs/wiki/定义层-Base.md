# 定义层-Base

> 中文 · [[定义层-Base-EN|English]]

*本页范围：定义层最底层的无行为、无状态基础数据：元素、牌族、编号、视角方，以及状态「身份 = 数据地址」的口径。*

---

## 1. 职责

提供**不含行为与状态**的基础数据：`Element`、`CardFamily`（= id 区域）、`CardId` / `SkillId`、`Perspective`；并给出状态身份「用指针当身份」的统一口径（实现位于 `families/action_card/status/state.rs`，不在 `base/` 目录）。

## 2. 关键概念与类型

| 概念 | 文件 | 说明 |
|---|---|---|
| `Element` | `crates/definitions/src/base/element.rs` | 7 种元素，顺序固定为火/水/雷/草/冰/风/岩；没有编号或优先级语义，`ELEMENTS` 是唯一全量数组 |
| `CardFamily` | `crates/definitions/src/base/family.rs` | 行动牌 / 角色 / 召唤物 / 状态 / 敌人；**牌族决定 id 区域**，跨牌族 id 不保证唯一 |
| `CardId` / `SkillId` | `crates/definitions/src/base/id.rs` | `Nat` 的 `#[repr(transparent)]` 新类型；`CardId` 只在所属牌族内唯一，`SkillId` 由角色 id 派生（×10 + 序号） |
| `Perspective` | `crates/definitions/src/base/perspective.rs` | 我方 / 敌方是两个具名相对侧实例（`Me` / `Opp`），不是编号；`other()` 翻转 |
| `StateIdentity` / `identity_address` | `crates/definitions/src/families/action_card/status/state.rs` | 身份 = **指向定义数据的地址**，不是 `&dyn` 的地址 |

### 为什么 identity_address 不比 &dyn 地址

Rust 的零大小类型（ZST）静态实例没有独立地址；本仓库实测（rustc 1.98.1）不同 ZST 静态实例地址会重合。若把不同零大小身份都转成 `&dyn StateIdentity` 再比地址，会把它们判成同一个身份（D57 / D58）。

因此每个身份必须有唯一一份定义数据（`StateText` / `StatusDef` / 反应身份数据）；`identity_address` 默认返回 `text()` 指向的 `StateText` 地址；比较一律用 `same_state_identity`。

## 3. 公开入口

| 入口 | 文件 | 说明 |
|---|---|---|
| `CardId::new(v: u32) -> CardId` | `base/id.rs` | `const`；编号一律走新类型，不写裸整型 |
| `SkillId::new(v: u32) -> SkillId` | `base/id.rs` | `const`；技能 id 由角色 id 派生 |
| `Perspective::other(self) -> Perspective` | `base/perspective.rs` | `const`；成对入口据此绑定视角 |
| `StateIdentity::{text, form, identity_address, name}` | `families/action_card/status/state.rs` | 身份 trait；`name` 默认取中文 |
| `same_state_identity(left, right)` | 同上 | 按 `identity_address` 比较；这是唯一正确比较方式 |

示例（类型名 + 文件 + 说明）：`ELEMENTS: [Element; 7]` 在 `base/element.rs`；状态 trait 定义在 `families/action_card/status/state.rs`。

### 源码（逐字）

> [!NOTE]
> 以下为当前仓库源码摘录（签名与文档注释逐字；函数体除关键处外从略，完整实现见文件）。

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
## 4. 不变量与禁止

> [!IMPORTANT]
> id 的唯一性按牌族：查找 / 存储一律用 `(CardFamily, CardId)`；游戏逻辑里没有「名字 → 卡」的查找。
> 身份比较只用 `same_state_identity`（内部比较 `identity_address`），不得引入 TypeId、字符串名或数字编号。

- `Element` / `CardFamily` 的变体顺序不表示规则语义，不要拿 `as usize` 当优先级。
- 标签不定义在 `base`：标签在 `tags/` 是一标签一文件一零大小类型（见 [[定义层-Tags]]）。
- 编号一律 `Nat` / 新类型；常量用 `CardId::new` 等。

## 5. 依据（规则书与 D 编号）

> [!NOTE]
> 依据优先级：规则书 > 权威关键字 > 卡面文本（D122）；本节编号均可查 `决策日志.md`。

| 编号 | 结论 |
|---|---|
| D51 | 角色状态 / 出战状态共用状态牌族同一个 id 区域 |
| D53 / D54 | 标签是独立顶层 `tags/` 的零大小类型，一标签一文件 |
| D29 / D34 | 敌我是相对侧具名实例，不是编号 |
| D57 / D58 | 身份 = 指向定义数据的地址 |
| D67 | 标签运行期身份 = `&TAGS.<field>` |

## 6. 常见误用

> [!WARNING]
> 以下两种写法都会把不同身份 / 不同牌族判成同一个；测试对此有专门断言。

1. 用 `let key = card_id` 当全局唯一键：不同牌族可复用同一数字，必须带牌族。
2. 用 `std::ptr::eq(&dyn_a as *const dyn, &dyn_b as *const dyn)` 判状态身份：ZST 地址会重合，应改 `same_state_identity`。
3. 把 `Perspective` 当 0/1 下标循环：它表达相对方，成对入口 `*_me` / `*_opp` 负责绑定。

## 7. 测试

| 文件 | 测试 |
|---|---|
| `base/element.rs` | `element_enum_is_complete_and_ordered`、`elements_are_all_distinct` |
| `base/family.rs` | `card_family_has_all_five_id_regions`、`card_family_is_not_a_numeric_encoding` |
| `base/id.rs` | `id_is_transparent_over_nat`、`uniqueness_is_per_family_not_global`、`skill_id_is_a_nat_newtype` |
| `base/perspective.rs` | `other_flips_between_the_two_named_sides` |
| `families/action_card/status/state.rs` | 不同 ZST 身份地址可重合、必须靠 `identity_address` 区分 |

## 8. 待接入与待实测

- `留空.md#16`：反应身份目前是 ZST，`identity_address` 默认取名字字符串数据地址；计划改成每反应一份静态 `ReactionDef` 并返回其地址（涉及 14 个反应文件）。
- `留空.md#6`：初始牌组 / 建局流程尚未实现（`cards_in_initial_deck` 只收外部传入的 `&[CardId]`）。

---

最后核对：2026-10-19（代码：crates/definitions/src/base/id.rs:CardId）