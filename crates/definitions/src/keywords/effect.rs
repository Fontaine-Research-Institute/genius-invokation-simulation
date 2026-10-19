//! **关键字函数的执行上下文**（EffectEnv）：效果主体 + 视角方 + 底层操作的依赖。
//!
//! ## 口径
//!
//! - **实体号一律从上下文来**（D206）：关键字函数的签名里不出现「让你传 owner / entity id」
//!   的参数；自己是 [crate::keywords::target::TargetSelector::Subject]，由 [EffectEnv::subject]
//!   解析；我方 / 敌方由 [EffectEnv::viewer] 解析；
//! - **只借不拥有**（D183）：伤害依赖、各区触发注册表、元素精通、通用增伤、反应局面摘要
//!   都是 & 引用；EffectEnv 自身是 Copy 级别的轻量值（内部只有引用 + Nat + 发号器）；
//! - **发号器**（EntityAlloc）：生成召唤物 / 生成卡牌要新实体号；本层只发号，实体表归状态层；
//! - **纯函数边界**：所有关键字函数吃 &DefState，返回新分支；EffectEnv 不被修改
//!   （发号推进通过返回值里的 alloc 带回；需要换 alloc 用 [EffectEnv::fork]）。
//!
//! ## 依据
//!
//! - D186：派发只排序、不执行；执行器是调用方；
//! - D190 / D191：观测点展开成分支 + 精确概率，不采样；
//! - D204 / D205 / D206（Lead 拍板）：一步原子；自己 / 视角方由上下文函数解析。

use crate::families::action_card::areas::support::SupportTriggers;
use crate::families::character::damage::DamageEnv;
use crate::families::character::damage_chain::bonus::DamageModifiers;
use crate::families::character::damage_chain::executor::{EntityAlloc, SideTriggers};
use crate::families::character::trigger::CharacterTriggers;
use crate::families::character::zones::equipment::EquipmentTriggers;
use crate::reactions::effect::EffectContext;
use gi_genius_invokation_engine::{EntityId, Nat};

use super::target::{Perspective, ResolveContext};

/// 空的一侧区触发注册表（默认值；静态是为了让 [EffectEnv::new] 不需要传一堆空表）。
static EMPTY_SIDE_TRIGGERS: SideTriggers = SideTriggers::new();
/// 空角色族触发注册表。
static EMPTY_CHARACTER_TRIGGERS: CharacterTriggers = CharacterTriggers::new();
/// 空支援区触发注册表。
static EMPTY_SUPPORT_TRIGGERS: SupportTriggers = SupportTriggers::new();
/// 空装备栏触发注册表。
static EMPTY_EQUIPMENT_TRIGGERS: EquipmentTriggers = EquipmentTriggers::new();

/// 空反应局面摘要（角色不是出战角色、双方手牌数 0）。
pub static EMPTY_EFFECT_CONTEXT: EffectContext = EffectContext::new(false, Nat::ZERO, Nat::ZERO);

/// **双方的触发注册表集合**（只借；D34：两个具名侧各一组）。
#[derive(Clone, Copy, Debug)]
pub struct TriggerBook<'a> {
    /// 我方的角色区 / 出战状态区 / 召唤物区 / 手牌状态触发。
    pub me: &'a SideTriggers,
    /// 敌方的角色区 / 出战状态区 / 召唤物区 / 手牌状态触发。
    pub opp: &'a SideTriggers,
    /// 我方的角色族触发（生命值变动 / 治疗后）。
    pub me_characters: &'a CharacterTriggers,
    /// 敌方的角色族触发（生命值变动 / 治疗后）。
    pub opp_characters: &'a CharacterTriggers,
    /// 我方的支援区触发。
    pub me_supports: &'a SupportTriggers,
    /// 敌方的支援区触发。
    pub opp_supports: &'a SupportTriggers,
    /// 我方的装备栏触发。
    pub me_equipment: &'a EquipmentTriggers,
    /// 敌方的装备栏触发。
    pub opp_equipment: &'a EquipmentTriggers,
}

impl TriggerBook<'static> {
    /// 双方都没有任何登记（测试 / 纯执行用）。
    pub const fn empty() -> TriggerBook<'static> {
        TriggerBook {
            me: &EMPTY_SIDE_TRIGGERS,
            opp: &EMPTY_SIDE_TRIGGERS,
            me_characters: &EMPTY_CHARACTER_TRIGGERS,
            opp_characters: &EMPTY_CHARACTER_TRIGGERS,
            me_supports: &EMPTY_SUPPORT_TRIGGERS,
            opp_supports: &EMPTY_SUPPORT_TRIGGERS,
            me_equipment: &EMPTY_EQUIPMENT_TRIGGERS,
            opp_equipment: &EMPTY_EQUIPMENT_TRIGGERS,
        }
    }
}

/// **关键字函数的执行上下文**。
pub struct EffectEnv<'a> {
    subject: EntityId,
    viewer: Perspective,
    reacted: Option<EntityId>,
    alloc: EntityAlloc,
    /// 伤害入口依赖（允许附着集合 + 反应规则表 + 反应注册表）——直接复用
    /// families::character::damage::DamageEnv，不另造一套。
    pub damage: DamageEnv<'a>,
    /// 双方的触发注册表。
    pub triggers: TriggerBook<'a>,
    /// 触发方元素精通（反应加伤 / 结晶护盾强化）。
    pub mastery: Nat,
    /// 通用增伤（卡面「造成的伤害+N」；顺序由调用方排好）。
    pub increases: &'a [Nat],
    /// 反应效果函数的局面摘要（伤害链求反应加伤用）。
    pub effect_context: &'a EffectContext,
    /// **反应效果造成的伤害**的乘算 / 除算来源（默认 NONE；从卡牌 / 关键字传入）。
    ///
    /// 它与关键字伤害自己的 [crate::keywords::op::DamageArgs::modifiers] 分开：
    /// 触发伤害用 DamageArgs.modifiers；反应效果里的 EffectOp::Damage 用本字段。
    reaction_modifiers: DamageModifiers<'a>,
    /// **本步开始时**我方手牌数（动态张数用；D204 一步是原子结算单位）。
    step_start_me_hand: Nat,
    /// **本步开始时**敌方手牌数。
    step_start_opp_hand: Nat,
}

impl<'a> EffectEnv<'a> {
    /// 建上下文：默认双方零触发、零精通、零增伤、空反应摘要、发号从 0 开始。
    pub fn new(subject: EntityId, viewer: Perspective, damage: DamageEnv<'a>) -> EffectEnv<'a> {
        EffectEnv {
            subject,
            viewer,
            reacted: None,
            alloc: EntityAlloc::starting_at(Nat::ZERO),
            damage,
            triggers: TriggerBook::empty(),
            mastery: Nat::ZERO,
            increases: &[],
            effect_context: &EMPTY_EFFECT_CONTEXT,
            reaction_modifiers: DamageModifiers::NONE,
            step_start_me_hand: Nat::ZERO,
            step_start_opp_hand: Nat::ZERO,
        }
    }

    /// 效果主体实体号（自己）。
    pub fn subject(&self) -> EntityId {
        self.subject
    }

    /// 视角方绑定（我方 / 我方是 me 还是 opp）。
    pub fn viewer(&self) -> Perspective {
        self.viewer
    }

    /// 反应承载角色（反应效果关键字用）。
    pub fn reacted(&self) -> Option<EntityId> {
        self.reacted
    }

    /// 反应效果伤害的乘算 / 除算来源（默认 NONE）。
    pub fn reaction_modifiers(&self) -> DamageModifiers<'a> {
        self.reaction_modifiers
    }

    /// 当前发号器（生成下一个实体号用）。
    pub fn alloc(&self) -> EntityAlloc {
        self.alloc
    }

    /// 解析上下文（目标选择器用）。
    pub fn resolve_context(&self) -> ResolveContext {
        ResolveContext {
            viewer: self.viewer,
            subject: self.subject,
            reacted: self.reacted,
        }
    }

    /// 换发号器（纯值；不改入参）。
    pub fn with_alloc(mut self, alloc: EntityAlloc) -> EffectEnv<'a> {
        self.alloc = alloc;
        self
    }

    /// 换视角方绑定（只由成对入口 execute_step_me / execute_step_opp 用）。
    pub fn with_viewer(mut self, viewer: Perspective) -> EffectEnv<'a> {
        self.viewer = viewer;
        self
    }

    /// 换双方触发注册表。
    pub fn with_triggers(mut self, triggers: TriggerBook<'a>) -> EffectEnv<'a> {
        self.triggers = triggers;
        self
    }

    /// 换元素精通。
    pub fn with_mastery(mut self, mastery: Nat) -> EffectEnv<'a> {
        self.mastery = mastery;
        self
    }

    /// 换通用增伤。
    pub fn with_increases(mut self, increases: &'a [Nat]) -> EffectEnv<'a> {
        self.increases = increases;
        self
    }

    /// 换反应局面摘要。
    pub fn with_effect_context(mut self, effect_context: &'a EffectContext) -> EffectEnv<'a> {
        self.effect_context = effect_context;
        self
    }

    /// 给**反应效果造成的伤害**挂乘算 / 除算来源（默认 NONE；从卡牌 / 关键字传入）。
    pub fn with_reaction_modifiers(mut self, modifiers: DamageModifiers<'a>) -> EffectEnv<'a> {
        self.reaction_modifiers = modifiers;
        self
    }

    /// 带上「反应承载角色」（反应效果关键字用）。
    pub fn with_reacted(mut self, reacted: Option<EntityId>) -> EffectEnv<'a> {
        self.reacted = reacted;
        self
    }

    /// 填「本步开始时的双方手牌数」（动态张数用；步执行器在步开始时自动填）。
    pub fn with_step_start_hands(mut self, me: Nat, opp: Nat) -> EffectEnv<'a> {
        self.step_start_me_hand = me;
        self.step_start_opp_hand = opp;
        self
    }

    /// 本步开始时某一方的手牌数。
    pub fn step_start_hand(&self, side: Perspective) -> Nat {
        match side {
            Perspective::Me => self.step_start_me_hand,
            Perspective::Opp => self.step_start_opp_hand,
        }
    }

    /// 换发号器并复制一份上下文（&self 版本；关键字层逐个操作推进发号器用）。
    pub fn fork(&self, alloc: EntityAlloc) -> EffectEnv<'a> {
        EffectEnv {
            subject: self.subject,
            viewer: self.viewer,
            reacted: self.reacted,
            alloc,
            damage: DamageEnv {
                allow: self.damage.allow,
                rules: self.damage.rules,
                registry: self.damage.registry,
            },
            triggers: self.triggers,
            mastery: self.mastery,
            increases: self.increases,
            effect_context: self.effect_context,
            reaction_modifiers: self.reaction_modifiers,
            step_start_me_hand: self.step_start_me_hand,
            step_start_opp_hand: self.step_start_opp_hand,
        }
    }

    /// 某一侧的区触发（角色区 / 出战状态 / 召唤物 / 手牌状态）。
    pub fn side_triggers(&self, side: Perspective) -> &'a SideTriggers {
        match side {
            Perspective::Me => self.triggers.me,
            Perspective::Opp => self.triggers.opp,
        }
    }

    /// 某一侧的角色族触发（生命值变动 / 治疗后）。
    pub fn character_triggers(&self, side: Perspective) -> &'a CharacterTriggers {
        match side {
            Perspective::Me => self.triggers.me_characters,
            Perspective::Opp => self.triggers.opp_characters,
        }
    }

    /// 某一侧的支援区触发。
    pub fn support_triggers(&self, side: Perspective) -> &'a SupportTriggers {
        match side {
            Perspective::Me => self.triggers.me_supports,
            Perspective::Opp => self.triggers.opp_supports,
        }
    }

    /// 某一侧的装备栏触发。
    pub fn equipment_triggers(&self, side: Perspective) -> &'a EquipmentTriggers {
        match side {
            Perspective::Me => self.triggers.me_equipment,
            Perspective::Opp => self.triggers.opp_equipment,
        }
    }
}
