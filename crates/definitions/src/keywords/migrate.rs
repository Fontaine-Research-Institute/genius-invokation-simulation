//! **迁移桥接**：把旧写法（SkillEffectPlan / EffectOp）转成「关键字函数 + 一步」的表达。
//!
//! 本轮不重写旧流程（D162 / D184 的既有测试必须全绿）；本模块只提供**等价表达**：
//!
//! 1. flow::skill_use::SkillEffectPlan -> [Step]：单段伤害 -> 一步一个「造成伤害」关键字；
//! 2. reactions::effect::EffectOp -> [KeywordOp]：反应效果的一条底层操作 -> 一个关键字调用；
//!    一整条反应分支（Vec<EffectOp>）-> **一个 [Step]**（D208：一次可观察效果内的全部动作同属一步）；
//! 3. 伤害链入口：关键字 cause_damage 内部调用 families::character::damage::damage_with_chain
//!    （不是重复实现）。
//!
//! ## 口径差异（待接入 / 待复核）
//!
//! - SkillEffectPlan 没有「目标选择器」，旧流程固定打对方出战角色 -> 迁成
//!   TargetSelector::opponent_active()；伤害来源类别未知 -> DamageSourceKind::Other（待接入：
//!   由技能定义给 普通攻击 / 元素战技 / 元素爆发）；
//! - EffectOp::Status 的 uses / stack_cap 在状态区没有对应入参（状态区的 attach 只收层数 /
//!   持续回合）-> 迁移时丢掉（待接入：状态区补「可用次数 / 叠加上限」）；
//! - EffectOp::Status 的 stacks 是 Nat：0 -> None（按布尔状态），非 0 -> Some(层数)；
//! - EffectOp 的 TargetScope::TargetOthers（承载方除承载角色外的全部角色）迁成
//!   TargetSelector::AllExceptReacted(Opponent)——要求执行上下文带 reacted（with_reacted）；
//! - EffectOp::HandCardStatus 的 index 是**手牌下标** -> TargetSelector::HandCardIndex；
//!   调用方应先用 reactions::effect::uniform_hand_branches 展开分支（D190），每支再迁一步。

use crate::flow::skill_use::SkillEffectPlan;
use crate::reactions::effect::{EffectOp, Side, StatusTarget, TargetScope};
use gi_genius_invokation_engine::Nat;

use super::op::{
    CardStatusArgs, DamageArgs, DamageSourceKind, DiscardArgs, ForceSwitchArgs, KeywordOp,
    StatusArgs, StatusPlace, SummonArgs, TargetPick,
};
use super::step::Step;
use super::target::{SideScope, TargetSelector};

/// 迁移桥接的结果（一步 + 需要调用方给执行上下文的东西）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MigratedStep {
    /// 迁移后的一步。
    pub step: Step,
    /// 是否需要执行上下文里的「反应承载角色」（EffectOp 的 Reacted / TargetOthers 用）。
    pub needs_reacted: bool,
}

impl MigratedStep {
    fn of(step: Step, needs_reacted: bool) -> MigratedStep {
        MigratedStep {
            step,
            needs_reacted,
        }
    }
}

/// **旧技能效果体** -> 一步（迁移桥接；旧流程行为不变）。
pub fn step_from_skill_effect_plan(plan: SkillEffectPlan) -> Step {
    match plan {
        SkillEffectPlan::None => Step::empty(),
        SkillEffectPlan::Damage { amount, ty } => Step::new(vec![KeywordOp::Damage(
            DamageArgs::new(
                TargetSelector::opponent_active(),
                amount,
                ty,
                DamageSourceKind::Other,
            ),
        )]),
    }
}

impl SkillEffectPlan {
    /// **迁移桥接**：旧技能效果体 -> 一步（等价表达；旧流程不变）。
    ///
    /// 旧流程固定打对方出战角色、没有伤害来源类别信息，所以这里是**保守映射**
    /// （待接入：由 SkillDef 提供来源类别 / 目标范围）。
    pub fn to_step(self) -> Step {
        step_from_skill_effect_plan(self)
    }
}

/// **反应效果的一条底层操作** -> 一个关键字调用（迁移桥接）。
pub fn keyword_op_from_effect_op(op: EffectOp) -> KeywordOp {
    match op {
        EffectOp::Damage { amount, ty, target } => KeywordOp::Damage(DamageArgs::new(
            selector_of(target),
            amount,
            ty,
            DamageSourceKind::Reaction,
        )),
        EffectOp::Status {
            card,
            target,
            stacks,
            uses: _uses,
            stack_cap,
        } => {
            let (place, targets) = match target {
                StatusTarget::Character(scope) => (StatusPlace::Character, selector_of(scope)),
                StatusTarget::Side(side) => (StatusPlace::CombatStatus, active_of(side)),
            };
            KeywordOp::Status(StatusArgs {
                targets,
                place,
                card,
                layers: if stacks == Nat::ZERO {
                    None
                } else {
                    Some(stacks)
                },
                duration: None,
                // Z15：反应执行器的 stack_cap 直接迁到关键字层（None = 不设上限）。
                stack_cap,
            })
        }
        EffectOp::Summon {
            card,
            side,
            uses,
            stack_limit,
        } => KeywordOp::Summon(SummonArgs {
            side: scope_of_side(side),
            card,
            uses: Some(uses),
            points: None,
            effect_kind: None,
            // Z15：同名叠加的可用次数上限（反应执行器与关键字层同一口径）。
            stack_limit,
        }),
        EffectOp::ForceSwitch { target } => KeywordOp::ForceSwitch(ForceSwitchArgs {
            targets: selector_of(target),
        }),
        EffectOp::AddCardToHand { card, side } => {
            KeywordOp::CreateCard(super::op::CreateCardArgs {
                side: scope_of_side(side),
                card,
            })
        }
        EffectOp::HandCardStatus {
            status,
            side,
            index,
        } => KeywordOp::CardStatus(CardStatusArgs {
            targets: TargetSelector::HandCardIndex {
                side: scope_of_side(side),
                index,
            },
            status,
            pick: TargetPick::All,
            // EffectOp::HandCardStatus 没有堆叠上限字段 -> 关键字层默认「没有上限」。
            stack_cap: None,
        }),
    }
}

/// **一条反应分支**（操作清单）-> **一步**（D208：分支里的全部动作同属一步）。
pub fn step_from_effect_ops(ops: &[EffectOp]) -> MigratedStep {
    let needs_reacted = ops.iter().any(op_uses_reacted);
    MigratedStep::of(
        Step::new(ops.iter().copied().map(keyword_op_from_effect_op).collect()),
        needs_reacted,
    )
}

/// **一条反应分支**（操作清单）-> 一步 + 是否需要 reacted 上下文。
pub fn step_from_reaction_ops(ops: &[EffectOp]) -> MigratedStep {
    step_from_effect_ops(ops)
}

impl EffectOp {
    /// **迁移桥接**：底层操作 -> 关键字调用。
    pub fn to_keyword_op(self) -> KeywordOp {
        keyword_op_from_effect_op(self)
    }
}

/// 旧选择的「反应作用范围」-> 目标选择器。
fn selector_of(scope: TargetScope) -> TargetSelector {
    match scope {
        TargetScope::Reacted => TargetSelector::Reacted,
        TargetScope::SourceActive => TargetSelector::Active(SideScope::Own),
        TargetScope::TargetActive => TargetSelector::Active(SideScope::Opponent),
        TargetScope::TargetOthers => TargetSelector::AllExceptReacted(SideScope::Opponent),
    }
}

/// 旧的「某一方」-> 相对侧。
fn scope_of_side(side: Side) -> SideScope {
    match side {
        Side::Source => SideScope::Own,
        Side::Target => SideScope::Opponent,
    }
}

/// 旧的「某一方」-> 那一方的出战角色选择器（出战状态挂载用）。
fn active_of(side: Side) -> TargetSelector {
    TargetSelector::Active(scope_of_side(side))
}

/// 这条操作是否依赖执行上下文里的反应承载角色。
fn op_uses_reacted(op: &EffectOp) -> bool {
    match op {
        EffectOp::Damage { target, .. } | EffectOp::ForceSwitch { target } => {
            matches!(target, TargetScope::Reacted | TargetScope::TargetOthers)
        }
        EffectOp::Status { target, .. } => match target {
            StatusTarget::Character(scope) => {
                matches!(scope, TargetScope::Reacted | TargetScope::TargetOthers)
            }
            StatusTarget::Side(_) => false,
        },
        EffectOp::Summon { .. }
        | EffectOp::AddCardToHand { .. }
        | EffectOp::HandCardStatus { .. } => false,
    }
}

/// 反应效果的一步（Discard 迁移示例：效果自己弃置自己 -> Discard(Subject)）。
pub fn discard_self_step() -> Step {
    Step::new(vec![KeywordOp::Discard(DiscardArgs {
        targets: TargetSelector::Subject,
    })])
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{CardId, Element};
    use crate::families::character::types::DamageType;
    use crate::reactions::effect::HandCardStatus;
    use gi_genius_invokation_engine::Nat;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    #[test]
    fn skill_effect_plan_migrates_to_a_step() {
        assert!(SkillEffectPlan::None.to_step().is_empty());
        let plan = SkillEffectPlan::Damage {
            amount: n(6),
            ty: DamageType::Elemental(Element::Electro),
        };
        let step = plan.to_step();
        assert_eq!(step.len(), n(1));
        match &step.ops()[0] {
            KeywordOp::Damage(args) => {
                assert_eq!(args.targets, TargetSelector::opponent_active());
                assert_eq!(args.amount, n(6));
                assert_eq!(args.ty, DamageType::Elemental(Element::Electro));
                assert_eq!(args.source_kind, DamageSourceKind::Other);
            }
            other => panic!("期望造成伤害，得到 {other:?}"),
        }
    }

    #[test]
    fn effect_ops_map_to_keyword_calls() {
        let damage = EffectOp::Damage {
            amount: n(2),
            ty: DamageType::Piercing,
            target: TargetScope::TargetActive,
        }
        .to_keyword_op();
        match damage {
            KeywordOp::Damage(args) => {
                assert_eq!(args.targets, TargetSelector::Active(SideScope::Opponent));
                assert_eq!(args.source_kind, DamageSourceKind::Reaction);
            }
            other => panic!("期望造成伤害，得到 {other:?}"),
        }

        let status = keyword_op_from_effect_op(EffectOp::Status {
            card: CardId::new(134),
            target: StatusTarget::Character(TargetScope::Reacted),
            stacks: Nat::ZERO,
            uses: None,
            stack_cap: None,
        });
        match status {
            KeywordOp::Status(args) => {
                assert_eq!(args.place, StatusPlace::Character);
                assert_eq!(args.targets, TargetSelector::Reacted);
                assert_eq!(args.layers, None);
            }
            other => panic!("期望施加状态，得到 {other:?}"),
        }

        let summon = keyword_op_from_effect_op(EffectOp::Summon {
            card: CardId::new(9001),
            side: Side::Target,
            uses: n(2),
            stack_limit: None,
        });
        match summon {
            KeywordOp::Summon(args) => {
                assert_eq!(args.side, SideScope::Opponent);
                assert_eq!(args.uses, Some(n(2)));
            }
            other => panic!("期望召唤，得到 {other:?}"),
        }

        let force = keyword_op_from_effect_op(EffectOp::ForceSwitch {
            target: TargetScope::Reacted,
        });
        assert_eq!(
            force,
            KeywordOp::ForceSwitch(ForceSwitchArgs {
                targets: TargetSelector::Reacted
            })
        );

        let add = keyword_op_from_effect_op(EffectOp::AddCardToHand {
            card: CardId::new(300001),
            side: Side::Source,
        });
        match add {
            KeywordOp::CreateCard(args) => assert_eq!(args.side, SideScope::Own),
            other => panic!("期望生成卡，得到 {other:?}"),
        }

        let hand_status = keyword_op_from_effect_op(EffectOp::HandCardStatus {
            status: HandCardStatus::ElectricShock,
            side: Side::Target,
            index: n(1),
        });
        match hand_status {
            KeywordOp::CardStatus(args) => {
                assert_eq!(
                    args.targets,
                    TargetSelector::HandCardIndex {
                        side: SideScope::Opponent,
                        index: n(1)
                    }
                );
                assert_eq!(args.pick, TargetPick::All);
            }
            other => panic!("期望卡牌状态，得到 {other:?}"),
        }
    }

    #[test]
    fn stack_caps_are_migrated_to_keyword_args() {
        let status = keyword_op_from_effect_op(EffectOp::Status {
            card: CardId::new(134),
            target: StatusTarget::Character(TargetScope::Reacted),
            stacks: n(1),
            uses: None,
            stack_cap: Some(n(2)),
        });
        match status {
            KeywordOp::Status(args) => assert_eq!(args.stack_cap, Some(n(2))),
            other => panic!("期望施加状态，得到 {other:?}"),
        }
        let summon = keyword_op_from_effect_op(EffectOp::Summon {
            card: CardId::new(9001),
            side: Side::Target,
            uses: n(2),
            stack_limit: Some(n(2)),
        });
        match summon {
            KeywordOp::Summon(args) => assert_eq!(args.stack_limit, Some(n(2))),
            other => panic!("期望召唤，得到 {other:?}"),
        }
    }

    #[test]
    fn a_reaction_branch_becomes_one_step() {
        let ops = vec![
            EffectOp::Damage {
                amount: n(1),
                ty: DamageType::Physical,
                target: TargetScope::Reacted,
            },
            EffectOp::ForceSwitch {
                target: TargetScope::Reacted,
            },
        ];
        let migrated = step_from_effect_ops(&ops);
        assert_eq!(migrated.step.len(), n(2));
        assert!(migrated.needs_reacted);
        // 确定性 / 不需要 reacted 的分支。
        let migrated = step_from_reaction_ops(&[EffectOp::Summon {
            card: CardId::new(9001),
            side: Side::Source,
            uses: n(1),
            stack_limit: None,
        }]);
        assert_eq!(migrated.step.len(), n(1));
        assert!(!migrated.needs_reacted);
    }

    #[test]
    fn discard_self_bridge_uses_subject_selector() {
        let step = discard_self_step();
        match &step.ops()[0] {
            KeywordOp::Discard(args) => assert_eq!(args.targets, TargetSelector::Subject),
            other => panic!("期望弃置，得到 {other:?}"),
        }
    }
}
