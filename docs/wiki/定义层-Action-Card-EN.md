# Definitions: Action Cards

> [[定义层-Action-Card|中文]] · English

*Scope: `crates/definitions/src/families/action_card/` — the definition carrier, the five carriers (hand / pile / support zone / hand-card states / current cost), and the arcane edict container.*

---

## 1. Responsibility

The action card family: **definition carriers** (type / text / base cost / play condition) + **zone state** (hand, pile, support zone, hand-card states, current cost) + **queries** (by type / tag) + the **arcane edict container** (`families/status/arcane_edict.rs`). Action cards are one of the only two families with a type tree (the other is status).

## 2. Key concepts and types

### 2.1 Definitions (`def.rs` / `mod.rs`)

| Type | File | Notes |
|---|---|---|
| `ActionCardType` | `action_card/mod.rs` | `Support` / `Equipment` / `Event`; classification only, no numeric semantics |
| `ActionCardDef<Tags>` | `action_card/def.rs` | `id` / `card_type` / four-language name and description / `tags` / `cost: &'static [CostComponent]` (base cost) / `play_condition: PlayConditionFn` |
| `PlayCondition` / `PlayConditionFn` | `action_card/def.rs` | `Ok` / `Rejected(&'static str)`; function pointer `fn(&DefState, ViewSide) -> PlayCondition` (D217 / D186) |

> [!IMPORTANT]
> The play condition runs **before the affordability check** (D217): a rejection changes nothing; only after passing do payment, leaving hand, placement and dispatch happen.
> Placement is not this layer's job: `flow::CardPlacement` expresses it; this layer only exposes `card_type()`.

### 2.2 Support zone (`areas/support.rs`)

```text
SupportValue    { None, Flag(bool), Count(Nat) }  // one of three, mutually exclusive (plan §4.4.15)
SupportDuration { Round(Nat), WholeGame }          // Z14: finite rounds vs whole game, explicit
SupportEntry    { card, entity, value, duration }  // card = action card id (no separate support definition)
SUPPORT_CAP = 4
```

- **Cap 4**: not full → one branch with probability 1; **full → `enumerate_select` expands "which one to discard" (4 branches, 1/4 each)**; dispatch order `Discarded` (old) → `Entered` (new).
- End Phase `tick_end_phase_with_triggers`: `WholeGame` skipped; `Round(n)` decrements by 1, remaining 1 → removed and `Discarded` dispatched; pending effects follow entry order.
- Timings: `SupportTiming::{Entered, Discarded}`; read-only pointers: `entries()/len()/is_full()/free_slots()/value_of()/support_duration_of()/stats()`.

### 2.3 Hand (`zones/hand/`)

```text
HandState { entries: Vec<CardInstance> }   // unordered; CardInstance = { entity, card }
HandState::MAX = 10; HandState::OPENING_SIZE = 5
```

- **Cap 10**: adding / generating when full → rejected (failure, state unchanged); **draw overflow** is handled by the pile draw branches (drawn card discarded when hand is full).
- **Opening hand belongs to the hand** (D90): `draw_opening` first draws arcane-legend cards by tag, then fills up to 5 (D17 / D90).
- Hand is **unordered** and has no discard pile; discarding = removing from the hand.

### 2.4 Hand-card states and the six identities (`areas/hand_state.rs` + `damage_chain/statuses.rs`)

```text
HandCardStates { entries: Vec<HandCardStateEntry> }   // one StateList per hand card, pos = HAND_STATE_POS = 0
HandCardStateTiming { StateAdded, StateRemoved }
```

Six hand-card statuses (D116; K3003–K3008):

| K code | Name | Identity | Form | Definition |
|---|---|---|---|---|
| K3003 | Electric Shock | `ELECTRIC_SHOCK` | boolean | `families/character/damage_chain/statuses.rs` (legacy entry) |
| K3004 | Cost Increase | `HAND_COST_INCREASE` | layered (to be verified) | `families/action_card/areas/hand_state.rs` |
| K3005 | Cost Reduction | `COST_REDUCTION` | layered | `families/character/damage_chain/statuses.rs` (legacy entry) |
| K3006 | Empowerment | `HAND_EMPOWERMENT` | boolean | `areas/hand_state.rs` |
| K3007 | Untunable | `HAND_UNTUNABLE` | boolean | `areas/hand_state.rs` |
| K3008 | Invalidation | `HAND_INVALIDATION` | boolean | `areas/hand_state.rs` |

- Enum `reactions::effect::HandCardStatus` (6 variants with `ALL` / `k_code` / `from_k_code`); bridge `hand_status_identity(HandCardStatus) -> &'static dyn StateIdentity` and the reverse `hand_status_of_identity` live in `damage_chain/statuses.rs` (Z13).
- Add / remove reuse `StateList` (boolean idempotent, layers add) and dispatch `StateAdded` / `StateRemoved`; `remove_card(entity)` clears a card's states when it leaves the hand.
- The four new identities (K3004/K3006/K3007/K3008) have **null descriptions → left empty** (D118 / 留空.md #10); their forms are symmetric / inferred (to be verified).

### 2.5 Pile (`zones/pile/`)

```text
PileState {
  entries: Vec<PileEntry>,        // position = index (counted from the top); PileEntry { entity, card }
  segments: Vec<Segment>,         // Fixed { start, len } | Random { pool, start, len }
  marks: StateList,               // position marks (D105 shared state list; pos = position)
  pending_marks: Vec<PendingMark> // D188/D189: unmaterialized "randomly attach a status" sources (by position list)
}
```

- **Position-keyed** (D93 / D94): a position is an index into `entries`; segments are contiguous ranges; a `Random` segment's `pool` is a same-source group id (no pool table), and segments with the same pool share their entries.
- **Marks follow positions** (D52 / D82): `marks` uses the shared `StateList` (D105); insertion / removal maintains positions.
- **Random status attachment is not materialized** (D188–D191): `add_pending_mark` records position lists only; it expands when drawn (`pending_marks` participates in the fold key).
- Drawing / folding lives in `pile/parse.rs` (this block's own selection list + dedup function, D91 / D92): `draw` / `draw_top` / `draw_by_card` / `draw_from_random` / `draw_many` return **`Vec<(DrawBranch, Fraction)>`** (branches + exact probabilities, no sampling).
- Other operations: `discard / discard_at / discard_random` (discard also maintains marks, D86), `put_bottom / put_bottom_many` (putting on the bottom keeps marks, D89), `insert_evenly / insert_evenly_with`, `split_random_segment`, `add_mark / add_mark_with_opposite / clear_mark / pick_unmarked_targets`.

### 2.6 Current cost (`areas/cost.rs`)

```text
CurrentCost { base: Vec<CostComponent>, deltas: Vec<CostDelta>, current: Vec<CostComponent> }
CurrentCost::{new, base, deltas, current, total, with_delta, with_deltas, with_increase, with_decrease}
```

- Current cost = base cost + increases / decreases replayed in order (`deltas` ordered and replayable).
- The computation **reuses** `resources::dice::apply_delta` (the single cost algorithm): an increase goes to the same class (specific element / same color / any; a zero-cost increase creates a same-color entry, D170 / D172 / D178); a decrease drains in order down to 0 (D169 / D170).
- A pure data field: no timings, no payment (payment belongs to the dice pool / arcane edict).

### 2.7 Arcane edict (`families/status/arcane_edict.rs` + `rules/ops/arcane.rs`)

```text
ArcaneEdict { available: bool, played: Nat }     // one per player (D264)
ArcanePlay { Allowed, AlreadyPlayed, NoEdict }
ARCANE_EDICT                                     // state identity (boolean; K311 text; no StatusDef / no card id)
```

- Rulebook "each player may use only one Arcane Legend card" + K311 "playable only while the edict exists; playing consumes it"; `can_play` checks the played count first, then the edict; `record_play` increments and consumes when allowed.
- The container is **held by the caller** (留空.md #68 / #98): normal `play_card_*` signatures stay unchanged; arcane cards go through `play_card_*_arcane`.
- There is **no status card row** for the arcane edict in the data → shared identity only, no fabricated `StatusDef`.

## 3. Public entry points

> [!NOTE]
> Code below is copied verbatim from the current repository (including use, derive, doc comments and other boilerplate). Over-long function bodies are truncated to the signature + first body lines with an explicit note; everything else is the full item.

### 3.1 Definition carriers

**`ActionCardDef`** — `crates/definitions/src/families/action_card/def.rs`

```rust
/// 一张**行动牌**的定义（纯数据；D217）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
/// 一份定义可以写成 `static` 常量：`new` 与全部访问器都是 `const fn`。
#[derive(Clone, Copy, Debug)]
pub struct ActionCardDef<Tags> {
    /// 行动牌的卡 id（行动牌牌族 id 区域内唯一）。
    id: CardId,
    /// 类型（支援 / 装备 / 事件，D51）；决定落位落点。
    card_type: ActionCardType,
    /// 4 语名字（本块只填中文，D120）。
    name: Localized,
    /// 4 语描述（本块只填中文，D120）。
    description: Localized,
    /// 标签集合（D54：标签是零大小类型；集合形态本轮未定）。
    tags: Tags,
    /// **基础费用**（静态切片；费用修正链未接，见「待接入」）。
    cost: &'static [CostComponent],
    /// **打出条件**（D217）：尝试打出时、**比较是否付得起之前**调用。
    play_condition: PlayConditionFn,
}
```

**`new`** — `crates/definitions/src/families/action_card/def.rs`

```rust
    /// 建一份行动牌定义（常量 / 测试用；7 个参数，不再拆构造器）。
    pub const fn new(
        id: CardId,
        card_type: ActionCardType,
        name: Localized,
        description: Localized,
        tags: Tags,
        cost: &'static [CostComponent],
        play_condition: PlayConditionFn,
    ) -> ActionCardDef<Tags> {
        ActionCardDef {
            id,
            card_type,
            name,
            description,
            tags,
            cost,
            play_condition,
        }
    }
```

**`check_play`** — `crates/definitions/src/families/action_card/def.rs`

```rust
    /// **问一次打出条件**（D217）：把本定义的函数指针套用到「局面 + 视角方」上。
    ///
    /// 纯读取：只借 `&DefState`，不改任何区、不采样（D190）。
    pub fn check_play(&self, state: &DefState, viewer: ViewSide) -> PlayCondition {
        (self.play_condition)(state, viewer)
    }
```

**`PlayCondition`** — `crates/definitions/src/families/action_card/def.rs`

```rust
/// **一次打出条件的结论**（D217）。
///
/// `待复核`：拒绝原因目前是 `&'static str` 调试文案；正式对玩家展示的文案应由
/// 本地化文本（`generated::Localized`）给，等本地化文案块接入。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayCondition {
    /// 条件通过：可以继续「比较是否付得起」。
    Ok,
    /// 条件不通过：**不许打出**，带调试文案（中文短句，不是给玩家看的本地化文本）。
    Rejected(&'static str),
}
```

**`PlayConditionFn`** — `crates/definitions/src/families/action_card/def.rs`

```rust
/// **打出条件函数**（D217 / D186）：吃一局状态 + 视角方，返回结论。
///
/// 用**函数指针**（不是闭包）：签名里没有可变引用，也没有捕获环境 ——
/// 条件**改不了任何状态**、也不依赖调用顺序（纯函数由签名保证）。
pub type PlayConditionFn = fn(&DefState, ViewSide) -> PlayCondition;

/// 一张**行动牌**的定义（纯数据；D217）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
/// 一份定义可以写成 `static` 常量：`new` 与全部访问器都是 `const fn`。
#[derive(Clone, Copy, Debug)]
pub struct ActionCardDef<Tags> {
    /// 行动牌的卡 id（行动牌牌族 id 区域内唯一）。
    id: CardId,
    /// 类型（支援 / 装备 / 事件，D51）；决定落位落点。
    card_type: ActionCardType,
    /// 4 语名字（本块只填中文，D120）。
    name: Localized,
    /// 4 语描述（本块只填中文，D120）。
    description: Localized,
    /// 标签集合（D54：标签是零大小类型；集合形态本轮未定）。
    tags: Tags,
    /// **基础费用**（静态切片；费用修正链未接，见「待接入」）。
    cost: &'static [CostComponent],
    /// **打出条件**（D217）：尝试打出时、**比较是否付得起之前**调用。
    play_condition: PlayConditionFn,
}
```

### 3.2 Support zone

**`SupportValue`** — `crates/definitions/src/families/action_card/areas/support.rs`

```rust
/// 支援牌的「值」（分工计划 §4.4.15）：三选一、互斥。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupportValue {
    /// 都没有（合法）。
    None,
    /// 布尔值（如赋能）。
    Flag(bool),
    /// 计数（如层数 / 次数）。
    Count(Nat),
}
```

**`SupportDuration`** — `crates/definitions/src/families/action_card/areas/support.rs`

```rust
/// 支援牌的**持续回合表达**（Z14：显式区分「有限回合」与「整局」）。
///
/// 规则书·特殊词条「持续回合」：「每个结束阶段，都会使『持续回合』-1。『持续回合』耗尽后，
/// 会立刻弃置这张牌。」整局状态（K60 / 无持续回合的支援牌）不参与递减，所以用
/// SupportDuration::WholeGame **显式**表达，而不是靠 duration = None 隐含。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupportDuration {
    /// 有限回合：每个结束阶段 -1，耗尽弃置。
    Round(Nat),
    /// 整局：不按回合递减（结束阶段跳过）。
    WholeGame,
}
```

**`SupportEntry`** — `crates/definitions/src/families/action_card/areas/support.rs`

```rust
/// 一张支援牌：卡 id（= 行动牌 id，唯一）+ 实体号 + 值 + 持续回合。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SupportEntry {
    /// 支援牌的定义 id（**就是行动牌 id**，支援区不产生新卡牌定义）。
    pub card: CardId,
    /// 支援牌实体号。
    pub entity: EntityId,
    /// 支援牌的值（三选一）。
    pub value: SupportValue,
    /// **持续回合**（D13 / 规则书·特殊词条「持续回合」）：显式区分有限回合与整局。
    ///
    /// 待复核 / TODO：规则书 S8.6 只给出「每个结束阶段 -1、耗尽立刻弃置」的通用条款，
    /// 支援牌本身**没有专条**说明哪些支援牌带持续回合；本字段由调用方（打牌 / 效果）给，
    /// 数据层暂不反推，待实测哪些支援牌（如有）使用它。
    pub duration: SupportDuration,
}
```

**`support_duration_of`** — `crates/definitions/src/families/action_card/areas/support.rs`

```rust
    /// 某张支援牌的**显式持续回合表达**（Z14；不在区里 -> None）。
    pub fn support_duration_of(&self, entity: EntityId) -> Option<SupportDuration> {
        self.entry_of(entity).map(|entry| entry.duration)
    }
```

**`SupportZone`** — `crates/definitions/src/families/action_card/areas/support.rs`

```rust
/// **支援区**：最多 4 张，按排列顺序（= 进场先后）。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的区域」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct SupportZone {
    /// 支援牌条目（顺序 = 进场先后 / 排列顺序）。
    entries: Vec<SupportEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: SupportStats,
}
```

**`add_with_triggers`** — `crates/definitions/src/families/action_card/areas/support.rs`

```rust
    /// **加一张支援牌**（规则书·游戏区域 III）：未满 -> 直接进；满 4 -> **选择弃置一张**（多分支）。
    ///
    /// 返回值 = 每个分支（状态 + 弃置了谁 + 待执行）与**精确概率**：
    /// - 未满：1 个分支、概率 1；
    /// - 满 4：4 个分支、每支概率 1/4（`enumerate_select` 的既定 v1 语义：count = 1 时均匀）。
    pub fn add_with_triggers(
        &self,
        entry: SupportEntry,
        triggers: &SupportTriggers,
    ) -> Vec<(SupportAddResult, Fraction)> {
        let entered_event = Event::of(entry.entity);
        if !self.is_full() {
    // ...（其余实现从略；见 crates/definitions/src/families/action_card/areas/support.rs）
```

**`tick_end_phase_with_triggers`** — `crates/definitions/src/families/action_card/areas/support.rs`

```rust
    /// 把 `entity` 移出（纯函数内部用；统计 +1 弃置）。
    /// **结束阶段**：所有带持续回合的支援牌 -1；耗尽的**立刻弃置**（规则书·特殊词条「持续回合」）。
    ///
    /// 语义（与出战状态区的 tick_end_phase_with_triggers 同口径）：
    /// - 整局（SupportDuration::WholeGame）不递减、不派发；
    /// - 有限回合（SupportDuration::Round）按**排列顺序**逐条处理：剩余 > 1 -> 减 1 并保留；
    ///   剩余 = 1（或 0）-> 移出支援区 + 派发 Discarded（本层只派发、不执行，D186）；
    /// - 返回的 pending 按排列顺序排列（规则书 III「按支援区排列顺序依次结算」）。
    ///
    /// TODO / 待实测：规则书 S8.6 只有通用条款，没说支援牌持续回合在**哪个时机**递减
    /// （本实现按「结束阶段」，与卡牌 / 状态持续回合的通用条文一致；若实测是回合开始需调整）。
    pub fn tick_end_phase_with_triggers(
        &self,
        triggers: &SupportTriggers,
    ) -> SupportTickResolution {
        let mut zone = self.clone();
        let mut depleted: Vec<EntityId> = Vec::new();
    // ...（其余实现从略；见 crates/definitions/src/families/action_card/areas/support.rs）
```

### 3.3 Hand and hand-card states

**`HandState`** — `crates/definitions/src/families/action_card/zones/hand/mod.rs`

```rust
/// 手牌区：一个多实体容器（元素 = 实体号 + 卡 id 一对），**无序、上限 10**。
///
/// 「这是谁的手牌」不靠参数：状态层用两个字段 me.hand / opp.hand（两个具名实例）表达，
/// 操作函数由状态层写成对入口（hand_me / hand_opp），规则只写一份。
#[derive(Clone, Debug, Default)]
pub struct HandState {
    /// 手牌里的卡牌实例。字段公开是为了让状态层 / 查询块能直接按字段访问（D64）；
    /// 修改请走本类型的操作，别绕过上限规则。
    pub entries: Vec<CardInstance>,
    /// 手牌统计（D187；feature 关闭时零大小、空操作）。
    stats: HandStats,
}
```

**`add`** — `crates/definitions/src/families/action_card/zones/hand/mod.rs`

```rust
    /// 把一张**已存在**的卡牌实体加入手牌；返回 (新状态, 是否加入)。
    ///
    /// 边界 / 失败语义（都不改状态）：
    /// - 手牌已满（10 张）-> (self.clone(), false)；
    /// - 这张实体**已经在手牌里** -> (self.clone(), false)（同一实体不能重复占位）。
    ///
    /// 注意（D99）：实际对局**认为不会发生手牌溢出**；上面「满 -> 不加」只是安全兜底，
    /// 不是正常流程。
    ///
    /// 新牌放在容器末尾（手牌无序，位置不表示任何语义）。
    pub fn add(&self, instance: CardInstance) -> (HandState, bool) {
        if !self.has_space() || self.contains(instance.entity) {
            return (self.clone(), false);
        }
        let mut next = self.clone();
        next.entries.push(instance);
        next.stats.on_added(Nat::new(1));
        (next, true)
    }
```

**`discard`** — `crates/definitions/src/families/action_card/zones/hand/mod.rs`

```rust
    /// 从手牌**舍弃**一张牌（移除，没有弃牌堆）。
    ///
    /// 返回新状态；这张实体不在手牌里 -> None（状态不变）。
    /// 手牌无序，移除后不承诺剩余元素的顺序。
    pub fn discard(&self, entity: EntityId) -> Option<HandState> {
        let mut next = self.clone();
        let index = next
            .entries
            .iter()
            .position(|entry| entry.entity == entity)?;
        next.entries.swap_remove(index);
        next.stats.on_discarded(Nat::new(1));
        Some(next)
    }
```

**`draw_opening`** — `crates/definitions/src/families/action_card/zones/hand/mod.rs`

```rust
    /// **起手**（D17 / D90）：先按「秘传」标签向牌库抓，再补足到 OPENING_SIZE 张。
    ///
    /// 参数：
    /// - pile：起手要抓的牌库（牌库快照，不改入参）；
    /// - arcane：调用方注入的谓词（拿 CardId 判断是不是「秘传」；zones 不认识标签语义）。
    ///
    /// 行为：
    /// 1. 循环：只要手牌没到 5 张、且牌库顶段还有匹配秘传的牌 -> 按谓词抓最上面一张；
    ///    抓不到匹配牌就进入下一步；
    /// 2. 补足：循环抓牌直到手牌 5 张或牌库空了；
    /// 3. 每一步的随机分支都由牌库的抓牌原语（内部走引擎 enumerate_random）给出，
    ///    这里只做**跨步分支的组合**：概率 = 前面各步概率之积（引擎给的权重相乘，不自己造分布）。
    ///
    /// 返回 (手牌, 牌库, 概率) 的分支列表；一条分支都没有 -> 空 Vec（不 panic）。
    pub fn draw_opening(
        &self,
        pile: &PileState,
        arcane: &dyn Fn(CardId) -> bool,
    ) -> Vec<(HandState, PileState, Fraction)> {
        let mut branches: Vec<(HandState, PileState, Fraction)> =
            vec![(self.clone(), pile.clone(), Fraction::ONE)];
    // ...（其余实现从略；见 crates/definitions/src/families/action_card/zones/hand/mod.rs）
```

**`HandCardStates`** — `crates/definitions/src/families/action_card/areas/hand_state.rs`

```rust
/// **手牌卡状态表**：手牌实体 -> 状态列表。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的手牌」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct HandCardStates {
    /// 条目（顺序 = 第一次加状态的先后；手牌本身无序，这里只是为了稳定）。
    entries: Vec<HandCardStateEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: HandCardStateStats,
}
```

**`add_state_with_triggers`** — `crates/definitions/src/families/action_card/areas/hand_state.rs`

```rust
    /// 给一张手牌卡**加状态**（纯函数；不改入参）+ 派发「状态加上」。
    ///
    /// 状态形态与合并口径完全复用 `StateList::add`（D104/D105）：布尔类幂等、层数类相加。
    pub fn add_state_with_triggers(
        &self,
        entity: EntityId,
        state: &'static dyn StateIdentity,
        layers: Option<Nat>,
        triggers: &HandCardStateTriggers,
    ) -> HandCardStateResolution {
        let mut next = self.clone();
        if let Some(entry) = next
    // ...（其余实现从略；见 crates/definitions/src/families/action_card/areas/hand_state.rs）
```

**`remove_card`** — `crates/definitions/src/families/action_card/areas/hand_state.rs`

```rust
    /// 一张手牌卡**离开手牌**：把它身上的状态整体清掉（卡片进出**手牌区**是手牌区的事实，
    /// 本层不派发；跨区搬运由上层组合）。
    ///
    /// 失败（返回 None、不改状态）：这张牌本来就没带状态。
    pub fn remove_card(&self, entity: EntityId) -> Option<HandCardStates> {
        let mut next = self.clone();
        let position = next
            .entries
            .iter()
            .position(|entry| entry.entity == entity)?;
        next.entries.remove(position);
        Some(next)
    }
```

**`ExtraHandCardStatus`** — `crates/definitions/src/families/action_card/areas/hand_state.rs`

```rust
/// 本文件补的 4 个手牌状态（K3004 / K3006 / K3007 / K3008）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ExtraHandCardStatus {
    /// K3004 费用增加。
    CostIncrease,
    /// K3006 赋能。
    Empowerment,
    /// K3007 不可调和。
    Untunable,
    /// K3008 无效化。
    Invalidation,
}
```

**`HAND_COST_INCREASE`** — `crates/definitions/src/families/action_card/areas/hand_state.rs`

```rust
/// 「费用增加」静态身份实例。
pub static HAND_COST_INCREASE: HandCostIncrease = HandCostIncrease(&HAND_COST_INCREASE_TEXT);
/// 「赋能」静态身份实例。
pub static HAND_EMPOWERMENT: HandEmpowerment = HandEmpowerment(&HAND_EMPOWERMENT_TEXT);
/// 「不可调和」静态身份实例。
pub static HAND_UNTUNABLE: HandUntunable = HandUntunable(&HAND_UNTUNABLE_TEXT);
/// 「无效化」静态身份实例。
pub static HAND_INVALIDATION: HandInvalidation = HandInvalidation(&HAND_INVALIDATION_TEXT);

macro_rules! impl_hand_status_identity {
    ($identity:ident, $value:expr, $form:expr) => {
        impl StateIdentity for $identity {
            fn text(&self) -> &'static StateText {
                self.0
            }

            fn form(&self) -> StateForm {
                $form
            }
        }

        impl $identity {
            /// 把自身当状态身份指针用（D95 / D96）。
            pub fn identity(&'static self) -> &'static dyn StateIdentity {
                $value
            }
        }
    };
}
```

**`hand_status_identity`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 手牌状态枚举 -> 状态身份（月感电 / 月绽放用；K3003–K3008 共 6 种）。
pub fn hand_status_identity(status: HandCardStatus) -> &'static dyn StateIdentity {
    match status {
        HandCardStatus::ElectricShock => &ELECTRIC_SHOCK,
        HandCardStatus::CostIncrease => &HAND_COST_INCREASE,
        HandCardStatus::CostReduction => &COST_REDUCTION,
        HandCardStatus::Empowerment => &HAND_EMPOWERMENT,
        HandCardStatus::Untunable => &HAND_UNTUNABLE,
        HandCardStatus::Invalidation => &HAND_INVALIDATION,
    }
}
```

**`hand_status_of_identity`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 状态身份 -> 手牌状态枚举（反向查询；不是这 6 种之一 -> None）。
pub fn hand_status_of_identity(identity: &'static dyn StateIdentity) -> Option<HandCardStatus> {
    HandCardStatus::ALL
        .iter()
        .copied()
        .find(|status| same_identity(hand_status_identity(*status), identity))
}
```

### 3.4 Pile and queries

**`PileState`** — `crates/definitions/src/families/action_card/zones/pile/mod.rs`

```rust
/// 牌库区：**位置为键的 entries + 段视图 + 公共状态列表**（D93/D105）。
///
/// 容器没有身份（D42）：它就是状态里的一个字段。
///
/// 不变式（本类型维护）：
/// 1. 同一个实体在 entries 里最多出现一次；
/// 2. 段区间覆盖 entries（首尾相接；同 pool 的随机段共享同一区间，可能重复覆盖）；
/// 3. 同 pool 的随机段 start / len 必须一致；
/// 4. states 里的位置都指向当前 entries 的位置（entries 一变就 on_insert / on_remove，D105）；
/// 5. 空段可以存在（操作不删段，避免段下标漂移；抽取 / 置底会跳过空段）。
#[derive(Clone, Default, Debug)]
pub struct PileState {
    /// 位置列表：下标 = 从牌库顶往下数的位置（D93）。
    pub entries: Vec<PileEntry>,
    /// 段序列（前 = 牌库顶；每段是 entries 的连续区间）。
    pub segments: Vec<Segment>,
    /// **公共状态列表**（D105：一个列表装所有状态；按 pos 过滤就是某位置的状态）。
    pub states: StateList,
    /// 牌库统计（D187；feature 关闭时零大小、空操作）。
    stats: PileStats,
    /// 折叠的随机塞状态来源（D188：只记位置，不物化）。
    pending: Vec<PendingMark>,
}
```

**`put_bottom`** — `crates/definitions/src/families/action_card/zones/pile/mod.rs`

```rust
    /// **置底**：把这张牌移到牌库最下面。
    ///
    /// 语义（D89/D93）：
    /// 1. 实体不在牌库 -> None（状态不变）；
    /// 2. 先按「移除」处理：它自己的状态随位置一起消失；
    /// 3. 追加到 entries 末尾；尾部追加不影响其它位置。最后一段的区间长度 +1
    ///    （同 pool 的随机段一起 +1）。
    pub fn put_bottom(&self, entity: EntityId) -> Option<PileState> {
        let position = self.position_of(entity)?;
        let mut next = self.clone();
        let entry = next.remove_at(position)?;
        next.append_entry(entry);
        next.stats.on_put_bottom();
        Some(next)
    }
```

**`put_bottom_many`** — `crates/definitions/src/families/action_card/zones/pile/mod.rs`

```rust
    /// **通用「把若干新牌放牌组底」**（D209）：放几张、按什么顺序放都由调用方指定。
    ///
    /// - cards：要放到底部的新牌（实体号 + 卡 id；已在牌库里的 / 重复的会被忽略）；
    /// - order：第 step 次（0 起）放 cards 里的哪一张（返回 Some(cards 的下标)）；None 表示结束；
    ///   越界 / 重复下标会被忽略；
    /// - 放置语义（例：330012「将我方所有手牌置于牌组底」）：逐张放到**底部区域顶端**——
    ///   即每次都插在「本次已经放好的那叠」的最上面，所以**后放的在底区更上面**；
    /// - 卡 id / 实体都来自调用方，本函数不建映射表。
    ///
    /// 失败语义：没有一张真正放进去 -> None（状态不变）。
    pub fn put_bottom_many(
        &self,
        cards: &[(EntityId, CardId)],
        order: impl Fn(Nat) -> Option<Nat>,
    ) -> Option<PileState> {
        if cards.is_empty() {
            return None;
    // ...（其余实现从略；见 crates/definitions/src/families/action_card/zones/pile/mod.rs）
```

**`draw_top`** — `crates/definitions/src/families/action_card/zones/pile/mod.rs`

```rust
    /// 按类型 / 标签抽最上面一张；matches 是调用方注入的谓词（拿 CardId 查类型 / 标签，D20/D26）。
    ///
    /// 命中规则（D88）：固定段 = 只看段顶那张；随机段 = 池内匹配者等概率。
    pub fn draw_top(
        &self,
        hand: &HandState,
        matches: &dyn Fn(CardId) -> bool,
    ) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_one(self, hand, Some(matches))
    }
```

**`draw_many`** — `crates/definitions/src/families/action_card/zones/pile/mod.rs`

```rust
    /// 抓 **n 张**（不够则少给）；返回折叠后的分支列表（D83/D84/D85）。
    ///
    /// - 一张都抓不到（空库 / n = 0）-> 空 Vec；
    /// - 折叠口径见 parse.rs 文件头；
    /// - 手牌溢出时退化成有序分支（概率仍精确）。**实际对局认为不会发生溢出（D99）**，
    ///   这一句只是安全兜底：保证分支后继状态与概率一致，不是待补功能。
    pub fn draw_many(&self, hand: &HandState, n: Nat) -> Vec<(DrawBranch, Fraction)> {
        parse::draw_many(self, hand, n)
    }
```

**`add_pending_mark`** — `crates/definitions/src/families/action_card/zones/pile/mod.rs`

```rust
    /// 记录一批「待赋状态的位置」（D188）：**不物化到牌上**，只存位置列表。
    ///
    /// - mark：要赋予的状态（状态身份 + 层数）；
    /// - positions：从牌库顶往下数的序号（重复 / 越界的会被忽略并去重排序）；
    /// - 位置会随牌库插删一起维护（insert_entry/remove_at 里同步），所以均匀置入后仍指向原来那批牌；
    /// - 抽牌时：抽到的位置若在列表里 -> 这张牌带状态、并从列表移除（展开成确定分支，D190）。
    ///
    /// 失败语义：没有任何合法位置 -> None（状态不变）。
    pub fn add_pending_mark(&self, mark: Mark, positions: &[Nat]) -> Option<PileState> {
        let mut valid: Vec<Nat> = positions
            .iter()
            .copied()
            .filter(|position| *position < self.len())
            .collect();
        valid.sort();
        valid.dedup();
        if valid.is_empty() {
            return None;
        }
        let mut next = self.clone();
        next.pending.push(PendingMark {
            mark,
            positions: valid,
        });
        Some(next)
    }
```

**`CardMeta`** — `crates/definitions/src/families/action_card/zones/query.rs`

```rust
/// 查询需要的**卡牌元数据接口**（调用方实现，框架不建注册表；D20 / D101）。
///
/// 查询块只认类型与标签，不问「这张卡叫什么、有什么效果」；具体卡牌的类型 / 标签集合
/// 将来由卡牌注册表或建局流程提供，这里只接一个注入点。
///
/// **标签集合的形态**：`&'static [&'static TagText]` —— 每个元素都是某个标签的
/// **静态元数据指针**（`crate::tags::…::data()` 的返回值，D57/D67）；比较一律
/// 按指针身份（`std::ptr::addr_eq`），不看名字、不比字符串。
///
/// **元数据缺失的卡**：`ty` 返回 `None`、`tags` 返回空切片即表示「未知」——
/// 指定类型 / 标签的查询不会命中它；两个过滤都没给时按**可发现**处理（它没有禁止标签）。
pub trait CardMeta {
    /// 这张行动牌的类型（未知 -> `None`）。
    fn ty(&self, card: CardId) -> Option<ActionCardType>;

    /// 这张行动牌的标签集合（按静态元数据指针给出；未知 -> 空切片）。
    fn tags(&self, card: CardId) -> &'static [&'static TagText];
}
```

**`cards_in_initial_deck`** — `crates/definitions/src/families/action_card/zones/query.rs`

```rust
/// **初始牌组**里：类型 = `ty`（`None` = 不限）、标签含 `tag`（`None` = 不限）的卡。
///
/// - 返回 **`Vec<CardId>`**：同一张卡在初始卡组里有几张就出现几次（重复 = 实际多张）；
/// - **无序**：不承诺顺序，调用方不要依赖（当前实现按传入列表顺序给出）；
/// - 空结果 = 传入列表为空，或没有任何卡命中过滤（返回空 `Vec`，不 panic）；
/// - 两个过滤都没给时应用排除规则（不可发现 / 弃用 / 自走牌）。
///
/// 初始卡组列表由调用方传入（建局流程还没实现；D101）。它可能 **30 张里带重复**，
/// 所以这里逐项判断，不去重。
pub fn cards_in_initial_deck(
    deck: &[CardId],
    meta: &dyn CardMeta,
    ty: Option<ActionCardType>,
    tag: Option<&'static TagText>,
) -> Vec<CardId> {
    deck.iter()
        .copied()
        .filter(|card| card_matches(meta, *card, ty, tag))
        .collect()
}
```

### 3.5 Current cost

**`CurrentCost`** — `crates/definitions/src/families/action_card/areas/cost.rs`

```rust
/// 一次行动的**当前元素骰费用**：基础费用 + 已叠加的加费 / 减费。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CurrentCost {
    /// 卡牌定义给的基础费用（不改）。
    base: Vec<CostComponent>,
    /// 已经叠加过的增减费（有序；同序重放能还原当前值）。
    deltas: Vec<CostDelta>,
    /// 当前费用（= 基础费用按 `deltas` 顺序逐个 `apply_delta` 的结果）。
    current: Vec<CostComponent>,
}
```

**`with_delta`** — `crates/definitions/src/families/action_card/areas/cost.rs`

```rust
    /// 叠加一次加费 / 减费，返回新的当前费用（不改入参）。
    pub fn with_delta(&self, delta: CostDelta) -> CurrentCost {
        let current = apply_delta(&self.current, delta);
        let mut deltas = self.deltas.clone();
        deltas.push(delta);
        CurrentCost {
            base: self.base.clone(),
            deltas,
            current,
        }
    }
```

**`with_increase`** — `crates/definitions/src/families/action_card/areas/cost.rs`

```rust
    /// 加费 `amount`（D178：0 费时新建 ② 同色）。
    pub fn with_increase(&self, amount: Nat) -> CurrentCost {
        self.with_delta(CostDelta::Increase { amount })
    }
```

**`with_decrease`** — `crates/definitions/src/families/action_card/areas/cost.rs`

```rust
    /// 减费 `amount`（D170：① -> ② -> ③ 依次减，剩余作废）。
    pub fn with_decrease(&self, amount: Nat) -> CurrentCost {
        self.with_delta(CostDelta::Decrease { amount })
    }
```

### 3.6 Arcane edict

**`ArcaneEdictStatus`** — `crates/definitions/src/families/status/arcane_edict.rs`

```rust
/// 「秘传揭令」状态的身份类型（没有独立卡牌数据 -> 不建 StatusDef；身份 = 静态文本地址）。
#[derive(Debug)]
pub struct ArcaneEdictStatus;

/// 「秘传揭令」的状态身份（布尔形态：存在即代表有）。
pub static ARCANE_EDICT: ArcaneEdictStatus = ArcaneEdictStatus;

impl StateIdentity for ArcaneEdictStatus {
    fn text(&self) -> &'static StateText {
        &ARCANE_EDICT_TEXT
    }

    fn form(&self) -> StateForm {
        StateForm::Boolean
    }
}
```

**`ARCANE_EDICT`** — `crates/definitions/src/families/status/arcane_edict.rs`

```rust
/// 「秘传揭令」的状态身份（布尔形态：存在即代表有）。
pub static ARCANE_EDICT: ArcaneEdictStatus = ArcaneEdictStatus;

impl StateIdentity for ArcaneEdictStatus {
    fn text(&self) -> &'static StateText {
        &ARCANE_EDICT_TEXT
    }

    fn form(&self) -> StateForm {
        StateForm::Boolean
    }
}
```

**`ArcaneEdict`** — `crates/definitions/src/families/status/arcane_edict.rs`

```rust
/// 每位牌手一份的**秘传揭令容器**（使用机会 + 已打出张数）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArcaneEdict {
    /// 秘传揭令是否仍存在（存在 = 还能打出秘传）。
    available: bool,
    /// 本局该牌手已打出的秘传张数。
    played: Nat,
}
```

**`can_play`** — `crates/definitions/src/families/status/arcane_edict.rs`

```rust
    /// 现在能不能打出秘传（复用规则层的判定顺序：先看已打出张数，再看揭令）。
    pub fn can_play(&self) -> ArcanePlay {
        can_play(self.played, self.available)
    }
```

**`record_play`** — `crates/definitions/src/families/status/arcane_edict.rs`

```rust
    /// 打出秘传：允许时记录「已打出 +1」并按 K311 消耗揭令；不允许 -> None（状态不变）。
    pub fn record_play(&self) -> Option<ArcaneEdict> {
        match self.can_play() {
            ArcanePlay::Allowed => Some(ArcaneEdict {
                available: if consumes_edict() {
                    false
                } else {
                    self.available
                },
                played: nadd!(self.played, Nat::new(1)),
            }),
            ArcanePlay::AlreadyPlayed | ArcanePlay::NoEdict => None,
        }
    }
```

### 3.7 Keyword and rules-ops interfaces referenced by this page

> [!NOTE]
> Real source of the keyword-layer / rules-ops functions referenced by this page (signatures and doc comments copied verbatim).

**`apply_delta`** — `crates/definitions/src/resources/dice/mod.rs`

```rust
/// 对费用做加减（D170/D172/D178）：顺序**写死**为 ① SpecificElement → ② SameColor → ③ AnyColor。
///
/// - 加费 +k（D172：**整体加到一个类，不拆类、不带元素**）：
///   有 ① -> 全加 ①；否则有 ② -> 全加 ②；否则有 ③ -> 全加 ③；
///   三类都不存在（0 费费用）-> **新建 ② `SameColor(k)`**（D178：0 费加费按相同元素算）。
/// - 减费 -k（D170）：先减 ①（减到 0 为止），还有余量再减 ②，再减 ③；
///   减到总费用 0 后剩余作废（最低 0）。
/// - 同类有多个项时：加费只加**第一个**该类项；减费按出现顺序把该类减到 0。
pub fn apply_delta(cost: &[CostComponent], delta: CostDelta) -> Vec<CostComponent> {
    validate_cost(cost);
    match delta {
        CostDelta::Increase { amount } => increase_cost(cost, amount),
        CostDelta::Decrease { amount } => decrease_cost(cost, amount),
    }
}
```

**`CostDelta`** — `crates/definitions/src/resources/dice/mod.rs`

```rust
/// 费用增减（D170/D172）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CostDelta {
    /// 加费 k。
    Increase {
        /// 加费数量。
        amount: Nat,
    },
    /// 减费 k。
    Decrease {
        /// 减费数量。
        amount: Nat,
    },
}
```

**`SUPPORT_CAP`** — `crates/definitions/src/rules/ops/support.rs`

```rust
/// 一方支援区上限（分工计划 §4.4.15 已定）。
pub const SUPPORT_CAP: Nat = Nat::new(4);
```

**`support_enter`** — `crates/definitions/src/rules/ops/support.rs`

```rust
/// 进场判定（纯函数）。
pub fn support_enter(occupied: Nat) -> SupportEnter {
    if occupied < SUPPORT_CAP {
        SupportEnter::Accept
    } else {
        SupportEnter::MustChooseDiscard
    }
}
```

**`can_play`** — `crates/definitions/src/rules/ops/arcane.rs`

```rust
/// 打出判定：played_this_game = 本局该牌手已打出的秘传张数，
/// edict_available = 秘传揭令是否仍存在。
pub fn can_play(played_this_game: Nat, edict_available: bool) -> ArcanePlay {
    if played_this_game >= ARCANE_LEGEND_PER_GAME {
        ArcanePlay::AlreadyPlayed
    } else if !edict_available {
        ArcanePlay::NoEdict
    } else {
        ArcanePlay::Allowed
    }
}
```

**`consumes_edict`** — `crates/definitions/src/rules/ops/arcane.rs`

```rust
/// 打出后是否消耗秘传揭令（关键字 #70：会消耗）。
pub fn consumes_edict() -> bool {
    true
}
```

**`duration_after_end_phase`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 经过一个结束阶段后的持续回合（减到 0 为止）。
pub fn duration_after_end_phase(duration: Nat) -> Nat {
    nsub!(duration, DURATION_PER_END_PHASE)
}
```

## 4. Invariants and prohibitions

- **A support card is the action card itself**: moving to the support zone is a zone change, not a new card definition (plan §4.4.3).
- **Support cap 4**: when full, the `enumerate_select` branch must be used (never silently overwrite / discard the first entry).
- **Hand is unordered**: never assume `entries` order means anything; use the pile when order matters.
- **Pile position = index**; every insert / remove must maintain `marks` and `pending_marks` (D52 / D82 / D86 / D89).
- **Drawing never samples**: return branches + exact probabilities (`Fraction`); folding happens at draw time (D52 / D83 / D97 / D98).
- **Random status attachment is not materialized** (D188 / D190 / D191): record position lists, expand on draw, fold back afterwards.
- **Queries exclude three categories by default**: undiscoverable / scrapped / automaton cards (D101); an explicit type or tag filter disables the extra exclusion.
- **One cost algorithm only**: `CurrentCost` reuses `resources::dice::apply_delta`.
- **One Arcane Legend per player per game**: one `ArcaneEdict` per player; call `can_play` before playing (D264).

> [!IMPORTANT]
> A zone is a field (D42): hand / pile / support containers are owned by `PlayerDefs`; no zone numbers, no "entity → zone" table.
> Every operation is pure: takes `&`, returns a new value; failure = state unchanged + `None` / `false` / empty branches.

> [!CAUTION]
> Mutating `entries` directly bypasses caps, position marks and stats; always use the entry points listed in §3.

## 5. Basis (rulebook and D-numbers)

| Topic | Basis |
|---|---|
| Support cap 4 / choose one to discard when full / settle in entry order | Rulebook "Game Zones III"; 留空.md #67 / #93 |
| Hand cap 10 / only hand action cards may be played or tuned | Rulebook "Game Zones VI" |
| Opening hand 5, Arcane Legend guaranteed | Rulebook "Game Preparation" + D17 / D90 |
| Pile position marks / draw folding / discard and put-bottom semantics | D52 / D80 / D82 / D83 / D84 / D86 / D89 / D93 / D94 / D97 / D98 |
| Deferred expansion (do not materialize unobserved randomness) | D188 / D189 / D190 / D191 |
| Queries: `CardMeta` injection, three excluded categories, per-segment grouping | D20 / D101 |
| Current cost = base + increases/decreases; increase/decrease order | D11 / D168 / D169 / D170 / D172 / D178 |
| Play condition before payment | D217 |
| One arcane edict per player | Rulebook "Special Entry: Arcane Legend" + K311; D264; 留空.md #68 / #98 / #99 |
| Six hand-card state identities | D116; K3003–K3008; 留空.md #10 / #106 / #115 |

### 5.1 Documentation vs code mismatches (for review)

> [!NOTE]
> `结构.md` draws support / equipment / event under `families/action_card/types/`; that directory does not exist in code (type-level behaviour is in `rules/` + `flow/`), and carriers are split into `zones/` and `areas/` as on this page.

1. `结构.md`'s `types/support|equipment|event/` and the `tags/` sub-layer do not exist; tags live in top-level `src/tags/` (D53).
2. The arcane edict has no standalone `StatusDef` (no status card row in the data), only the `ARCANE_EDICT` shared identity + `ArcaneEdict` container.

## 6. Common misuse

> [!WARNING]
> The patterns below break cost / probability / position semantics, and most of them do not fail at the type level.

1. **Treating a support card as a new card**: the support entry's `card` is the action card id; do not create a "support definition".
2. **Discarding the first entry when full**: always expand the selection branches (`enumerate_select`), otherwise probabilities and queries are wrong.
3. **Encoding rules in hand order**: the hand is unordered; "first drawn, first played" is not a rule.
4. **Attaching marks to cards**: pile marks follow positions (D52 / D82); never store them on `PileEntry`.
5. **Re-implementing the cost algorithm in definitions**: use `apply_delta` only; payment uses `DicePool::pay`.
6. **Treating `can_afford` as paid**: it is a "possibly enough" upper bound; `pay` may still fail when folded groups are not expanded.
7. **Playing the arcane card before checking**: the order is hand check → arcane check → play condition (留空.md #99); a failure consumes nothing.
8. **Treating `with_duration(None)` as "no duration"**: the old API's None meant whole game; new code uses `SupportDuration::WholeGame` / `Round(n)` (Z14).

## 7. Tests

| File | Representative tests |
|---|---|
| `action_card/def.rs` | `check_play_binds_the_viewer_side`, `check_play_is_a_pure_read_only_function`, `cost_is_a_borrowed_static_slice_not_a_copy` |
| `areas/support.rs` | `full_zone_gives_one_branch_per_discard_candidate`, `full_zone_branches_dispatch_discard_then_enter`, `end_phase_duration_ticks_and_depleted_support_is_discarded`, `whole_game_duration_skips_tick_and_round_duration_still_ticks` |
| `areas/hand_state.rs` | `add_state_creates_entry_and_merges_by_state_list_rules`, `extra_hand_status_identities_have_names_forms_and_queries`, `remove_state_and_card` |
| `areas/cost.rs` | `increase_on_zero_cost_creates_same_color`, `decrease_drains_specific_then_same_color_then_any`, `deltas_are_replayable_and_ordered` |
| `zones/hand/mod.rs` | `add_fills_up_to_ten_then_rejects`, `opening_hand_draws_five_then_runs_out`, `opening_hand_prefers_arcane_cards` |
| `zones/pile/mod.rs` | `draw_many_unmarked_folds_to_combinations`, `draw_many_marks_fold_by_combination_d98`, `pending_marks_expand_when_drawn`, `states_follow_insert_and_discard` |
| `zones/query.rs` | `query_all_excludes_non_discoverable_three_tags`, `multi_segment_query_maps_segment_by_segment`, `filter_by_tag_matches_any_of_card_tags_by_pointer` |
| `families/status/arcane_edict.rs` | `fresh_container_allows_arcane_and_records_play`, `consumed_edict_blocks_arcane_with_no_edict` |

## 8. Pending and to be verified

- **Increase / decrease priority** (payment side): 留空.md #1 (code follows D168–D178, still to be measured).
- **Built-in tuning algorithm when no source die is specified**: D117 / 留空.md #9.
- **Missing descriptions for four hand-card states**: 留空.md #10 / #106.
- **Arcane edict**: container ownership and the order of check vs play condition: 留空.md #68 / #98 / #99.
- **Support duration**: tick timing (End Phase) and the production write path: 留空.md #67 / #93 / #105 / #116 / #117.
- **Current-cost table / `drawn_marks` materialization after the End-Phase draw**: 留空.md #45 / #46 / #90 / #122.
- **Support effect execution order deviating from S2.4**: 留空.md #94.
- **Elemental transmutation** and other action-card hooks: 留空.md #108 (dice roll wired, opening flow not).
- **Revive food**: 留空.md #109 / #119 (combined with satiety in [[定义层-Character-EN]] and `flow/revive.rs`).

---
Last verified: 2026-10-19 (code: `crates/definitions/src/families/action_card/areas/support.rs::SupportZone::add_with_triggers`)
