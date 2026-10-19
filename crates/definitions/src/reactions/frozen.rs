//! 冻结（Frozen）：冰 + 水 -> **本伤害 +1**，使目标角色附属[冻结]状态。
//!
//! 规则书依据：规则书·元素反应·效果「冻结」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_CRYO` + `GCG_ELEMENT_HYDRO`）：
//! 「本伤害+1，使目标角色附属[冻结]状态：角色无法使用技能（状态持续到回合结束为止）；
//! 角色受到火元素伤害或物理伤害时伤害值+2，并移除此状态」。
//!
//! 注意：冻结状态**自己的**「+2 并移除」规则属于状态牌实现（状态 106），不在本文件；
//! 本文件只产出「附属冻结状态」这一个底层操作请求（状态区，**待接入**）。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, StatusTarget, TargetScope};
use crate::reactions::entities;
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Frozen;

impl ReactionIdentity for Frozen {
    fn name(&self) -> &'static str {
        "冻结"
    }
}

/// 全局唯一的反应身份实例。
pub static FROZEN: Frozen = Frozen;

/// 触发条件（冰 / 水两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Cryo, Element::Hydro);

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&FROZEN, &CONDITIONS)
}

/// 效果：+1；给承载反应的角色附属 1 层[冻结]（角色状态，持续到回合结束由状态牌自己管）。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(
        DAMAGE_BONUS,
        vec![EffectOp::Status {
            card: entities::FROZEN,
            target: StatusTarget::Character(TargetScope::Reacted),
            stacks: Nat::new(1),
            uses: None,
            stack_cap: None,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_adds_one_and_applies_frozen_status_to_the_reacted_character() {
        let event =
            ReactionEvent::new(&FROZEN, Element::Cryo, Element::Hydro, Nat::new(1), Nat::new(2));
        let result = effect(&event, &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::Status {
                card: entities::FROZEN,
                target: StatusTarget::Character(TargetScope::Reacted),
                stacks: Nat::new(1),
                uses: None,
                stack_cap: None,
            }]
        );
    }
}
