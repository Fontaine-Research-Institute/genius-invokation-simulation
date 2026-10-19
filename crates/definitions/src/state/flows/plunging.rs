//! **下落攻击标记的自动清除点**（S8.10）。
//!
//! 规则书·特殊词条 下落攻击：「角色被切换为『出战角色』后，本回合内的下一个战斗行动若为
//! 『普通攻击』，则被视为『下落攻击』。」
//!
//! ## 持有区与三个读写点
//!
//! - 标记值：`PlungingMark`（纯值，`families/character/skill_flow/attack_form.rs`）；
//! - **持有区**：`PlayerDefs::plunging`（每方一个，D34；`state/player.rs`）；
//! - **置位**：切换角色成功后，把 flow 返回的 `SwitchCharacterOutcome::plunging` 用
//!   `PlayerDefs::with_plunging` 写回（flow 现有的**参数 / 返回值**传法保持不变）；
//! - **清除点 1（本模块）**：一次战斗行动结束 -> `battle_action_end_me/opp`（配对入口，D35）；
//! - **清除点 2（回合边界）**：`state::flows::round_begin::begin_round` 自动调
//!   `PlayerDefs::with_plunging_cleared_round`。
//!
//! ## 口径 / TODO（规则书没写清，不自行规定）
//!
//! - 「一次战斗行动」= 使用技能 / 切换角色 / 宣布结束（规则书·进行行动）；本模块只提供
//!   **按侧清除**的入口，「什么时候算一次战斗行动结束」由 flow / 上层决定并调用
//!   （flow 尚未把标记写进根状态，仍以参数 / 返回值传递，见 `flow/mod.rs` 的 TODO）；
//! - 非普通攻击的战斗行动是否保留下落标记：规则书没写 -> 现按「一次战斗行动结束即清」
//!   （留空 #50）；
//! - 多次切换是否刷新标记：布尔状态，置位与保留等价（留空 #60）。

use crate::state::def_state::DefState;

/// **我方的一次战斗行动结束**：清除我方的下落攻击标记（S8.10；纯函数）。
pub fn battle_action_end_me(state: &DefState) -> DefState {
    state
        .clone()
        .with_me(state.me().with_plunging_after_combat_action())
}

/// **敌方的一次战斗行动结束**：清除敌方的下落攻击标记（S8.10；纯函数；D35 共用实现）。
pub fn battle_action_end_opp(state: &DefState) -> DefState {
    state
        .clone()
        .with_opp(state.opp().with_plunging_after_combat_action())
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::skill_flow::PlungingMark;
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{characters, n};

    /// 双方都持有「待消耗」的下落攻击标记（各自切换过角色）。
    fn both_armed() -> DefState {
        let me = PlayerDefs::new()
            .with_characters(characters(&[201]))
            .with_plunging(PlungingMark::NONE.armed());
        let opp = PlayerDefs::new()
            .with_characters(characters(&[401]))
            .with_plunging(PlungingMark::NONE.armed());
        DefState::from_players(me, opp).with_round(n(2))
    }

    #[test]
    fn battle_action_end_clears_only_the_side_that_acted() {
        let before = both_armed();
        let after_me = battle_action_end_me(&before);
        assert!(!after_me.me().plunging().is_pending());
        // 另一侧不动
        assert!(after_me.opp().plunging().is_pending());
        let after_opp = battle_action_end_opp(&before);
        assert!(after_opp.me().plunging().is_pending());
        assert!(!after_opp.opp().plunging().is_pending());
        // 纯函数：入参的标记不变
        assert!(before.me().plunging().is_pending());
        assert!(before.opp().plunging().is_pending());
    }

    #[test]
    fn battle_action_end_is_a_noop_without_a_mark() {
        let plain = DefState::from_players(PlayerDefs::new(), PlayerDefs::new());
        assert_eq!(battle_action_end_me(&plain), plain);
        assert_eq!(battle_action_end_opp(&plain), plain);
    }
}
