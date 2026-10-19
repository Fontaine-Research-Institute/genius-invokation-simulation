//! 结晶（Crystallize）：岩 + 冰/水/火/雷 -> **本伤害 +1**，提供 1 点护盾，
//! 保护**我方出战角色**（护盾值可叠加，最多 2 点）。
//!
//! 规则书依据：规则书·元素反应·效果「结晶」共 4 条（`elementalreactionseffects.json`）：
//! 元素对 `GCG_ELEMENT_GEO` + 冰 / 水 / 火 / 雷，内容均为
//! 「本伤害+1，提供1点护盾，保护我方出战角色（护盾值可叠加，最多2点）」。
//! 4 条规则效果完全相同 -> **一个反应身份**。
//!
//! 「我方」= 规则书写法中的**造成伤害一方**（[`Side::Source`]）；岩无法附着（D134），
//! 所以岩只能作为「本次攻击元素」。
//!
//! 待复核：护盾状态（203）本身卡面写「可叠加，没有上限」，而结晶条写「最多2点」——
//! 暂按「结晶给的护盾叠到 2 点封顶」写成 `stack_cap: Some(2)`；**待实测**。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, StatusTarget, TargetScope};
use crate::reactions::entities;
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Crystallize;

impl ReactionIdentity for Crystallize {
    fn name(&self) -> &'static str {
        "结晶"
    }
}

/// 全局唯一的反应身份实例。
pub static CRYSTALLIZE: Crystallize = Crystallize;

/// 触发条件：岩 + 冰 / 水 / 火 / 雷，每种两种先后（D132）共 8 条。
pub static CONDITIONS: [ReactionCondition; 8] = [
    ReactionCondition::new(Element::Cryo, Element::Geo),
    ReactionCondition::new(Element::Geo, Element::Cryo),
    ReactionCondition::new(Element::Hydro, Element::Geo),
    ReactionCondition::new(Element::Geo, Element::Hydro),
    ReactionCondition::new(Element::Pyro, Element::Geo),
    ReactionCondition::new(Element::Geo, Element::Pyro),
    ReactionCondition::new(Element::Electro, Element::Geo),
    ReactionCondition::new(Element::Geo, Element::Electro),
];

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 护盾点数（规则书「提供1点护盾」）。
pub const SHIELD_POINTS: Nat = Nat::new(1);

/// 护盾叠加上限（规则书「最多2点」；**待实测**）。
pub const SHIELD_STACK_CAP: Nat = Nat::new(2);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&CRYSTALLIZE, &CONDITIONS)
}

/// 效果：+1；给触发方出战角色 1 点护盾（可叠加，暂按最多 2 点）。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(
        DAMAGE_BONUS,
        vec![EffectOp::Status {
            card: entities::SHIELD,
            target: StatusTarget::Character(TargetScope::SourceActive),
            stacks: SHIELD_POINTS,
            uses: None,
            stack_cap: Some(SHIELD_STACK_CAP),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crystallize_adds_one_and_shields_the_source_active_character() {
        let event = ReactionEvent::new(
            &CRYSTALLIZE,
            Element::Hydro,
            Element::Geo,
            Nat::new(1),
            Nat::new(2),
        );
        let result = effect(&event, &EffectContext::no_hands(false));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::Status {
                card: entities::SHIELD,
                target: StatusTarget::Character(TargetScope::SourceActive),
                stacks: Nat::new(1),
                uses: None,
                stack_cap: Some(Nat::new(2)),
            }]
        );
    }

    #[test]
    fn crystallize_has_eight_conditions() {
        assert_eq!(CONDITIONS.len(), 8);
        // 护盾在规则书写法里是「我方」（触发方）的，不是承载方。
        let result = effect(
            &ReactionEvent::new(
                &CRYSTALLIZE,
                Element::Pyro,
                Element::Geo,
                Nat::new(1),
                Nat::new(2),
            ),
            &EffectContext::no_hands(true),
        );
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::Status {
                card: entities::SHIELD,
                target: StatusTarget::Character(TargetScope::SourceActive),
                stacks: Nat::new(1),
                uses: None,
                stack_cap: Some(Nat::new(2)),
            }]
        );
    }
}