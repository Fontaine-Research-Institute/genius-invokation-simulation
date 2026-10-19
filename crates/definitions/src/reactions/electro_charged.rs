//! 感电（Electro-Charged）：雷 + 水 -> **本伤害 +1**，对**目标以外的所有敌方角色**
//! 造成 1 点**穿透伤害**。
//!
//! 规则书依据：规则书·元素反应·效果「感电」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_ELECTRO` + `GCG_ELEMENT_HYDRO`）：
//! 「本伤害+1，对目标以外的所有敌方角色造成1点穿透伤害」。
//!
//! 月感电是它的**覆写**（规则书另有一条「月感电」，元素对相同；D14：由卡牌文字
//! 「符合感电条件、触发感电时，改为触发月感电」表达）——本文件只实现基础感电。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::families::character::types::DamageType;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, TargetScope};
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct ElectroCharged;

impl ReactionIdentity for ElectroCharged {
    fn name(&self) -> &'static str {
        "感电"
    }
}

/// 全局唯一的反应身份实例。
pub static ELECTRO_CHARGED: ElectroCharged = ElectroCharged;

/// 触发条件（雷 / 水两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Electro, Element::Hydro);

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 对目标以外敌方角色的穿透伤害（规则书「1点穿透伤害」）。
pub const SPREAD_DAMAGE: Nat = Nat::new(1);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&ELECTRO_CHARGED, &CONDITIONS)
}

/// 效果：+1；对承载方除承载角色外的全部角色造成 1 点穿透伤害。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(
        DAMAGE_BONUS,
        vec![EffectOp::Damage {
            amount: SPREAD_DAMAGE,
            ty: DamageType::Piercing,
            target: TargetScope::TargetOthers,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn electro_charged_adds_one_and_spreads_one_piercing() {
        let event = ReactionEvent::new(
            &ELECTRO_CHARGED,
            Element::Electro,
            Element::Hydro,
            Nat::new(1),
            Nat::new(2),
        );
        let result = effect(&event, &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::Damage {
                amount: Nat::new(1),
                ty: DamageType::Piercing,
                target: TargetScope::TargetOthers,
            }]
        );
    }
}
