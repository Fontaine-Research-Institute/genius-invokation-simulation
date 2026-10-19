//! 原激化（Quicken）：雷 + 草 -> **本伤害 +1**，生成[激化领域]。
//!
//! 规则书依据：规则书·元素反应·效果「原激化」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_ELECTRO` + `GCG_ELEMENT_DENDRO`）：
//! 「本伤害+1，生成[激化领域]：我方对敌方出战角色造成雷元素伤害或草元素伤害时，
//! 伤害值+1。（可用次数2）」。
//!
//! 激化领域是**出战状态**（我方场上，[`Side::Source`]）；它自己的加伤规则属于状态牌 117，
//! 不在本文件。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, Side, StatusTarget};
use crate::reactions::entities;
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Quicken;

impl ReactionIdentity for Quicken {
    fn name(&self) -> &'static str {
        "原激化"
    }
}

/// 全局唯一的反应身份实例。
pub static QUICKEN: Quicken = Quicken;

/// 触发条件（雷 / 草两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Electro, Element::Dendro);

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 激化领域的初始可用次数（规则书「可用次数2」）。
pub const USES: Nat = Nat::new(2);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&QUICKEN, &CONDITIONS)
}

/// 效果：+1；在触发方场上生成 1 层激化领域（可用次数 2）。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(
        DAMAGE_BONUS,
        vec![EffectOp::Status {
            card: entities::CATALYZING_FIELD,
            target: StatusTarget::Side(Side::Source),
            stacks: Nat::new(1),
            uses: Some(USES),
            stack_cap: None,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quicken_adds_one_and_creates_catalyzing_field() {
        let event = ReactionEvent::new(
            &QUICKEN,
            Element::Electro,
            Element::Dendro,
            Nat::new(1),
            Nat::new(2),
        );
        let result = effect(&event, &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::Status {
                card: entities::CATALYZING_FIELD,
                target: StatusTarget::Side(Side::Source),
                stacks: Nat::new(1),
                uses: Some(Nat::new(2)),
                stack_cap: None,
            }]
        );
    }
}
