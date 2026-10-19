//! **回合开始流程**：回合数 +1 + 每回合计数清零。
//!
//! ## 依据（只读规则书 `tcgdetailedrules_2lang.json` + D13 的口径修正）
//!
//! - 规则书·回合流程 结束阶段：「3、开始新的回合！」→ 新回合开始时，把**每回合的计数**清零；
//! - 规则书·进行行动 使用技能：技能自带「每回合 N 次」这类上限（`SkillUses`）；
//!   清零点在**回合开始**（`skill_flow::usage::SkillUses::reset_round`，只清「已用次数」，上限保留）；
//! - 规则书·其他规则 对局回合上限：「第15回合开始时，如果仍未分出胜负，则双方都会输掉这场对局」
//!   → 回合数在**回合开始**推进（[`crate::state::terminal::next_round`]，到 15 封顶）；
//!   终局判定本身是纯函数 [`crate::state::terminal::terminal`]，本流程只负责让回合数正确；
//! - **持续回合不在回合开始减**：规则书·特殊词条 持续回合写的是「每个结束阶段…-1」，
//!   所以递减在 [`crate::state::flows::end_phase`]（D13 的旧写法「回合开始清减」已按规则书纠正）。
//!
//! ## 本流程做什么 / 不做什么
//!
//! - 做：**回合数 +1**（第 15 回合封顶）+ 双方 `SkillUses` 清零（规则书只约束「每回合」的
//!   计数，没规定先手先后，故两侧同批做）+ 双方**下落攻击标记清除**（规则书·特殊词条 下落攻击
//!   「本回合内的下一个战斗行动」-> 回合边界必须清）；
//! - 不做：终局判定（纯判定在 `state::terminal`，由调用方在结算完成后调用）、投掷阶段
//!   （骰子生成）/ 行动阶段开始 / 抓 2 张牌 —— 那些各有归属（骰池 / 牌库 / 元素幻变钩子），
//!   本层不越界；
//!
//! ## 纯函数
//!
//! 吃 `&DefState`、返回新 `DefState`，入参一个字节不动。

use crate::state::def_state::DefState;
use crate::state::terminal::next_round;

/// **回合开始**：回合数 +1（到第 15 回合封顶）+ 双方每回合计数清零（技能使用次数；上限保留）。
///
/// 依据：规则书·其他规则 对局回合上限（回合数推进点）+ 规则书·回合流程「开始新的回合！」
/// （每回合计数清零）。
pub fn begin_round(state: &DefState) -> DefState {
    state
        .clone()
        .with_me(
            state
                .me()
                .with_skill_uses_reset()
                .with_plunging_cleared_round(),
        )
        .with_opp(
            state
                .opp()
                .with_skill_uses_reset()
                .with_plunging_cleared_round(),
        )
        .with_round(next_round(state.round()))
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::SkillId;
    use crate::families::character::skill_flow::SkillUses;
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{characters, hand, n};
    use gi_genius_invokation_engine::Nat;

    const NORMAL: SkillId = SkillId::new(12161);

    fn limited_player() -> PlayerDefs {
        // 一个「每回合 1 次」的技能已经用掉 1 次；另外手牌 / 角色区有东西，用来验证别的区没被动过。
        PlayerDefs::new()
            .with_characters(characters(&[201]))
            .with_hand(hand(&[101, 102]))
            .with_skill_uses(SkillUses::new().with_limit(NORMAL, n(1)).recorded(NORMAL))
    }

    #[test]
    fn begin_round_clears_usage_and_keeps_limits_and_other_zones() {
        let before = DefState::from_players(limited_player(), PlayerDefs::new());
        let after = begin_round(&before);
        // 计数清零、上限保留
        assert_eq!(after.me().skill_uses().used_count(NORMAL), Nat::ZERO);
        assert_eq!(after.me().skill_uses().limit(NORMAL), Some(Nat::new(1)));
        assert!(after.me().skill_uses().can_use(NORMAL));
        // 别的区一个字节都没动（手牌 / 角色区）
        assert_eq!(after.me().hand(), before.me().hand());
        assert_eq!(after.me().characters(), before.me().characters());
        // 纯函数：入参不变
        assert_eq!(before.me().skill_uses().used_count(NORMAL), Nat::new(1));
    }

    #[test]
    fn begin_round_advances_the_round_counter() {
        // 建局回合数 0 = 第一回合还没开始；第一次回合开始 -> 1，之后依次 +1。
        let first = begin_round(&DefState::from_players(
            limited_player(),
            PlayerDefs::new(),
        ));
        assert_eq!(first.round(), n(1));
        let second = begin_round(&first);
        assert_eq!(second.round(), n(2));
        // 纯函数：入参的回合数不变
        assert_eq!(first.round(), n(1));
    }

    #[test]
    fn begin_round_stops_at_the_round_limit_and_terminal_judges_it() {
        use crate::state::terminal::{terminal, Outcome};
        // 双方都有角色（避免「全灭」判定抢先于回合上限）
        let before =
            DefState::from_players(limited_player(), limited_player()).with_round(n(14));
        let fifteenth = begin_round(&before);
        assert_eq!(fifteenth.round(), n(15));
        // S7.7：第 15 回合开始时仍未分胜负 -> 双方都输
        assert_eq!(terminal(&fifteenth), Outcome::RoundLimit);
        // 第 15 回合开始即终局：边界之后回合数不再增长
        assert_eq!(begin_round(&fifteenth).round(), n(15));
    }

    #[test]
    fn begin_round_clears_the_plunging_marks_on_both_sides() {
        use crate::families::character::skill_flow::PlungingMark;
        // S8.10 的标记只在「本回合内」有效 -> 回合边界清（规则书·特殊词条 下落攻击）。
        let armed = PlayerDefs::new().with_plunging(PlungingMark::NONE.armed());
        let before = DefState::from_players(armed.clone(), armed);
        let after = begin_round(&before);
        assert!(!after.me().plunging().is_pending());
        assert!(!after.opp().plunging().is_pending());
        // 纯函数：入参的标记还在
        assert!(before.me().plunging().is_pending());
        assert!(before.opp().plunging().is_pending());
    }

    #[test]
    fn begin_round_resets_both_sides_independently() {
        let before = DefState::from_players(limited_player(), limited_player());
        let after = begin_round(&before);
        assert_eq!(after.me().skill_uses().used_count(NORMAL), Nat::ZERO);
        assert_eq!(after.opp().skill_uses().used_count(NORMAL), Nat::ZERO);
        // 两侧是各自独立的值：改一侧不影响另一侧（这里用「重新记录」验证独立寻址）
        let me_recorded = after.me().clone().with_skill_uses(
            after.me().skill_uses().recorded(NORMAL),
        );
        assert_eq!(me_recorded.skill_uses().used_count(NORMAL), Nat::new(1));
        assert_eq!(after.me().skill_uses().used_count(NORMAL), Nat::ZERO);
    }
}
