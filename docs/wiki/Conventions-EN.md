# Design Conventions Quick Reference

> [[Conventions|中文]] · English

*Scope: hard conventions to check before writing code; each item gives the requirement, rationale, locations, counterexamples, and basis.*

---

## 1. Types Define Identity: Zero-Sized Types + Pointer Identity

**Requirement**

- A tag is one zero-sized type in one file (D54); it is not an enum constant, a string, or a number.
- A tag's runtime identity is the pointer to its single piece of static data (D57 / D58); the tag file's `data()` returns a reference only and contains no literals.
- State / reaction identity comparisons always use `identity_address()` (a `*const ()` pointing at the definition data), compared with `std::ptr::eq` / `std::ptr::addr_eq` (D199 / D200 / D202).
- Except card ids (which must remain numbers), every id disappears after compilation (D41): no `TypeId`, no string keys, no numeric ids.

**Locations**

- Tag type + `data()`: `definitions/src/tags/action_card/support/location.rs`
- Single source of tag data: `TAGS` in `definitions/src/generated/mod.rs` (one field per tag)
- State identity: `identity_address()` in `definitions/src/status/def.rs` and `status/state.rs`
- Reaction identity: `ReactionIdentity::identity_address()` in `definitions/src/reactions/mod.rs`

> [!WARNING]
> Different zero-sized statics can share the same address; comparing `ptr::addr_eq` / `ptr::eq` on the ZSTs themselves merges distinct identities (measured in D200).
> Identity must land on the address of the definition data.

## 2. A Single Source of Data

**Requirement**

- Each tag has exactly one piece of static data; access is by field (`TAGS.location.name`), never arrays, index tables, name-to-index maps, or `HashMap` (D64 / D67).
- Tag files must not contain name string literals; they reference generated data only (a test guards this, D64).
- One format has one definition site: `CostComponent` belongs to the dice module and others only re-export it (D161); source-embedding macros and types belong to `definitions`, and only the summary table travels with the crate (D242).

> [!TIP]
> When "convenient access" seems necessary, first ask whether a single copy already exists; if so, use it instead of copying.

## 3. Pure Functions + `Option` / `Result`

**Requirement**

- Public entry points are pure: take `&DefState` and return a new `DefState` (with pending work / branches + exact probabilities when needed); failures return `Err` or `None`.
- Inputs are untouched byte for byte; internally you may `clone` first, and that clone becomes the return value.
- `&mut` appears only inside executors (for example `flow::pending::execute_pending` calling each area's `execute_*`); the public shape stays pure.
- Multiple branches use `Vec<(DefState, Fraction)>`; never sample and never merge probabilities implicitly.

> [!IMPORTANT]
> The rationale is in `definitions/src/state/flows/mod.rs`: search must branch, insertion-based settlement must be able to "return without executing", and rollback / comparison / unit tests require unchanged inputs.

## 4. Zero Floating Point

**Requirement**

- `crates/` contains no `f64` (D159; `grep -r "f64" new/crates` must yield zero hits).
- Probabilities, weights, and probability mass are always `Fraction` (numerator / denominator `ExactNat128`; D152 / D153 / D158).
- The random source exposes 128-bit integers only: `Rng::next_u128` / `Rng::next_below_u128` (D160).
- Observation points expand into concrete branches with exact probabilities; never sample (D190).

> [!NOTE]
> Only a future external display or solver boundary may convert to floating point once, and never inside the core crates (pending item #14).

## 5. Nat and ExactNat

- Business numbers default to `Nat` = `Nat32` (saturating)
  - Detail: Other widths (`Nat8/16/64/128`) only for deliberate designs and must be registered in `不入库/nat-宽度统计.md`
  - Basis: D37
- Probability-related values use `ExactNat` = `ExactNat128` (exact)
  - Detail: Add / sub / mul panic on overflow or underflow
  - Basis: D153 / D156
- Arithmetic macros are shared by both families
  - Detail: `nadd!` / `nsub!` / `nmul!` / `ndiv!`; semantics follow the type
  - Basis: D155
- No bare integers or bare `+ - * /`
  - Detail: `definitions` / `cards` deny `clippy::arithmetic_side_effects`; three scanners also advise
  - Basis: Section 8.3

> [!WARNING]
> Counterexamples: `let hp: u32 = 10;`, `let n = 2 + 3;`, using saturating `Nat` for probabilities, adding a `Nat64` without registration.

## 6. Saturating Arithmetic

**Requirement**

- Saturating family: add / sub / mul saturate; overflow stops at `MAX` and underflow stops at `ZERO`, without panicking (overflow is normal business).
- Division is the only operation that can crash: a zero divisor panics (D44; `Div` and `ndiv!` agree, debug and release alike).
- No `div_or_zero` / `saturating_div` variants (D44).

**Locations**: `engine/src/nat.rs` `saturating_add/sub/mul`, `Div`, and the four macros; `Fraction::new` panics on a zero denominator.

> [!WARNING]
> Do not rely on "overflow panics" to find business errors, and do not hand-wrap with `max/min` instead of saturating semantics.

## 7. A Step Is Atomic

**Requirement**

- A step is every action inside one observable effect (including multiple targets, multiple instances, per-card processing, and Repeat), executed in written order (D204 / D207 / D208 / D209).
- No foreign settlement may be inserted inside a step (triggers, other effects, sub-settlements).
- Triggers and pending work are dispatched and executed only at step ends; `timeline::Resolution::insert` is allowed only between steps.
- A step is both the display unit and the non-interruptible atomic settlement unit.

**Locations**: `definitions/src/keywords/step.rs`, the hard constraints in `keywords/mod.rs`, and the step-end flush in `flow/skill_use.rs`.

> [!IMPORTANT]
> Counterexamples: executing pending work or inserting a sub-settlement inside a keyword function; reading "a step" as "one action".

## 8. Effects Are Function Pointers

**Requirement**

- A trigger entry is: timing key + `owner` + `source_kind` + `entry` + condition function pointer + effect function pointer.
- A registry is a plain value (one instance per side, stored in a state field); there is no global singleton, no `TypeId`, and no string key (D185).
- `dispatch` only matches timings/conditions and sorts; it never executes effects (D186). Execution belongs to the caller.
- Multiple instances of the same card all register into the same family registry; use `owner` for instance filtering inside the effect function.

**Locations**: `definitions/src/timeline/trigger.rs`, `timeline/mod.rs` (`dispatch` / `resolve`); the single order constant source is `rules/ops/order.rs`.

> [!WARNING]
> Do not register closures, trait objects, or effect-data enums; do not execute effects inside `dispatch`; do not create one registry per instance.

## 9. Record API

**Requirement**

- The unified recorder lives in `definitions/src/records/`: `Record<T> { step: Nat, content: T }`, `Recorder<T>::push / entries / step_entries / render`.
- The recorder presets no kind or field; display is defined by the content type implementing `ShowRecord` (D224).
- Each module's record content type lives in its own file (D225): `keywords/record.rs`, `damage_chain/record.rs`, `zones/record.rs`, `flow/record.rs`, ...; `push` calls stay in the business code.
- Feature `records` is on by default; when off, `Recorder` is a zero-sized type and `push` is a no-op, with no effect on settlement.

**Entry points**: the `_recorded` wrappers (for example `flow::recorded::use_skill_me_recorded`).

> [!WARNING]
> Do not define business enums or fixed fields in `records`; do not build parallel record formats per module or call `println!` directly.

## 10. Sides Are Not Numbered; Areas Are Variables

**Requirement**

- A side is one of two named instances of the same structure (`me` / `opp`); every "do it for one side" entry point appears in pairs (`xxx_me` / `xxx_opp`) sharing one implementation.
- Do not pass `Side`, do not use `[T; 2]` with indices, and do not encode sides as 0/1.
- An area is a field in state: no area id, no area table, and the engine never records which area an entity belongs to; containers are only `Area` / `SingleArea`.

**Locations**: `definitions/src/state/def_state.rs`, `state/player.rs`; `engine/src/area.rs` / `entity.rs`.

> [!WARNING]
> Do not introduce a `Side` enum and pass it around inside `definitions`; do not use `players[0]` / `players[1]` for sides.

## 11. Settlement Order and Insertion

**Requirement**

- Within one timing, the workspace-root `结算顺序.md` is the single authority (provisional); the order constants exist only in `rules/ops/order.rs`.
- Settlement is insertion-based, not a stack: frames + `cursor`; after a sub-settlement, execution resumes at the insertion point; insertion never reorders the parent settlement (D196).

**Locations**: `definitions/src/timeline/mod.rs`, `timeline/suspend.rs`, `rules/ops/order.rs`.

> [!WARNING]
> Do not duplicate the "character card -> ... -> support card" order table elsewhere; do not implement trigger queues with a LIFO stack.

## 12. Related Pages

- [[Architecture-EN]]: dependency direction, data flow, known documentation deviations
- [[Crate-Engine-EN]]: numeric, fraction, and random primitives
- [[定义层-Keywords-EN]]: steps and keyword functions
- [[定义层-Timeline-EN]]: trigger registration and insertion-based settlement
- [[定义层-Records-EN]]: recorder implementation

---

## 13. Real Source Excerpts (matching the sections above)

The implementation of Tag type (section 1) follows:

File: crates/definitions/src/tags/action_card/support/location.rs:8-10

```rust
/// 「场地」：支援牌中的场地牌。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Location;
```

The code above defines Tag type (section 1); see the file for the full implementation.


The implementation of Tag data entry `data()` (sections 1 / 2) follows:

File: crates/definitions/src/tags/action_card/support/location.rs:12-15

```rust
/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.location
}
```

The code above defines Tag data entry `data()` (sections 1 / 2); see the file for the full implementation.


The implementation of State identity trait (section 1) follows:

File: crates/definitions/src/families/action_card/status/state.rs:43-78

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
```

The code above defines State identity trait (section 1); see the file for the full implementation.


The implementation of Identity address `identity_address()` (section 1) follows:

File: crates/definitions/src/families/action_card/status/state.rs:55-66

```rust
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
```

The code above defines Identity address `identity_address()` (section 1); see the file for the full implementation.


The implementation of Shared arithmetic macro (sections 4 / 5 / 6) follows:

File: crates/engine/src/nat.rs:426-435

```rust
/// 加法宏（**两族共用**，语义由类型决定，见 [`NatArith`]；D155）。
///
/// - `nadd!(Nat::MAX, Nat::new(1))` -> `Nat::MAX`（饱和，不 panic）；
/// - `nadd!(ExactNat::MAX, ExactNat::new(1))` -> panic（精确族不允许饱和）。
#[macro_export]
macro_rules! nadd {
    ($a:expr, $b:expr) => {
        $crate::nat::NatArith::nat_add($a, $b)
    };
}
```

The code above defines Shared arithmetic macro (sections 4 / 5 / 6); see the file for the full implementation.


The implementation of Fraction construction (section 4) follows:

File: crates/engine/src/prob.rs:77-86

```rust
    /// 构造 `num / den`（**不约分**；需要规范化时用加 / 乘 / 除的结果）。
    ///
    /// **分母为 0 -> panic**（与引擎除法一致：除零是逻辑错误，不是饱和业务）。
    pub const fn new(num: ExactNat128, den: ExactNat128) -> Fraction {
        assert!(
            den.get() != 0,
            "除零：分数分母为 0（与引擎除法一致，这是逻辑错误）"
        );
        Fraction { num, den }
    }
```

The code above defines Fraction construction (section 4); see the file for the full implementation.


The implementation of Step `Step` (section 7) follows:

File: crates/definitions/src/keywords/step.rs:44-48

```rust
/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}
```

The code above defines Step `Step` (section 7); see the file for the full implementation.


The implementation of Trigger entry `Trigger` (section 8) follows:

File: crates/definitions/src/timeline/trigger.rs:44-67

```rust
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
```

The code above defines Trigger entry `Trigger` (section 8); see the file for the full implementation.


The implementation of Record entry `Record<T>` (section 9) follows:

File: crates/definitions/src/records/mod.rs:34-41

```rust
/// 一条记录：**属于哪一步** + 任意内容（内容类型由使用方定义）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record<T> {
    /// 步号（0 起；由写入方给）。
    pub step: Nat,
    /// 记录内容（类型由写入方定义）。
    pub content: T,
}
```

The code above defines Record entry `Record<T>` (section 9); see the file for the full implementation.


The implementation of Record write `Recorder::push` (section 9) follows:

File: crates/definitions/src/records/mod.rs:88-91

```rust
    /// **唯一写入入口**：把一条内容记到某一步。
    pub fn push(&mut self, step: Nat, content: T) {
        self.entries.push(Record::new(step, content));
    }
```

The code above defines Record write `Recorder::push` (section 9); see the file for the full implementation.


The implementation of Cost component `CostComponent` (sections 2 / 5) follows:

File: crates/definitions/src/resources/dice/mod.rs:157-168

```rust
/// 花费 / 消耗的一位（D168：只有三类，枚举顺序 = 加费/减费优先级）。
///
/// 骰子模块只提供底层能力：can_afford（可能就行）与 pay（确定性扣减），**不做支付策略**（D147）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CostComponent {
    /// ① 指定元素骰：n 个**同一种指定元素**（只能是一种，不传多元素列表）。
    SpecificElement(Element, Nat),
    /// ② 相同元素骰：n 个**同色**（颜色在支付时再定）。
    SameColor(Nat),
    /// ③ 任意元素骰：n 个任意颜色。
    AnyColor(Nat),
}
```

The code above defines Cost component `CostComponent` (sections 2 / 5); see the file for the full implementation.


The implementation of Keyword function `cause_damage` (section 8) follows:

File: crates/definitions/src/keywords/keyword.rs:241-292

```rust
/// **造成伤害**：对选择器解析出的每个角色走伤修链（加伤 -> 护盾 -> 减伤 -> 扣血 / 附着 / 反应）。
///
/// 每次伤害都派发「生命值变动」待执行（不执行，步末统一做，D204）。
pub fn cause_damage(state: &DefState, env: &EffectEnv<'_>, args: &DamageArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    fan_out(state, env, candidates, args.pick, |current, side, target| {
        let target_state = character_state(current, side, target)?;
        let player = player_of(current, side);
        // 乘算 / 除算修饰随参数进来（卡面「此次伤害翻倍」等）；默认 NONE。
        let chain_env = DamageChainEnv::new(player.combat_status(), &target_state.states)
            .with_modifiers(args.modifiers);
        let chained = damage_with_chain(
            &target_state,
            env.subject(),
            target,
            args.amount,
            args.ty,
            &ChainDamageEnv {
                damage: &env.damage,
                chain: &chain_env,
                bonus: DamageBonus {
                    increases: env.increases,
                    mastery: env.mastery,
                    reaction_bonus: Nat::ZERO,
                    reacted: false,
                },
                context: env.effect_context,
                triggers: env.character_triggers(side),
            },
        );
        let next = with_character(current, side, target, chained.outcome.state);
        let mut pending = PendingSet::new();
        for item in chained.pending {
            push_pending(
                &mut pending,
                side,
                FlowPending::Character {
                    character: target,
                    pending: item,
                },
            );
        }
        // R4：伤害路径判出的反应不在这里执行（D204 步内不插）；记成反应子结算，
        // 由步执行器在步边界插入执行（做完回到原步骤）。
        let mut reactions = Vec::new();
        if let Some(hit) = chained.reaction {
            reactions.push(ReactionRequest::new(hit, env.viewer(), side, false));
        }
        Some((next, pending, reactions))
    })
}
```

The code above defines Keyword function `cause_damage` (section 8); see the file for the full implementation.


The implementation of Settlement order constant (section 11) follows:

File: crates/definitions/src/rules/ops/order.rs:29-36

```rust
/// 同一牌手内、不同来源类别的结算顺序（规则书第 2 条）。
pub const SOURCE_ORDER: [EffectSource; 5] = [
    EffectSource::CharacterCard,
    EffectSource::CharacterEquipmentOrStatus,
    EffectSource::CombatStatus,
    EffectSource::Summon,
    EffectSource::Support,
];
```

The code above defines Settlement order constant (section 11); see the file for the full implementation.


The implementation of Order predicate `resolves_before` (section 11) follows:

File: crates/definitions/src/rules/ops/order.rs:38-59

```rust
/// 同一时间点、同一牌手内：first 是否先于 second 结算（规则书第 2 条）。
///
/// 同一类别 -> false（同类按进场先后，规则书第 3 条，见 same_source_uses_entry_order）。
pub fn resolves_before(first: EffectSource, second: EffectSource) -> bool {
    // 同一类别不比先后（同类按进场先后，规则书第 3 条）。
    if first == second {
        return false;
    }
    let mut seen_first = false;
    for source in SOURCE_ORDER {
        if source == first {
            if seen_first {
                return false;
            }
            seen_first = true;
        }
        if source == second {
            return seen_first;
        }
    }
    false
}
```

The code above defines Order predicate `resolves_before` (section 11); see the file for the full implementation.



---

## Summary

All conventions serve one goal: identity by pointer, one copy of data, pure entry points. Skim this page before writing code.

---

Last reviewed: 2026-10-19 (code: `new/crates/definitions/src/keywords/step.rs:Step`)
