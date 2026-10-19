//! 月感电（Lunar-Charged）：水 + 雷 -> 生成[雷暴云]。
//!
//! 规则书依据：规则书·元素反应·效果「月感电」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_HYDRO` + `GCG_ELEMENT_ELECTRO`）：
//! 「生成[雷暴云]：结束阶段：造成2点雷元素伤害。（可用次数1，可叠加，没有上限）
//! [雷暴云]入场或可用次数增加时，赋予敌方随机1张手牌[电击]」。
//!
//! 本反应**没有「本伤害 +N」**（规则书与关键字 K3001 均无加伤）。
//! 与基础感电是**同一元素对**：月感电是感电的**覆写**（D14 / D128 / D129：转化类反应只注册
//! 效果；由卡牌文字「符合感电条件、触发感电时，改为触发月感电」表达，框架不设
//! 「角色级启用字段」）。所以月反应规则必须排在基础规则**前面**，见 `crate::reactions::table`。
//!
//! 「赋予敌方随机1张手牌[电击]」按 D190 展开成「分支 + 精确概率」，不采样：
//! 每张手牌一支、概率 1/手牌数；手牌为空 -> 只有「生成雷暴云」这一支（概率 1）。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{
    uniform_hand_branches, EffectContext, EffectOp, HandCardStatus, ReactionEffectResult, Side,
};
use crate::reactions::entities;
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct LunarCharged;

impl ReactionIdentity for LunarCharged {
    fn name(&self) -> &'static str {
        "月感电"
    }
}

/// 全局唯一的反应身份实例。
pub static LUNAR_CHARGED: LunarCharged = LunarCharged;

/// 触发条件（水 / 雷两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Hydro, Element::Electro);

/// 雷暴云的初始可用次数（规则书「可用次数1」）。
pub const USES: Nat = Nat::new(1);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&LUNAR_CHARGED, &CONDITIONS)
}

/// 效果：0 加伤；在触发方场上生成 / 叠加雷暴云（无叠加上限）；
/// 同时把「赋予承载方随机 1 张手牌[电击]」展开成手牌下标分支。
pub fn effect(_event: &ReactionEvent, ctx: &EffectContext) -> ReactionEffectResult {
    let summon = EffectOp::Summon {
        card: entities::THUNDERCLOUD,
        side: Side::Source,
        uses: USES,
        stack_limit: None,
    };
    if ctx.target_hand_len.get() == 0 {
        // 手牌为空：没有可赋予的手牌，只剩「生成雷暴云」这一支（概率 1）。
        return ReactionEffectResult::single(Nat::ZERO, vec![summon]);
    }
    ReactionEffectResult {
        damage_bonus: Nat::ZERO,
        branches: uniform_hand_branches(ctx.target_hand_len, |index| {
            vec![
                summon,
                EffectOp::HandCardStatus {
                    status: HandCardStatus::ElectricShock,
                    side: Side::Target,
                    index,
                },
            ]
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
            &LUNAR_CHARGED,
            Element::Hydro,
            Element::Electro,
            Nat::new(1),
            Nat::new(2),
        )
    }

    fn thundercloud() -> EffectOp {
        EffectOp::Summon {
            card: entities::THUNDERCLOUD,
            side: Side::Source,
            uses: Nat::new(1),
            stack_limit: None,
        }
    }

    #[test]
    fn lunar_charged_has_no_damage_bonus_and_creates_thundercloud() {
        let result = effect(&event(), &EffectContext::new(true, Nat::ZERO, Nat::ZERO));
        assert_eq!(result.damage_bonus, Nat::ZERO);
        assert_eq!(result.branches.len(), 1);
        assert_eq!(result.branches[0].ops, vec![thundercloud()]);
        assert!(result.branches[0].probability.is_one());
    }

    #[test]
    fn lunar_charged_expands_random_hand_into_uniform_branches() {
        // 承载方 3 张手牌 -> 3 支，每支概率 1/3；每支都先召唤雷暴云、再给对应下标手牌电击。
        let result = effect(&event(), &EffectContext::new(true, Nat::ZERO, Nat::new(3)));
        assert_eq!(result.branches.len(), 3);
        let expected = Fraction::new(ExactNat128::new(1), ExactNat128::new(3));
        for (index, branch) in result.branches.iter().enumerate() {
            assert_eq!(branch.probability, expected);
            assert_eq!(
                branch.ops,
                vec![
                    thundercloud(),
                    EffectOp::HandCardStatus {
                        status: HandCardStatus::ElectricShock,
                        side: Side::Target,
                        index: Nat::new(index as u32),
                    },
                ]
            );
        }
        // 概率守恒（Σ = 1）：3 × 1/3。
        assert!(result.probability_total().is_one());
    }
}
