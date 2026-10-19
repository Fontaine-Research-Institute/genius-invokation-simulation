//! **特技（L4）端到端**（规则书·特殊词条 特技 S8.4 + K58）。
//!
//! ## 依据（权威顺序 RULES §三.3）
//!
//! 规则书·特殊词条 **特技**（第一权威，原文四条）：
//! 1. 出战角色装备有「特技」卡牌时，就可以使用对应的「特技」。
//! 2. 使用「特技」是一种「战斗行动」。
//! 3. 角色因冻结、石化、眩晕等原因无法使用技能时，也会同时无法使用「特技」。
//! 4. 角色使用「特技」时不视为使用技能，无法触发「使用技能后」「角色引发元素反应后」等效果；
//!    角色使用「特技」造成的伤害不被视为角色造成的伤害。
//!
//! 原语（只复用、不重写）：
//! - families::character::skill_flow::technique::can_use_technique / use_technique：
//!   出战 / 特技槽 / 装备栏 owner / 状态禁用 的判定顺序；
//! - technique::dispatch_technique：**恒空**（K58 第 4 条：不派发「使用技能时 / 后」）。
//!
//! ## 本层做什么
//!
//! 把「判定 -> 支付（卡面费用，没给就不收）-> 返回战斗行动 + 空 SkillUsed 派发」串成纯函数入口：
//! 吃 &DefState、返回新 DefState（+ 使用结果）；失败返回 Err、**入参一个字节都不改**。
//!
//! ## 费用（规则书没写）
//!
//! 规则书没有规定特技费用；本层只按**卡面**给（TechniqueRequest.cost）：
//! - Some(cost)：从使用方骰池支付（SameColor 需要 same_color）；付不起 -> NotEnoughDice；
//! - None：不收（卡面没有费用 / 费用由效果自己付）。
//!
//! ## 待复核 / TODO（不自行编排）
//!
//! - **特技是否占用「每回合使用次数」**：规则书未写 -> 本层不记次数（待实测）；
//! - **特技的费用规则**：规则书未写 -> 只按卡面给（见上）；
//! - **特技造成的伤害**：K58「不被视为角色造成」-> TechniqueUse.damage_is_character_damage() = false，
//!   实际伤害结算由卡牌效果 / 关键字层按「非角色来源」给；
//! - **目标合法性**：can_use_technique 不检查目标（规则书未写）-> 本层只透传 target 给恒空派发入口；
//! - **派发「角色引发元素反应后」**：K58 第 4 条说不触发；派发层的伤害来源过滤不在本任务写范围，
//!   本层只保证不派发 SkillUsed（dispatch_technique 恒空）。

use crate::families::character::skill_flow::technique::{
    dispatch_technique, use_technique, CannotUseTechnique, TechniqueContext, TechniqueUse,
};
use crate::families::character::skill_flow::{SkillPending, SkillTriggers};
use crate::families::character::zones::equipment::EquipmentZone;
use crate::resources::dice::CostComponent;
use crate::rules::ops::stage::Timing;
use crate::state::DefState;
use gi_genius_invokation_engine::EntityId;

/// 特技流程的**外部依赖**（只借不拥有）。
pub struct TechniqueEnv<'a> {
    /// 「使用技能时 / 后」的注册表 —— 只用来调用 dispatch_technique 证明**恒空**（K58）。
    pub skill_triggers: &'a SkillTriggers,
}

impl<'a> TechniqueEnv<'a> {
    /// 建特技依赖。
    pub const fn new(skill_triggers: &'a SkillTriggers) -> TechniqueEnv<'a> {
        TechniqueEnv { skill_triggers }
    }
}

/// 一次使用特技的**请求**。
#[derive(Clone, Copy, Debug)]
pub struct TechniqueRequest<'a> {
    /// 使用特技的角色实体（S8.4 第 1 条：必须是出战角色，且装备了特技）。
    pub who: EntityId,
    /// 卡面费用（规则书未写；None = 不收，Some = 从使用方骰池付）。
    pub cost: Option<&'a [CostComponent]>,
    /// 同色费用支付时选的颜色（SameColor 费用需要）。
    pub same_color: Option<crate::base::Element>,
    /// 目标（只透传给恒空的 SkillUsed 派发入口；目标合法性不归本层）。
    pub target: Option<EntityId>,
}

/// 使用特技失败的原因（所有失败都保证入参不变、不支付）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TechniqueError {
    /// 判定不通过（非出战 / 无特技槽 / 装备栏 owner 不符 / 状态禁用）。
    CannotUse(CannotUseTechnique),
    /// 使用方角色区里没有这个角色（调用方给错实体）。
    NoSuchCharacter,
    /// 骰子不够付卡面费用（只可能在 cost 为 Some 时出现）。
    NotEnoughDice,
}

/// 一次使用特技的结果（纯数据）。
#[derive(Clone, Debug)]
pub struct TechniqueOutcome {
    /// 使用后的一局（只可能动了使用方的骰池）。
    pub state: DefState,
    /// 特技使用结果（谁 / 装备 / 特技牌 / 分类 / 行动权）。
    pub used: TechniqueUse,
    /// 行动权：战斗行动（S8.4 第 2 条；= used.timing，方便直接读）。
    pub timing: Timing,
    /// K58 第 4 条：特技不派发「使用技能时 / 后」-> **恒空**（可观测 / 可测试）。
    pub skill_used: Vec<SkillPending>,
    /// 是否真的从骰池收了卡面费用（cost 为 None -> false）。
    pub paid_cost: bool,
}

impl TechniqueOutcome {
    /// 是不是战斗行动（S8.4 第 2 条；恒 true）。
    pub fn is_combat_action(&self) -> bool {
        self.timing == Timing::Combat
    }
}

/// **使用特技**（我方；L4）：判定 -> 支付卡面费用 -> 返回战斗行动 + 恒空的 SkillUsed 派发。
pub fn use_technique_me(
    state: &DefState,
    env: &TechniqueEnv<'_>,
    request: TechniqueRequest<'_>,
) -> Result<TechniqueOutcome, TechniqueError> {
    use_technique_side(state, env, request, true)
}

/// **使用特技**（敌方；与 [use_technique_me] 共用一份实现，D35）。
pub fn use_technique_opp(
    state: &DefState,
    env: &TechniqueEnv<'_>,
    request: TechniqueRequest<'_>,
) -> Result<TechniqueOutcome, TechniqueError> {
    use_technique_side(state, env, request, false)
}

/// 共用实现：on_me = true 表示我方使用。
fn use_technique_side(
    state: &DefState,
    env: &TechniqueEnv<'_>,
    request: TechniqueRequest<'_>,
    on_me: bool,
) -> Result<TechniqueOutcome, TechniqueError> {
    let player = if on_me { state.me() } else { state.opp() };
    // 1) 角色必须在角色区（谁 / 装备栏 / 状态都从这一方读）。
    let Some(slot) = player.characters().slot_of(request.who) else {
        return Err(TechniqueError::NoSuchCharacter);
    };
    let character = slot.state.clone();
    let equipment = player
        .equipment_of(request.who)
        .cloned()
        .unwrap_or_else(|| EquipmentZone::new(request.who));
    let is_active = player.characters().is_active(request.who);
    let context = TechniqueContext {
        who: request.who,
        character: &character,
        equipment: &equipment,
        is_active,
    };
    // 2) 判定（S8.4 第 1 / 3 条：出战 + 有特技槽 + 状态未禁用）。
    let used = use_technique(&context).map_err(TechniqueError::CannotUse)?;
    // 3) 支付（规则书未写特技费用：卡面给多少付多少，没给就不收）。
    let (dice, paid_cost) = match request.cost {
        None => (player.dice().clone(), false),
        Some(cost) => {
            let Some(paid) = player.dice().pay(cost, request.same_color) else {
                return Err(TechniqueError::NotEnoughDice);
            };
            (paid, true)
        }
    };
    let next_player = player.clone().with_dice(dice);
    let next_state = if on_me {
        state.clone().with_me(next_player)
    } else {
        state.clone().with_opp(next_player)
    };
    // 4) K58 第 4 条：特技不视为使用技能 -> 不派发 SkillUsed（dispatch_technique 恒空）。
    let skill_used = dispatch_technique(env.skill_triggers, request.who, request.target);
    Ok(TechniqueOutcome {
        state: next_state,
        used,
        timing: used.timing,
        skill_used,
        paid_cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
    use crate::families::character::skill_flow::{SkillTrigger, AFTER_ANY, BEFORE_ANY};
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::families::status::known::FROZEN;
    use crate::resources::dice::{DicePool, DieFace};
    use crate::rules::ops::equip::SlotKind;
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{characters, equipment, n};
    use crate::state::PlayerDefs;
    use crate::timeline::Event;
    use gi_genius_invokation_engine::Nat;

    const TECHNIQUE_EQ: u32 = 900;
    const TECHNIQUE_CARD: u32 = 321001;
    static ONE_ANY: [CostComponent; 1] = [CostComponent::AnyColor(Nat::new(1))];

    fn dice(count: u32) -> DicePool {
        DicePool::empty().add(DieFace::Omni, Nat::new(count))
    }

    fn with_technique(who: u32) -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(characters(&[who, 202]))
            .with_equipment_zone(equipment(
                n(who),
                &[(TECHNIQUE_EQ, SlotKind::Technique, TECHNIQUE_CARD)],
            ))
    }

    fn env(triggers: &SkillTriggers) -> TechniqueEnv<'_> {
        TechniqueEnv::new(triggers)
    }

    fn noop(_state: &mut CharacterState, _event: &Event, _owner: EntityId) {}

    #[test]
    fn technique_is_a_combat_action_and_dispatches_no_skill_used() {
        let triggers = {
            let mut triggers = SkillTriggers::new();
            triggers.register(SkillTrigger::new(
                BEFORE_ANY,
                n(201),
                EffectSource::CharacterCard,
                Nat::ZERO,
                noop,
            ));
            triggers.register(SkillTrigger::new(
                AFTER_ANY,
                n(201),
                EffectSource::CharacterCard,
                Nat::ZERO,
                noop,
            ));
            triggers
        };
        let before = DefState::from_players(
            with_technique(201).with_dice(dice(2)),
            PlayerDefs::new().with_characters(characters(&[401])),
        );
        let request = TechniqueRequest {
            who: n(201),
            cost: Some(&ONE_ANY),
            same_color: None,
            target: Some(n(401)),
        };
        let outcome = use_technique_me(&before, &env(&triggers), request).expect("可用");
        // 战斗行动 + 特技结果。
        assert!(outcome.is_combat_action());
        assert_eq!(outcome.timing, Timing::Combat);
        assert_eq!(outcome.used.technique, CardId::new(TECHNIQUE_CARD));
        assert_eq!(outcome.used.equip, n(TECHNIQUE_EQ));
        assert!(!outcome.used.damage_is_character_damage());
        assert!(!outcome.used.dispatches_skill_used());
        assert!(!outcome.used.gains_energy());
        // K58 第 4 条：恒空（即使登记了 before/after 任意触发）。
        assert!(outcome.skill_used.is_empty());
        // 支付 1 个任意骰、入参不变。
        assert_eq!(outcome.state.me().dice().total(), Nat::new(1));
        assert!(outcome.paid_cost);
        assert_eq!(before.me().dice().total(), Nat::new(2));
    }

    #[test]
    fn technique_without_a_card_cost_does_not_pay() {
        let triggers = SkillTriggers::new();
        let before =
            DefState::from_players(with_technique(201).with_dice(dice(0)), PlayerDefs::new());
        let request = TechniqueRequest {
            who: n(201),
            cost: None,
            same_color: None,
            target: None,
        };
        let outcome = use_technique_me(&before, &env(&triggers), request).expect("可用");
        assert!(!outcome.paid_cost);
        assert_eq!(outcome.state.me().dice().total(), Nat::ZERO);
    }

    #[test]
    fn technique_rejection_reasons_are_surfaced() {
        let triggers = SkillTriggers::new();
        // 非出战角色（后台 202）：S8.4 第 1 条。
        let before = DefState::from_players(with_technique(201), PlayerDefs::new());
        assert_eq!(
            use_technique_me(
                &before,
                &env(&triggers),
                TechniqueRequest {
                    who: n(202),
                    cost: None,
                    same_color: None,
                    target: None,
                }
            )
            .expect_err("非出战"),
            TechniqueError::CannotUse(CannotUseTechnique::NotActiveCharacter)
        );
        // 没有特技槽。
        let no_slot = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[201])),
            PlayerDefs::new(),
        );
        assert_eq!(
            use_technique_me(
                &no_slot,
                &env(&triggers),
                TechniqueRequest {
                    who: n(201),
                    cost: None,
                    same_color: None,
                    target: None,
                }
            )
            .expect_err("无特技"),
            TechniqueError::CannotUse(CannotUseTechnique::NoTechniqueEquipped)
        );
        // 状态禁用（冻结）：S8.4 第 3 条。
        let mut frozen = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        frozen.states.add(CHARACTER_STATE_POS, &FROZEN, None);
        let zone = CharacterZone::try_from_slots(vec![CharacterSlot::new(n(201), frozen)])
            .expect("合法槽位");
        let frozen_state = DefState::from_players(
            PlayerDefs::new()
                .with_characters(zone)
                .with_equipment_zone(equipment(
                    n(201),
                    &[(TECHNIQUE_EQ, SlotKind::Technique, TECHNIQUE_CARD)],
                )),
            PlayerDefs::new(),
        );
        let reason = use_technique_me(
            &frozen_state,
            &env(&triggers),
            TechniqueRequest {
                who: n(201),
                cost: None,
                same_color: None,
                target: None,
            },
        )
        .expect_err("冻结禁用");
        assert!(matches!(
            reason,
            TechniqueError::CannotUse(CannotUseTechnique::ForbiddenByStatus { .. })
        ));
        // 角色不在角色区。
        assert_eq!(
            use_technique_me(
                &before,
                &env(&triggers),
                TechniqueRequest {
                    who: n(999),
                    cost: None,
                    same_color: None,
                    target: None,
                }
            )
            .expect_err("没有这个角色"),
            TechniqueError::NoSuchCharacter
        );
    }

    #[test]
    fn technique_rejects_when_the_card_cost_cannot_be_paid() {
        let triggers = SkillTriggers::new();
        let before =
            DefState::from_players(with_technique(201).with_dice(dice(1)), PlayerDefs::new());
        let three: [CostComponent; 1] = [CostComponent::AnyColor(Nat::new(3))];
        let error = use_technique_me(
            &before,
            &env(&triggers),
            TechniqueRequest {
                who: n(201),
                cost: Some(&three),
                same_color: None,
                target: None,
            },
        )
        .expect_err("付不起");
        assert_eq!(error, TechniqueError::NotEnoughDice);
        // 失败不改状态。
        assert_eq!(before.me().dice().total(), Nat::new(1));
    }

    #[test]
    fn use_technique_opp_binds_the_other_side() {
        let triggers = SkillTriggers::new();
        let me = PlayerDefs::new().with_characters(characters(&[201]));
        let opp = PlayerDefs::new()
            .with_characters(characters(&[401]))
            .with_equipment_zone(equipment(
                n(401),
                &[(TECHNIQUE_EQ, SlotKind::Technique, TECHNIQUE_CARD)],
            ))
            .with_dice(dice(1));
        let before = DefState::from_players(me, opp);
        let me_snapshot = before.me().clone();
        let outcome = use_technique_opp(
            &before,
            &env(&triggers),
            TechniqueRequest {
                who: n(401),
                cost: Some(&ONE_ANY),
                same_color: None,
                target: None,
            },
        )
        .expect("可用");
        assert_eq!(outcome.state.opp().dice().total(), Nat::ZERO);
        assert_eq!(outcome.state.me(), &me_snapshot);
        assert!(outcome.is_combat_action());
    }
}
