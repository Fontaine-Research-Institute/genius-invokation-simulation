//! 融化（Melt）：冰 + 火 -> **本伤害 +2**。
//!
//! 规则书依据：
//! - 规则书·元素反应·效果「融化」（`elementalreactionseffects.json`；
//!   元素对 `GCG_ELEMENT_CRYO` + `GCG_ELEMENT_PYRO`）：「本伤害+2」；
//! - 规则书·元素反应·规则：对已具有元素附着的目标造成特定类型元素伤害 -> 引发元素反应；
//!   参与反应的元素消耗由伤害入口按 D167 完成（**本文件不自己扣附着**）。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::families::character::attachment::ReactionEvent;
use crate::reactions::effect::{EffectContext, ReactionEffectResult};
use gi_genius_invokation_engine::Nat;

/// 反应身份（指针身份：具体反应模块的零大小类型 + 一份静态实例，D95 / D96）。
#[derive(Debug)]
pub struct Melt;

impl ReactionIdentity for Melt {
    fn name(&self) -> &'static str {
        "融化"
    }
}

/// 全局唯一的反应身份实例。
pub static MELT: Melt = Melt;

/// 触发条件（D132：同一对元素**两种先后各一条**）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Cryo, Element::Pyro);

/// 本伤害加成（规则书「本伤害+2」）。
pub const DAMAGE_BONUS: Nat = Nat::new(2);

/// 规则表条目：交给 `damage::DamageEnv.rules`（已有接口，不另造注册机制）。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&MELT, &CONDITIONS)
}

/// 效果：只加伤，无额外底层操作。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(DAMAGE_BONUS, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> ReactionEvent {
        ReactionEvent::new(&MELT, Element::Cryo, Element::Pyro, Nat::new(1), Nat::new(2))
    }

    #[test]
    fn melt_adds_two_damage_with_no_extra_ops() {
        let result = effect(&event(), &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(2));
        assert_eq!(result.branches.len(), 1);
        assert!(result.branches[0].ops.is_empty());
        assert!(result.branches[0].probability.is_one());
    }

    #[test]
    fn melt_conditions_match_both_orders() {
        assert_eq!(MELT.name(), "融化");
        assert_eq!(CONDITIONS.len(), 2);
    }
}
