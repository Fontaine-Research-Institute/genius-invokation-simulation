//! 可用次数 / 持续回合（规则书·特殊词条「可用次数」「持续回合」）。
//!
//! **权威原文（中文）**：
//! - 可用次数：每次触发这张牌的效果，都会消耗 1 次「可用次数」；耗尽后，会立刻弃置这张牌。
//! - 持续回合：每个**结束阶段**，都会使「持续回合」-1；耗尽后，会立刻弃置这张牌。
//!
//! **English（消除歧义）**：Usage —— "Each time the effect of such a card is triggered, 1 Usage will be consumed.
//! Once a card has 0 Usages remaining, it will be discarded immediately."；
//! Duration (Rounds) —— "Each End Phase, 1 Round will be removed from such a card's Duration."
//!
//! 状态 / 区域由各自块持有（// 待接入 timeline 的结束阶段）；本模块只给纯计算。

use gi_genius_invokation_engine::{nsub, Nat};

/// 每次触发消耗的可用次数（规则书：1 次）。
pub const USES_PER_TRIGGER: Nat = Nat::new(1);

/// 每个结束阶段扣减的持续回合（规则书：1）。
pub const DURATION_PER_END_PHASE: Nat = Nat::new(1);

/// 触发一次后的可用次数（减到 0 为止；调用方随后弃置）。
pub fn uses_after_trigger(uses: Nat) -> Nat {
    nsub!(uses, USES_PER_TRIGGER)
}

/// 是否耗尽（= 0 -> 立刻弃置）。
pub fn is_depleted(value: Nat) -> bool {
    value == Nat::ZERO
}

/// 经过一个结束阶段后的持续回合（减到 0 为止）。
pub fn duration_after_end_phase(duration: Nat) -> Nat {
    nsub!(duration, DURATION_PER_END_PHASE)
}

/// 持续回合的递减发生在**结束阶段**（不是回合开始、也不是行动阶段开始）。
///
/// 规则书原文：「每个结束阶段，都会使『持续回合』-1」；英文 "Each End Phase"。
/// 注意：这与决策日志 D13 的旧写法（回合开始递减）冲突，已按规则书为准改口径。
pub fn ticks_in_end_phase() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_ticks_per_trigger_and_discards_at_zero() {
        assert_eq!(USES_PER_TRIGGER, Nat::new(1));
        assert_eq!(uses_after_trigger(Nat::new(3)), Nat::new(2));
        assert_eq!(uses_after_trigger(Nat::new(1)), Nat::ZERO);
        // 已经 0 了再触发也不会变负
        assert_eq!(uses_after_trigger(Nat::ZERO), Nat::ZERO);
        assert!(is_depleted(Nat::ZERO));
        assert!(!is_depleted(Nat::new(1)));
    }

    #[test]
    fn duration_ticks_at_end_phase_only() {
        assert_eq!(DURATION_PER_END_PHASE, Nat::new(1));
        assert_eq!(duration_after_end_phase(Nat::new(2)), Nat::new(1));
        assert_eq!(duration_after_end_phase(Nat::new(1)), Nat::ZERO);
        assert_eq!(duration_after_end_phase(Nat::ZERO), Nat::ZERO);
        assert!(ticks_in_end_phase());
    }
}
