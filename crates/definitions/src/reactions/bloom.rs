//! 绽放（Bloom）：水 + 草 -> **本伤害 +1**，生成[草原核]。
//!
//! 规则书依据：规则书·元素反应·效果「绽放」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_HYDRO` + `GCG_ELEMENT_DENDRO`）：
//! 「本伤害+1，生成[草原核]：我方对敌方出战角色造成火元素伤害或雷元素伤害时，
//! 伤害值+2。（可用次数1）」。
//!
//! 草原核是**出战状态**（我方场上，[`Side::Source`]）；它自己的加伤规则属于状态牌 116，
//! 不在本文件。月绽放是它的**覆写**（同元素对，D14 由卡牌文字表达）。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, Side, StatusTarget};
use crate::reactions::entities;
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Bloom;

impl ReactionIdentity for Bloom {
    fn name(&self) -> &'static str {
        "绽放"
    }
}

/// 全局唯一的反应身份实例。
pub static BLOOM: Bloom = Bloom;

/// 触发条件（水 / 草两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Hydro, Element::Dendro);

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 草原核的初始可用次数（规则书「可用次数1」）。
pub const USES: Nat = Nat::new(1);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&BLOOM, &CONDITIONS)
}

/// 效果：+1；在触发方场上生成 1 层草原核（可用次数 1）。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(
        DAMAGE_BONUS,
        vec![EffectOp::Status {
            card: entities::DENDRO_CORE,
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
    fn bloom_adds_one_and_creates_dendro_core() {
        let event =
            ReactionEvent::new(&BLOOM, Element::Hydro, Element::Dendro, Nat::new(1), Nat::new(2));
        let result = effect(&event, &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::Status {
                card: entities::DENDRO_CORE,
                target: StatusTarget::Side(Side::Source),
                stacks: Nat::new(1),
                uses: Some(Nat::new(1)),
                stack_cap: None,
            }]
        );
    }
}
