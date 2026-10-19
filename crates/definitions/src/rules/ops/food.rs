//! 料理与饱腹（规则书·特殊词条「料理」）。
//!
//! **权威原文（中文）**：
//! 1. 当「料理」事件牌对角色生效后，角色会进入「饱腹」状态。
//! 2. 处于「饱腹」状态下的角色，无法成为其他「料理」事件牌的目标，也无法受到其他「料理」事件牌的效果影响。
//! 3. 「饱腹」状态会在回合结束或角色被击倒时解除。
//! 4. 打出具有「复苏」效果的「料理」事件牌后，本回合内就不能打出其他具有「复苏」效果的「料理」事件牌了。
//!
//! **English（消除歧义）**：Food / Satiated —— "Characters in the Satiated state cannot be targeted by other
//! Food Event Cards nor experience any effects from said Food Event Cards. The Satiated state will be removed
//! at the end of the Round or if the character is defeated."
//!
//! D17：料理只负责赋予饱腹，限制由饱腹自身决定 —— 本模块就是「饱腹」的权威口径。

use gi_genius_invokation_engine::Nat;

/// 每个角色每回合最多食用 1 次料理（卡面括号规则的写法；由饱腹状态导出）。
pub const FOOD_PER_CHARACTER_PER_ROUND: Nat = Nat::new(1);

/// 每回合最多通过料理复苏 1 个角色（规则书第 4 条 + 卡面 333009）。
pub const FOOD_REVIVE_PER_ROUND: Nat = Nat::new(1);

/// 饱腹会在**回合结束**时解除（规则书第 3 条）。
pub const SATIATED_CLEARED_AT_ROUND_END: bool = true;

/// 饱腹会在**角色被击倒**时解除（规则书第 3 条）。
pub const SATIATED_CLEARED_ON_DEFEAT: bool = true;

/// 食用判定结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EatOutcome {
    /// 可以食用（同时会赋予饱腹）。
    Eat,
    /// 已饱腹：本回合该角色不能再食用。
    Fullness,
}

/// 该角色本回合已食用 eaten_this_round 次时的食用判定。
pub fn eat(eaten_this_round: Nat) -> EatOutcome {
    if eaten_this_round < FOOD_PER_CHARACTER_PER_ROUND {
        EatOutcome::Eat
    } else {
        EatOutcome::Fullness
    }
}

/// 处于饱腹状态的角色**不能成为**其他料理事件牌的目标（规则书第 2 条）。
pub fn can_be_targeted_by_food(satiated: bool) -> bool {
    !satiated
}

/// 处于饱腹状态的角色**不会受到**其他料理事件牌的效果影响（规则书第 2 条）。
pub fn can_be_affected_by_food(satiated: bool) -> bool {
    !satiated
}

/// 本回合已通过料理复苏 revived_this_round 个角色时，还能不能复苏（规则书第 4 条）。
pub fn can_revive(revived_this_round: Nat) -> bool {
    revived_this_round < FOOD_REVIVE_PER_ROUND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_food_per_character_per_round() {
        assert_eq!(FOOD_PER_CHARACTER_PER_ROUND, Nat::new(1));
        assert_eq!(eat(Nat::ZERO), EatOutcome::Eat);
        assert_eq!(eat(Nat::new(1)), EatOutcome::Fullness);
        assert_eq!(eat(Nat::new(2)), EatOutcome::Fullness);
    }

    #[test]
    fn satiated_blocks_food_targeting_and_effects() {
        assert!(can_be_targeted_by_food(false));
        assert!(!can_be_targeted_by_food(true));
        assert!(can_be_affected_by_food(false));
        assert!(!can_be_affected_by_food(true));
        const {
            assert!(SATIATED_CLEARED_AT_ROUND_END);
            assert!(SATIATED_CLEARED_ON_DEFEAT);
        }
    }

    #[test]
    fn one_revive_by_food_per_round() {
        assert!(can_revive(Nat::ZERO));
        assert!(!can_revive(Nat::new(1)));
    }
}
