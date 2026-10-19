//! **统一待执行执行器**（D186 的落地：派发只排序、不执行；这里**只做分派**，不写任何效果）。
//!
//! 各区的待执行类型互不相同（它们是各自区定义的 Queued 别名）：
//!
//! | 待执行类型 | 执行器（各区提供） | 作用对象 |
//! |---|---|---|
//! | CharacterPending（角色族触发） | execute_character | 某个角色的 CharacterState |
//! | CharacterZonePending | execute_character_zone | 角色区（整区） |
//! | CombatStatusPending | execute_combat_status | 出战状态区 |
//! | EquipmentPending | execute_equipment | 某个角色的装备栏 |
//! | SummonPending | execute_summon | 召唤物区 |
//! | SupportPending | execute_support | 支援区 |
//! | HandCardStatePending | execute_hand_card_states | 手牌卡状态表 |
//!
//! ## 为什么需要这一层
//!
//! 各区只提供「一条列表 -> 一个区」的 execute_*，流程会把多个区的待执行一起交回调用方；
//! 没有统一入口时，调用方要自己记住「哪个待执行交给哪个区、哪一方」。本层把这件事收成
//! [FlowPending] / [PendingSet] 两个纯数据类型 + 一个分派函数，**不新增任何效果语义**：
//! 效果函数指针仍是各区登记表里的那一个，这里只负责按类型把它送到对应的 execute_*。
//!
//! ## 口径
//!
//! - **纯函数边界**：对外入口吃 &DefState、返回新 DefState（内部在克隆上执行各区 execute_*）；
//! - **区自持**（D182 / D183）：本层不解释区内部状态，只把区整体克隆出来交给 execute_*，
//!   再把新区整个写回；写回一律走各区自己的入口（with_* / parts_mut / with_equipment_zone）；
//! - **敌我不编号**（D34）：两侧用 [PendingSet] 的两个具名字段 me / opp 表达，
//!   不传 Side 参数；执行顺序 = 先 me 后 opp（对应结算顺序.md §一 第 1 条「当前牌手优先」的
//!   最外层形态；同一方内部的来源类别顺序由各 pending 列表自身的顺序保证）；
//! - **只分派**：不认识时机 / 条件 / 效果内容；待执行项里的 effect 是函数指针，本层只调用它。
//!
//! ## 待接入 / 待复核
//!
//! - **CharacterPending 的作用角色**：角色族 execute_character 只接收一个 CharacterState，
//!   「这条触发想作用在谁身上」不在待执行里（只有触发 owner）。本层用 FlowPending::Character
//!   的 character 字段显式指定作用角色（由派发方给），待复核「一条触发作用多个角色」的情形；
//! - **EquipmentPending 的装备栏归属**：Pending.owner 是触发登记时的 owner（可能是装备牌实体），
//!   不一定是装备栏的角色。本层用 FlowPending::Equipment 的 owner 字段显式指定装备栏角色；
//! - **同一方内部的执行顺序**：本层按 PendingSet 里 Vec 的顺序执行；各区 pending 之间的
//!   来源类别顺序（角色牌 -> 角色装备和状态 -> 出战状态 -> 召唤物 -> 支援牌）由产生方排好，
//!   本层不重排（重排会破坏各区自己已经排好的顺序）。

use crate::families::action_card::areas::hand_state::{
    execute_hand_card_states, HandCardStatePending,
};
use crate::families::action_card::areas::support::{execute_support, SupportPending};
use crate::families::character::trigger::{execute_character, CharacterEffect, CharacterPending};
use crate::families::character::zones::combat_status::{
    execute_combat_status, CombatStatusPending,
};
use crate::families::character::zones::equipment::{
    execute_equipment, EquipmentPending, EquipmentZone,
};
use crate::families::character::zones::field::{execute_character_zone, CharacterZonePending};
use crate::families::summon::zones::{execute_summon, SummonPending};
use crate::state::{DefState, PlayerDefs};
use crate::timeline::Resolution;
use gi_genius_invokation_engine::{nadd, EntityId, Nat};

/// **一条待执行**（按类型分派用）。
///
/// 变体 = 各区的待执行类型；需要额外定位信息的两种（角色状态 / 装备栏）多带一个实体号。
#[derive(Clone, Debug)]
pub enum FlowPending {
    /// 角色族触发：对 character 这个角色的 CharacterState 执行。
    Character {
        /// 作用在哪个角色身上。
        character: EntityId,
        /// 角色族待执行（execute_character）。
        pending: CharacterPending,
    },
    /// 角色区待执行（整区）。
    CharacterZone(CharacterZonePending),
    /// 出战状态区待执行。
    CombatStatus(CombatStatusPending),
    /// 装备栏待执行：对 owner 这个角色的装备栏执行。
    Equipment {
        /// 装备栏属于哪个角色。
        owner: EntityId,
        /// 装备栏待执行。
        pending: EquipmentPending,
    },
    /// 召唤物区待执行。
    Summon(SummonPending),
    /// 支援区待执行。
    Support(SupportPending),
    /// 手牌卡状态待执行。
    HandCardState(HandCardStatePending),
}

/// **一局对局里双方的待执行**（两个具名字段，D34：不用 Side 枚举 / 不用 [Vec; 2] + 下标）。
#[derive(Clone, Debug, Default)]
pub struct PendingSet {
    /// 我方的待执行。
    pub me: Vec<FlowPending>,
    /// 敌方的待执行。
    pub opp: Vec<FlowPending>,
}

impl PendingSet {
    /// 空集合。
    pub const fn new() -> PendingSet {
        PendingSet {
            me: Vec::new(),
            opp: Vec::new(),
        }
    }

    /// 两侧都没有待执行。
    pub fn is_empty(&self) -> bool {
        self.me.is_empty() && self.opp.is_empty()
    }

    /// 总条数（Nat）。
    pub fn len(&self) -> Nat {
        nadd!(Nat::new(self.me.len() as _), Nat::new(self.opp.len() as _))
    }

    /// 把一条待执行放进我方。
    pub fn push_me(&mut self, pending: FlowPending) {
        self.me.push(pending);
    }

    /// 把一条待执行放进敌方。
    pub fn push_opp(&mut self, pending: FlowPending) {
        self.opp.push(pending);
    }
}

/// **执行一批待执行**（纯函数：吃 &DefState、返回新 DefState）。
///
/// 顺序：先我方列表、再敌方列表（两个具名实例，D34）；各自按 Vec 顺序。
pub fn execute_pending(state: &DefState, set: &PendingSet) -> DefState {
    let mut next = state.clone();
    run_pending(&mut next, set);
    next
}

/// **在一份可变局面上执行**（流程内部循环用；对外仍建议用纯函数入口 [execute_pending]）。
pub fn execute_pending_mut(state: &mut DefState, set: &PendingSet) {
    run_pending(state, set);
}

/// 共用实现（D35：一份实现，两个入口）。
fn run_pending(state: &mut DefState, set: &PendingSet) {
    for item in &set.me {
        apply(state.me_mut(), item);
    }
    for item in &set.opp {
        apply(state.opp_mut(), item);
    }
}

/// 对**一方的全部区**执行一条待执行（分派到对应区的 execute_*）。
fn apply(player: &mut PlayerDefs, item: &FlowPending) {
    match item {
        FlowPending::Character { character, pending } => {
            let Some(slot) = player.characters().slot_of(*character) else {
                return;
            };
            let mut state = slot.state.clone();
            execute_character(std::slice::from_ref(pending), &mut state);
            if let Some(zone) = player.characters().with_state(*character, state) {
                *player.parts_mut().characters = zone;
            }
        }
        FlowPending::CharacterZone(pending) => {
            let mut zone = player.characters().clone();
            execute_character_zone(std::slice::from_ref(pending), &mut zone);
            *player.parts_mut().characters = zone;
        }
        FlowPending::CombatStatus(pending) => {
            let mut zone = player.combat_status().clone();
            execute_combat_status(std::slice::from_ref(pending), &mut zone);
            *player.parts_mut().combat_status = zone;
        }
        FlowPending::Equipment { owner, pending } => {
            let mut zone = player
                .equipment_of(*owner)
                .cloned()
                .unwrap_or_else(|| EquipmentZone::new(*owner));
            execute_equipment(std::slice::from_ref(pending), &mut zone);
            let equipment = player.parts_mut().equipment;
            match equipment
                .iter_mut()
                .find(|existing| existing.owner() == *owner)
            {
                Some(existing) => *existing = zone,
                None => equipment.push(zone),
            }
        }
        FlowPending::Summon(pending) => {
            let mut zone = player.summons().clone();
            execute_summon(std::slice::from_ref(pending), &mut zone);
            *player.parts_mut().summons = zone;
        }
        FlowPending::Support(pending) => {
            let mut zone = player.supports().clone();
            execute_support(std::slice::from_ref(pending), &mut zone);
            let next = player.clone().with_supports(zone);
            *player = next;
        }
        FlowPending::HandCardState(pending) => {
            let mut states = player.hand_states().clone();
            execute_hand_card_states(std::slice::from_ref(pending), &mut states);
            let next = player.clone().with_hand_states(states);
            *player = next;
        }
    }
}

/// **执行结算帧里的「角色族」待执行**（D196：插入式结算）。
///
/// 按 Resolution 的帧机制取步骤：当前帧有剩余就取它，当前帧做完弹回父帧继续 ——
/// 也就是说「插入的子结算」先做完，再回到插入点继续原结算（不是 LIFO 重排）。
/// 所有步骤都作用在 character 这个角色的 CharacterState 上（角色族 execute_character 的签名）。
///
/// 失败（返回 None、不改入参）：character 不在双方角色区里。
pub fn execute_frames(
    state: &DefState,
    character: EntityId,
    frames: &mut Resolution<CharacterEffect>,
) -> Option<DefState> {
    if state.me().characters().contains(character) {
        let player = drain_frames(state.me(), character, frames)?;
        Some(state.clone().with_me(player))
    } else if state.opp().characters().contains(character) {
        let player = drain_frames(state.opp(), character, frames)?;
        Some(state.clone().with_opp(player))
    } else {
        None
    }
}

/// 对一方的某个角色把结算帧全部执行完（纯函数）。
fn drain_frames(
    player: &PlayerDefs,
    character: EntityId,
    frames: &mut Resolution<CharacterEffect>,
) -> Option<PlayerDefs> {
    let mut state = player.characters().slot_of(character)?.state.clone();
    while let Some(step) = frames.next_step() {
        (step.effect)(&mut state, &step.event, step.owner);
    }
    let zone = player.characters().with_state(character, state)?;
    Some(player.clone().with_characters(zone))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::areas::hand_state::{
        HandCardStatePending, HandCardStateTriggers,
    };
    use crate::families::action_card::areas::support::{
        SupportEntry, SupportPending, SupportTriggers, SupportValue, SupportZone,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::trigger::{CharacterEffect, CharacterPending};
    use crate::families::character::zones::combat_status::{
        CombatStatusPending, CombatStatusTriggers, CombatStatusZone,
    };
    use crate::families::character::zones::equipment::{EquipmentPending, EquipmentTriggers};
    use crate::families::character::zones::field::{CharacterZone, CharacterZonePending};
    use crate::families::summon::zones::{SummonPending, SummonTriggers, SummonZone};
    use crate::rules::ops::equip::SlotKind;
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{
        characters, combat_status, entity, equipment, hand, n, summons, TEST_STATUS,
    };
    use crate::state::{DefState, PlayerDefs};
    use crate::timeline::{Event, Queued, Resolution};
    use gi_genius_invokation_engine::{EntityId, Nat};

    /// 造一条 value 化的待执行（各区 Queued 的公共字段一致）。
    fn queued<E: Copy>(
        effect: E,
        who: Option<EntityId>,
        owner: EntityId,
        source_kind: EffectSource,
    ) -> Queued<E> {
        Queued {
            effect,
            event: Event::new(who, None, Nat::ZERO, None, None),
            owner,
            source_kind,
            entry: Nat::ZERO,
        }
    }

    // ------------------------------------------------------------------
    // 各类型的「效果函数指针」（测试用：只做能被观察到的区变化）
    // ------------------------------------------------------------------

    fn set_hp_three(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        *state = state.hp_set(Nat::new(3));
    }

    fn set_zone_slot_hp_one(zone: &mut CharacterZone, event: &Event, _owner: EntityId) {
        let Some(target) = event
            .who
            .and_then(|who| zone.slot_of(who).map(|slot| (who, slot.state.clone())))
        else {
            return;
        };
        if let Some(next) = zone.with_state(target.0, target.1.hp_set(Nat::new(1))) {
            *zone = next;
        }
    }

    fn attach_test_status(zone: &mut CombatStatusZone, event: &Event, _owner: EntityId) {
        let Some(active) = event.who else {
            return;
        };
        let resolution = zone.attach_with_triggers(
            active,
            &TEST_STATUS,
            None,
            None,
            &CombatStatusTriggers::new(),
        );
        *zone = resolution.zone;
    }

    fn detach_equipment(zone: &mut EquipmentZone, event: &Event, _owner: EntityId) {
        let Some(equip) = event.who else {
            return;
        };
        if let Some(resolution) = zone.detach_with_triggers(equip, &EquipmentTriggers::new()) {
            *zone = resolution.zone;
        }
    }

    fn discard_summon(zone: &mut SummonZone, event: &Event, _owner: EntityId) {
        let Some(summon) = event.who else {
            return;
        };
        if let Some(resolution) = zone.discard_with_triggers(summon, &SummonTriggers::new()) {
            *zone = resolution.zone;
        }
    }

    fn discard_support(zone: &mut SupportZone, event: &Event, _owner: EntityId) {
        let Some(support) = event.who else {
            return;
        };
        if let Some(resolution) = zone.discard_with_triggers(support, &SupportTriggers::new()) {
            *zone = resolution.zone;
        }
    }

    fn attach_hand_status(
        states: &mut crate::families::action_card::areas::hand_state::HandCardStates,
        event: &Event,
        _owner: EntityId,
    ) {
        let Some(card) = event.who else {
            return;
        };
        let resolution =
            states.add_state_with_triggers(card, &TEST_STATUS, None, &HandCardStateTriggers::new());
        *states = resolution.states;
    }

    // ------------------------------------------------------------------
    // 分派：每种类型送到对应的区
    // ------------------------------------------------------------------

    #[test]
    fn character_pending_applies_to_that_character_state() {
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201, 202])),
            PlayerDefs::new().with_characters(characters(&[401])),
        );
        let pending: CharacterPending = queued(
            set_hp_three,
            Some(n(201)),
            n(201),
            EffectSource::CharacterCard,
        );
        let mut set = PendingSet::new();
        set.push_me(FlowPending::Character {
            character: n(201),
            pending,
        });
        let after = execute_pending(&before, &set);
        assert_eq!(
            after
                .me()
                .characters()
                .slot_of(n(201))
                .expect("有")
                .state
                .hp(),
            Nat::new(3)
        );
        // 同侧另一个角色不受影响
        assert_eq!(
            after
                .me()
                .characters()
                .slot_of(n(202))
                .expect("有")
                .state
                .hp(),
            Nat::new(10)
        );
        // 敌方一个字节都没动
        assert_eq!(after.opp(), before.opp());
        // 纯函数：入参不变
        assert_eq!(
            before
                .me()
                .characters()
                .slot_of(n(201))
                .expect("有")
                .state
                .hp(),
            Nat::new(10)
        );
    }

    #[test]
    fn character_zone_pending_applies_to_the_zone() {
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new(),
        );
        let pending: CharacterZonePending = queued(
            set_zone_slot_hp_one,
            Some(n(201)),
            n(201),
            EffectSource::CharacterCard,
        );
        let mut set = PendingSet::new();
        set.push_me(FlowPending::CharacterZone(pending));
        let after = execute_pending(&before, &set);
        assert_eq!(
            after
                .me()
                .characters()
                .slot_of(n(201))
                .expect("有")
                .state
                .hp(),
            Nat::new(1)
        );
    }

    #[test]
    fn combat_status_pending_applies_to_the_zone() {
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new(),
        );
        let pending: CombatStatusPending = queued(
            attach_test_status,
            Some(n(201)),
            n(201),
            EffectSource::CombatStatus,
        );
        let mut set = PendingSet::new();
        set.push_me(FlowPending::CombatStatus(pending));
        let after = execute_pending(&before, &set);
        assert_eq!(after.me().combat_status().len(), Nat::new(1));
        assert!(after.me().combat_status().contains(&TEST_STATUS));
    }

    #[test]
    fn equipment_pending_applies_to_that_owners_equipment_zone() {
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201]))
                .with_equipment_zone(equipment(n(201), &[(11, SlotKind::Artifact, 312001)])),
            PlayerDefs::new(),
        );
        let pending: EquipmentPending = queued(
            detach_equipment,
            Some(n(11)),
            n(201),
            EffectSource::CharacterEquipmentOrStatus,
        );
        let mut set = PendingSet::new();
        set.push_me(FlowPending::Equipment {
            owner: n(201),
            pending,
        });
        let after = execute_pending(&before, &set);
        assert!(after
            .me()
            .equipment_of(n(201))
            .expect("有装备栏")
            .is_empty());
    }

    #[test]
    fn summon_pending_applies_to_the_zone() {
        let before = DefState::from_players(
            PlayerDefs::new().with_summons(summons(&[(301, 2)])),
            PlayerDefs::new(),
        );
        let pending: SummonPending =
            queued(discard_summon, Some(n(301)), n(301), EffectSource::Summon);
        let mut set = PendingSet::new();
        set.push_me(FlowPending::Summon(pending));
        let after = execute_pending(&before, &set);
        assert!(after.me().summons().is_empty());
    }

    #[test]
    fn support_pending_applies_to_the_zone() {
        let supports = SupportZone::new()
            .add_with_triggers(
                SupportEntry::new(crate::base::CardId::new(322001), n(401), SupportValue::None),
                &SupportTriggers::new(),
            )
            .into_iter()
            .next()
            .expect("未满一支")
            .0
            .zone;
        let before =
            DefState::from_players(PlayerDefs::new().with_supports(supports), PlayerDefs::new());
        let pending: SupportPending =
            queued(discard_support, Some(n(401)), n(401), EffectSource::Support);
        let mut set = PendingSet::new();
        set.push_me(FlowPending::Support(pending));
        let after = execute_pending(&before, &set);
        assert!(after.me().supports().is_empty());
    }

    #[test]
    fn hand_card_state_pending_applies_to_the_table() {
        let before =
            DefState::from_players(PlayerDefs::new().with_hand(hand(&[101])), PlayerDefs::new());
        let pending: HandCardStatePending = queued(
            attach_hand_status,
            Some(n(101)),
            n(101),
            EffectSource::CharacterEquipmentOrStatus,
        );
        let mut set = PendingSet::new();
        set.push_me(FlowPending::HandCardState(pending));
        let after = execute_pending(&before, &set);
        assert!(after
            .me()
            .hand_states()
            .contains_state(n(101), &TEST_STATUS));
    }

    #[test]
    fn pending_set_routes_me_and_opp_independently() {
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new().with_characters(characters(&[401])),
        );
        let me_pending: CharacterPending = queued(
            set_hp_three,
            Some(n(201)),
            n(201),
            EffectSource::CharacterCard,
        );
        let opp_pending: CharacterPending = queued(
            set_hp_three,
            Some(n(401)),
            n(401),
            EffectSource::CharacterCard,
        );
        let mut set = PendingSet::new();
        set.push_me(FlowPending::Character {
            character: n(201),
            pending: me_pending,
        });
        set.push_opp(FlowPending::Character {
            character: n(401),
            pending: opp_pending,
        });
        assert_eq!(set.len(), Nat::new(2));
        assert!(!set.is_empty());
        let after = execute_pending(&before, &set);
        assert_eq!(
            after
                .me()
                .characters()
                .slot_of(n(201))
                .expect("有")
                .state
                .hp(),
            Nat::new(3)
        );
        assert_eq!(
            after
                .opp()
                .characters()
                .slot_of(n(401))
                .expect("有")
                .state
                .hp(),
            Nat::new(3)
        );
    }

    #[test]
    fn empty_set_is_a_pure_noop() {
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new(),
        );
        assert!(PendingSet::new().is_empty());
        assert_eq!(PendingSet::new().len(), Nat::ZERO);
        assert_eq!(execute_pending(&before, &PendingSet::new()), before);
    }

    #[test]
    fn character_pending_for_missing_character_is_ignored() {
        let before = DefState::from_players(PlayerDefs::new(), PlayerDefs::new());
        let pending: CharacterPending = queued(
            set_hp_three,
            Some(n(999)),
            n(999),
            EffectSource::CharacterCard,
        );
        let mut set = PendingSet::new();
        set.push_me(FlowPending::Character {
            character: n(999),
            pending,
        });
        assert_eq!(execute_pending(&before, &set), before);
    }

    // ------------------------------------------------------------------
    // 结算帧执行（D196：插入的子结算先做完，再回到插入点继续）
    // ------------------------------------------------------------------

    /// 「顺序记号」角色区：生命 0 / 上限 MAX（记号 hp * 10 + digit 不受钳制）。
    fn marking_characters(value: u32) -> CharacterZone {
        CharacterZone::try_from_slots(vec![
            crate::families::character::zones::field::CharacterSlot::new(
                n(value),
                CharacterState::new(Nat::ZERO, Nat::MAX, Nat::ZERO, Nat::new(2)),
            ),
        ])
        .expect("合法槽位")
    }

    fn mark_one(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let scaled = gi_genius_invokation_engine::nmul!(state.hp(), Nat::new(10));
        *state = state.hp_set(gi_genius_invokation_engine::nadd!(scaled, Nat::new(1)));
    }

    fn mark_two(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let scaled = gi_genius_invokation_engine::nmul!(state.hp(), Nat::new(10));
        *state = state.hp_set(gi_genius_invokation_engine::nadd!(scaled, Nat::new(2)));
    }

    fn mark_three(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        let scaled = gi_genius_invokation_engine::nmul!(state.hp(), Nat::new(10));
        *state = state.hp_set(gi_genius_invokation_engine::nadd!(scaled, Nat::new(3)));
    }

    #[test]
    fn execute_frames_drains_inserted_sub_resolution_then_returns() {
        let before = DefState::from_players(
            PlayerDefs::new().with_characters(marking_characters(201)),
            PlayerDefs::new(),
        );
        let mut frames: Resolution<CharacterEffect> = Resolution::new(vec![
            queued(mark_one, Some(n(201)), n(201), EffectSource::CharacterCard),
            queued(mark_two, Some(n(201)), n(201), EffectSource::CharacterCard),
        ]);
        // 模拟「当前结算位置插入子结算」：子结算先做，再回到插入点继续。
        frames.insert(vec![queued(
            mark_three,
            Some(n(201)),
            n(201),
            EffectSource::CharacterCard,
        )]);
        let after = execute_frames(&before, n(201), &mut frames).expect("角色在区里");
        assert_eq!(
            after
                .me()
                .characters()
                .slot_of(n(201))
                .expect("有")
                .state
                .hp(),
            Nat::new(312)
        );
        // 帧执行完
        assert!(frames.is_finished());
        // 纯函数：入参不变
        assert_eq!(
            before
                .me()
                .characters()
                .slot_of(n(201))
                .expect("有")
                .state
                .hp(),
            Nat::ZERO
        );
    }

    #[test]
    fn execute_frames_returns_none_when_character_is_not_present() {
        let before = DefState::new();
        let mut frames: Resolution<CharacterEffect> = Resolution::new(Vec::new());
        assert!(execute_frames(&before, n(999), &mut frames).is_none());
    }

    #[test]
    fn execute_frames_works_on_the_opponent_side_too() {
        let before = DefState::from_players(
            PlayerDefs::new(),
            PlayerDefs::new().with_characters(marking_characters(401)),
        );
        let mut frames: Resolution<CharacterEffect> = Resolution::new(vec![queued(
            mark_one,
            Some(n(401)),
            n(401),
            EffectSource::CharacterCard,
        )]);
        let after = execute_frames(&before, n(401), &mut frames).expect("角色在区里");
        assert_eq!(
            after
                .opp()
                .characters()
                .slot_of(n(401))
                .expect("有")
                .state
                .hp(),
            Nat::new(1)
        );
        assert_eq!(after.me(), before.me());
    }

    #[test]
    fn combat_status_helper_is_used_by_fixture_shapes() {
        // 确认测试夹具的出战状态区形状（防止 execute_pending 测试与夹具口径漂移）
        // 布尔类状态：层数归一为 None（有该状态 = 1 层），持续回合单独存。
        let zone = combat_status(n(201), &[(&TEST_STATUS, None, Some(n(2)))]);
        assert_eq!(zone.attached_to(), Some(n(201)));
        assert_eq!(zone.layers(&TEST_STATUS), Some(None));
        assert_eq!(zone.duration(&TEST_STATUS), Some(n(2)));
        let _ = entity(1);
    }
}
