//! **终局判定接线**（规则书 S1.4 胜利条件 / S7.7 对局回合上限）：`state::terminal` 的只读薄入口。
//!
//! ## 依据（唯一权威）
//!
//! - 规则书·胜利条件（S1.4）：「击倒对方牌手的所有『角色牌』，即可获得胜利！」
//! - 规则书·其他规则 对局回合上限（S7.7）：「第15回合开始时，如果仍未分出胜负，则双方都会输掉
//!   这场对局。」
//! - 判定本体：`state::terminal::{terminal, Outcome}`（纯函数、只读 `&DefState`；含
//!   `ROUND_LIMIT = 15`、双方同时全灭的未定规则占位）。
//!
//! ## 调用时点（重要；与 state/terminal.rs 的口径一致）
//!
//! 终局判定必须在**每步结算 + 击倒清理完成之后**调用：
//!
//! 1. 技能 / 效果结算完成（`flow::{skill_use, play_card}` 的步末 flush 之后）；
//! 2. 击倒清理（`state::flows::defeat` 的 `defeat_me/opp`）执行之后 —— 规则书·其他规则
//!    击倒：「生命值被降为 0，则在技能或效果结算完成后，角色就会被『击倒』」→ HP 归 0 但
//!    清理还没跑时角色仍在角色区，[terminal] 会如实地判 `Outcome::Ongoing`。
//!
//! **击倒清理通过注入点完成**（D255 / D270 / D272）：清理本体 `defeat_me/opp` 需要按角色给
//! `DefeatHpReset{定义 max_hp, GrowthList}`，角色定义 / 成长聚合不在 definitions 层，
//! flow 拿不到；所以 flow 在 [crate::flow::hooks] 提供 `DefeatCleanup`
//! （`fn(&DefState, Perspective) -> (DefState, PendingSet)`）**回调注入点**，由上层（cards）
//! 注入真实实现。默认 [crate::flow::hooks::NO_DEFEAT_CLEANUP] 是 **no-op**（不清理），
//! 此时本函数判的就是清理前局面。
//!
//! **调用位置**：`flow::skill_use::use_skill_*` 与 `flow::play_card::play_card_*` 在
//! **每支结算 + 步末 flush 之后、调用 [settlement_outcome] 之前**调用注入的清理
//! （`apply_defeat_cleanup`，先 me 后 opp）；顺序固定为「结算 -> 清理 -> 判终局」
//! （D255 / D270 / D272）。因此：
//! - 注入真实清理实现 -> 分支 `outcome` 是**清理后**判定；
//! - 默认 `NO_DEFEAT_CLEANUP` -> 分支 `outcome` 是**清理前**判定（HP 归 0 的角色仍在
//!   角色区，如实判 `Ongoing`）。
//!
//! ## 回合流程里的接入点
//!
//! - [crate::flow::turn::round_begin] / [crate::flow::turn::begin_next_round]：回合上限（S7.7）；
//! - [crate::flow::turn::TurnEndBranch]：结束阶段分支（尚未接入 `DefeatCleanup`，击倒清理
//!   仍由调用方在终局前做）；
//! - [crate::flow::skill_use::SkillBranch] / [crate::flow::play_card::PlayBranch]：技能 / 打牌
//!   分支的终局判定，已按上面的注入顺序在每支结算 + 清理之后做；调用方不要再自行清理一次。
//!
//! ## TODO（规则书未写清，不自行规定）
//!
//! - **双方同时全灭的胜负**：规则书没写；`state::terminal` 给
//!   `Outcome::BothLose` 独立结果，双方都输（双负，D267/D268）（本层原样转出）；
//! - **复苏 / 复活后是否重判**：见 `state/terminal.rs` 的待实测条目。

pub use crate::state::terminal::Outcome;

use crate::state::terminal::terminal;
use crate::state::DefState;

/// **每步结算 + 击倒清理之后**的终局判定（规则书 S1.4 / S7.7）。
///
/// 纯读取：只借 `&DefState`、不改任何区。调用时点见模块文档（必须先完成结算与
/// `state::flows::defeat` 清理；否则 HP 已 0 但角色仍在区里的局面会被判成 `Ongoing`）。
/// 技能 / 打牌入口已通过 `DefeatCleanup` 注入完成清理；直接调用本函数时由调用方保证顺序。
pub fn settlement_outcome(state: &DefState) -> Outcome {
    terminal(state)
}

/// 这局是否已经终局（[Outcome::Ongoing] 之外都是；含回合上限 / 同时全灭）。
pub fn is_terminal(state: &DefState) -> bool {
    settlement_outcome(state).is_terminal()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{characters, n};

    #[test]
    fn settlement_outcome_reads_the_state_without_changing_it() {
        // 「击倒清理已完成」的局面：对方角色区已空、回合数 > 0 -> 我方胜。
        let won = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new(),
        )
        .with_round(n(1));
        let snapshot = won.clone();
        assert_eq!(settlement_outcome(&won), Outcome::MeWins);
        assert!(is_terminal(&won));
        // 纯读取：一个字节都不改
        assert_eq!(won, snapshot);
    }

    #[test]
    fn unstarted_or_ongoing_games_are_not_terminal() {
        // 回合数 0 = 第一回合还没开始：不判终局。
        assert_eq!(settlement_outcome(&DefState::new()), Outcome::Ongoing);
        assert!(!is_terminal(&DefState::new()));
        // 双方都有角色、未到 15 回合 -> Ongoing。
        let alive = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new().with_characters(characters(&[401])),
        )
        .with_round(n(1));
        assert_eq!(settlement_outcome(&alive), Outcome::Ongoing);
    }
}
