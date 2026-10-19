//! 月结晶（Lunar-Crystallize）：水 + 岩 -> **本伤害 +1**，生成 1 张[月笼协奏]加入手牌。
//!
//! 规则书依据：规则书·元素反应·效果「月结晶」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_HYDRO` + `GCG_ELEMENT_GEO`）：
//! 「本伤害+1，并生成1张可以召唤并强化月笼的[月笼协奏]加入手牌」。
//!
//! 与基础结晶（岩 + 冰/水/火/雷）只在水 + 岩这一对**重合**：月结晶只覆写这一对
//! （D14 / D128 / D129：覆写是反应部分级，只替换效果），所以规则表把月结晶放在基础
//! 结晶前面即可命中水 + 岩；其余三对仍走基础结晶（见 `crate::reactions::table`）。
//!
//! 月笼协奏（行动牌 211）自己的「召唤月笼 / 使其效果量+1」属于卡牌实现，不在本文件。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, Side};
use crate::reactions::entities;
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct LunarCrystallize;

impl ReactionIdentity for LunarCrystallize {
    fn name(&self) -> &'static str {
        "月结晶"
    }
}

/// 全局唯一的反应身份实例。
pub static LUNAR_CRYSTALLIZE: LunarCrystallize = LunarCrystallize;

/// 触发条件（水 / 岩两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Hydro, Element::Geo);

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&LUNAR_CRYSTALLIZE, &CONDITIONS)
}

/// 效果：+1；生成 1 张月笼协奏加入触发方手牌。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(
        DAMAGE_BONUS,
        vec![EffectOp::AddCardToHand {
            card: entities::MOONDRIFT_HARMONY,
            side: Side::Source,
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lunar_crystallize_adds_one_and_creates_moondrift_harmony() {
        let event = ReactionEvent::new(
            &LUNAR_CRYSTALLIZE,
            Element::Hydro,
            Element::Geo,
            Nat::new(1),
            Nat::new(2),
        );
        let result = effect(&event, &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::AddCardToHand {
                card: entities::MOONDRIFT_HARMONY,
                side: Side::Source,
            }]
        );
    }
}
