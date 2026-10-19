//! 超载（Overloaded）：雷 + 火 -> **本伤害 +2**；如果目标是「出战角色」，
//! 就强制切换到下一个角色。
//!
//! 规则书依据：规则书·元素反应·效果「超载」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_ELECTRO` + `GCG_ELEMENT_PYRO`）：
//! 「本伤害+2；如果目标是「出战角色」，就强制切换到下一个角色」。
//! 强制切换交给角色区的「切换出战角色」操作（**待接入**）；本文件只产出操作请求。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, TargetScope};
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Overloaded;

impl ReactionIdentity for Overloaded {
    fn name(&self) -> &'static str {
        "超载"
    }
}

/// 全局唯一的反应身份实例。
pub static OVERLOADED: Overloaded = Overloaded;

/// 触发条件（雷 / 火两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Electro, Element::Pyro);

/// 本伤害加成（规则书「本伤害+2」）。
pub const DAMAGE_BONUS: Nat = Nat::new(2);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&OVERLOADED, &CONDITIONS)
}

/// 效果：+2；目标是出战角色时追加「强制切换到下一个角色」。
pub fn effect(_event: &ReactionEvent, ctx: &EffectContext) -> ReactionEffectResult {
    if ctx.target_is_active {
        ReactionEffectResult::single(
            DAMAGE_BONUS,
            vec![EffectOp::ForceSwitch {
                target: TargetScope::Reacted,
            }],
        )
    } else {
        ReactionEffectResult::single(DAMAGE_BONUS, Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> ReactionEvent {
        ReactionEvent::new(
            &OVERLOADED,
            Element::Electro,
            Element::Pyro,
            Nat::new(1),
            Nat::new(2),
        )
    }

    #[test]
    fn overloaded_forces_switch_only_when_target_is_active() {
        let active = effect(&event(), &EffectContext::no_hands(true));
        assert_eq!(active.damage_bonus, Nat::new(2));
        assert_eq!(
            active.branches[0].ops,
            vec![EffectOp::ForceSwitch {
                target: TargetScope::Reacted,
            }]
        );

        let backstage = effect(&event(), &EffectContext::no_hands(false));
        assert_eq!(backstage.damage_bonus, Nat::new(2));
        assert!(backstage.branches[0].ops.is_empty());
    }
}
