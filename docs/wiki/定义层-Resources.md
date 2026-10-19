# 定义层-Resources

> 中文 · [[定义层-Resources-EN|English]]

*本页范围: 骰池 (纯计数) , 三类费用, 够不够付 / 扣减, 折叠组的按需展开与查询快照, 以及行动牌的当前元素骰费用.*

---

## 1. 职责

资源层管"被花的东西": 骰池是纯计数 (不是实体) ; 提供三类费用, `can_afford` / `pay`, 折叠组的按需展开与查询快照; 行动牌的"当前元素骰费用"在同目录的 cost 块.id / 元素约定见 [[定义层-Base]].

## 2. 关键概念与类型

| 概念 | 文件 | 说明 |
|---|---|---|
| `DieFace` | `crates/definitions/src/resources/dice/mod.rs` | 8 种骰面 (7 元素 + 万能) , 含 `element` / `is_omni` / `from_element` |
| `DicePool` | 同上 | 三块表示: `omni` + `folded` (颜色未知同色组) + `known` (已展开具体色) ; 字段私有 |
| `FoldedGroup` | 同上 | `size` + `color_mass: [ExactNat128; 7]` (真实条件概率质量) + `source` 同源标记; 同源组必须一起展开 |
| `DiceSnapshot` | 同上 | 查询固定快照: `omni` / `known` / `folded` + 本次采样展开的 `expanded` |
| `CostComponent` / `CostDelta` | 同上 | 费用三类 (指定元素 / 同色 / 任意) 与增减费 |
| `CurrentCost` | `crates/definitions/src/families/action_card/areas/cost.rs` | 行动牌的当前费用 = 基础费用 + 加费 / 减费 |

> [!NOTE]
> 骰池不占实体区, 不发号 (D47) ; 上限 16; 全模块不用浮点, 概率量为 `ExactNat128` / `Fraction`.
> 随机生成只给分支与概率; **采样只发生在 `all_dice` 查询** (D146 / D149) .

## 3. 公开入口

| 入口 | 文件 | 说明 |
|---|---|---|
| `DicePool::{empty, new, total, add, remove, convert}` | `resources/dice/mod.rs` | 构造 / 计数 / 增减 / 转化; `add` 与 `new` 按上限 16 截断 |
| `DicePool::{expand_source, condition}` | 同上 | 同源组一起展开; 按谓词条件化, 均返回分支 + 精确概率 |
| `DicePool::{can_afford, pay, all_dice}` | 同上 | "可能够"判定; 确定性扣减; 查询快照 |
| `DicePool::{generate_random, generate}` | 同上 | 随机 n 颗 / 按有序内容表达式生成 |
| `validate_cost` / `apply_delta` | 同上 | 校验费用 (非法组合 panic) ; 叠加一次增减费 |
| `CurrentCost::{new, base, deltas, current, total, with_delta, with_increase, with_decrease}` | `families/action_card/areas/cost.rs` | 当前费用纯数据 |

### 源码 

> [!NOTE]
> 以下为骰池与费用的真实源码摘录 (签名 + 文档注释; 函数体从略, 完整实现见文件) .

**`crates/definitions/src/resources/dice/mod.rs(FoldedGroup / DicePool / DiceSnapshot)`**

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
**`crates/definitions/src/resources/dice/mod.rs(DicePool 的公开方法签名, 含 impl 行与文档注释)`**

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
**`crates/definitions/src/families/action_card/areas/cost.rs(CurrentCost 定义与方法签名)`**

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

## 4. 不变量与禁止

> [!IMPORTANT]
> `pay` 是确定性扣减: 折叠组必须先展开才能用于指定色 / 同色, 因此 `can_afford == true` 不保证 `pay` 成功.
> `SameColor` 费用必须由调用方给 `same_color`; 不给就返回 `None` (不猜颜色) .

- 骰池是纯计数: 不要发实体号 / 放进实体区.
- 上限 16: `add` / `new` / 生成都截断; `new` 规范化顺序为万能 -> 具体色 (元素序) -> 折叠组 (source, size) .
- 同一次随机的多个折叠组必须一起展开, 视为同色 (D148) , 不能独立采样.
- 概率一律精确分数 / 整数质量, 禁止浮点; 抽样只经引擎 `Rng`.
- 所有操作返回新 `DicePool`, 不改入参.

## 5. 依据 (规则书与 D 编号) 

> [!NOTE]
> 依据优先级: 规则书 > 权威关键字 > 卡面文本 > 参考实现; 本页编号均可查 `决策日志.md`.

| 编号 | 结论 |
|---|---|
| D46 / D47 | 骰子用计数表示, 按需展开; 上限 16; 支付只扣计数 |
| D48 / D49 | 非万能按颜色分组; 三块表示 + 展开规则 |
| D117 / D121 | 调和 = 转化 (不是获得) , 必须指定目标 |
| D139 / D140 / D141 / D148 | 生成给 n, 可限定基础; 有序表达式; 同源折叠组 |
| D146 / D149 | 查询按当前局面的真实条件概率采样展开, 快照保留折叠组 |
| D156 / D158 / D159 | 概率精确族, 全程不经过浮点 |
| D168 / D169 | 费用三类与扣减顺序 ① -> ② -> ③ |

## 6. 常见误用

> [!WARNING]
> 以下写法会让"看起来够"变成运行时 `None` / panic; 先检查折叠组与费用合法性.

1. `can_afford` 成功后直接 `expect` `pay`: 折叠组未展开时 `pay` 返回 `None`.
2. 给 `SameColor` 传 `None` 并期望自动挑色: 本模块不猜, 由调用方给.
3. 把 `all_dice` 的 `expanded` 当稳定值: 每次查询都会重新采样; `folded` 才保留原状.
4. 直接改 `DicePool` 字段: 字段私有, 必须走 `add/remove/convert/pay/new`.
5. 费用里放两个不同元素的 ①: `validate_cost` 会 panic (逻辑错误) .

## 7. 测试

| 文件 | 测试 |
|---|---|
| `resources/dice/mod.rs` | `add_stops_at_cap`, `constructor_normalizes_and_caps`, `can_afford_uses_omni_and_possible_folded`, `pay_is_deterministic_and_needs_same_color`, `all_dice_returns_owned_snapshot`, `expand_source_uses_stored_mass_not_uniform`, `condition_enumerates_hidden_joint_colors`, `delta_*` |
| `resources/dice/parse.rs` | `random_two_basic_folds_into_count_vectors`, `content_slot_order_does_not_change_result`, `fold_branches_merges_equivalent_pools` |

## 8. 待接入与待实测

- `留空.md#1`: 加费 / 减费同时存在时的支付优先顺序规则书未写, 由调用方定序列.
- `留空.md#9`: 调和未指定目标时的自动选择算法明确不做.
- `留空.md#37`: 目标非法 vs 骰不够的优先级未写 (flow 先校验目标再支付) .
- `留空.md#38`: 来源骰是否允许万能 / 同面转换未写, 当前不限制.
- `留空.md#45` / `#58`: 新抓手牌 / 反应写回时的当前费用表同步缺口.

---

最后核对: 2026-10-19 (代码: crates/definitions/src/resources/dice/mod.rs:DicePool) 