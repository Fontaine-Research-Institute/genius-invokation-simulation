//! **flow 的注入点**：击倒清理回调（D255）与抓牌落地 hooks（F10 / D188）。
//!
//! ## 为什么需要注入
//!
//! 1. **击倒清理**：规则书·其他规则 击倒「生命值被降为 0，则在技能或效果结算完成后，角色就会
//!    被『击倒』」+ D255 顺序「结算 -> 清理 -> 判终局」。清理本体在
//!    state::flows::defeat::{defeat_me, defeat_opp}，但它需要按角色给
//!    DefeatHpReset{定义 max_hp, GrowthList}；角色定义 / 成长聚合不在 definitions 层
//!    （state/** 不在本任务写范围），所以 flow 只留**回调注入点**，由上层（cards）给真实实现。
//! 2. **抓牌落地**（F10 / D188）：抓牌后新进手牌的 CurrentCost 需要按基础费用重建
//!    （state::player::PlayerDefs::with_synced_hand_costs），基础费用来自卡牌定义（上层给）；
//!    抽到牌携带的待展开随机状态（DrawBranch::drawn_marks）要物化进**手牌卡状态表**
//!    （families::action_card::areas::hand_state::HandCardStates）。两件事都需要上层给数据 /
//!    回调，所以 flow 用 [DrawHooks] 注入。
//!
//! ## 口径
//!
//! - 回调是**函数指针 / 只读 trait 对象**（D186：不登记闭包捕获的可变状态）；
//! - 默认实现 [NO_DEFEAT_CLEANUP] / [DrawHooks::none] **不改任何东西**：不注入时行为与旧版
//!   完全一致；
//! - 本文件只做「把注入的东西接到已有入口」，不重写规则：击倒清理调 state::flows::defeat、
//!   费用表调 PlayerDefs::with_synced_hand_costs、状态落表调 HandCardStates。
//!
//! ## TODO / 待接入（规则书 / 数据没写清，不自行编排）
//!
//! - **击倒清理回调的签名**：本层用「fn(&DefState, Perspective) -> (DefState, PendingSet)」
//!   （纯函数形状，便于 flow 保持纯函数）；上层实现里自己扫 HP=0 的角色、按角色构造
//!   DefeatHpReset 并调 defeat_me/opp。Lead 给的参考签名是「fn(&mut DefState, Perspective,
//!   &DefeatHpReset) -> PendingSet」（每角色一次）；因为 DefeatHpReset 借 GrowthList、
//!   且「哪些角色 / 哪个 reset」只有上层知道，本层选整侧一次的形式，语义等价、少一次穿越；
//! - **drawn_marks 物化时是否派发「手牌卡状态加上」触发**：规则书没写；本层用空注册表物化
//!   （只落数据、不派发），派发待接入（上层注入 HandCardStateTriggers 后接）；
//! - **抓牌费用表**：只对新进手牌建 CurrentCost；已在手牌上的 delta 保留（with_synced_hand_costs
//!   的既有语义）；
//! - **击倒清理派发出来的待执行**：本层并进分支的 PendingSet（D186：不在结算内执行），
//!   由调用方在边界执行 / 插入。

use crate::base::CardId;
use crate::families::action_card::areas::hand_state::HandCardStateTriggers;
use crate::families::action_card::zones::pile::DrawBranch;
use crate::flow::pending::PendingSet;
use crate::keywords::target::Perspective;
use crate::resources::dice::CostComponent;
use crate::state::{DefState, PlayerDefs};

/// **击倒清理回调**（D255：结算 -> 清理 -> 判终局）。
///
/// 入参 = 当前一局 + 要清理哪一方（Perspective::Me / Opp）；返回 = 清理后的局面 +
/// 清理派发出来的待执行（本层会并进分支的 PendingSet，由调用方在边界执行）。
///
/// 上层（cards）注入真实实现：对该方所有 HP=0 的角色调
/// state::flows::defeat::defeat_me/opp（按角色给 DefeatHpReset：定义 max_hp + 成长）。
pub type DefeatCleanup = fn(&DefState, Perspective) -> (DefState, PendingSet);

/// **不清理**：直接交回原局面 + 空待执行（默认；不注入时行为与旧版完全一致）。
pub fn no_defeat_cleanup(state: &DefState, _side: Perspective) -> (DefState, PendingSet) {
    (state.clone(), PendingSet::new())
}

/// 默认的击倒清理回调（= [no_defeat_cleanup]；常量方便 env 默认值使用）。
pub const NO_DEFEAT_CLEANUP: DefeatCleanup = no_defeat_cleanup;

/// 抓牌路径的**注入点**（F10 手牌费用表 + D188 drawn_marks 物化）。
#[derive(Clone, Copy)]
pub struct DrawHooks<'a> {
    /// 卡牌基础费用（卡 id -> 基础费用）：新进手牌按它建 CurrentCost。
    ///
    /// None = 不同步费用表（旧行为；调用方自己保证费用表正确）。
    pub base_cost: Option<&'a dyn Fn(CardId) -> Vec<CostComponent>>,
}

impl<'a> DrawHooks<'a> {
    /// 不注入任何抓牌 hook（旧行为）。
    pub const fn none() -> DrawHooks<'a> {
        DrawHooks { base_cost: None }
    }

    /// 只注入基础费用回调。
    pub const fn with_base_cost(
        base_cost: &'a dyn Fn(CardId) -> Vec<CostComponent>,
    ) -> DrawHooks<'a> {
        DrawHooks {
            base_cost: Some(base_cost),
        }
    }
}

/// **把一条抽牌分支落到一方**（F10 / D188）：
///
/// 1. 手牌 / 牌库换成抽牌后的区；
/// 2. 有 base_cost 回调 -> PlayerDefs::with_synced_hand_costs（新牌按基础费用建 CurrentCost，
///    已有手牌的 delta 保留，离开手牌的删除）；
/// 3. 抽到牌携带的 drawn_marks（D188 展开结果）物化进**手牌卡状态表**：
///    与 drawn 一一对应；因为手牌溢出被弃置的牌不再物化；
///    物化用空触发注册表（只落状态数据；触发派发待接入，见模块 TODO）。
pub fn apply_draw_branch(
    player: &PlayerDefs,
    branch: &DrawBranch,
    hooks: DrawHooks<'_>,
) -> PlayerDefs {
    let mut next = player
        .clone()
        .with_hand(branch.hand.clone())
        .with_pile(branch.pile.clone());
    if let Some(base_cost) = hooks.base_cost {
        next = next.with_synced_hand_costs(base_cost);
    }
    // D188：抽到牌携带的待展开随机状态 -> 手牌卡状态表（手牌区自己持有的状态表）。
    let mut states = next.hand_states().clone();
    for (entry, marks) in branch.drawn.iter().zip(branch.drawn_marks.iter()) {
        // 手牌溢出的牌已被弃置 -> 不再落状态（状态跟着牌走）。
        if !next.hand().contains(entry.entity) {
            continue;
        }
        for mark in marks {
            let triggers = HandCardStateTriggers::new();
            let resolution =
                states.add_state_with_triggers(entry.entity, mark.state, mark.layers, &triggers);
            states = resolution.states;
        }
    }
    next.with_hand_states(states)
}

/// **执行一次击倒清理**（D255 顺序：结算 -> 清理 -> 判终局）。
///
/// 先我方、再敌方（两方都要清；具体清理哪些角色由注入的实现决定）。清理派发出来的待执行
/// 并进 pending（D186：本层不执行，由调用方在边界执行）。
pub fn apply_defeat_cleanup(
    state: &DefState,
    cleanup: DefeatCleanup,
    pending: &mut PendingSet,
) -> DefState {
    let (state, me_cleanup) = cleanup(state, Perspective::Me);
    merge_pending(pending, me_cleanup);
    let (state, opp_cleanup) = cleanup(&state, Perspective::Opp);
    merge_pending(pending, opp_cleanup);
    state
}

/// 合并待执行（保持来源顺序，接在后面）。
fn merge_pending(into: &mut PendingSet, from: PendingSet) {
    into.me.extend(from.me);
    into.opp.extend(from.opp);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::Mark;
    use crate::families::action_card::zones::hand::HandState;
    use crate::families::character::character::CharacterState;
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::flow::pending::FlowPending;
    use crate::resources::dice::CostComponent;
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{n, pile, TEST_STATUS};
    use crate::timeline::{Event, Queued};
    use gi_genius_invokation_engine::Nat;

    /// 基础费用回调：所有牌都按 2 个任意骰（测试用）。
    fn base_two(_card: CardId) -> Vec<CostComponent> {
        vec![CostComponent::AnyColor(Nat::new(2))]
    }

    fn draw_one(player: &PlayerDefs, draw_hooks: DrawHooks<'_>) -> PlayerDefs {
        let branches = player.pile().draw_many(player.hand(), Nat::new(1));
        let (branch, _probability) = branches.first().expect("有牌可抓");
        apply_draw_branch(player, branch, draw_hooks)
    }

    #[test]
    fn draw_hooks_sync_the_cost_table_for_new_hand_cards() {
        let player = PlayerDefs::new()
            .with_pile(pile(&[101]))
            .with_hand(HandState::new());
        // 不注入：费用表不同步（旧行为）。
        let plain = draw_one(&player, DrawHooks::none());
        let drawn = plain.hand().entries().first().expect("抓到").entity;
        assert!(plain.costs().cost_of(drawn).is_none());
        // 注入 base_cost：新抓的牌有基础费用（2 个任意骰）。
        let hooked = draw_one(&player, DrawHooks::with_base_cost(&base_two));
        let drawn = hooked.hand().entries().first().expect("抓到").entity;
        let cost = hooked.costs().cost_of(drawn).expect("有费用");
        assert_eq!(cost.total(), Nat::new(2));
        // 纯函数：入参不变。
        assert!(player.costs().cost_of(drawn).is_none());
    }

    #[test]
    fn draw_branch_materializes_drawn_marks_into_hand_card_states() {
        // D188：牌库上「待展开的随机状态」在抽到时展开成 drawn_marks；本层把它物化进
        // 手牌卡状态表（手牌区自己持有的状态表）。
        let marked_pile = pile(&[101])
            .add_pending_mark(
                Mark {
                    state: &TEST_STATUS,
                    layers: None,
                },
                &[Nat::ZERO],
            )
            .expect("位置合法");
        let player = PlayerDefs::new()
            .with_pile(marked_pile)
            .with_hand(HandState::new());
        let next = draw_one(&player, DrawHooks::none());
        let drawn = next.hand().entries().first().expect("抓到").entity;
        assert!(next.hand_states().contains_state(drawn, &TEST_STATUS));
        // 牌库侧的 pending 已被展开消耗掉。
        assert!(next.pile().pending_marks().is_empty());
    }

    fn noop_zone(
        _zone: &mut CharacterZone,
        _event: &Event,
        _owner: gi_genius_invokation_engine::EntityId,
    ) {
    }

    /// 假击倒清理：把该方 HP=0 的角色从角色区拿掉（真实实现是 state::flows::defeat）。
    fn fake_cleanup(state: &DefState, side: Perspective) -> (DefState, PendingSet) {
        let player = if side == Perspective::Me {
            state.me().clone()
        } else {
            state.opp().clone()
        };
        let kept: Vec<CharacterSlot> = player
            .characters()
            .slots()
            .iter()
            .filter(|slot| slot.state.hp() != Nat::ZERO)
            .cloned()
            .collect();
        let zone = CharacterZone::try_from_slots(kept).unwrap_or_default();
        let player = player.with_characters(zone);
        let next = if side == Perspective::Me {
            state.clone().with_me(player)
        } else {
            state.clone().with_opp(player)
        };
        let mut pending = PendingSet::new();
        if side == Perspective::Me {
            pending.push_me(FlowPending::CharacterZone(Queued {
                effect: noop_zone,
                event: Event::of(n(201)),
                owner: n(201),
                source_kind: EffectSource::CharacterCard,
                entry: Nat::ZERO,
            }));
        }
        (next, pending)
    }

    #[test]
    fn apply_defeat_cleanup_cleans_both_sides_and_merges_pending() {
        let me = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![
                CharacterSlot::new(
                    n(201),
                    CharacterState::new(Nat::ZERO, n(10), Nat::ZERO, n(2)),
                ),
                CharacterSlot::new(n(202), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
            ])
            .expect("合法槽位"),
        );
        let opp = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(
                n(401),
                CharacterState::new(Nat::ZERO, n(10), Nat::ZERO, n(2)),
            )])
            .expect("合法槽位"),
        );
        let before = DefState::from_players(me, opp);
        let mut pending = PendingSet::new();
        let cleaned = apply_defeat_cleanup(&before, fake_cleanup, &mut pending);
        // 两方 HP=0 的角色都被清理掉。
        assert_eq!(cleaned.me().characters().len(), Nat::new(1));
        assert!(cleaned.opp().characters().is_empty());
        // 清理派发出来的待执行并进了 pending。
        assert_eq!(pending.me.len(), 1);
        // 入参不变。
        assert_eq!(before.me().characters().len(), Nat::new(2));
        assert_eq!(before.opp().characters().len(), Nat::new(1));
    }

    #[test]
    fn default_cleanup_is_a_noop() {
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(crate::state::testing::characters(&[201])),
            PlayerDefs::new().with_characters(crate::state::testing::characters(&[401])),
        );
        let mut pending = PendingSet::new();
        let after = apply_defeat_cleanup(&before, NO_DEFEAT_CLEANUP, &mut pending);
        assert_eq!(after, before);
        assert!(pending.is_empty());
    }
}
