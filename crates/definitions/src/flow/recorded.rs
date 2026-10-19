//! **flow 的记录接线**（D223 / D224 / D225 / D227）：在「使用技能」「打出行动牌」这些
//! 干活处调用统一记录 API。
//!
//! 只加输出、不改流程语义：先走原来的 [use_skill_me] / [play_card_me] 等入口，成功后再把
//! 该次结算产生的流程级事件（[FlowRecord]）原样 push 进调用方给的 [Recorder]。
//! 事件内容由 flow/record.rs 定义（D225：内容类型单独文件）；本文件只负责写。
//!
//! 记录器可整体裁剪：关闭 records feature 时 [Recorder::push] 为空操作（零开销）。
//!
//! 打牌带「战斗行动（行动牌）」规则时（S8.8），用
//! [play_card_me_recorded_with_timing] / [play_card_opp_recorded_with_timing] 传入
//! `has_combat_action_tag`；旧入口保持快速行动假定与签名不变。

use crate::families::character::skill_flow::SkillFrames;
use crate::flow::play_card::{
    play_card_me, play_card_me_with_timing, play_card_opp, play_card_opp_with_timing, PlayCardEnv,
    PlayCardError, PlayCardOutcome, PlayCardRequest,
};
use crate::flow::record::FlowRecord;
use crate::flow::skill_use::{
    use_skill_me, use_skill_opp, SkillFlowEnv, SkillOutcome, SkillUseError, SkillUseRequest,
};
use crate::records::Recorder;
use crate::state::DefState;
use gi_genius_invokation_engine::Nat;

/// 把一次结算的事件按「调用方给的步号」写进记录器（顺序 = 事件发生顺序）。
fn push_events(events: &[FlowRecord], step: Nat, recorder: &mut Recorder<FlowRecord>) {
    for event in events {
        recorder.push(step, event.clone());
    }
}

/// **使用技能（我方）并记录**：行为与 [use_skill_me] 完全一致，成功时记录流程级事件。
pub fn use_skill_me_recorded(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
    step: Nat,
    recorder: &mut Recorder<FlowRecord>,
) -> Result<SkillOutcome, SkillUseError> {
    let outcome = use_skill_me(state, env, request, settlement)?;
    push_events(&outcome.events, step, recorder);
    Ok(outcome)
}

/// **使用技能（敌方）并记录**：行为与 [use_skill_opp] 完全一致，成功时记录流程级事件。
pub fn use_skill_opp_recorded(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
    step: Nat,
    recorder: &mut Recorder<FlowRecord>,
) -> Result<SkillOutcome, SkillUseError> {
    let outcome = use_skill_opp(state, env, request, settlement)?;
    push_events(&outcome.events, step, recorder);
    Ok(outcome)
}

/// **打出行动牌（我方）并记录**：行为与 [play_card_me] 完全一致，成功时记录流程级事件。
pub fn play_card_me_recorded(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    step: Nat,
    recorder: &mut Recorder<FlowRecord>,
) -> Result<PlayCardOutcome, PlayCardError> {
    let outcome = play_card_me(state, env, request)?;
    push_events(&outcome.events, step, recorder);
    Ok(outcome)
}

/// **打出行动牌（敌方）并记录**：行为与 [play_card_opp] 完全一致，成功时记录流程级事件。
pub fn play_card_opp_recorded(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    step: Nat,
    recorder: &mut Recorder<FlowRecord>,
) -> Result<PlayCardOutcome, PlayCardError> {
    let outcome = play_card_opp(state, env, request)?;
    push_events(&outcome.events, step, recorder);
    Ok(outcome)
}

/// **打出行动牌（我方）并记录（按牌是否带「战斗行动」规则判时机）**（S3.1 / S8.8）：
/// 行为与 [play_card_me_with_timing] 一致，成功时记录流程级事件。
///
/// 旧入口 [play_card_me_recorded] 仍是「快速行动」假定；带「战斗行动（行动牌）」规则的牌走本入口。
pub fn play_card_me_recorded_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    has_combat_action_tag: bool,
    step: Nat,
    recorder: &mut Recorder<FlowRecord>,
) -> Result<PlayCardOutcome, PlayCardError> {
    let outcome = play_card_me_with_timing(state, env, request, has_combat_action_tag)?;
    push_events(&outcome.events, step, recorder);
    Ok(outcome)
}

/// **打出行动牌（敌方）并记录（按牌是否带「战斗行动」规则判时机）**：行为与
/// [play_card_opp_with_timing] 一致，成功时记录流程级事件。
pub fn play_card_opp_recorded_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    has_combat_action_tag: bool,
    step: Nat,
    recorder: &mut Recorder<FlowRecord>,
) -> Result<PlayCardOutcome, PlayCardError> {
    let outcome = play_card_opp_with_timing(state, env, request, has_combat_action_tag)?;
    push_events(&outcome.events, step, recorder);
    Ok(outcome)
}

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::base::{Element, SkillId};
    use crate::families::action_card::areas::support::SupportValue;
    use crate::families::action_card::def::PlayCondition;
    use crate::families::character::attachment::{AttachmentAllowSet, ReactionRegistry};
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::DamageEnv;
    use crate::families::character::damage_chain::executor::{EntityAlloc, ReactionTriggers};
    use crate::families::character::skill::{SkillCost, SkillDef};
    use crate::families::character::skill_flow::{PreparedSkills, SkillTriggers, SkillUsePolicy};
    use crate::families::character::types::{DamageType, SkillKind};
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::flow::play_card::{CardPlacement, KeywordEnv, PlayCardEnv, PlayCardRequest};
    use crate::flow::skill_use::{SkillEffectBody, SkillEffectPlan, SkillFlowEnv, SkillUseRequest};
    use crate::generated::Localized;
    use crate::keywords::effect::TriggerBook;
    use crate::keywords::op::{DamageArgs, DamageSourceKind, KeywordOp};
    use crate::keywords::step::Step;
    use crate::keywords::target::{Perspective, TargetSelector};
    use crate::reactions::effect::EffectContext;
    use crate::state::testing::{hand, n};
    use crate::state::PlayerDefs;
    use crate::timeline::Resolution;
    use gi_genius_invokation_engine::Nat;

    fn noop() {}

    static FREE_SKILL: SkillDef = SkillDef {
        id: SkillId::new(12161),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::ElementalSkill,
        cost: SkillCost::FREE,
        effect: noop,
    };

    fn duel() -> DefState {
        let me = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(
                n(201),
                CharacterState::new(n(10), n(10), Nat::ZERO, n(2)),
            )])
            .expect("合法槽位"),
        );
        let opp = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(
                n(401),
                CharacterState::new(n(10), n(10), Nat::ZERO, n(2)),
            )])
            .expect("合法槽位"),
        );
        DefState::from_players(me, opp)
    }

    fn hp_of(
        state: &DefState,
        side: Perspective,
        character: gi_genius_invokation_engine::EntityId,
    ) -> Nat {
        crate::keywords::target::player_of(state, side)
            .characters()
            .slot_of(character)
            .expect("角色在区里")
            .state
            .hp()
    }

    // ------------------------------------------------------------------
    // 技能：旧路径 / 一步路径
    // ------------------------------------------------------------------

    struct SkillBundle {
        triggers: SkillTriggers,
        target_triggers: SkillTriggers,
        reaction_triggers: ReactionTriggers,
        registry: ReactionRegistry,
        context: EffectContext,
    }

    impl SkillBundle {
        fn new() -> SkillBundle {
            SkillBundle {
                triggers: SkillTriggers::new(),
                target_triggers: SkillTriggers::new(),
                reaction_triggers: ReactionTriggers::none(),
                registry: ReactionRegistry::new(),
                context: EffectContext::new(true, Nat::ZERO, Nat::ZERO),
            }
        }

        fn env(&self) -> SkillFlowEnv<'_> {
            SkillFlowEnv {
                triggers: crate::flow::skill_use::SkillTriggerBook {
                    me: &self.triggers,
                    opp: &self.target_triggers,
                },
                keyword_triggers: TriggerBook::empty(),
                allow: AttachmentAllowSet::STANDARD,
                rules: &[],
                reaction_registry: &self.registry,
                reaction_triggers: &self.reaction_triggers,
                mastery: Nat::ZERO,
                increases: &[],
                effect_context: &self.context,
                alloc: EntityAlloc::starting_at(n(1000)),
                policy: SkillUsePolicy::DEFAULT,
                energy_gain_bonus: &[],
                same_color: None,
                defeat_cleanup: crate::flow::hooks::NO_DEFEAT_CLEANUP,
            }
        }
    }

    fn single_damage_step() -> Step {
        Step::new(vec![KeywordOp::Damage(DamageArgs::new(
            TargetSelector::opponent_active(),
            n(2),
            DamageType::Physical,
            DamageSourceKind::Other,
        ))])
    }

    #[test]
    fn plan_path_records_the_skill_use_summary() {
        let before = duel();
        let bundle = SkillBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = Resolution::new(Vec::new());
        let mut recorder: Recorder<FlowRecord> = Recorder::new();
        let outcome = use_skill_me_recorded(
            &before,
            &bundle.env(),
            &request,
            &mut settlement,
            n(3),
            &mut recorder,
        )
        .expect("无消耗技能可用");
        assert_eq!(outcome.branches.len(), 1);
        // 旧路径：一条汇总记录（没有一步、没有 flush；无 after 触发也没有插入）。
        assert_eq!(recorder.len(), n(1));
        assert_eq!(recorder.entries()[0].step, n(3));
        match &recorder.entries()[0].content {
            FlowRecord::UseSkill {
                skill,
                effect,
                branches,
                ..
            } => {
                assert_eq!(*skill, FREE_SKILL.id);
                assert_eq!(*effect, "旧路径（效果体计划）");
                assert_eq!(*branches, n(1));
            }
            other => panic!("期望使用技能汇总，得到 {other:?}"),
        }
        assert!(recorder.render().contains("[步 3]"));
        // 行为不变：入参局面不变。
        assert_eq!(before, duel());
    }

    #[test]
    fn step_path_records_step_flush_and_summary_in_one_step() {
        let before = duel();
        let bundle = SkillBundle::new();
        let prepared = PreparedSkills::new();
        let step = single_damage_step();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Step(&step),
        };
        let mut settlement = Resolution::new(Vec::new());
        let mut recorder: Recorder<FlowRecord> = Recorder::new();
        let outcome = use_skill_me_recorded(
            &before,
            &bundle.env(),
            &request,
            &mut settlement,
            n(5),
            &mut recorder,
        )
        .expect("无消耗技能可用");
        // 一步伤害落地（2 点物理）。
        assert_eq!(
            hp_of(
                &outcome.single().expect("一支").state,
                Perspective::Opp,
                n(401)
            ),
            n(8)
        );
        // 记录：一步 -> flush -> 汇总，全部属于调用方给的步号 5。
        let kinds: Vec<&'static str> = recorder
            .entries()
            .iter()
            .map(|entry| match entry.content {
                FlowRecord::Step { .. } => "一步",
                FlowRecord::Flush { .. } => "flush",
                FlowRecord::UseSkill { .. } => "使用技能",
                _ => "其它",
            })
            .collect();
        assert_eq!(kinds, vec!["一步", "flush", "使用技能"]);
        assert!(recorder.entries().iter().all(|entry| entry.step == n(5)));
        assert!(recorder.render().contains("关键字一步"));
        // 按步读：只有这一步有记录。
        assert_eq!(recorder.step_entries(n(5)).count(), 3);
        assert_eq!(recorder.step_entries(n(4)).count(), 0);
        // 行为一致：与不记录的入口结果相同。
        let mut plain_settlement = Resolution::new(Vec::new());
        let plain = use_skill_me(&before, &bundle.env(), &request, &mut plain_settlement)
            .expect("无消耗技能可用");
        assert_eq!(
            plain.single().expect("一支").state,
            outcome.single().expect("一支").state
        );
    }

    #[test]
    fn failed_skill_use_records_nothing() {
        let empty = DefState::new();
        let bundle = SkillBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = Resolution::new(Vec::new());
        let mut recorder: Recorder<FlowRecord> = Recorder::new();
        let error = use_skill_me_recorded(
            &empty,
            &bundle.env(),
            &request,
            &mut settlement,
            n(0),
            &mut recorder,
        );
        assert!(error.is_err());
        assert!(recorder.is_empty());
        let _ = Element::Hydro;
    }

    // ------------------------------------------------------------------
    // 行动牌：事件牌效果体走一步并记录
    // ------------------------------------------------------------------

    fn event_card_state() -> DefState {
        DefState::from_players(
            PlayerDefs::new()
                .with_characters(
                    CharacterZone::try_from_slots(vec![CharacterSlot::new(
                        n(201),
                        CharacterState::new(n(10), n(10), Nat::ZERO, n(2)),
                    )])
                    .expect("合法槽位"),
                )
                .with_hand(hand(&[211])),
            PlayerDefs::new().with_characters(
                CharacterZone::try_from_slots(vec![CharacterSlot::new(
                    n(401),
                    CharacterState::new(n(10), n(10), Nat::ZERO, n(2)),
                )])
                .expect("合法槽位"),
            ),
        )
    }

    fn unconditional(_state: &DefState, _viewer: Perspective) -> PlayCondition {
        PlayCondition::Ok
    }

    #[test]
    fn play_card_records_event_step_and_summary() {
        let before = event_card_state();
        let triggers = crate::flow::play_card::PlayTriggers::new();
        let supports = crate::families::action_card::areas::support::SupportTriggers::new();
        let equipment = crate::families::character::zones::equipment::EquipmentTriggers::new();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let keywords = KeywordEnv {
            triggers: TriggerBook::empty(),
            damage: DamageEnv {
                allow: AttachmentAllowSet::STANDARD,
                rules: &[],
                registry: &registry,
            },
            mastery: Nat::ZERO,
            increases: &[],
            effect_context: &context,
            alloc: EntityAlloc::starting_at(n(1000)),
        };
        let triggers_me = crate::flow::play_card::PlayTriggers::new();
        let env = PlayCardEnv {
            triggers: crate::flow::play_card::PlayTriggerBook {
                me: &triggers,
                opp: &triggers_me,
            },
            support_triggers: &supports,
            equipment_triggers: &equipment,
            keywords: Some(keywords),
            defeat_cleanup: crate::flow::hooks::NO_DEFEAT_CLEANUP,
        };
        let step = Step::new(vec![KeywordOp::Damage(DamageArgs::new(
            TargetSelector::opponent_active(),
            n(3),
            DamageType::Physical,
            DamageSourceKind::Other,
        ))]);
        let request = PlayCardRequest {
            entity: n(211),
            cost: Vec::new(),
            same_color: None,
            placement: CardPlacement::Event {
                effect: Some(&step),
            },
            play_condition: Some(unconditional),
        };
        let mut recorder: Recorder<FlowRecord> = Recorder::new();
        let outcome =
            play_card_me_recorded(&before, &env, &request, n(7), &mut recorder).expect("打得出");
        assert_eq!(
            hp_of(
                &outcome.single().expect("一支").state,
                Perspective::Opp,
                n(401)
            ),
            n(7)
        );
        let kinds: Vec<&'static str> = recorder
            .entries()
            .iter()
            .map(|entry| match entry.content {
                FlowRecord::Step { .. } => "一步",
                FlowRecord::Flush { .. } => "flush",
                FlowRecord::PlayCard { .. } => "打出牌",
                _ => "其它",
            })
            .collect();
        assert_eq!(kinds, vec!["一步", "flush", "打出牌"]);
        assert!(recorder.entries().iter().all(|entry| entry.step == n(7)));
        assert!(recorder.render().contains("实体 211"));
        // 行为一致：与不记录的入口结果相同。
        let plain = play_card_me(&before, &env, &request).expect("打得出");
        assert_eq!(
            plain.single().expect("一支").state,
            outcome.single().expect("一支").state
        );
        let _ = SupportValue::None;
    }
    #[test]
    fn play_card_recorded_with_timing_carries_the_action_timing_and_still_records() {
        let before = event_card_state();
        let triggers = crate::flow::play_card::PlayTriggers::new();
        let triggers_opp = crate::flow::play_card::PlayTriggers::new();
        let supports = crate::families::action_card::areas::support::SupportTriggers::new();
        let equipment = crate::families::character::zones::equipment::EquipmentTriggers::new();
        let env = PlayCardEnv {
            triggers: crate::flow::play_card::PlayTriggerBook {
                me: &triggers,
                opp: &triggers_opp,
            },
            support_triggers: &supports,
            equipment_triggers: &equipment,
            keywords: None,
            defeat_cleanup: crate::flow::hooks::NO_DEFEAT_CLEANUP,
        };
        let request = PlayCardRequest {
            entity: n(211),
            cost: Vec::new(),
            same_color: None,
            placement: CardPlacement::Event { effect: None },
            play_condition: Some(unconditional),
        };
        // 无「战斗行动」规则 -> 快速行动（S3.1）。
        let mut recorder = Recorder::new();
        let fast =
            play_card_me_recorded_with_timing(&before, &env, &request, false, n(1), &mut recorder)
                .expect("打得出");
        assert_eq!(fast.timing, crate::rules::ops::stage::Timing::Fast);
        assert!(!recorder.entries().is_empty());
        // 带「战斗行动（行动牌）」规则 -> 战斗行动（S8.8）；记录仍写入。
        let mut recorder = Recorder::new();
        let combat =
            play_card_me_recorded_with_timing(&before, &env, &request, true, n(2), &mut recorder)
                .expect("打得出");
        assert_eq!(combat.timing, crate::rules::ops::stage::Timing::Combat);
        assert!(!recorder.entries().is_empty());
        assert!(recorder.entries().iter().all(|entry| entry.step == n(2)));
        // 旧入口保持「快速行动」假定（签名不变）。
        let mut recorder = Recorder::new();
        let legacy =
            play_card_me_recorded(&before, &env, &request, n(3), &mut recorder).expect("打得出");
        assert_eq!(legacy.timing, crate::rules::ops::stage::Timing::Fast);
    }
}
