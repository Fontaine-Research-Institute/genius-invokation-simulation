//! **「当前元素骰费用」字段**（行动牌的当前费用 = 基础费用 + 加费 / 减费）。
//!
//! ## 依据
//!
//! - 分工计划 §4.4.15（Lead 已定）：`cost` 只是**花费属性**（「要花什么」）；
//!   「当前情况」属于**被花的东西**（骰池 / 秘传揭令 / 充能），支付在那边。
//! - 决策 D170 / D172 / D178（`resources::dice::apply_delta` 已实现）：
//!   加费**整体加到一个类**（① 指定元素 -> ② 同色 -> ③ 任意；0 费加费新建 ② 同色）；
//!   减费按 ① -> ② -> ③ 依次减到 0、剩余作废。
//! - 卡面「本回合中，我方打出的下一张『X』费用 -N」/「费用 +N」这类效果 -> 就是往这个字段叠 delta。
//!
//! ## 口径（D182 / D183）
//!
//! - 这是**手牌卡 / 行动牌的属性**（一个普通值变量，D42 / D182）：每个实例自己持有一个；
//! - 对外只给**只读指针**：`base()` / `deltas()` / `current()` 都是 `&self` 借出切片（D183），
//!   不返回可变引用、不暴露字段、不拷贝；
//! - 计算**复用** `resources::dice::{CostComponent, apply_delta, CostDelta}`（本层不重写费用算法）；
//! - 纯函数：`with_delta` 吃 `&self` 返回新的当前费用，不改入参；零浮点、数值全 `Nat`。

use crate::resources::dice::{apply_delta, CostComponent, CostDelta};
use gi_genius_invokation_engine::{nadd, Nat};

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

impl Default for CurrentCost {
    /// 默认 = 0 费（没有任何费用项、没有增减费）。
    fn default() -> CurrentCost {
        CurrentCost::new(Vec::new())
    }
}

/// 一项费用的数量（三类都带一个 `Nat`）。
fn component_count(component: CostComponent) -> Nat {
    match component {
        CostComponent::SpecificElement(_, count)
        | CostComponent::SameColor(count)
        | CostComponent::AnyColor(count) => count,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    #[test]
    fn new_has_empty_deltas_and_current_equals_base() {
        let cost = CurrentCost::new(vec![CostComponent::AnyColor(n(3))]);
        assert_eq!(cost.base(), [CostComponent::AnyColor(n(3))]);
        assert_eq!(cost.current(), [CostComponent::AnyColor(n(3))]);
        assert!(cost.deltas().is_empty());
        assert_eq!(cost.total(), n(3));
        // 默认 = 0 费。
        let zero = CurrentCost::default();
        assert_eq!(zero.total(), Nat::ZERO);
        assert!(zero.current().is_empty());
    }

    #[test]
    fn increase_adds_to_the_same_class() {
        // ① 指定元素存在 -> 全加 ①（D172）。
        let cost = CurrentCost::new(vec![CostComponent::SpecificElement(Element::Pyro, n(2))]);
        let increased = cost.with_increase(n(1));
        assert_eq!(
            increased.current(),
            [CostComponent::SpecificElement(Element::Pyro, n(3))]
        );
        assert_eq!(increased.total(), n(3));
        assert_eq!(increased.deltas(), [CostDelta::Increase { amount: n(1) }]);
        // 入参不改（纯函数）。
        assert_eq!(cost.total(), n(2));
    }

    #[test]
    fn increase_on_zero_cost_creates_same_color() {
        // D178：0 费加费按相同元素算（新建 ② SameColor）。
        let zero = CurrentCost::new(Vec::new());
        let increased = zero.with_increase(n(2));
        assert_eq!(increased.current(), [CostComponent::SameColor(n(2))]);
        assert_eq!(increased.total(), n(2));
    }

    #[test]
    fn decrease_drains_specific_then_same_color_then_any() {
        // 三类各 1 点 -> 减 2：① 先减到 0，再减 ②；③ 留着。
        let cost = CurrentCost::new(vec![
            CostComponent::SpecificElement(Element::Hydro, n(1)),
            CostComponent::SameColor(n(1)),
            CostComponent::AnyColor(n(1)),
        ]);
        let decreased = cost.with_decrease(n(2));
        assert_eq!(decreased.current(), [CostComponent::AnyColor(n(1))]);
        assert_eq!(decreased.total(), n(1));
        // 减过头：最低 0（剩余作废；费用项被减完就移除）。
        let free = cost.with_decrease(n(9));
        assert_eq!(free.total(), Nat::ZERO);
        assert!(free.current().is_empty());
    }

    #[test]
    fn deltas_are_replayable_and_ordered() {
        let base = vec![CostComponent::SpecificElement(Element::Pyro, n(2))];
        let cost = CurrentCost::new(base.clone())
            .with_increase(n(1))
            .with_decrease(n(2));
        // 同序重放 = 当前值。
        let replayed = CurrentCost::new(base.clone()).with_deltas(cost.deltas());
        assert_eq!(replayed.current(), cost.current());
        // 顺序不同结果不同：先 +1 再 -2 -> ① 还剩 1；
        // 先 -2（① 减到 0 被移除）再 +1 -> 0 费加费新建 ② 同色 1。
        let reversed = CurrentCost::new(base)
            .with_decrease(n(2))
            .with_increase(n(1));
        assert_ne!(reversed.current(), cost.current());
        assert_eq!(reversed.current(), [CostComponent::SameColor(n(1))]);
    }
}
