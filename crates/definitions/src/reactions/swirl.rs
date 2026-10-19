//! 扩散（Swirl）：风 + 冰/水/火/雷 -> 对**目标以外的所有敌方角色**造成 1 点**该元素伤害**。
//!
//! 规则书依据：规则书·元素反应·效果「扩散」共 4 条（`elementalreactionseffects.json`）：
//! - `GCG_ELEMENT_CRYO` + `GCG_ELEMENT_ANEMO`：「对目标以外的所有敌方角色造成1点冰元素伤害」；
//! - `GCG_ELEMENT_HYDRO` + `GCG_ELEMENT_ANEMO`：「…1点水元素伤害」；
//! - `GCG_ELEMENT_PYRO` + `GCG_ELEMENT_ANEMO`：「…1点火元素伤害」；
//! - `GCG_ELEMENT_ELECTRO` + `GCG_ELEMENT_ANEMO`：「…1点雷元素伤害」。
//!
//! 4 条规则的效果只差「扩散出去的元素」，该元素 = 命中的**已覆盖元素**（灾风攻击时
//! 附着在目标身上的那个元素），所以**一个反应身份**即可，元素由 `event.covered` 推出。
//! 风 / 岩无法附着（D134），所以风不可能作为「已覆盖元素」出现。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::families::character::types::DamageType;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, TargetScope};
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Swirl;

impl ReactionIdentity for Swirl {
    fn name(&self) -> &'static str {
        "扩散"
    }
}

/// 全局唯一的反应身份实例。
pub static SWIRL: Swirl = Swirl;

/// 触发条件：风 + 冰 / 水 / 火 / 雷，每种两种先后（D132）共 8 条。
pub static CONDITIONS: [ReactionCondition; 8] = [
    ReactionCondition::new(Element::Cryo, Element::Anemo),
    ReactionCondition::new(Element::Anemo, Element::Cryo),
    ReactionCondition::new(Element::Hydro, Element::Anemo),
    ReactionCondition::new(Element::Anemo, Element::Hydro),
    ReactionCondition::new(Element::Pyro, Element::Anemo),
    ReactionCondition::new(Element::Anemo, Element::Pyro),
    ReactionCondition::new(Element::Electro, Element::Anemo),
    ReactionCondition::new(Element::Anemo, Element::Electro),
];

/// 对目标以外敌方角色的扩散伤害（规则书「1点」）。
pub const SPREAD_DAMAGE: Nat = Nat::new(1);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&SWIRL, &CONDITIONS)
}

/// 效果：0 加伤；对承载方除承载角色外的全部角色造成 1 点已覆盖元素伤害。
///
/// 已覆盖元素只可能是冰 / 水 / 火 / 雷；其它值（构造异常）按「不扩散」处理。
pub fn effect(event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    let element = match event.covered {
        Element::Cryo | Element::Hydro | Element::Pyro | Element::Electro => event.covered,
        // 草 + 风不是扩散（规则书没有这一条）；草 / 风 / 岩都不可能是扩散的已覆盖元素。
        Element::Dendro | Element::Anemo | Element::Geo => {
            return ReactionEffectResult::single(Nat::ZERO, Vec::new())
        }
    };
    ReactionEffectResult::single(
        Nat::ZERO,
        vec![EffectOp::Damage {
            amount: SPREAD_DAMAGE,
            ty: DamageType::Elemental(element),
            target: TargetScope::TargetOthers,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(covered: Element) -> ReactionEvent {
        ReactionEvent::new(&SWIRL, covered, Element::Anemo, Nat::new(1), Nat::new(2))
    }

    #[test]
    fn swirl_spreads_one_damage_of_the_covered_element() {
        for (covered, expected) in [
            (Element::Cryo, DamageType::Elemental(Element::Cryo)),
            (Element::Hydro, DamageType::Elemental(Element::Hydro)),
            (Element::Pyro, DamageType::Elemental(Element::Pyro)),
            (Element::Electro, DamageType::Elemental(Element::Electro)),
        ] {
            let result = effect(&event(covered), &EffectContext::no_hands(true));
            assert_eq!(result.damage_bonus, Nat::ZERO);
            assert_eq!(
                result.branches[0].ops,
                vec![EffectOp::Damage {
                    amount: Nat::new(1),
                    ty: expected,
                    target: TargetScope::TargetOthers,
                }]
            );
        }
    }

    #[test]
    fn swirl_has_no_bonus_and_eight_conditions() {
        assert_eq!(CONDITIONS.len(), 8);
        // 风不可能作为已覆盖元素（D134）：异常输入 -> 不扩散。
        let abnormal = effect(&event(Element::Anemo), &EffectContext::no_hands(true));
        assert!(abnormal.branches[0].ops.is_empty());
    }
}