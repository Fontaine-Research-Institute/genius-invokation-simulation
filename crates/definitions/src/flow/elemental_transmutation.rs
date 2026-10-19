//! **元素幻变（K67）的回合流程钩子**（L9）：判定 / 转换原语在 `rules::ops::opening`，
//! 本文件把它们接成 flow 层**可调用入口**。
//!
//! ## 依据
//!
//! - K67「元素幻变」+ 卡面 331004–331012（覆盖报告 round3 L9：依据是关键字 + 卡面，
//!   **规则书 S1–S8 没有该词条**；权威顺序 RULES §三.3 下属于第 2 档）；
//! - 原语：`ElementalTransmutation{team_matches, roll_phase_dice, convert_opening_dice,
//!   apply_roll_phase, opening_steps}`、`is_opening_round`；
//! - 「投掷 2+2」与「首次生成」的**时序待实测**（`留空.md#15` / #64）：所以本层**不**把钩子
//!   接进 `roll_phase` / `round_begin` / `roll_me` / `roll_opp`（接进去会改变现有流程结果）；
//!   先给可调用入口，由调用方在观测到骰池之后决定何时调。
//!
//! ## 两个钩子（成对入口，D34）
//!
//! - [transmute_roll_phase_me] / [transmute_roll_phase_opp]：**投掷阶段（每轮，牌在场）**
//!   「总是投出 2 个 X + 2 个 Y」—— 在 `flow::turn::roll_me/roll_opp` 的某一支骰池
//!   **观测完成之后**调用（骰子不足 4 -> None、不改局面）；
//! - [transmute_opening_me] / [transmute_opening_opp]：**「首次生成」= 第 1 轮**（K67 / 留空 #64）；
//!   复用同一「4 骰 -> 2+2」转换，并在非第 1 轮返回 None（`is_opening_round`）。
//!
//! ## 口径
//!
//! - **纯函数**：吃 `&DefState`、返回新 `DefState`；失败返回 None、入参一个字节不改；
//! - **不硬编码具体卡**：哪两种元素、队伍是否满足「恰等于 {X,Y}」（K23 / `team_matches`）
//!   由卡牌层 / 调用方给（rules 层只做通用判定）；
//! - 折叠组（`DicePool::folded`）：`convert` 不使用折叠组（D144）；调用方应先把骰池
//!   **观测展开**成具体骰面再接本钩子（roll_me/roll_opp 的分支已满足）。
//!
//! ## TODO / 待实测（规则书没写清，不自行规定）
//!
//! 1. **时序**：投掷阶段的 2+2 在「投掷 8 骰之后 / 重投之前」还是「重投之后」？
//!    「首次生成」在「抓牌 -> 弃牌 -> 上场」的哪一步之后？（留空 #15 / #64，待实测）；
//! 2. **触发条件**：牌在支援区 / 手牌 / 牌库时是否都生效（卡面「牌在场时」），
//!    本层不做区域判定（调用方 / 卡牌层给）；
//! 3. **牌库中的形态**：`opening_steps` 里「从牌库抓起 / 弃置行动牌形态 / 生成到支援区」的
//!    区域搬运不属 flow 本模块（需要卡牌层 + 区域操作），本文件只接骰子转换两步。

use crate::rules::ops::opening::{is_opening_round, ElementalTransmutation};
use crate::state::DefState;

/// 把某一方的骰池写回（纯函数）。
fn with_dice(state: &DefState, side_me: bool, dice: crate::resources::dice::DicePool) -> DefState {
    if side_me {
        state.clone().with_me(state.me().clone().with_dice(dice))
    } else {
        state.clone().with_opp(state.opp().clone().with_dice(dice))
    }
}

/// 某一方骰池的只读快照（只为避免重复借用）。
fn dice_of(state: &DefState, side_me: bool) -> crate::resources::dice::DicePool {
    if side_me {
        state.me().dice().clone()
    } else {
        state.opp().dice().clone()
    }
}

/// **投掷阶段 2+2 转换（我方）**：把 4 颗骰转成该卡两元素各 2 颗（总数不变）。
///
/// 调用时点：`flow::turn::roll_me` 的某一支骰池观测完成之后（本钩子不改 roll 流程）。
/// 骰子不足 4 -> None（不改入参）。
pub fn transmute_roll_phase_me(state: &DefState, rule: ElementalTransmutation) -> Option<DefState> {
    let converted = rule.apply_roll_phase(&dice_of(state, true))?;
    Some(with_dice(state, true, converted))
}

/// **投掷阶段 2+2 转换（敌方）**（与 [transmute_roll_phase_me] 共用一份实现，D35）。
pub fn transmute_roll_phase_opp(
    state: &DefState,
    rule: ElementalTransmutation,
) -> Option<DefState> {
    let converted = rule.apply_roll_phase(&dice_of(state, false))?;
    Some(with_dice(state, false, converted))
}

/// **「首次生成」4 骰 -> 2+2（我方）**：只在**第 1 轮**生效（K67 / 留空 #64 的判据）。
///
/// 调用时点：第 1 轮行动阶段开始时（具体在抓牌 / 弃牌 / 上场前后的哪一步待实测，见模块 TODO）。
/// 非第 1 轮 / 骰子不足 4 -> None（不改入参）。
pub fn transmute_opening_me(state: &DefState, rule: ElementalTransmutation) -> Option<DefState> {
    if !is_opening_round(state.round()) {
        return None;
    }
    let converted = rule.convert_opening_dice(&dice_of(state, true))?;
    Some(with_dice(state, true, converted))
}

/// **「首次生成」4 骰 -> 2+2（敌方）**（与 [transmute_opening_me] 共用一份实现，D35）。
pub fn transmute_opening_opp(state: &DefState, rule: ElementalTransmutation) -> Option<DefState> {
    if !is_opening_round(state.round()) {
        return None;
    }
    let converted = rule.convert_opening_dice(&dice_of(state, false))?;
    Some(with_dice(state, false, converted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::resources::dice::{DicePool, DieFace};
    use crate::state::player::PlayerDefs;
    use crate::state::testing::{characters, n};

    fn superconduct() -> ElementalTransmutation {
        // 331004 元素幻变：超导祝佑 = 冰 + 雷（顺序不影响规则）。
        ElementalTransmutation::from_elements([Element::Cryo, Element::Electro])
    }

    fn four_pyro() -> DicePool {
        DicePool::empty().add(DieFace::Pyro, n(4))
    }

    fn state_with_dice(round: u32, me: DicePool, opp: DicePool) -> DefState {
        DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_dice(me),
            PlayerDefs::new()
                .with_characters(characters(&[401]))
                .with_dice(opp),
        )
        .with_round(n(round))
    }

    fn two_two() -> DicePool {
        DicePool::empty()
            .add(DieFace::from_element(Element::Cryo), n(2))
            .add(DieFace::from_element(Element::Electro), n(2))
    }

    #[test]
    fn roll_phase_hook_turns_four_dice_into_two_plus_two() {
        let before = state_with_dice(2, four_pyro(), four_pyro());
        let after = transmute_roll_phase_me(&before, superconduct()).expect("够 4 颗");
        assert_eq!(after.me().dice(), &two_two());
        assert_eq!(after.me().dice().total(), n(4));
        // 敌方不动；纯函数：入参一个字节不改。
        assert_eq!(after.opp().dice(), &four_pyro());
        assert_eq!(before.me().dice(), &four_pyro());
        // 对方侧入口同理。
        let after_opp = transmute_roll_phase_opp(&before, superconduct()).expect("够 4 颗");
        assert_eq!(after_opp.opp().dice(), &two_two());
        assert_eq!(after_opp.me().dice(), &four_pyro());
    }

    #[test]
    fn roll_phase_hook_rejects_fewer_than_four_dice_without_changing_state() {
        let short = state_with_dice(
            2,
            DicePool::empty().add(DieFace::Pyro, n(3)),
            DicePool::empty(),
        );
        assert!(transmute_roll_phase_me(&short, superconduct()).is_none());
        assert_eq!(short.me().dice().total(), n(3));
    }

    #[test]
    fn opening_hook_only_applies_in_round_one() {
        // 第 1 轮：可以转换。
        let opening = state_with_dice(1, four_pyro(), four_pyro());
        let after = transmute_opening_me(&opening, superconduct()).expect("第 1 轮");
        assert_eq!(after.me().dice(), &two_two());
        // 非第 1 轮：None、不改局面。
        let later = state_with_dice(2, four_pyro(), four_pyro());
        assert!(transmute_opening_me(&later, superconduct()).is_none());
        assert_eq!(later.me().dice(), &four_pyro());
    }

    #[test]
    fn hook_is_additive_and_does_not_change_existing_flow_results() {
        // 钩子只返回新局、不改入参：roll_phase 的既有「状态不变」语义不受影响。
        let before = state_with_dice(2, four_pyro(), four_pyro());
        let snapshot = before.clone();
        let _ = transmute_roll_phase_me(&before, superconduct());
        assert_eq!(before, snapshot);
        let (rolled, pending) = crate::flow::turn::roll_phase(
            &before,
            &crate::flow::turn::TurnEnv {
                triggers: &crate::flow::turn::TurnTriggers::new(),
                combat_status:
                    &crate::families::character::zones::combat_status::CombatStatusTriggers::new(),
                summons: &crate::families::summon::zones::SummonTriggers::new(),
                supports: &crate::families::action_card::areas::support::SupportTriggers::new(),
                draw: crate::flow::hooks::DrawHooks::none(),
            },
        );
        assert!(pending.is_empty());
        assert_eq!(rolled, before);
    }
}
