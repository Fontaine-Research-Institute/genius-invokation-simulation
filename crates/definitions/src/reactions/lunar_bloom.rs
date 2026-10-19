//! 月绽放（Lunar-Bloom）：水 + 草 -> **本伤害 +1**，赋予我方随机 1 张手牌[费用降低]。
//!
//! 规则书依据：规则书·元素反应·效果「月绽放」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_HYDRO` + `GCG_ELEMENT_DENDRO`）：
//! 「本伤害+1，赋予我方随机1张手牌[费用降低]」。
//!
//! 与基础绽放是**同一元素对**：月绽放是绽放的**覆写**（D14 / D128 / D129），
//! 由卡牌文字表达，框架不设「角色级启用字段」。
//!
//! 「随机 1 张手牌」按 D190 展开成「分支 + 精确概率」，不采样：每张手牌一支、概率 1/手牌数；
//! 手牌为空 -> 只有「+1 伤害」这一支（概率 1）。具体是哪张牌由分支里的下标 + 手牌区
//! 只读指针确定（D183）。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{
    uniform_hand_branches, EffectContext, EffectOp, HandCardStatus, ReactionEffectResult, Side,
};
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct LunarBloom;

impl ReactionIdentity for LunarBloom {
    fn name(&self) -> &'static str {
        "月绽放"
    }
}

/// 全局唯一的反应身份实例。
pub static LUNAR_BLOOM: LunarBloom = LunarBloom;

/// 触发条件（水 / 草两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Hydro, Element::Dendro);

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&LUNAR_BLOOM, &CONDITIONS)
}

/// 效果：+1；把「赋予触发方随机 1 张手牌[费用降低]」展开成手牌下标分支。
pub fn effect(_event: &ReactionEvent, ctx: &EffectContext) -> ReactionEffectResult {
    if ctx.source_hand_len.get() == 0 {
        return ReactionEffectResult::single(DAMAGE_BONUS, Vec::new());
    }
    ReactionEffectResult {
        damage_bonus: DAMAGE_BONUS,
        branches: uniform_hand_branches(ctx.source_hand_len, |index| {
            vec![EffectOp::HandCardStatus {
                status: HandCardStatus::CostReduction,
                side: Side::Source,
                index,
            }]
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gi_genius_invokation_engine::nat::ExactNat128;
    use gi_genius_invokation_engine::prob::Fraction;

    fn event() -> ReactionEvent {
        ReactionEvent::new(
            &LUNAR_BLOOM,
            Element::Hydro,
            Element::Dendro,
            Nat::new(1),
            Nat::new(2),
        )
    }

    #[test]
    fn lunar_bloom_adds_one_and_expands_random_own_hand() {
        let result = effect(&event(), &EffectContext::new(true, Nat::new(2), Nat::ZERO));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(result.branches.len(), 2);
        let expected = Fraction::new(ExactNat128::new(1), ExactNat128::new(2));
        for (index, branch) in result.branches.iter().enumerate() {
            assert_eq!(branch.probability, expected);
            assert_eq!(
                branch.ops,
                vec![EffectOp::HandCardStatus {
                    status: HandCardStatus::CostReduction,
                    side: Side::Source,
                    index: Nat::new(index as u32),
                }]
            );
        }
        assert!(result.probability_total().is_one());
    }

    #[test]
    fn lunar_bloom_with_empty_hand_only_adds_damage() {
        let result = effect(&event(), &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(result.branches.len(), 1);
        assert!(result.branches[0].ops.is_empty());
    }
}
