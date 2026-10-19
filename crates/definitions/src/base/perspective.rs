//! **视角方绑定**：一局对局里「哪一侧是视角方」的具名两侧（我方 `Me` / 敌方 `Opp`）。
//!
//! 它是**相对侧**的表达，**不是敌我编号、不是下标**（D29 / D34）：卡面写法里写「我方 / 敌方」，
//! 由 `keywords::target::SideScope` 在解析时换成具体的一侧；成对入口
//! （`execute_step_me` / `execute_step_opp`）负责自动绑定视角方。
//!
//! ## 为什么放在 base
//!
//! - 状态层也要用它：先手牌手（[`crate::state::DefState::first_player`]）决定本回合行动 /
//!   结束阶段的起始方（规则书·进行行动 战斗行动【宣布结束】：「先『宣布回合结束』的牌手会成为
//!   『先手牌手』，并在下回合的行动阶段优先行动」；其他规则·结算顺序第 1 条：
//!   「先结算当前牌手的所有卡牌效果」）；
//! - **state 不能依赖 keywords**（分层方向：keywords 建在 state 之上），所以类型放最底层
//!   `base`；`keywords::target` 仍 `pub use` 它，旧路径
//!   `crate::keywords::target::Perspective` 继续可用（不破坏调用方）。

/// **视角方绑定**：只由成对入口（execute_step_me / execute_step_opp）自动填。
///
/// 它是「两个具名实例（me / opp）里哪一个是视角方」的绑定，**不是敌我编号**：
/// 卡面写法里不出现它（写的是 `SideScope::Own` / `SideScope::Opponent`，
/// 由本绑定在解析时换成具体的一侧）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Perspective {
    /// 我方（`DefState::me`）是视角方。
    Me,
    /// 敌方（`DefState::opp`）是视角方。
    Opp,
}

impl Perspective {
    /// 另一方（Me <-> Opp）。
    pub const fn other(self) -> Perspective {
        match self {
            Perspective::Me => Perspective::Opp,
            Perspective::Opp => Perspective::Me,
        }
    }
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn other_flips_between_the_two_named_sides() {
        assert_eq!(Perspective::Me.other(), Perspective::Opp);
        assert_eq!(Perspective::Opp.other(), Perspective::Me);
        // 两次 flips 回到原值
        assert_eq!(Perspective::Me.other().other(), Perspective::Me);
        assert_ne!(Perspective::Me, Perspective::Opp);
    }
}
