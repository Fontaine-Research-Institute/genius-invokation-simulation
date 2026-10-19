# Definitions Layer: Timeline

> [[定义层-Timeline|中文]] · English

*Scope: the generic trigger / event scheduling mechanism — timing keys, trigger registries, `dispatch` / `resolve` ordering, and inserted-settlement frames / interruption / resumption.*

---

## 1. Responsibility

The generic **trigger / event scheduling mechanism**: it knows no concrete timing. It provides a registry of "timing key + condition function pointer + effect function pointer", ordered dispatch following the settlement order, and the frames / interruption / resumption of inserted settlements. This layer **only dispatches, never executes** (D185 / D186 / D194 / D196).

## 2. Key concepts and types

| Concept | File | Notes |
|---|---|---|
| `Event` | `crates/definitions/src/timeline/event.rs` | Pure-data event (who / target / amount / element / source); no semantics interpreted |
| `Trigger<K, E>` | `timeline/trigger.rs` | Timing key + `owner` + `EffectSource` + `entry` + `fn(&Event) -> bool` + effect function pointer |
| `TriggerRegistry<K, E>` | same | An ordinary value (not a global singleton): `register` / `register_all` / `entries` / `clear` |
| `dispatch` / `resolve` | `timeline/dispatch.rs` | Single-side / two-side dispatch; source kind -> entry -> stable registration order; returns `Pending`, never executes |
| `Pending` / `Queued` | `dispatch.rs` / `suspend.rs` | Pending work = effect function pointer + event + owner / source_kind / entry; `Queued` is owned |
| `Frame` / `Resolution` | `timeline/suspend.rs` | Settlement frame + cursor; `Resolution` supports inserted sub-settlements, step consumption, discard, read-only snapshot; not a LIFO stack |

Timing keys are defined by the blocks that need them (e.g. `CharacterZoneTiming` / `SummonTiming` / `SupportTiming` / phase-broadcast enums); timeline provides no concrete timing enums.

## 3. Public entry points

| Entry | File | Notes |
|---|---|---|
| `Trigger::{new, with_condition, applies}` | `timeline/trigger.rs` | `new` always matches; `with_condition` adds a predicate |
| `TriggerRegistry::{new, entries, register, register_all, clear}` | same | The registry is an ordinary field in state |
| `dispatch(registry, timing, event)` | `timeline/dispatch.rs` | Ordered single-side dispatch |
| `resolve(current, other, timing, event)` | same | Current player's registry first; each side internally ordered |
| `Resolution::{new, empty, insert, next_step, discard, snapshot}` | `timeline/suspend.rs` | Inserted settlement / interruption / resumption; `Frame` keeps the cursor |
| `Pending::into_queued` | `timeline/dispatch.rs` | Turns a borrowed event into an owned `Queued` for frames |

### Source

> [!NOTE]
> Real timeline source excerpts (types + public method signatures + doc comments; bodies omitted).

**`crates/definitions/src/timeline/trigger.rs (Condition / Effect and Trigger / TriggerRegistry)`**

```rust
/// 触发条件：`fn(&Event) -> bool` 的函数指针（D186：函数指针，不是闭包 / trait object）。
///
/// 只看事件（谁 / 目标 / 数值 / 元素 / 来源）；返回 `true` = 这条触发命中。
pub type Condition = fn(&Event) -> bool;

/// 恒成立的条件：登记时不额外过滤。
pub const ALWAYS: Condition = always;

/// 默认效果函数指针：只吃事件（无状态家族 / 纯通知用）。
///
/// 需要局面上下文的家族把注册表的 `E` 换成带自家状态参数的函数指针类型
/// （例如 `fn(&mut CharacterState, &Event)`）；**本层只登记和排序，不调用**（D186）。
pub type Effect = fn(&Event);

fn always(_event: &Event) -> bool {
    true
}

/// 一条**触发登记项**（纯数据；`K` / `E` 都是 `Copy` 时整体 `Copy`）。

/// 一条**触发登记项**（纯数据；`K` / `E` 都是 `Copy` 时整体 `Copy`）。
///
/// 字段与依据：
/// - `timing`：时机键（家族定义）；
/// - `owner`：这条触发挂在哪个实体上（「谁」）；
/// - `source_kind`：结算顺序第 2 条的来源类别；
/// - `entry`：进场先后（结算顺序第 3 条）；
/// - `condition`：条件函数指针（D186）；
/// - `effect`：执行函数指针（D186；派发只把它放进有序列表，不执行）。
#[derive(Clone, Copy, Debug)]
pub struct Trigger<K, E = Effect> {
    /// 时机键（由牌族 / 类型定义）。
    pub timing: K,
    /// 这条触发挂在谁身上（实体号；派发结果原样带回）。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条；由区域在登记时给出）。
    pub entry: Nat,
    /// 条件函数指针（D186）。
    pub condition: Condition,
    /// 执行函数指针（D186）。
    pub effect: E,
}

    /// 登记一条触发：条件恒真（[`ALWAYS`]）。
    pub const fn new(
        timing: K,
        owner: EntityId,
        source_kind: EffectSource,
        entry: Nat,
        effect: E,
    ) -> Trigger<K, E>

    /// 登记一条触发：带条件函数指针。
    pub const fn with_condition(
        timing: K,
        owner: EntityId,
        source_kind: EffectSource,
        entry: Nat,
        condition: Condition,
        effect: E,
    ) -> Trigger<K, E>

/// 触发注册表（**容器没有身份**，D42：它只是某个状态结构里的一个字段）。
///
/// - 泛型 `K` = 时机键类型（由家族给；D185）；
/// - 泛型 `E` = 效果函数指针类型（默认 [`Effect`]；D186）。
///
/// 只收集登记项，不做匹配 / 排序（那在 `timeline::dispatch`）；重复登记**不去重**
/// （每条登记独立结算，与 `ReactionRegistry` 同口径）。
#[derive(Clone, Debug)]
pub struct TriggerRegistry<K, E = Effect> {
    entries: Vec<Trigger<K, E>>,
}

    /// 空注册表。
    pub const fn new() -> TriggerRegistry<K, E>

    /// 全部登记项（读）。
    pub fn entries(&self) -> &[Trigger<K, E>]

    /// 登记项条数（`Nat`，不是裸整型）。
    pub fn len(&self) -> Nat

    /// 是否没有任何登记项。
    pub fn is_empty(&self) -> bool

    /// 登记一条触发（追加；重复登记不去重）。
    pub fn register(&mut self, trigger: Trigger<K, E>)

    /// 从一份**静态登记表**整批拷入（「静态数据 + 函数指针表」，D186）。
    ///
    /// 卡牌 / 状态的定义把「时机 -> 效果」写成 `static [Trigger; N]`，
    /// 初始化时整表登记进所属家族的注册表；数据只有一份，运行期没有额外效果数据。
    pub fn register_all(&mut self, triggers: &[Trigger<K, E>])
    where
        K: Copy,
        E: Copy,


    /// 清空（例如该注册表跟随某个状态一起被移除）。
    pub fn clear(&mut self)
```

**`crates/definitions/src/timeline/dispatch.rs (Pending / dispatch / resolve)`**

```rust
/// 一条**待执行**（D186）：效果函数指针 + 触发它的事件，外加「该谁 / 按什么顺序」的标识。
///
/// - `effect`：注册方提供的执行函数指针（类型 `E` 由家族决定；本层不调用）；
/// - `event`：触发它的事件（纯数据；执行时原样传回）；
/// - `owner` / `source_kind` / `entry`：这条触发挂在谁身上、来源类别、进场先后
///   （调用方据此定位实例、扣可用次数 / 弃置等）。
#[derive(Clone, Copy, Debug)]
pub struct Pending<'a, E> {
    /// 执行函数指针（本层不调用）。
    pub effect: E,
    /// 触发它的事件。
    pub event: &'a Event,
    /// 这条触发挂在谁身上。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条）。
    pub entry: Nat,
}

pub fn dispatch<'e, K, E>(
    registry: &TriggerRegistry<K, E>,
    timing: K,
    event: &'e Event,
) -> Vec<Pending<'e, E>>
where
    K: Copy + PartialEq,
    E: Copy,
{
}


pub fn resolve<'e, K, E>(
    current: &TriggerRegistry<K, E>,
    other: &TriggerRegistry<K, E>,
    timing: K,
    event: &'e Event,
) -> Vec<Pending<'e, E>>
where
    K: Copy + PartialEq,
    E: Copy,
{
}

```

**`crates/definitions/src/timeline/suspend.rs (Queued / Frame / Resolution)`**

```rust
/// 一条 **owned 的待执行**：函数指针 + 事件 + 「谁 / 按什么顺序」标识。
///
/// 与 [`crate::timeline::Pending`] 字段一一对应，区别只在 `event` 是**按值**保存的 ——
/// 这样才能组成结算步骤 / 结算帧。
#[derive(Clone, Copy, Debug)]
pub struct Queued<E> {
    /// 执行函数指针（本层不调用）。
    pub effect: E,
    /// 触发它的事件（纯数据、owned）。
    pub event: Event,
    /// 这条触发挂在谁身上。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条）。
    pub entry: Nat,
}

/// **一帧结算**：待执行步骤 + `cursor`（插入点 / 下一条要执行的步骤下标）。
///
/// 插入子结算时**不动**本帧的 `cursor`（原步骤顺序不变，也不重排）；
/// 子帧执行完后回到本帧 `cursor` 继续。
#[derive(Clone, Debug)]
pub struct Frame<E> {
    steps: Vec<Queued<E>>,
    cursor: Nat,
}

    /// 用一串待执行步骤建一帧（`cursor` 从 0 开始）。
    pub fn new(steps: Vec<Queued<E>>) -> Frame<E>

    /// 这帧的步骤总数。
    pub fn len(&self) -> Nat

    /// 这帧是否没有步骤。
    pub fn is_empty(&self) -> bool

    /// 插入点：下一条要执行的步骤下标。
    pub fn cursor(&self) -> Nat

    /// 剩余步骤条数（总数 - `cursor`）。
    pub fn remaining_len(&self) -> Nat

    /// 这帧是否已经执行完（`cursor` 到末尾）。
    pub fn is_finished(&self) -> bool

    /// 剩余步骤（按原顺序；从 `cursor` 起）。
    pub fn remaining(&self) -> &[Queued<E>]

    /// 取下一步（`cursor` 前进 1；只移动游标，不执行效果）；已到末尾 -> `None`。
    pub fn take_step(&mut self) -> Option<Queued<E>>
    where
        E: Copy,


/// **结算帧栈**：插入式结算的当前状态（D196）。
///
/// - 最后一帧 = 当前帧；前面的帧是「被插入点保留的父结算」；
/// - [`Resolution::insert`] 在当前位置压入子结算帧；子帧做完自动弹回父帧的 `cursor`；
/// - 中断 = 保留这个值（帧与 `cursor` 都在）；确实被击倒 = [`Resolution::discard`]。
#[derive(Clone, Debug)]
pub struct Resolution<E> {
    frames: Vec<Frame<E>>,
}

    /// 用一串待执行步骤建单帧结算。
    pub fn new(steps: Vec<Queued<E>>) -> Resolution<E>

    /// 空结算（没有帧）。
    pub fn empty() -> Resolution<E>

    /// 全部帧（从外到内；读）。
    pub fn frames(&self) -> &[Frame<E>]

    /// 当前帧（最内层；没有帧 -> `None`）。
    pub fn current(&self) -> Option<&Frame<E>>

    /// 嵌套层数（帧数）。
    pub fn depth(&self) -> Nat

    /// 整个结算是否已完成（没有剩余步骤）。
    pub fn is_finished(&self) -> bool

    /// **插入子结算**：在当前帧当前位置压入新帧（父帧的 `cursor` 不动）。
    ///
    /// - 子结算执行完后**回到插入点**，继续父帧剩余步骤；
    /// - 不重排父帧已有步骤（不是 LIFO 触发栈）；
    /// - 空插入 = 不做任何事。
    pub fn insert(&mut self, steps: Vec<Queued<E>>)

    /// 取下一步：当前帧有剩余就取它；当前帧做完则弹回父帧继续。
    ///
    /// 整个嵌套结算都做完 -> `None`（只移动游标，不执行效果）。
    pub fn next_step(&mut self) -> Option<Queued<E>>
    where
        E: Copy,


    /// **确实被击倒**：丢弃结算帧（终止后续，不再继续）。
    ///
    /// 消费掉整个结算对象：类型上不能再恢复。
    pub fn discard(self)

    /// **只读快照**：当前结算嵌套状态（当前帧 / 剩余步骤 / 插入点）。
    ///
    /// 不改状态、不含函数指针；供打印 / 导出对照游戏结算顺序。
    pub fn snapshot(&self) -> ResolutionSnapshot
```

## 4. Invariants and prohibitions

> [!IMPORTANT]
> Dispatch never executes effects: `dispatch` / `resolve` only produce ordered lists; execution is done by the registrant / caller (D186).
> Ordering constants exist only in `rules::ops::order`; timeline keeps no second source-ordering table.

- No TypeId / string timing keys / global singleton; a timing key is a `Copy` value defined by a block.
- Inserted settlements do not reorder the original frame's steps: after the sub-settlement, resume at the insertion cursor.
- Interruption = keep frames and cursor; truly defeated = discard frames; never simulate with LIFO.
- Conditions only see the event; instance-level filtering goes in the effect function (`Pending::owner`), never as an owner parameter.

## 5. Basis (rulebook and D-numbers)

> [!NOTE]
> The sole authority for settlement order is the workspace-root `结算顺序.md` (provisional); ordering constants reuse `rules::ops::order`.

| Number | Conclusion |
|---|---|
| D185 | Timing keys are defined by the corresponding families/types, and blocks dispatch proactively |
| D186 | Register execution function pointers, not effect data; dispatch does not execute |
| D194 / D196 | Inserted settlements (new frames), interruption / resumption / discard, debug snapshot |
| D258 | The generic `Resolution` and the reaction-specific frame type coexist (tech debt) |

Rulebook · other rules · settlement order: ① current player first; ② character card -> character equipment and status -> side combat status -> summon -> support; ③ same source by entry order; fully equal order uses stable sort.

## 6. Common misuse

> [!WARNING]
> The first two patterns change the already ordered settlement sequence and diverge from the rulebook.

1. Assuming `dispatch` executes effects: it only returns a list of function pointers.
2. Implementing inserted settlements with a "trigger stack": use `Resolution::insert` new frames; LIFO changes the original order.
3. Capturing state in a condition closure: `Condition` is a function pointer and cannot capture.
4. Turning timing keys into strings / `TypeId`: blocks define concrete types.
5. Writing another source-ordering table: always reuse `rules::ops::order`.

## 7. Tests

| File | Tests |
|---|---|
| `timeline/dispatch.rs` | `dispatch_only_matches_the_requested_timing`, `same_timing_multiple_sources_follow_rulebook_order`, `same_source_orders_by_entry_order`, `ties_keep_registration_order_and_are_stable`, `resolve_puts_current_player_effects_first_even_for_later_source_kinds`, `dispatch_does_not_execute_effects`, `same_card_two_instances_only_the_owner_responds` |
| `timeline/trigger.rs` | `applies_requires_the_same_timing`, `register_all_copies_a_static_table`, `duplicate_registrations_are_not_deduplicated` |
| `timeline/suspend.rs` | `inserted_sub_resolution_returns_to_the_insertion_point`, `nested_insertions_return_level_by_level`, `interruption_keeps_frames_and_cursor_then_resumes`, `truly_defeated_discards_frames`, `snapshot_exposes_current_frame_remaining_steps_and_cursor` |

## 8. Not yet wired and to be verified

- `留空.md#51`: `timeline::Resolution` (generic) and `keywords::reaction::ReactionFrames` (reaction-specific) have the same semantics but different types; later `Resolution` should be made generic over "frame data" and merged.
- Whether each block defines all rulebook timings as timing keys and dispatches proactively: tracked per block and in `留空.md#15` / `#66`.

---

Last verified: 2026-10-19 (code: crates/definitions/src/timeline/dispatch.rs:dispatch)