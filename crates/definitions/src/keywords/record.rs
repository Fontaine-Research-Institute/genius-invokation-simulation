//! **keywords 的记录内容类型 + 统一记录器接线**（D223 / D224）。
//!
//! 关键字的回放轨迹有两层：
//! - [super::trace::Trace]：keywords 自己的**结构化轨迹**（只读纯数据；可以精确断言）；
//! - [KeywordRecord]：写进 [crate::records::Recorder] 的**统一记录内容**（极薄容器：
//!   步号 + 任意内容；显示由本模块的 ShowRecord 实现决定）。
//!
//! 记录器不含业务逻辑：这里只把 Trace 翻译成记录条目（不改变结算结果）。
//!
//! ## 依据
//!
//! D221（插入式结算回放）、D223 / D224（记录 API 极薄：属于哪一步 + 任意内容；
//! 内容类型自己定义显示）。

use crate::records::{Recorder, ShowRecord};
use gi_genius_invokation_engine::Nat;

use super::effect::EffectEnv;
use super::step::{Step, StepOutcome};
use super::trace::{
    execute_step_me_traced, execute_step_opp_traced, execute_steps_me_traced,
    execute_steps_opp_traced, OpTrace, ReactionTrace, Trace,
};

/// keywords 的记录内容（**本模块自己的类型**；记录器不预设种类）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeywordRecord {
    /// 一步开始。
    Step {
        /// 步号。
        index: Nat,
    },
    /// 一个关键字操作。
    Op {
        /// 步号。
        step: Nat,
        /// 步内下标。
        index: Nat,
        /// 操作种类（中文）。
        kind: &'static str,
        /// 解析后的入参 / 目标 / 结果（文本）。
        detail: String,
    },
    /// 一条待执行（步末统一执行）。
    Pending {
        /// 步号。
        step: Nat,
        /// 哪个区。
        zone: &'static str,
        /// 详情（owner / 来源类别 / 进场）。
        detail: String,
    },
    /// 一条反应子结算（R4 / R2：步边界插入执行；D204 步内不插）。
    Reaction {
        /// 步号。
        step: Nat,
        /// 详情（反应名 / 非伤害 / 概率 / 结果）。
        detail: String,
    },
    /// 一次插入（D204：只发生在步与步之间）。
    Insert {
        /// 发生在第几步之后。
        after_step: Nat,
        /// 详情。
        detail: String,
    },
}

impl ShowRecord for KeywordRecord {
    fn show(&self, out: &mut String) {
        match self {
            KeywordRecord::Step { index } => {
                out.push_str(&format!("步 {} 开始", index.get()));
            }
            KeywordRecord::Op {
                index,
                kind,
                detail,
                ..
            } => {
                out.push_str(&format!("操作 #{} {}：{}", index.get(), kind, detail));
            }
            KeywordRecord::Pending { zone, detail, .. } => {
                out.push_str(&format!("待执行 [{}] {}", zone, detail));
            }
            KeywordRecord::Reaction { detail, .. } => {
                out.push_str(&format!("反应子结算：{}", detail));
            }
            KeywordRecord::Insert {
                after_step,
                detail,
            } => {
                out.push_str(&format!("步 {} 之后插入：{}", after_step.get(), detail));
            }
        }
    }
}

/// 一条操作轨迹 -> 一行详情文本（目标 / 入参 / 结果 / 概率）。
fn op_detail(op: &OpTrace) -> String {
    let mut detail = String::new();
    if !op.targets.is_empty() {
        detail.push_str("目标 [");
        for (index, target) in op.targets.iter().enumerate() {
            if index > 0 {
                detail.push_str(", ");
            }
            detail.push_str(&format!("{:?}:{}", target.side, target.entity.get()));
        }
        detail.push_str("] ");
    }
    if !op.inputs.is_empty() {
        detail.push_str("入参 {");
        for (index, input) in op.inputs.iter().enumerate() {
            if index > 0 {
                detail.push_str(", ");
            }
            detail.push_str(&format!("{}={}", input.name, input.value));
        }
        detail.push_str("} ");
    }
    if !op.outcomes.is_empty() {
        detail.push_str(&format!("结果 {:?} ", op.outcomes));
    }
    detail.push_str(&format!(
        "概率 {}/{}",
        op.probability.num().get(),
        op.probability.den().get()
    ));
    detail
}

/// 一条反应轨迹 -> 一行详情文本（反应名 / 非伤害 / 概率 / 结果）。
fn reaction_detail(reaction: &ReactionTrace) -> String {
    format!(
        "{}（{} -> {}，非伤害 {}，概率 {}/{}）结果 {:?}",
        reaction.reaction,
        reaction.source.get(),
        reaction.target.get(),
        reaction.non_damage,
        reaction.probability.num().get(),
        reaction.probability.den().get(),
        reaction.outcomes
    )
}

/// **把结构化轨迹写进统一记录器**（只读 Trace；不改结算结果）。
pub fn record_trace(trace: &Trace, recorder: &mut Recorder<KeywordRecord>) {
    for step in &trace.steps {
        recorder.push(step.index, KeywordRecord::Step { index: step.index });
        for branch in &step.branches {
            for op in &branch.ops {
                record_op(step.index, op, recorder);
            }
            for reaction in &branch.reactions {
                recorder.push(
                    step.index,
                    KeywordRecord::Reaction {
                        step: step.index,
                        detail: reaction_detail(reaction),
                    },
                );
            }
            for pending in &branch.pending {
                recorder.push(
                    step.index,
                    KeywordRecord::Pending {
                        step: step.index,
                        zone: pending.zone,
                        detail: format!(
                            "owner {} / 来源 {:?} / 进场 {}",
                            pending.owner.get(),
                            pending.source_kind,
                            pending.entry.get()
                        ),
                    },
                );
            }
        }
        // 步内插入（D204：正常恒为空；仍显式记录，便于回放核对）。
        for insertion in &step.intra_step_insertions {
            recorder.push(
                step.index,
                KeywordRecord::Insert {
                    after_step: step.index,
                    detail: format!("步内插入（违反 D204）{} 条", insertion.executed.get()),
                },
            );
        }
        // 步间插入（D204：插入只能发生在这里）。
        for insertion in &step.insertions {
            recorder.push(
                insertion.after_step,
                KeywordRecord::Insert {
                    after_step: insertion.after_step,
                    detail: format!("执行 {} 条待执行", insertion.executed.get()),
                },
            );
        }
    }
}

/// 一条操作（含 Repeat 的子操作）写进记录器。
fn record_op(step: Nat, op: &OpTrace, recorder: &mut Recorder<KeywordRecord>) {
    recorder.push(
        step,
        KeywordRecord::Op {
            step,
            index: op.index,
            kind: op.kind,
            detail: op_detail(op),
        },
    );
    for child in &op.children {
        record_op(step, child, recorder);
    }
}

/// **执行一步（我方视角）并记录**：行为与 [super::step::execute_step_me] 完全一致。
pub fn execute_step_me_recorded(
    state: &crate::state::DefState,
    step: &Step,
    env: &EffectEnv<'_>,
    recorder: &mut Recorder<KeywordRecord>,
) -> StepOutcome {
    let traced = execute_step_me_traced(state, step, env);
    record_trace(&traced.trace, recorder);
    traced.outcome
}

/// **执行一步（敌方视角）并记录**。
pub fn execute_step_opp_recorded(
    state: &crate::state::DefState,
    step: &Step,
    env: &EffectEnv<'_>,
    recorder: &mut Recorder<KeywordRecord>,
) -> StepOutcome {
    let traced = execute_step_opp_traced(state, step, env);
    record_trace(&traced.trace, recorder);
    traced.outcome
}

/// **顺序执行多步（我方视角）并记录**（步间插入也有记录）。
pub fn execute_steps_me_recorded(
    state: &crate::state::DefState,
    steps: &[Step],
    env: &EffectEnv<'_>,
    recorder: &mut Recorder<KeywordRecord>,
) -> StepOutcome {
    let traced = execute_steps_me_traced(state, steps, env);
    record_trace(&traced.trace, recorder);
    traced.outcome
}

/// **顺序执行多步（敌方视角）并记录**。
pub fn execute_steps_opp_recorded(
    state: &crate::state::DefState,
    steps: &[Step],
    env: &EffectEnv<'_>,
    recorder: &mut Recorder<KeywordRecord>,
) -> StepOutcome {
    let traced = execute_steps_opp_traced(state, steps, env);
    record_trace(&traced.trace, recorder);
    traced.outcome
}

// ======================================================================
// 测试（D221 / D223 / D224）
// ======================================================================

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::families::character::attachment::{
        Attachment, AttachmentAllowSet, ReactionRegistry,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::{DamageEnv, ReactionRule};
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::reactions::effect::EffectContext;
    use crate::families::character::trigger::{CharacterEffect, CharacterTiming, CharacterTriggers};
    use crate::families::character::types::DamageType;
    use crate::keywords::effect::TriggerBook;
    use crate::keywords::op::{DamageArgs, DamageSourceKind, KeywordOp, RepeatArgs};
    use crate::keywords::target::Perspective;
    use crate::state::DefState;
    use gi_genius_invokation_engine::EntityId;
    use crate::keywords::step::{execute_step_me, Step};
    use crate::keywords::target::TargetSelector;
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{characters, n};
    use crate::state::PlayerDefs;
    use crate::timeline::{Event, Trigger};

    static NO_ELEMENTS: [Element; 0] = [];

    fn env<'a>(
        subject: EntityId,
        viewer: Perspective,
        registry: &'a ReactionRegistry,
        me_characters: &'a CharacterTriggers,
        opp_characters: &'a CharacterTriggers,
    ) -> EffectEnv<'a> {
        let damage = DamageEnv {
            allow: AttachmentAllowSet::new(&NO_ELEMENTS),
            rules: &[],
            registry,
        };
        let mut book: TriggerBook<'a> = TriggerBook::empty();
        book.me_characters = me_characters;
        book.opp_characters = opp_characters;
        EffectEnv::new(subject, viewer, damage).with_triggers(book)
    }

    fn env_with_rules<'a>(
        subject: EntityId,
        viewer: Perspective,
        registry: &'a ReactionRegistry,
        rules: &'a [ReactionRule],
        context: &'a EffectContext,
    ) -> EffectEnv<'a> {
        let damage = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules,
            registry,
        };
        EffectEnv::new(subject, viewer, damage).with_effect_context(context)
    }

    fn duel() -> DefState {
        DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(characters(&[301])),
        )
    }

    fn damage_op(targets: TargetSelector, amount: Nat) -> KeywordOp {
        KeywordOp::Damage(DamageArgs::new(
            targets,
            amount,
            DamageType::Physical,
            DamageSourceKind::Other,
        ))
    }

    fn heal_and_count(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        *state = state.hp_add(n(5));
    }

    fn triggers_with(effect: CharacterEffect) -> CharacterTriggers {
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            n(301),
            EffectSource::CharacterCard,
            Nat::ZERO,
            effect,
        ));
        triggers
    }

    fn hp_of(state: &DefState, side: Perspective, character: EntityId) -> Nat {
        crate::keywords::target::player_of(state, side)
            .characters()
            .slot_of(character)
            .expect("角色在区里")
            .state
            .hp()
    }

    #[test]
    fn recorded_step_writes_step_and_ops_in_order() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![
            damage_op(TargetSelector::opponent_active(), n(3)),
            damage_op(TargetSelector::opponent_active(), n(2)),
        ]);
        let mut recorder: Recorder<KeywordRecord> = Recorder::new();
        let outcome = execute_step_me_recorded(&state, &step, &effect, &mut recorder);
        let kinds: Vec<&'static str> = recorder
            .entries()
            .iter()
            .map(|record| match record.content {
                KeywordRecord::Step { .. } => "步",
                KeywordRecord::Op { kind, .. } => kind,
                KeywordRecord::Pending { .. } => "待执行",
                KeywordRecord::Reaction { .. } => "反应",
                KeywordRecord::Insert { .. } => "插入",
            })
            .collect();
        assert_eq!(kinds, vec!["步", "造成伤害", "造成伤害"]);
        // 每条记录的步号都正确（同一步）。
        assert!(recorder
            .entries()
            .iter()
            .all(|record| record.step == Nat::ZERO));
        assert!(recorder.render().contains("步 0"));
        assert_eq!(
            hp_of(&outcome.single().expect("单支").state, Perspective::Opp, n(301)),
            n(5)
        );
    }

    #[test]
    fn recorded_sequence_records_insertions_after_steps() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = triggers_with(heal_and_count);
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let steps = vec![
            Step::new(vec![damage_op(TargetSelector::opponent_active(), n(5))]),
            Step::new(vec![damage_op(TargetSelector::opponent_active(), n(5))]),
        ];
        let mut recorder: Recorder<KeywordRecord> = Recorder::new();
        let outcome = execute_steps_me_recorded(&state, &steps, &effect, &mut recorder);
        let insertions: Vec<Nat> = recorder
            .entries()
            .iter()
            .filter_map(|record| match record.content {
                KeywordRecord::Insert { after_step, .. } => Some(after_step),
                _ => None,
            })
            .collect();
        assert_eq!(insertions, vec![Nat::ZERO, Nat::new(1)]);
        // 步间插入让两次伤害之间发生治疗：最终 10 血。
        assert_eq!(
            hp_of(&outcome.single().expect("单支").state, Perspective::Opp, n(301)),
            n(10)
        );
    }

    #[test]
    fn recording_does_not_change_the_outcome() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![damage_op(
            TargetSelector::opponent_active(),
            n(4),
        )]);
        let plain = execute_step_me(&state, &step, &effect);
        let mut recorder: Recorder<KeywordRecord> = Recorder::new();
        let recorded = execute_step_me_recorded(&state, &step, &effect, &mut recorder);
        assert_eq!(
            plain.single().expect("单支").state,
            recorded.single().expect("单支").state
        );
        assert_eq!(plain.alloc.next(), recorded.alloc.next());
        assert!(!recorder.is_empty());
    }

    #[test]
    fn repeat_children_are_recorded_in_the_same_step() {
        fn one_damage(_index: Nat) -> Vec<KeywordOp> {
            vec![damage_op(TargetSelector::opponent_active(), n(1))]
        }
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![KeywordOp::Repeat(RepeatArgs::indexed(
            n(3),
            one_damage,
        ))]);
        let mut recorder: Recorder<KeywordRecord> = Recorder::new();
        let outcome = execute_step_me_recorded(&state, &step, &effect, &mut recorder);
        let ops: Vec<&'static str> = recorder
            .entries()
            .iter()
            .filter_map(|record| match record.content {
                KeywordRecord::Op { kind, .. } => Some(kind),
                _ => None,
            })
            .collect();
        // 重复本体 + 3 个子伤害，全部记在同一步。
        assert_eq!(ops, vec!["重复", "造成伤害", "造成伤害", "造成伤害"]);
        assert!(recorder
            .entries()
            .iter()
            .all(|record| record.step == Nat::ZERO));
        assert_eq!(
            hp_of(&outcome.single().expect("单支").state, Perspective::Opp, n(301)),
            n(7)
        );
    }

    #[test]
    fn recorded_step_includes_reaction_records() {
        // 关键字一步路径在步边界执行超载（强制切换），统一记录器里应有 Reaction 记录。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let mut active = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        active.attachment = Attachment::with(Element::Electro);
        let opp = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![
                CharacterSlot::new(n(401), active),
                CharacterSlot::new(n(402), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
                CharacterSlot::new(n(403), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
            ])
            .expect("合法角色区"),
        );
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            opp,
        );
        let effect = env_with_rules(n(101), Perspective::Me, &registry, &rules, &context);
        let step = Step::new(vec![KeywordOp::Damage(DamageArgs::new(
            TargetSelector::opponent_active(),
            n(2),
            DamageType::Elemental(Element::Pyro),
            DamageSourceKind::Other,
        ))]);
        let mut recorder: Recorder<KeywordRecord> = Recorder::new();
        let outcome = execute_step_me_recorded(&state, &step, &effect, &mut recorder);
        // 结算结果与不记录一致：超载 +2 -> 掉 4；强制切换到 402。
        let branch = outcome.single().expect("单支");
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(401)), n(6));
        assert_eq!(
            branch.state.opp().characters().active_entity(),
            Some(n(402))
        );
        let reactions: Vec<String> = recorder
            .entries()
            .iter()
            .filter_map(|record| match &record.content {
                KeywordRecord::Reaction { detail, .. } => Some(detail.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(reactions.len(), 1);
        assert!(reactions[0].contains("超载"));
        assert!(recorder.render().contains("反应子结算"));
    }
}
