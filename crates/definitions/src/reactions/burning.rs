//! 燃烧（Burning）：火 + 草 -> **本伤害 +1**，生成[燃烧烈焰]。
//!
//! 规则书依据：规则书·元素反应·效果「燃烧」（`elementalreactionseffects.json`；
//! 元素对 `GCG_ELEMENT_PYRO` + `GCG_ELEMENT_DENDRO`）：
//! 「本伤害+1，生成[燃烧烈焰]：结束阶段：造成1点火元素伤害。（可用次数1，最多叠加到2）」。
//!
//! 召唤物自己的「结束阶段造成1点火元素伤害」属于召唤物牌（115）的实现，不在本文件；
//! 这里只产出「生成 / 叠加燃烧烈焰」操作（召唤物区，**待接入**）。

use crate::base::Element;
use crate::families::character::attachment::{ReactionCondition, ReactionEvent, ReactionIdentity};
use crate::families::character::damage::ReactionRule;
use crate::reactions::effect::{EffectContext, EffectOp, ReactionEffectResult, Side};
use crate::reactions::entities;
use gi_genius_invokation_engine::Nat;

/// 反应身份。
#[derive(Debug)]
pub struct Burning;

impl ReactionIdentity for Burning {
    fn name(&self) -> &'static str {
        "燃烧"
    }
}

/// 全局唯一的反应身份实例。
pub static BURNING: Burning = Burning;

/// 触发条件（火 / 草两种先后）。
pub static CONDITIONS: [ReactionCondition; 2] =
    ReactionCondition::both_orders(Element::Pyro, Element::Dendro);

/// 本伤害加成（规则书「本伤害+1」）。
pub const DAMAGE_BONUS: Nat = Nat::new(1);

/// 燃烧烈焰的初始可用次数（规则书「可用次数1」）。
pub const USES: Nat = Nat::new(1);

/// 燃烧烈焰的叠加上限（规则书「最多叠加到2」）。
pub const STACK_LIMIT: Nat = Nat::new(2);

/// 规则表条目。
pub fn rule() -> ReactionRule {
    ReactionRule::new(&BURNING, &CONDITIONS)
}

/// 效果：+1；在**触发方**场上生成 / 叠加燃烧烈焰（可叠到 2）。
pub fn effect(_event: &ReactionEvent, _ctx: &EffectContext) -> ReactionEffectResult {
    ReactionEffectResult::single(
        DAMAGE_BONUS,
        vec![EffectOp::Summon {
            card: entities::BURNING_FLAME,
            side: Side::Source,
            uses: USES,
            stack_limit: Some(STACK_LIMIT),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burning_adds_one_and_creates_burning_flame_on_the_source_side() {
        let event =
            ReactionEvent::new(&BURNING, Element::Pyro, Element::Dendro, Nat::new(1), Nat::new(2));
        let result = effect(&event, &EffectContext::no_hands(true));
        assert_eq!(result.damage_bonus, Nat::new(1));
        assert_eq!(
            result.branches[0].ops,
            vec![EffectOp::Summon {
                card: entities::BURNING_FLAME,
                side: Side::Source,
                uses: Nat::new(1),
                stack_limit: Some(Nat::new(2)),
            }]
        );
    }
}
