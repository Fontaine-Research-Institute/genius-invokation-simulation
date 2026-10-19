//! **一次「元素调和」端到端**（快速行动）：丢弃 1 张手牌 -> 把 1 个元素骰转换成
//! 我方当前出战角色的元素类型。
//!
//! ## 依据（唯一权威：规则书 S1–S8 常规模式）
//!
//! - 规则书·进行行动 快速行动【元素调和】：「丢弃一张手牌，将一个元素骰转换成我方当前
//!   『出战角色』的元素类型。」
//! - 规则书·进行行动 行动：「【快速行动】完成一个快速行动后，我方可以继续行动。
//!   『打出手牌』和『元素调和』是快速行动。」
//! - 规则书·游戏区域 VI.手牌：「只有『手牌』中的行动牌可以被打出和用于元素调和。」
//!
//! ## 本层做什么 / 不做什么
//!
//! - **做**：校验牌在手牌里 -> 从手牌 / 当前费用表 / 手牌卡状态表里移除这张牌（三者是
//!   同一张牌的三个视角，与 `play_card` 的离手口径一致）-> 骰池 `convert` 1 颗骰
//!   （D117：转化，不是获得；总数不变）；
//! - **不做**：行动权推进（快速行动不换手，`rules::ops::stage::timing_of_action` 已给判定；
//!   也**不校验现在轮到哪一方**）；不写「调和后」类卡牌效果（规则书没有定义通用的
//!   「元素调和后」时机；卡面触发由卡牌层在自己的结算里派发）；不决定「挑哪一颗骰」（见下）。
//!
//! ## 规则书没写清 / 明确不做（TODO）
//!
//! 1. **未指定来源骰时的「内置选择算法」**：规则书只说「将一个元素骰转换」，没写挑哪一颗。
//!    D117 裁定**留空**（见 `留空.md` §一.9：复杂机制、明确不做）。本层因此要求调用方
//!    在 [TuningRequest::source] 里指定来源骰面；`None` 直接返回
//!    [TuningError::SourceNotSpecified]，**不**退化成骰池的固定顺序兜底；
//! 2. **目标元素类型由调用方给**：规则书要求转成「我方当前出战角色的元素类型」，
//!    但角色区只存「实体号 + `CharacterState`」，**没有**角色元素类型（角色元素在角色牌定义 /
//!    卡牌层）。所以 [TuningRequest::target] 由调用方按角色定义算好
//!    （`DieFace::from_element(出战角色元素)`）。TODO(角色元素不在 DefState)：接入角色定义后
//!    应由本层从出战角色自动派生，不再由调用方给；
//! 3. **手牌「不可调和」限制**：权威关键字 K3007「不可调和」在 keyword 数据里**只有名字、
//!    没有描述**（D116），`HandCardStatus` 也没有对应身份；卡面「禁忌知识」（301020）
//!    另有「无法使用此牌进行元素调和」。两者都需要卡牌层 / 状态层提供判定，本层目前
//!    **只校验「牌在手牌里」**（规则书 VI），不校验这两条 —— TODO(待接入)；
//! 4. **调和后的时机**：规则书没有「元素调和后」这类通用时机；不新造时机键（D185：时机键
//!    由需要广播的块定义，接卡牌效果时再由卡牌层给）。
//!
//! ## 口径
//!
//! - **纯函数**：吃 &DefState、返回新 DefState；失败返回 Err、**入参一个字节不改**；
//! - **敌我不编号**（D34）：[tuning_me] / [tuning_opp] 成对入口，共用一份实现（D35）；
//! - **数量按规则书原文**：丢弃 **1** 张手牌（[TUNING_HAND_DISCARD]）、转换 **1** 个元素骰
//!   （[TUNING_DICE_COUNT]）；没有其他费用。
//!
//! ## 待复核
//!
//! - **来源骰是否允许是万能骰**：规则书只说「一个元素骰」；万能骰也是元素骰（可以当任何元素
//!   使用），本层不限制来源骰面，交给调用方选择（待复核是否要排除某些骰面）；
//! - **来源骰与目标骰面相同**：规则书没写；本层不当作非法（骰池 `convert` 会保持原样），
//!   是否要在调用方禁止这种无意义选择，待复核。

use crate::resources::dice::DieFace;
use crate::state::DefState;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 元素调和**丢弃的手牌张数**：1 张（规则书原文「丢弃一张手牌」）。
pub const TUNING_HAND_DISCARD: Nat = Nat::new(1);

/// 元素调和**转换的元素骰个数**：1 个（规则书原文「将一个元素骰转换」）。
pub const TUNING_DICE_COUNT: Nat = Nat::new(1);

/// 一次元素调和的**请求**（本层不替你挑牌 / 挑骰）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TuningRequest {
    /// 要弃置的手牌实体号（必须真在手牌里）。
    pub hand_entity: EntityId,
    /// 要转换的**来源骰面**；`None` = 未指定 -> 内置选择算法明确留空（D117）-> Err。
    pub source: Option<DieFace>,
    /// 转换后的骰面 = 我方当前出战角色的元素类型（由调用方按角色定义给）。
    pub target: DieFace,
}

/// **一次元素调和的结果**：新的一局（骰池 -1 来源 +1 目标、手牌 -1）。
#[derive(Clone, Debug, PartialEq)]
pub struct TuningOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
}

/// 元素调和失败的原因（所有失败都保证入参不变）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TuningError {
    /// 这张牌不在调和方手牌里（规则书 VI：只有手牌中的行动牌可以用于元素调和）。
    NotInHand,
    /// 没指定要转换的骰面；未指定目标时的内置选择算法明确留空（D117 / 留空.md §一.9）。
    SourceNotSpecified,
    /// 骰池里没有指定骰面的骰子（不够转换）。
    SourceNotInPool,
    /// 目标不是 7 种基础元素之一（出战角色的元素类型不可能是万能元素）。
    TargetNotBasicElement,
}

/// **元素调和**（我方，快速行动）：丢弃手牌 `hand_entity`，把 1 个骰转成 `target`。
pub fn tuning_me(state: &DefState, request: &TuningRequest) -> Result<TuningOutcome, TuningError> {
    tune(state, request, true)
}

/// **元素调和**（敌方，快速行动；与 [tuning_me] 共用一份实现，D35）。
pub fn tuning_opp(state: &DefState, request: &TuningRequest) -> Result<TuningOutcome, TuningError> {
    tune(state, request, false)
}

/// 共用实现：`tuning_me = true` 表示我方调和。
fn tune(
    state: &DefState,
    request: &TuningRequest,
    tuning_me: bool,
) -> Result<TuningOutcome, TuningError> {
    let user = if tuning_me { state.me() } else { state.opp() };

    // 1) 规则书 VI：只有手牌里的行动牌可以用于元素调和。
    if !user.hand().contains(request.hand_entity) {
        return Err(TuningError::NotInHand);
    }

    // 2) 来源骰必须由调用方指定：未指定目标时的内置选择算法明确留空（D117 / 留空.md §一.9）。
    //    TODO(内置算法)：这里不做「自动挑一颗骰」；需要时在此处接（会产出分支 + 概率）。
    let Some(source) = request.source else {
        return Err(TuningError::SourceNotSpecified);
    };

    // 3) 目标必须是基础元素骰面（出战角色的元素类型不可能是万能）。
    if request.target.element().is_none() {
        return Err(TuningError::TargetNotBasicElement);
    }

    // 4) 骰池转化 1 颗（D117：转化不是获得；总数不变）。
    let Some(converted) = user
        .dice()
        .convert(Some(source), TUNING_DICE_COUNT, request.target)
    else {
        return Err(TuningError::SourceNotInPool);
    };

    // 5) 手牌离手：手牌区 + 当前费用表 + 手牌卡状态表（同一张牌的三个视角，与 play_card 一致）。
    let hand = user
        .hand()
        .discard(request.hand_entity)
        .ok_or(TuningError::NotInHand)?;
    let costs = user.costs().without(request.hand_entity);
    let hand_states = user
        .hand_states()
        .remove_card(request.hand_entity)
        .unwrap_or_else(|| user.hand_states().clone());
    let player = user
        .clone()
        .with_dice(converted)
        .with_hand(hand)
        .with_costs(costs)
        .with_hand_states(hand_states);

    let next = if tuning_me {
        state.clone().with_me(player)
    } else {
        state.clone().with_opp(player)
    };
    Ok(TuningOutcome { state: next })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::areas::cost::CurrentCost;
    use crate::families::action_card::areas::hand_state::HandCardStateTriggers;
    use crate::resources::dice::{DicePool, DieFace};
    use crate::state::player::{HandCosts, PlayerDefs};
    use crate::state::testing::{characters, hand, n, TEST_STATUS};
    use crate::state::DefState;
    use gi_genius_invokation_engine::Nat;

    /// 我方：1 个角色 + 指定手牌 + 指定骰池。
    fn me(hand_entities: &[u32], dice: DicePool) -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(characters(&[201]))
            .with_hand(hand(hand_entities))
            .with_dice(dice)
    }

    fn request(hand_entity: u32, source: Option<DieFace>, target: DieFace) -> TuningRequest {
        TuningRequest {
            hand_entity: n(hand_entity),
            source,
            target,
        }
    }

    #[test]
    fn tuning_discards_one_card_and_converts_one_die() {
        let before = DefState::from_players(
            me(&[211, 212], DicePool::empty().add(DieFace::Pyro, n(2))),
            PlayerDefs::new(),
        );
        let outcome = tuning_me(&before, &request(211, Some(DieFace::Pyro), DieFace::Hydro))
            .expect("可以调和");
        let after = &outcome.state;

        // 手牌：丢了 211，212 还在
        assert_eq!(after.me().hand().len(), Nat::new(1));
        assert!(!after.me().hand().contains(n(211)));
        assert!(after.me().hand().contains(n(212)));
        // 骰池：火 2 -> 火 1 + 水 1（总数不变）
        assert_eq!(
            after.me().dice(),
            &DicePool::empty()
                .add(DieFace::Pyro, n(1))
                .add(DieFace::Hydro, n(1))
        );
        assert_eq!(after.me().dice().total(), Nat::new(2));
        // 敌方没被碰
        assert_eq!(after.opp(), before.opp());
        // 纯函数：入参不变
        assert_eq!(before.me().hand().len(), Nat::new(2));
        assert_eq!(
            before.me().dice(),
            &DicePool::empty().add(DieFace::Pyro, n(2))
        );
    }

    #[test]
    fn tuning_removes_the_card_costs_and_hand_states_too() {
        let hand_states = crate::families::action_card::areas::hand_state::HandCardStates::new()
            .add_state_with_triggers(n(211), &TEST_STATUS, None, &HandCardStateTriggers::new())
            .states;
        let before = DefState::from_players(
            me(&[211], DicePool::empty().add(DieFace::Cryo, n(1)))
                .with_costs(HandCosts::new().with_cost(n(211), CurrentCost::new(Vec::new())))
                .with_hand_states(hand_states),
            PlayerDefs::new(),
        );
        let outcome = tuning_me(&before, &request(211, Some(DieFace::Cryo), DieFace::Pyro))
            .expect("可以调和");
        let after = &outcome.state;
        assert!(after.me().hand().is_empty());
        // 费用表 / 手牌卡状态表都不再留这张牌的条目
        assert!(after.me().costs().cost_of(n(211)).is_none());
        assert!(after.me().hand_states().states_of(n(211)).is_none());
    }

    #[test]
    fn unspecified_source_is_explicitly_not_implemented() {
        let before = DefState::from_players(
            me(&[211], DicePool::empty().add(DieFace::Pyro, n(1))),
            PlayerDefs::new(),
        );
        // D117 / 留空.md §一.9：未指定目标时的内置选择算法明确不做 -> 必须由调用方指定来源骰。
        let error = tuning_me(&before, &request(211, None, DieFace::Hydro)).expect_err("未指定");
        assert_eq!(error, TuningError::SourceNotSpecified);
        // 不改任何状态
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert_eq!(before.me().dice().total(), Nat::new(1));
    }

    #[test]
    fn tuning_rejects_missing_card_or_die_or_bad_target() {
        let before = DefState::from_players(
            me(&[211], DicePool::empty().add(DieFace::Pyro, n(1))),
            PlayerDefs::new(),
        );
        // 牌不在手牌
        assert_eq!(
            tuning_me(&before, &request(999, Some(DieFace::Pyro), DieFace::Hydro))
                .expect_err("不在手牌"),
            TuningError::NotInHand
        );
        // 骰池没有这个骰面
        assert_eq!(
            tuning_me(&before, &request(211, Some(DieFace::Cryo), DieFace::Hydro))
                .expect_err("没有冰骰"),
            TuningError::SourceNotInPool
        );
        // 目标不能是万能元素
        assert_eq!(
            tuning_me(&before, &request(211, Some(DieFace::Pyro), DieFace::Omni))
                .expect_err("目标不是基础元素"),
            TuningError::TargetNotBasicElement
        );
        // 三次失败都不改入参
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert_eq!(before.me().dice().total(), Nat::new(1));
    }

    #[test]
    fn tuning_can_convert_an_omni_die() {
        let before = DefState::from_players(
            me(&[211], DicePool::empty().add(DieFace::Omni, n(1))),
            PlayerDefs::new(),
        );
        let outcome = tuning_me(
            &before,
            &request(211, Some(DieFace::Omni), DieFace::Electro),
        )
        .expect("万能骰可以转");
        assert_eq!(
            outcome.state.me().dice(),
            &DicePool::empty().add(DieFace::Electro, n(1))
        );
    }

    #[test]
    fn opp_tuning_only_touches_the_opponent_side() {
        let me_side = me(&[211], DicePool::empty().add(DieFace::Pyro, n(1)));
        let opp = PlayerDefs::new()
            .with_characters(characters(&[301]))
            .with_hand(hand(&[311]));
        let before = DefState::from_players(me_side.clone(), opp);
        let outcome = tuning_opp(&before, &request(311, Some(DieFace::Omni), DieFace::Cryo))
            .expect_err("敌方骰池里没有万能骰");
        assert_eq!(outcome, TuningError::SourceNotInPool);
        // 我方完全没动
        assert_eq!(before.me(), &me_side);

        // 敌方有骰子时可以调和，且不碰我方
        let before = DefState::from_players(
            me_side.clone(),
            before
                .opp()
                .clone()
                .with_dice(DicePool::empty().add(DieFace::Omni, n(1))),
        );
        let outcome = tuning_opp(&before, &request(311, Some(DieFace::Omni), DieFace::Cryo))
            .expect("可以调和");
        assert!(outcome.state.me().hand().contains(n(211)));
        assert!(!outcome.state.opp().hand().contains(n(311)));
        assert_eq!(
            outcome.state.opp().dice(),
            &DicePool::empty().add(DieFace::Cryo, n(1))
        );
    }
}
