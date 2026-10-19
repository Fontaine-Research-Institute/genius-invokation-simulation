//! 蒸发（Vaporize）：水 + 火 -> **本伤害 +2**。
//!
//! 规则书依据：规则书·元素反应·效果「蒸发」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_HYDRO` + `GCG_ELEMENT_PYRO`）：「本伤害+2」。
//! 元素附着 / 反应判定见规则书·元素反应·规则；附着消耗按 D167 由伤害入口做。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, ReactionEffectResult};
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Vaporize;

impl ReactionIdentity for Vaporize {
    fn name(&self) -> &'static str {
        "蒸发"
    }
}

/// 全局唯一的反应身份实例。
pub static VAPORIZE: Vaporize = Vaporize;

/// 触发条件（水 / 火两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Hydro, Element::Pyro);

/// 本伤害加成（规则书「本伤害+2」）。
pub const DAMAGE_BONUS: Nat = Nat::new(2);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&VAPORIZE, &CONDITIONS)
}

/// 效果：只加伤。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(DAMAGE_BONUS, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vaporize_adds_two_damage() {
        let event =
            ReactionEvent::new(&VAPORIZE, Element::Hydro, Element::Pyro, Nat::new(1), Nat::new(2));
        let result = effect(&event, &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(2));
        assert_eq!(result.branches.len(), 1);
        assert!(result.branches[0].ops.is_empty());
    }
}
