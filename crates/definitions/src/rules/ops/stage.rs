//! 行动权与回合流程（规则书·进行行动 / 回合流程 + 特殊词条「战斗行动（行动牌）」）。
//!
//! **权威原文（中文）**：
//! - 进行行动：「行动」分「战斗行动」和「快速行动」；完成战斗行动后轮到对方行动，
//!   完成快速行动后我方可以继续行动。「使用技能」「切换角色」「宣布结束」是战斗行动；
//!   「打出手牌」「元素调和」是快速行动。
//! - 特殊词条 战斗行动（行动牌）：打出带此规则的手牌，视为进行战斗行动，而非通常的快速行动。
//! - 回合流程：投掷阶段 -> 行动阶段 -> 结束阶段；投掷阶段每回合投 8 个元素骰、1 次重投；
//!   结束阶段：从先手开始处理结束阶段效果，然后双方各抓 2 张牌。
//!
//! **English（消除歧义）**：Take Action —— "Using Skills, switching characters, and declaring Round end
//! count as Combat Actions." / "Playing a card and Elemental Tuning are considered Fast Actions."
//! Round Flow —— Roll Phase > Action Phase > End Phase。
//!
//! 回合 / 阶段调度归后续 timeline 块（// 待接入）；本模块只给纯判定与规则数字。

use gi_genius_invokation_engine::Nat;

/// 一次行动的行动权类型。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Timing {
    /// 战斗行动：结算后轮到对方（规则书·进行行动）。
    Combat,
    /// 快速行动：结算后我方可以继续行动（规则书·进行行动）。
    Fast,
}

/// 行动种类（规则书·进行行动列出的 5 类行动）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionKind {
    /// 使用技能：战斗行动。
    UseSkill,
    /// 切换角色：战斗行动（支付 1 个任意元素骰）。
    SwitchCharacter,
    /// 宣布回合结束：战斗行动。
    DeclareRoundEnd,
    /// 打出手牌：快速行动（带「战斗行动」规则的牌除外）。
    PlayCard,
    /// 元素调和：快速行动（丢 1 张手牌，转 1 个骰为出战角色元素类型）。
    ElementalTuning,
}

/// 回合的三个阶段（规则书·回合流程）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoundPhase {
    /// 投掷阶段。
    Roll,
    /// 行动阶段。
    Action,
    /// 结束阶段。
    End,
}

/// 回合流程顺序（规则书·回合流程）。
pub const ROUND_PHASES: [RoundPhase; 3] = [
    RoundPhase::Roll,
    RoundPhase::Action,
    RoundPhase::End,
];

/// 投掷阶段投掷的元素骰个数（规则书：8）。
pub const DICE_PER_ROLL: Nat = Nat::new(8);

/// 投掷阶段的重投机会（规则书：1 次）。
pub const REROLL_CHANCES: Nat = Nat::new(1);

/// 结束阶段双方各抓的牌数（规则书：2 张）。
pub const END_PHASE_DRAW: Nat = Nat::new(2);

/// 这次行动结算后是否轮到对方（规则书：战斗行动 -> 轮到对方；快速行动 -> 继续）。
pub fn passes_turn(timing: Timing) -> bool {
    match timing {
        Timing::Combat => true,
        Timing::Fast => false,
    }
}

/// 按规则书给「行动种类」定行动权（使用技能 / 切换角色 / 宣布结束 = 战斗行动）。
pub fn timing_of_action(kind: ActionKind) -> Timing {
    match kind {
        ActionKind::UseSkill | ActionKind::SwitchCharacter | ActionKind::DeclareRoundEnd => {
            Timing::Combat
        }
        ActionKind::PlayCard | ActionKind::ElementalTuning => Timing::Fast,
    }
}

/// 这次行动是不是战斗行动。
pub fn is_combat_action(kind: ActionKind) -> bool {
    passes_turn(timing_of_action(kind))
}

/// 打出一张**行动牌**的行动权：默认快速行动；带「战斗行动」标签时改为战斗行动
/// （规则书·特殊词条 战斗行动（行动牌））。
pub fn timing_of_card_play(has_combat_action_tag: bool) -> Timing {
    if has_combat_action_tag {
        Timing::Combat
    } else {
        Timing::Fast
    }
}

/// 下一个阶段（结束阶段之后回到投掷阶段）。
pub fn next_phase(phase: RoundPhase) -> RoundPhase {
    match phase {
        RoundPhase::Roll => RoundPhase::Action,
        RoundPhase::Action => RoundPhase::End,
        RoundPhase::End => RoundPhase::Roll,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_action_passes_turn_fast_does_not() {
        assert!(passes_turn(Timing::Combat));
        assert!(!passes_turn(Timing::Fast));
    }

    #[test]
    fn action_kinds_follow_rulebook() {
        // 规则书·进行行动：使用技能 / 切换角色 / 宣布结束 = 战斗行动；
        // 打出手牌 / 元素调和 = 快速行动。
        assert!(is_combat_action(ActionKind::UseSkill));
        assert!(is_combat_action(ActionKind::SwitchCharacter));
        assert!(is_combat_action(ActionKind::DeclareRoundEnd));
        assert!(!is_combat_action(ActionKind::PlayCard));
        assert!(!is_combat_action(ActionKind::ElementalTuning));
    }

    #[test]
    fn card_play_is_fast_unless_combat_action_tag() {
        assert_eq!(timing_of_card_play(false), Timing::Fast);
        assert_eq!(timing_of_card_play(true), Timing::Combat);
    }

    #[test]
    fn round_flows_roll_action_end_and_back() {
        assert_eq!(
            ROUND_PHASES,
            [RoundPhase::Roll, RoundPhase::Action, RoundPhase::End]
        );
        assert_eq!(next_phase(RoundPhase::Roll), RoundPhase::Action);
        assert_eq!(next_phase(RoundPhase::Action), RoundPhase::End);
        assert_eq!(next_phase(RoundPhase::End), RoundPhase::Roll);
        assert_eq!(DICE_PER_ROLL, Nat::new(8));
        assert_eq!(REROLL_CHANCES, Nat::new(1));
        assert_eq!(END_PHASE_DRAW, Nat::new(2));
    }
}
