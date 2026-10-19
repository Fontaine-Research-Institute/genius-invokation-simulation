# Definitions Layer: Resources

> [[定义层-Resources|中文]] · English

*Scope: the dice pool (pure counting), the three cost classes, afford / pay, on-demand expansion and query snapshots of folded groups, and the current elemental dice cost of action cards.*

---

## 1. Responsibility

The resources layer owns "things that get spent": the dice pool is pure counting (not an entity); it provides the three cost classes, `can_afford` / `pay`, on-demand expansion and query snapshots of folded groups; the "current elemental dice cost" of action cards lives in the sibling cost block. Id / element conventions are in [[定义层-Base-EN]].

## 2. Key concepts and types

| Concept | File | Notes |
|---|---|---|
| `DieFace` | `crates/definitions/src/resources/dice/mod.rs` | Eight die faces (seven elements + omni); `element` / `is_omni` / `from_element` |
| `DicePool` | same | Three blocks: `omni` + `folded` (same-color groups with unknown color) + `known` (expanded concrete colors); fields private |
| `FoldedGroup` | same | `size` + `color_mass: [ExactNat128; 7]` (true conditional probability mass) + `source`; same-source groups must expand together |
| `DiceSnapshot` | same | Fixed query snapshot: `omni` / `known` / `folded` + `expanded` sampled for this query |
| `CostComponent` / `CostDelta` | same | Three cost classes (specific element / same color / any) and cost deltas |
| `CurrentCost` | `crates/definitions/src/families/action_card/areas/cost.rs` | Current cost of an action card = base cost + increases / decreases |

> [!NOTE]
> The dice pool occupies no entity area and allocates no entity ids (D47); cap 16; no floating point — probability masses are `ExactNat128` / `Fraction`.
> Random generation only returns branches and probabilities; **sampling happens only in `all_dice` queries** (D146 / D149).

## 3. Public entry points

| Entry | File | Notes |
|---|---|---|
| `DicePool::{empty, new, total, add, remove, convert}` | `resources/dice/mod.rs` | Construct / count / add / remove / convert; `add` and `new` truncate at cap 16 |
| `DicePool::{expand_source, condition}` | same | Expand same-source groups together / condition on a predicate; both return branches + exact probabilities |
| `DicePool::{can_afford, pay, all_dice}` | same | "Possibly enough" check; deterministic deduction; query snapshot |
| `DicePool::{generate_random, generate}` | same | Random n dice / generated from an ordered content expression |
| `validate_cost` / `apply_delta` | same | Validate a cost (panics on illegal combinations); apply one delta |
| `CurrentCost::{new, base, deltas, current, total, with_delta, with_increase, with_decrease}` | `families/action_card/areas/cost.rs` | Pure data for the current cost |

### Source (verbatim)

> [!NOTE]
> Real source excerpts for the dice pool and costs (signatures and doc comments verbatim; bodies omitted).

**`crates/definitions/src/resources/dice/mod.rs (FoldedGroup / DicePool / DiceSnapshot)`**

```rust
/// 一个颜色未知的**同色组**（D49 第 2 块）。
///
/// - size：组内骰子数；
/// - color_mass：该组颜色是各元素的**未归一化概率质量**（7 个元素顺序，见 ELEMENTS）；
///   分布就是「到当前局面后的真实条件概率」，不是写死的 1/7（D146/D148）；
/// - source：同源标记。同一次随机产生的多个组共用同一个 source，
///   **必须一起展开、且保守地视为同一种颜色**（D148；联合分布若将来需要再升级）。
#[derive(Clone, PartialEq, Debug)]
pub struct FoldedGroup {
    /// 组内骰子数。
    pub size: Nat,
    /// 该组颜色是各元素的未归一化概率质量（7 元素顺序；精确族，D156/D159：概率量不用浮点）。
    pub color_mass: [ExactNat128; 7],
    /// 同源标记：同源组必须一起展开。
    pub source: Nat,
}

/// 骰池 = 三块（D49）：万能 / 折叠的同色组 / 已展开的具体色。
///
/// 字段私有：对外只有一个读取入口 all_dice（D143），返回固定快照（D145）。
#[derive(Clone, PartialEq, Debug)]
pub struct DicePool {
    /// 万能骰个数。
    omni: Nat,
    /// 颜色未知的同色组（规范序：按 source 再按 size）。
    folded: Vec<FoldedGroup>,
    /// 已展开的具体颜色计数（按 Element 序，无 0 项）。
    known: Vec<(Element, Nat)>,
}

/// 「当前所有骰子」的**固定快照**（D143/D145/D146/D148）。
///
/// - omni / known：确定的部分；
/// - folded：仍折叠的组（原样保留，用来看出哪些是折叠组，D148）；
/// - expanded：本次查询按真实条件概率**采样**展开出的折叠骰具体色（每次查询采样一次）。
///
/// 具体骰总数 = omni + known + expanded；folded 与 expanded 描述的是同一批折叠骰的两面。
/// 快照拥有全部数据，查询后骰池再变化也不会影响它（D145）。
#[derive(Clone, PartialEq, Debug)]
pub struct DiceSnapshot {
    /// 万能骰个数。
    pub omni: Nat,
    /// 已展开的具体颜色计数。
    pub known: Vec<(Element, Nat)>,
    /// 仍折叠的同色组（原样保留）。
    pub folded: Vec<FoldedGroup>,
    /// 本次查询采样展开出的具体颜色计数。
    pub expanded: Vec<(Element, Nat)>,
}
```
**`crates/definitions/src/resources/dice/mod.rs (public DicePool method signatures, with impl lines and doc comments)`**

```rust
impl DicePool {
    /// 空骰池。
    pub const fn empty() -> DicePool

    /// 由三块构造（外部 / 后续构造折叠组用；本块的随机生成只产出 known/omni）。
    ///
    /// 规范化：known 合并同色、去 0、按 Element 序；folded 去 0、按 (source, size) 排序，
    /// 并把同一 source 的所有组的 color_mass 统一成第一组的（保守：同源组同色同分布，D148）；
    /// 最后按「万能 -> 具体色（元素序）-> 折叠组（规范序）」顺序截断到上限 16（D47）。
    pub fn new(omni: Nat, folded: Vec<FoldedGroup>, known: Vec<(Element, Nat)>) -> DicePool

    /// 骰子总数（omni + 折叠组 + 具体色）；0~16。
    pub fn total(&self) -> Nat

    /// 「当前所有骰子」：返回**固定快照**（D143/D145），折叠组按真实条件概率采样展开（D146）。
    ///
    /// 随机数由调用方传入引擎的 Rng（D149/D151：本块算概率，引擎给真实随机数）。
    pub fn all_dice(&self, rng: &mut Rng) -> DiceSnapshot

    /// 增加指定骰面的 count 颗（确定性；超出上限 16 的部分不生成，D47）。
    pub fn add(&self, face: DieFace, count: Nat) -> DicePool

    /// 减少指定骰面的 count 颗（确定性；不够 -> None）。
    ///
    /// 只扣 known / omni；**折叠组不动**（要看颜色就必须先显式展开，D144）。
    pub fn remove(&self, face: DieFace, count: Nat) -> Option<DicePool>

    /// 转化（D117）：把 count 颗来源骰转成目标骰面；**不是获得**，总数不变。
    ///
    /// - target 必须由调用方指定；未指定目标时的内置选择算法留空（调用处 TODO）。
    /// - source = Some(face)：只从该面的 known / omni 扣（折叠组要先展开）；
    /// - source = None（任意来源）：按固定顺序扣「折叠组 -> 具体色（元素序）-> 万能」，
    ///   不产生分支（D144：选哪颗骰的分支由上层做）。
    pub fn convert(&self, source: Option<DieFace>, count: Nat, target: DieFace) -> Option<DicePool>

    /// 展开一个**同源**的折叠组集合（D49/D146/D148）：按 color_mass 枚举 7 种颜色分支，
    /// 概率 = 归一化质量；整组一起变成该颜色（同源组一起展开、同色），并记进 known。
    ///
    /// 精确枚举（不采样）；只返回概率 > 0 的颜色分支。source 不存在 -> 空。
    pub fn expand_source(&self, source: Nat) -> Vec<(DicePool, Fraction)>

    /// 条件化（D148）：按「具体基础元素计数」上的条件，枚举命中的联合颜色取值。
    ///
    /// 折叠组可能相关（同源组同色），所以保守实现把**命中的联合取值直接展开成具体色**，
    /// 返回 (展开后的骰池, 该取值的无条件概率)；概率和 = P(条件成立)。
    /// 需要条件概率的调用方自己归一化。
    pub fn condition(&self, predicate: &dyn Fn(&BasicColors) -> bool) -> Vec<(DicePool, Fraction)>

    /// 够不够付（**可能就行**，D144）：折叠组按「可能是什么色」计入上界。
    ///
    /// 注意：pay 要求折叠组先展开，所以 can_afford = true 不保证 pay 直接成功。
    pub fn can_afford(&self, cost: &[CostComponent]) -> bool

    /// 扣减（确定性，不产生分支；D47/D144）。付不起 -> None。
    ///
    /// - 按 ① SpecificElement → ② SameColor → ③ AnyColor 顺序扣（D168/D169）；
    /// - SpecificElement：先用该色的 known，再用万能；
    /// - SameColor：付哪个颜色由调用方给（same_color）；没给 -> None；
    /// - AnyColor：按固定顺序扣「折叠组 -> 具体色（元素序）-> 万能」（不产生分支）。
    ///
    /// 折叠组未显式展开时不能用于指定色 / 同色（返回 None）。
    pub fn pay(&self, cost: &[CostComponent], same_color: Option<Element>) -> Option<DicePool>
}

impl DicePool {
    /// 随机生成 n 颗骰（D139/D140/D148），返回折叠后的分支。
    ///
    /// - count 必须 >= 1（0 视为非法，返回空）；
    /// - limited_to_basic = true：只在七种基础元素里随机（不含万能）；false：可以出万能（D140）；
    /// - distinct = true：n 颗互不相同；仅基础时 n > 7 非法、含万能时 n > 8 非法（先检测再枚举，D148）；
    /// - 上限 16：只生成放得下的 k = min(n, 16 - 当前总数) 颗；池已满 -> 1 支「池不变、概率 1」。
    pub fn generate_random(
        &self,
        count: Nat,
        limited_to_basic: bool,
        distinct: bool,
    ) -> Vec<(DicePool, Fraction)>

    /// 按**有序内容表达式**生成（D141）：数量 = 表达式长度，顺序当前无语义。
    ///
    /// - Element(e)：确定性生成一颗该元素骰；Omni：确定性生成一颗万能骰；
    /// - Any：随机一颗（颜色范围受 limited_to_basic 约束，D141 A2）；
    /// - 上限 16：按书写顺序取前 k 项生成，池已满 -> 1 支「池不变、概率 1」；空表达式非法 -> 空。
    pub fn generate(&self, content: &[DiceSlot], limited_to_basic: bool) -> Vec<(DicePool, Fraction)>
}
```
**`crates/definitions/src/families/action_card/areas/cost.rs (CurrentCost definition and method signatures)`**

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

impl CurrentCost {
    /// 由基础费用建当前费用（还没有任何增减费）。
    pub fn new(base: Vec<CostComponent>) -> CurrentCost {
        CurrentCost {
            current: base.clone(),
            base,
            deltas: Vec::new(),
        }
    }

    /// 基础费用（**只读指针**，D183）。
    pub fn base(&self) -> &[CostComponent] {
        &self.base
    }

    /// 已叠加的增减费（**只读指针**，D183；顺序 = 施加顺序）。
    pub fn deltas(&self) -> &[CostDelta] {
        &self.deltas
    }

    /// **当前费用**（**只读指针**，D183；对外只读的入口就是它）。
    pub fn current(&self) -> &[CostComponent] {
        &self.current
    }

    /// 当前费用的总骰数（把各项数量相加；用于「够不够付」的粗略比较，真正的支付归骰池）。
    pub fn total(&self) -> Nat {
        let mut total = Nat::ZERO;
        for component in &self.current {
            total = nadd!(total, component_count(*component));
        }
        total
    }

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

    /// 按顺序叠加一批增减费（等价于逐个 `with_delta`）。
    pub fn with_deltas(&self, deltas: &[CostDelta]) -> CurrentCost {
        let mut next = self.clone();
        for delta in deltas {
            next = next.with_delta(*delta);
        }
        next
    }

    /// 加费 `amount`（D178：0 费时新建 ② 同色）。
    pub fn with_increase(&self, amount: Nat) -> CurrentCost {
        self.with_delta(CostDelta::Increase { amount })
    }

    /// 减费 `amount`（D170：① -> ② -> ③ 依次减，剩余作废）。
    pub fn with_decrease(&self, amount: Nat) -> CurrentCost {
        self.with_delta(CostDelta::Decrease { amount })
    }
}
```

## 4. Invariants and prohibitions

> [!IMPORTANT]
> `pay` is deterministic: folded groups must be expanded before paying for specific / same colors, so `can_afford == true` does not guarantee `pay` succeeds.
> `SameColor` costs require the caller to pass `same_color`; without it the result is `None` (no guessing).

- The dice pool is pure counting: never allocate entity ids or put dice in an entity area.
- Cap 16: `add` / `new` / generation all truncate; `new` normalizes omni -> concrete colors (element order) -> folded groups (source, size).
- Multiple folded groups from one random event must expand together and be treated as the same color (D148); never sample them independently.
- Probabilities are exact fractions / integer masses; floating point is forbidden; sampling goes only through the engine `Rng`.
- Every operation returns a new `DicePool` and never mutates its input.

## 5. Basis (rulebook and D-numbers)

> [!NOTE]
> Evidence priority: rulebook > authoritative keywords > card text > reference implementations. All numbers are in `决策日志.md`.

| Number | Conclusion |
|---|---|
| D46 / D47 | Dice are counts, expanded on demand; cap 16; payment only deducts counts |
| D48 / D49 | Non-omni dice are grouped by color; three-block representation + expansion rules |
| D117 / D121 | Tuning = conversion (not gain); a target must be specified |
| D139 / D140 / D141 / D148 | Generation takes n, optionally basic-only; ordered expressions; same-source folded groups |
| D146 / D149 | Queries sample using the true conditional probability of the current position; snapshots keep folded groups |
| D156 / D158 / D159 | Exact probability family; no floating point |
| D168 / D169 | Three cost classes and deduction order ① -> ② -> ③ |

## 6. Common misuse

> [!WARNING]
> These patterns turn "looks affordable" into a runtime `None` / panic; check folded groups and cost legality first.

1. Calling `expect` on `pay` after `can_afford`: `pay` returns `None` while folded groups are unexpanded.
2. Passing `None` for `SameColor` and expecting automatic color choice: this module does not guess.
3. Treating `all_dice`'s `expanded` as stable: every query re-samples; only `folded` is preserved as-is.
4. Mutating `DicePool` fields directly: they are private; use `add/remove/convert/pay/new`.
5. Putting two ① components with different elements into one cost: `validate_cost` panics (logic error).

## 7. Tests

| File | Tests |
|---|---|
| `resources/dice/mod.rs` | `add_stops_at_cap`, `constructor_normalizes_and_caps`, `can_afford_uses_omni_and_possible_folded`, `pay_is_deterministic_and_needs_same_color`, `all_dice_returns_owned_snapshot`, `expand_source_uses_stored_mass_not_uniform`, `condition_enumerates_hidden_joint_colors`, `delta_*` |
| `resources/dice/parse.rs` | `random_two_basic_folds_into_count_vectors`, `content_slot_order_does_not_change_result`, `fold_branches_merges_equivalent_pools` |

## 8. Not yet wired and to be verified

- `留空.md#1`: the rulebook does not specify payment priority when cost increases and decreases coexist; the caller defines the sequence.
- `留空.md#9`: the automatic selection algorithm for unspecified tuning targets is explicitly not implemented.
- `留空.md#37`: the priority between "illegal target" and "not enough dice" is unspecified (flow validates the target before paying).
- `留空.md#38`: whether source dice may be omni / same-face conversion is unspecified; currently unrestricted.
- `留空.md#45` / `#58`: the current-cost table is not synchronized when new cards are drawn / reaction results are written back.

---

Last verified: 2026-10-19 (code: crates/definitions/src/resources/dice/mod.rs:DicePool)