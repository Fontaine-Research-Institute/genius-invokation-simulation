//! 底层伤害入口：**扣血 + 元素附着 + 反应判定 / 消耗**（D121 / D133 / D164 / D167）。
//!
//! 用户口径：
//! - D164：「造成伤害会附着对应元素，触发反应的」；
//! - D167：附着是**布尔集合**（可多元素共存、无计量）；**反应会消耗掉所有参与反应的元素**，
//!   选「都不附着（全部清掉）」——即命中的已覆盖元素与本次攻击元素都被移除，本次攻击元素**不再附着**。
//!
//! 流程（D167）：
//! 1. 记录**攻击前**的已覆盖集合（反应判定 / 消耗都基于它）；
//! 2. **先判定反应**（攻击前已覆盖 + 本次攻击；在调用方给的规则表里命中第一条）；
//! 3. 有反应：扣血 -> **移除所有参与元素**（命中的已覆盖元素 + 本次攻击元素）
//!    -> **本次攻击元素不附着** -> 触发反应注册（`ReactionRegistry::trigger`，D136 敌我双方都算）；
//!    其余未参与的共存元素保留；
//! 4. 无反应（且是元素伤害）：扣血 -> **附着本次攻击元素**（受允许集合限制；风 / 岩等不落地，D134）；
//! 5. 物理 / 穿透：**只扣血**（不附着、不判定、不触发反应）。
//!
//! **不做**（D133 / 用户裁决）：
//! - 目标选择：本层没有「实体 -> CharacterState」映射（D29 / D34），`state` 由调用方给，
//!   `target` 只作为记录 / 传给反应事件；
//! - 增减伤 / 减免 / 穿透不吃加成等**伤害结算**；
//! - **反应的具体效果**（超载 / 感电 / 结晶 … 的数值与状态变化由注册方给；本块只负责
//!   判定、消耗参与元素、触发注册）。
//!
//! 纯函数：吃 `&CharacterState`、返回 `DamageOutcome`（新状态 + 记录 + 本次附着 / 反应结果），
//! 不改入参。

use crate::families::character::attachment::{
    AttachmentAllowSet, ReactionCondition, ReactionEvent, ReactionIdentity, ReactionRegistry,
};
use crate::families::character::character::CharacterState;
use crate::families::character::damage_chain::bonus::{predict_reaction, DamageBonus, ReactionHit};
use crate::families::character::damage_chain::chain::{
    settle, settle_records, ChainOutcome, DamageChainEnv, DamageInput,
};
use crate::families::character::damage_chain::record::{
    write_records, DamageChainRecord, RecordedStep,
};
use crate::families::character::trigger::{
    dispatch_character, hp_changed_event, CharacterPending, CharacterTiming, CharacterTriggers,
};
use crate::families::character::types::DamageType;
use crate::reactions::effect::EffectContext;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 本模块源码声明用的模块名（多语；D216）。模块自述名，不是卡牌 / 规则数据。
///
/// 待复核：日 / 韩译名是本轮临时填写，等术语表确认后替换。
pub static MODULE_NAME: crate::generated::Localized = crate::generated::Localized::from_all(
    "角色伤害",
    "Character Damage",
    "キャラクターへのダメージ",
    "캐릭터 피해",
);

// D216：非卡牌模块自声明 —— 路径分段 + 自己的多语模块名；原文与 use 依赖编译期嵌入。
crate::module_source!(path: ["families", "character", "damage"], name: MODULE_NAME);

/// 一条**反应规则**（接口数据）。
///
/// **具体反应表由上层 / 后续反应块提供**（超载 / 感电 / 结晶 … 的定义不在本块）：
/// - `reaction`：反应身份（`ReactionIdentity`，指针身份；注册表按它匹配「按种类」注册）；
/// - `conditions`：触发条件列表（D132：可写多条，命中任一条即触发；空列表 = 永不触发，D128）。
#[derive(Clone, Copy, Debug)]
pub struct ReactionRule {
    /// 反应身份。
    pub reaction: &'static dyn ReactionIdentity,
    /// 触发条件列表（D132）。
    pub conditions: &'static [ReactionCondition],
}

impl ReactionRule {
    /// 一条反应规则。
    pub const fn new(
        reaction: &'static dyn ReactionIdentity,
        conditions: &'static [ReactionCondition],
    ) -> ReactionRule {
        ReactionRule {
            reaction,
            conditions,
        }
    }
}

/// 伤害入口的**外部依赖**（调用方提供；本层不拥有、只使用）。
///
/// - `allow`：允许附着的元素集合（由角色 / 卡牌定义提供，D130）；
/// - `rules`：反应规则表（具体反应由上层提供，本块不实现任何具体反应）；
/// - `registry`：「反应时」触发注册表（D127 / D136）。
#[derive(Debug)]
pub struct DamageEnv<'a> {
    /// 允许附着的元素集合（D130）。
    pub allow: AttachmentAllowSet,
    /// 反应规则表（顺序 = 判定顺序；命中第一条即触发，D132）。
    pub rules: &'a [ReactionRule],
    /// 反应触发注册表（D127 / D136）。
    pub registry: &'a ReactionRegistry,
}

/// 一条**伤害记录**（纯数据，D121）：谁打谁、多少、什么类型。
///
/// 它只是伤害入口的**返回值之一**（记录用），不再有单独的 `apply` 入口 ——
/// 扣血 / 附着 / 反应统一走 [damage]（D164 / D167）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DamageRecord {
    /// 目标角色实体。
    pub target: EntityId,
    /// 伤害数值（原始值；不钳到生命值，扣血时由 `hp_sub` 饱和）。
    pub amount: Nat,
    /// 伤害类型。
    pub ty: DamageType,
    /// 来源实体（可选；只带数据，不参与结算）。
    pub source: Option<EntityId>,
}

impl DamageRecord {
    /// 建一条伤害记录（不带来源）。
    pub const fn new(target: EntityId, amount: Nat, ty: DamageType) -> DamageRecord {
        DamageRecord {
            target,
            amount,
            ty,
            source: None,
        }
    }

    /// 建一条伤害记录，并记下来源实体。
    pub const fn from_source(
        target: EntityId,
        amount: Nat,
        ty: DamageType,
        source: EntityId,
    ) -> DamageRecord {
        DamageRecord {
            target,
            amount,
            ty,
            source: Some(source),
        }
    }
}

/// 一次伤害入口的结果（纯数据）。
#[derive(Clone, Debug)]
pub struct DamageOutcome {
    /// 伤害落地后的角色状态（只可能动了 `hp` / `attachment`）。
    pub state: CharacterState,
    /// 本次伤害记录。
    pub record: DamageRecord,
    /// 本次元素伤害的附着是否落地：
    /// - 物理 / 穿透恒为 `false`；
    /// - 发生反应时恒为 `false`（D167：参与元素全部清掉，攻击元素不再附着）；
    /// - 无反应时：元素不可附着（风 / 岩等）或不在允许集合内也为 `false`。
    pub attachment_landed: bool,
    /// 本次命中的反应（物理 / 穿透 / 没有命中 -> `None`）。
    pub reaction: Option<&'static dyn ReactionIdentity>,
    /// 本次触发的注册条数（`ReactionRegistry::trigger` 的返回值；未命中 -> 0）。
    pub fired: Nat,
}

/// **底层伤害入口**（D164 / D167）：扣血 + 元素附着 + 反应判定 / 消耗。
///
/// 参数：
/// - `state`：目标角色的当前状态（调用方保证它确实对应 `target`）；
/// - `source`：谁触发（攻击方角色实体；只用于反应事件，不参与数值）；
/// - `target`：目标角色实体（只作记录 / 反应事件，本层不校验）；
/// - `amount` / `ty`：数值与伤害类型；
/// - `env`：允许集合 + 反应规则表 + 反应注册表（见 [DamageEnv]）。
///
/// 具体反应效果不在本层（注册方负责）；参与元素的消耗由本层按 D167 直接完成。
pub fn damage(
    state: &CharacterState,
    source: EntityId,
    target: EntityId,
    amount: Nat,
    ty: DamageType,
    env: &DamageEnv<'_>,
) -> DamageOutcome {
    let record = DamageRecord::from_source(target, amount, ty, source);
    // 1) 记录**攻击前**的已覆盖集合（D167）；反应判定与消耗都基于它。
    let covered_before = state.attachment;

    // 扣血（只做数值；不判倒下、不做增减伤）。
    let mut next = state.hp_sub(amount);

    let mut attachment_landed = false;
    let mut reaction = None;
    let mut fired = Nat::ZERO;

    match ty {
        DamageType::Elemental(attack) => {
            // 2) 先判定反应：规则表按顺序找第一条命中（用攻击前的集合）。
            let matched = env.rules.iter().find_map(|rule| {
                covered_before
                    .matching_covered(attack, rule.conditions)
                    .map(|covered| (rule, covered))
            });

            if let Some((rule, covered)) = matched {
                // 3) 有反应：移除参与元素（命中的已覆盖 + 本次攻击）；攻击元素不再附着（D167）。
                next.attachment = covered_before.remove_all(&[covered, attack]);
                reaction = Some(rule.reaction);
                let event = ReactionEvent::new(rule.reaction, covered, attack, source, target);
                fired = env.registry.trigger(&event);
            } else {
                // 4) 无反应：附着本次攻击元素（受允许集合限制，风 / 岩等不落地，D134）。
                let (attachment, landed) = covered_before.attach(attack, env.allow);
                next.attachment = attachment;
                attachment_landed = landed;
            }
        }
        DamageType::Physical | DamageType::Piercing => {
            // 5) 物理 / 穿透：只扣血，附着集合原样保留、不判定反应。
            next.attachment = covered_before;
        }
    }

    DamageOutcome {
        state: next,
        record,
        attachment_landed,
        reaction,
        fired,
    }
}

// ======================================================================
// 带触发入口（D185：角色族在自己的数值操作里主动派发）
// ======================================================================

/// 一次**带触发**的伤害结算结果。
#[derive(Clone, Debug)]
pub struct TriggeredDamage {
    /// 底层伤害结算结果（与 [`damage`] 完全一致；本入口不改任何数值语义）。
    pub outcome: DamageOutcome,
    /// 本次伤害派发出来的「生命值变动」待执行（由调用方执行；本入口不执行）。
    pub pending: Vec<CharacterPending>,
}

/// **带触发的伤害入口**：先走 [`damage`]（纯函数：扣血 / 附着 / 反应判定语义一行没改），
/// 再把「生命值变动」事件交给角色族注册表派发，返回待执行列表。
///
/// 规则书依据：其他规则 击倒「当角色的生命值被降为0，则在技能或效果**结算完成后**，角色就会被
/// 『击倒』」—— 所以「生命值变动」与「被击倒」是两个时机：这里只派发前者，击倒由调用方在结算
/// 完成后调 `CharacterTiming::Defeated`（见 `trigger.rs`）。
///
/// **插入点（`结算顺序.md` §三「常见插入点」，暂定）**：
/// 1. 造成伤害 -> 「受伤时」修正（加伤 -> 护盾抵消 -> 减伤扣减；穿透三样都不吃）：
///    `rules::ops::damage::mitigate`；本入口**不接修正**（只做扣血后的派发），
///    要修正走 [`damage_with_chain`]（伤修链子结算，见 `damage_chain::chain`）；
/// 2. 扣血（[`damage`]）；
/// 3. 扣血后 -> 派发「生命值变动」（本入口）；
/// 4. 元素伤害：附着 -> 判定反应 -> 反应效果（在 [`damage`] 内，D167）。
pub fn damage_with_triggers(
    state: &CharacterState,
    source: EntityId,
    target: EntityId,
    amount: Nat,
    ty: DamageType,
    env: &DamageEnv<'_>,
    triggers: &CharacterTriggers,
) -> TriggeredDamage {
    let outcome = damage(state, source, target, amount, ty, env);
    let event = hp_changed_event(target, amount, ty.element(), Some(source));
    let pending = dispatch_character(triggers, CharacterTiming::HpChanged, &event);
    TriggeredDamage { outcome, pending }
}

// ======================================================================
// 走「伤修链」的伤害入口（`结算顺序.md` §三 第 1 行插入点 / D196）
// ======================================================================

/// 走「伤修链」的伤害入口的**外部依赖**（把一长串参数收成一组；本层只借不拥有）。
pub struct ChainDamageEnv<'a> {
    /// 底层伤害入口依赖（允许附着集合 + 反应规则表 + 反应注册表）。
    pub damage: &'a DamageEnv<'a>,
    /// 伤修链的区只读指针 + 护盾 / 减伤身份（D183：只借、不新建承载）。
    pub chain: &'a DamageChainEnv<'a>,
    /// 加伤（通用增伤 + 元素精通；**反应加伤**由本入口按规则表求）。
    pub bonus: DamageBonus<'a>,
    /// 反应效果函数用的最小局面摘要（**只用来求「本伤害 +N」**；所有反应的加伤都是常数）。
    pub context: &'a EffectContext,
    /// 角色族触发注册表（「生命值变动」派发）。
    pub triggers: &'a CharacterTriggers,
}

/// 一次**走伤修链**的伤害结算结果。
#[derive(Clone, Debug)]
pub struct ChainedDamage {
    /// 伤修链的展开结果（加伤 / 护盾 / 减伤 / 扣血数值 + 各段记录）。
    pub chain: ChainOutcome,
    /// 本次伤害**预测**会命中的反应（只读预测；与实际判定同口径）。
    pub reaction: Option<ReactionHit>,
    /// 底层伤害结算结果（与 [`damage`] 完全一致：扣血 / 附着 / 反应判定语义一行没改）。
    pub outcome: DamageOutcome,
    /// 「生命值变动」待执行（与 [`damage_with_triggers`] 同口径）。
    pub pending: Vec<CharacterPending>,
}

/// **走伤修链的伤害入口**（`结算顺序.md` §三 第 1 行「造成伤害」的插入点 / D196）。
///
/// 流程（插入式子结算，做完回到原步骤）：
/// 1. 只读预测本次元素伤害会命中的反应（`damage_chain::bonus::predict_reaction`），求「本伤害 +N」；
/// 2. 跑伤修链子结算（`damage_chain::chain::settle`）：加伤 -> 护盾 -> 减伤 -> 扣血数值；
/// 3. 用扣血数值调 [`damage`]（**扣血 / 附着 / 反应判定 / 消耗参与元素 / 触发注册全走原来那份**）；
/// 4. 派发「生命值变动」（D185）。
///
/// **不做什么**：不改 [`damage`] 的任何语义；不重做反应判定（预测只用于加伤，实际判定仍在
/// [`damage`] 里）；**不执行反应效果**（那是 `damage_chain::executor` 的事：调用方拿到
/// [`DamageOutcome::reaction`] 之后按 `reactions::table::resolve` 的分支去执行）。
///
/// **伤害类型转换**：若 [`DamageChainEnv::type_override`] 为 Some，本入口在**进入计算前**
/// 把类型换成覆盖值（effective_ty），反应预测 / 链内数值 / 实际判定 / 事件元素全按新类型
/// （卡面「使角色造成的物理伤害变为水元素伤害」先转类型再判反应）；None 时逐字节等价。
///
/// **待复核**：护盾把伤害全部吸掉（`to_hp == 0`）时，元素伤害是否仍然附着 / 引发反应——
/// 本入口按「仍然走 [`damage`]」处理（不额外特判）。
pub fn damage_with_chain(
    state: &CharacterState,
    source: EntityId,
    target: EntityId,
    base: Nat,
    ty: DamageType,
    env: &ChainDamageEnv<'_>,
) -> ChainedDamage {
    damage_with_chain_impl(state, source, target, base, ty, env, None)
}

/// **走伤修链的伤害入口（带记录）**：行为与 [damage_with_chain] **完全一致**，只多写记录
/// （D223 / D224：把「属于哪一步 + 任意内容」写进调用方的记录器）。
///
/// 记录内容（内容类型 [DamageChainRecord]；见 `damage_chain::record`）：
/// - [DamageChainRecord::Hit]：基础值 + **原始**伤害类型 + 来源实体 + 来源属性标签 + 目标；
/// - [DamageChainRecord::TypeOverride]：若有类型覆盖，记「原始类型 -> 实际类型」（链内写）；
/// - [DamageChainRecord::Reaction]：命中的元素反应（身份名 + 已覆盖 / 本次攻击元素；用实际类型判定）；
/// - 链内记录（加伤明细 / 合计 / 护盾吸收 / 减伤合计 / 穿透影响 / 最终扣血）由
///   `chain::settle_records` 在**计算处**写出，并**重盖为同一个结算步号** [RecordedStep::step]。
///
/// 关闭 records feature 时所有 push 都是空操作（零开销），结算结果一字不变。
pub fn damage_with_chain_recorded(
    state: &CharacterState,
    source: EntityId,
    target: EntityId,
    base: Nat,
    ty: DamageType,
    env: &ChainDamageEnv<'_>,
    recorded: &mut RecordedStep<'_>,
) -> ChainedDamage {
    damage_with_chain_impl(state, source, target, base, ty, env, Some(recorded))
}

/// 两个入口的**唯一实现**（`recorded = None` = 不记录、不构造任何记录内容）。
fn damage_with_chain_impl(
    state: &CharacterState,
    source: EntityId,
    target: EntityId,
    base: Nat,
    ty: DamageType,
    env: &ChainDamageEnv<'_>,
    recorded: Option<&mut RecordedStep<'_>>,
) -> ChainedDamage {
    // 「伤害类型转换」在进入计算前生效：之后反应预测 / 链内数值 / 实际判定全按新类型。
    // 没有覆盖时 effective_ty == ty，逐字节等价。
    let effective_ty = env.chain.effective_type(ty);
    // 1) 只读预测反应 -> 反应加伤（物理 / 穿透不判反应）。
    let hit = effective_ty.element().and_then(|attack| {
        predict_reaction(state.attachment, attack, env.damage.rules, source, target)
    });
    let mut bonus = env.bonus;
    bonus.reacted = hit.is_some();
    bonus.reaction_bonus = hit
        .map(|hit| hit.damage_bonus(env.context))
        .unwrap_or(Nat::ZERO);
    // 2) 伤修链子结算（插入点：扣血之前）。
    //    记录版入口：先记基础值 / 类型 / 来源属性 / 目标 / 命中的反应（入口处就知道，不需要 diff），
    //    再把链内记录重盖到同一个结算步号。
    let chain = match recorded {
        Some(recorded) => {
            recorded.recorder.push(
                recorded.step,
                DamageChainRecord::Hit {
                    base,
                    ty,
                    source: Some(source),
                    target: Some(target),
                    source_label: recorded.source_label,
                },
            );
            // 传给链的是**原始类型**：链自己按 type_override 在进入计算前重写并记 TypeOverride。
            let (outcome, records) =
                settle_records(DamageInput::new(base, ty), env.chain, &bonus);
            // 链内记录（含 TypeOverride「原始 -> 实际」）先写；反应记录放后面，
            // 记录顺序 = Hit(原始) -> TypeOverride(原始->实际) -> Reaction(覆盖后的类型)。
            write_records(&records, recorded.step, recorded.recorder);
            if let Some(hit) = hit {
                recorded.recorder.push(
                    recorded.step,
                    DamageChainRecord::Reaction {
                        name: hit.reaction.name(),
                        covered: hit.covered,
                        attack: hit.attack,
                    },
                );
            }
            outcome
        }
        None => settle(DamageInput::new(base, ty), env.chain, &bonus),
    };
    // 3) 既有伤害入口（扣血 / 附着 / 反应判定 / 触发注册，语义一行不改）。
    let outcome = damage(state, source, target, chain.to_hp, effective_ty, env.damage);
    // 4) 「生命值变动」派发（D185）。
    let event = hp_changed_event(target, chain.to_hp, effective_ty.element(), Some(source));
    let pending = dispatch_character(env.triggers, CharacterTiming::HpChanged, &event);
    ChainedDamage {
        chain,
        reaction: hit,
        outcome,
        pending,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::families::character::attachment::{Attachment, ReactionHandler};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 测试用反应身份（具体反应由上层定义，本块不实现）。
    #[derive(Debug)]
    struct SampleReaction(&'static str);

    impl ReactionIdentity for SampleReaction {
        fn name(&self) -> &'static str {
            self.0
        }
    }

    static VAPORIZE: SampleReaction = SampleReaction("蒸发");
    static CRYSTALLIZE: SampleReaction = SampleReaction("结晶");
    static MELT: SampleReaction = SampleReaction("融化");
    static OTHER: SampleReaction = SampleReaction("另一个");

    /// 测试用 counter 触发器。
    #[derive(Debug)]
    struct Counter(&'static AtomicUsize);

    impl ReactionHandler for Counter {
        fn on_reaction(&self, _event: &ReactionEvent) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// 水 + 火（蒸发类反应，两种先后各一条，D132）。
    static VAPORIZE_CONDITIONS: [ReactionCondition; 2] =
        ReactionCondition::both_orders(Element::Hydro, Element::Pyro);

    /// 水 + 岩（结晶类反应：岩不可附着，但可按已有水附着判定，D134）。
    static CRYSTALLIZE_CONDITIONS: [ReactionCondition; 2] =
        ReactionCondition::both_orders(Element::Hydro, Element::Geo);

    /// 冰 + 火（融化类反应）。
    static MELT_CONDITIONS: [ReactionCondition; 2] =
        ReactionCondition::both_orders(Element::Cryo, Element::Pyro);

    fn vaporize_rule() -> ReactionRule {
        ReactionRule {
            reaction: &VAPORIZE,
            conditions: &VAPORIZE_CONDITIONS,
        }
    }

    fn crystallize_rule() -> ReactionRule {
        ReactionRule {
            reaction: &CRYSTALLIZE,
            conditions: &CRYSTALLIZE_CONDITIONS,
        }
    }

    fn melt_rule() -> ReactionRule {
        ReactionRule {
            reaction: &MELT,
            conditions: &MELT_CONDITIONS,
        }
    }

    /// 建一个已附着若干元素的角色状态（直接设定集合，D167 多元素共存）。
    fn with_attachments(elements: &[Element]) -> CharacterState {
        let state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let mut next = state;
        next.attachment = Attachment::from_elements(elements);
        next
    }

    fn env<'a>(rules: &'a [ReactionRule], registry: &'a ReactionRegistry) -> DamageEnv<'a> {
        DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules,
            registry,
        }
    }

    #[test]
    fn elemental_damage_without_reaction_subtracts_hp_and_attaches_element() {
        let registry = ReactionRegistry::new();
        let rules: [ReactionRule; 0] = [];
        let state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let outcome = damage(
            &state,
            n(1),
            n(2),
            n(3),
            DamageType::elemental(Element::Hydro),
            &env(&rules, &registry),
        );
        // 扣血（只减 hp）。
        assert_eq!(outcome.state.hp(), n(7));
        assert_eq!(outcome.state.energy(), Nat::ZERO);
        // 元素伤害、无反应 -> 附着攻击元素。
        assert!(outcome.attachment_landed);
        assert!(outcome.state.attachment.has(Element::Hydro));
        assert!(outcome.reaction.is_none());
        assert_eq!(outcome.fired, Nat::ZERO);
        // 记录里带来源。
        assert_eq!(outcome.record.source, Some(n(1)));
        assert_eq!(outcome.record.target, n(2));
        assert_eq!(outcome.record.amount, n(3));
    }

    #[test]
    fn reaction_consumes_participating_elements_and_attack_element_is_not_attached() {
        // D167：有反应 -> 命中的已覆盖元素 + 本次攻击元素都被移除；攻击元素不再附着。
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&VAPORIZE, &HANDLER);

        let rules = [vaporize_rule()];
        let state = with_attachments(&[Element::Hydro]);
        let outcome = damage(
            &state,
            n(1),
            n(2),
            n(2),
            DamageType::elemental(Element::Pyro),
            &env(&rules, &registry),
        );
        assert_eq!(outcome.reaction.map(|r| r.name()), Some("蒸发"));
        assert_eq!(outcome.fired, Nat::new(1));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        // 参与元素（水 + 火）全部清掉；本次攻击元素（火）不附着。
        assert!(!outcome.attachment_landed);
        assert!(outcome.state.attachment.is_empty());
        assert!(!outcome.state.attachment.has(Element::Hydro));
        assert!(!outcome.state.attachment.has(Element::Pyro));
        // 扣血照常。
        assert_eq!(outcome.state.hp(), n(8));
    }

    #[test]
    fn ice_and_grass_coexist_only_the_reacting_element_is_consumed() {
        // D167：多元素共存（冰 + 草）；本次火只和冰反应 -> 只消耗冰 + 火，草保留。
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&MELT, &HANDLER);

        let rules = [melt_rule()];
        let state = with_attachments(&[Element::Cryo, Element::Dendro]);
        let outcome = damage(
            &state,
            n(1),
            n(2),
            n(1),
            DamageType::elemental(Element::Pyro),
            &env(&rules, &registry),
        );
        assert_eq!(outcome.reaction.map(|r| r.name()), Some("融化"));
        assert_eq!(outcome.fired, Nat::new(1));
        // 冰（参与）+ 火（攻击元素）被移除；草（未参与）保留。
        assert!(!outcome.state.attachment.has(Element::Cryo));
        assert!(!outcome.state.attachment.has(Element::Pyro));
        assert!(outcome.state.attachment.has(Element::Dendro));
        assert_eq!(outcome.state.attachment.elements(), vec![Element::Dendro]);
    }

    #[test]
    fn attack_element_already_attached_is_also_removed_by_reaction() {
        // 攻击元素原本就在集合里（极端构造）：反应仍然把「攻击元素」移掉（D167：都不附着）。
        let registry = ReactionRegistry::new();
        let rules = [vaporize_rule()];
        let state = with_attachments(&[Element::Hydro, Element::Pyro]);
        let outcome = damage(
            &state,
            n(1),
            n(2),
            n(1),
            DamageType::elemental(Element::Pyro),
            &env(&rules, &registry),
        );
        assert!(outcome.reaction.is_some());
        assert!(outcome.state.attachment.is_empty());
    }

    #[test]
    fn non_attachable_element_still_reacts_and_consumes_existing_attachment() {
        // D134 / D167：岩不可附着（本来就不会落地），但按已有水判定 -> 结晶；
        // 反应消耗参与元素（水 + 岩）—— 水里已附着的那一个被清掉。
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&CRYSTALLIZE, &HANDLER);

        let rules = [crystallize_rule()];
        let state = with_attachments(&[Element::Hydro]);
        let outcome = damage(
            &state,
            n(1),
            n(2),
            n(2),
            DamageType::elemental(Element::Geo),
            &env(&rules, &registry),
        );
        assert_eq!(outcome.reaction.map(|r| r.name()), Some("结晶"));
        assert_eq!(outcome.fired, Nat::new(1));
        // 附着本来就不可能落地；水参与反应被移除；岩没有落地。
        assert!(!outcome.attachment_landed);
        assert!(outcome.state.attachment.is_empty());
        assert!(!outcome.state.attachment.has(Element::Geo));
    }

    #[test]
    fn physical_and_piercing_only_subtract_hp_and_keep_attachments() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_any(&HANDLER);
        let rules = [vaporize_rule()];
        let state = with_attachments(&[Element::Cryo, Element::Dendro]);

        for ty in [DamageType::Physical, DamageType::Piercing] {
            let outcome = damage(&state, n(1), n(2), n(2), ty, &env(&rules, &registry));
            assert_eq!(outcome.state.hp(), n(8));
            // 物理 / 穿透：不附着、不判定反应；多元素集合原样保留。
            assert!(!outcome.attachment_landed);
            assert!(outcome.reaction.is_none());
            assert_eq!(outcome.fired, Nat::ZERO);
            assert_eq!(
                outcome.state.attachment.elements(),
                vec![Element::Dendro, Element::Cryo]
            );
        }
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn no_matching_rule_attaches_attack_element_on_top_of_existing() {
        // 无反应时元素正常附着；多元素共存（冰 + 草 + 火）。
        let registry = ReactionRegistry::new();
        let rules = [vaporize_rule()];
        let state = with_attachments(&[Element::Cryo, Element::Dendro]);
        let outcome = damage(
            &state,
            n(1),
            n(2),
            n(1),
            DamageType::elemental(Element::Pyro),
            &env(&rules, &registry),
        );
        assert!(outcome.reaction.is_none());
        assert!(outcome.attachment_landed);
        assert!(outcome.state.attachment.has(Element::Cryo));
        assert!(outcome.state.attachment.has(Element::Dendro));
        assert!(outcome.state.attachment.has(Element::Pyro));
    }

    #[test]
    fn only_the_first_matching_rule_fires_and_consumes() {
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static HANDLER: Counter = Counter(&CALLS);
        let mut registry = ReactionRegistry::new();
        registry.register_any(&HANDLER);

        let rules = [
            vaporize_rule(),
            ReactionRule {
                reaction: &OTHER,
                conditions: &VAPORIZE_CONDITIONS,
            },
        ];
        let state = with_attachments(&[Element::Hydro]);
        let outcome = damage(
            &state,
            n(1),
            n(2),
            n(1),
            DamageType::elemental(Element::Pyro),
            &env(&rules, &registry),
        );
        // 两条规则都满足，但只按第一条触发一次（一次伤害最多一个反应）。
        assert_eq!(outcome.reaction.map(|r| r.name()), Some("蒸发"));
        assert_eq!(outcome.fired, Nat::new(1));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert!(outcome.state.attachment.is_empty());
    }

    #[test]
    fn reaction_event_carries_source_target_and_matched_elements() {
        // D136 / D167：事件带「谁触发 / 作用在谁身上」与命中的那一个已覆盖元素。
        static SEEN: Mutex<Option<(Nat, Nat, Element, Element)>> = Mutex::new(None);

        #[derive(Debug)]
        struct Recorder;

        impl ReactionHandler for Recorder {
            fn on_reaction(&self, event: &ReactionEvent) {
                let mut slot = SEEN.lock().expect("锁");
                *slot = Some((event.source, event.target, event.covered, event.attack));
            }
        }

        static RECORDER: Recorder = Recorder;
        let mut registry = ReactionRegistry::new();
        registry.register_by_kind(&VAPORIZE, &RECORDER);

        let rules = [vaporize_rule()];
        let state = with_attachments(&[Element::Hydro]);
        let _ = damage(
            &state,
            n(1),
            n(2),
            n(1),
            DamageType::elemental(Element::Pyro),
            &env(&rules, &registry),
        );
        let seen = SEEN.lock().expect("锁").expect("应记录一次反应");
        assert_eq!(seen, (n(1), n(2), Element::Hydro, Element::Pyro));
    }

    #[test]
    fn damage_is_pure() {
        // 纯函数：入参状态一个字段都不改。
        let registry = ReactionRegistry::new();
        let rules = [vaporize_rule()];
        let state = with_attachments(&[Element::Hydro]);
        let _ = damage(
            &state,
            n(1),
            n(2),
            n(3),
            DamageType::elemental(Element::Pyro),
            &env(&rules, &registry),
        );
        assert_eq!(state.hp(), n(10));
        assert_eq!(state.attachment.elements(), vec![Element::Hydro]);
    }

    // ==================================================================
    // 伤修链入口（damage_with_chain）
    // ==================================================================

    /// 空伤修链环境（没有护盾 / 减伤；两侧都空）。
    fn empty_chain_state() -> (
        crate::families::character::zones::combat_status::CombatStatusZone,
        crate::families::action_card::status::StateList,
    ) {
        (
            crate::families::character::zones::combat_status::CombatStatusZone::new(),
            crate::families::action_card::status::StateList::new(),
        )
    }

    #[test]
    fn damage_with_chain_adds_reaction_bonus_before_hp() {
        // 已有冰附着 + 本次火 2 点：融化「本伤害+2」必须先加上，再扣血。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let (combat_status, states) = empty_chain_state();
        let chain_env = DamageChainEnv::new(&combat_status, &states);
        let triggers = CharacterTriggers::new();
        let state = with_attachments(&[Element::Cryo]);
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(
            &state,
            n(1),
            n(2),
            n(2),
            DamageType::elemental(Element::Pyro),
            &env,
        );
        // 加伤 2 -> 伤害 4 -> 扣血 4（10 -> 6）。
        assert_eq!(result.chain.bonus_total, n(2));
        assert_eq!(result.chain.to_hp, n(4));
        assert_eq!(result.outcome.state.hp(), n(6));
        assert_eq!(result.reaction.map(|hit| hit.reaction.name()), Some("融化"));
        // 反应判定 / 消耗参与元素仍走原来那份（D167）。
        assert!(result.outcome.reaction.is_some());
        assert!(!result.outcome.attachment_landed);
        assert!(result.outcome.state.attachment.is_empty());
    }

    #[test]
    fn damage_with_chain_without_reaction_is_the_plain_damage() {
        // 没有附着 -> 没有反应加伤 -> 与 damage() 完全一致（2 点扣 2）。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let (combat_status, states) = empty_chain_state();
        let chain_env = DamageChainEnv::new(&combat_status, &states);
        let triggers = CharacterTriggers::new();
        let state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(
            &state,
            n(1),
            n(2),
            n(2),
            DamageType::elemental(Element::Pyro),
            &env,
        );
        assert_eq!(result.chain.bonus_total, Nat::ZERO);
        assert_eq!(result.outcome.state.hp(), n(8));
        assert!(result.reaction.is_none());
    }

    #[test]
    fn damage_with_chain_piercing_ignores_bonus_and_shield() {
        // 穿透：不吃加伤 / 护盾 / 减伤（规则书·特殊词条 穿透伤害）。
        use crate::families::character::damage_chain::statuses::SHIELD_COMBAT;
        use crate::families::character::zones::combat_status::CombatStatusTriggers;

        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let triggers = CharacterTriggers::new();
        let combat_status =
            crate::families::character::zones::combat_status::CombatStatusZone::new()
                .attach_with_triggers(
                    n(2),
                    &SHIELD_COMBAT,
                    Some(n(10)),
                    None,
                    &CombatStatusTriggers::new(),
                )
                .zone;
        let states = crate::families::action_card::status::StateList::new();
        let chain_env = DamageChainEnv::new(&combat_status, &states);
        let state = with_attachments(&[Element::Hydro]);
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus {
                increases: &[n(3)],
                ..DamageBonus::NONE
            },
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(&state, n(1), n(2), n(2), DamageType::Piercing, &env);
        assert!(result.chain.bypassed);
        assert_eq!(result.chain.shield_used, Nat::ZERO);
        assert_eq!(result.chain.to_hp, n(2));
        assert_eq!(result.outcome.state.hp(), n(8));
        assert!(result.reaction.is_none());
    }

    #[test]
    fn damage_with_chain_shield_reduces_the_damage_that_lands() {
        // 护盾 1 点：2 点伤害 -> 只扣 1 点血（伤修链的 to_hp 传给原 damage()）。
        use crate::families::character::damage_chain::statuses::SHIELD_COMBAT;
        use crate::families::character::zones::combat_status::CombatStatusTriggers;

        let rules: [ReactionRule; 0] = [];
        let registry = ReactionRegistry::new();
        let triggers = CharacterTriggers::new();
        let combat_status =
            crate::families::character::zones::combat_status::CombatStatusZone::new()
                .attach_with_triggers(
                    n(2),
                    &SHIELD_COMBAT,
                    Some(n(1)),
                    None,
                    &CombatStatusTriggers::new(),
                )
                .zone;
        let states = crate::families::action_card::status::StateList::new();
        let chain_env = DamageChainEnv::new(&combat_status, &states);
        let state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(&state, n(1), n(2), n(2), DamageType::Physical, &env);
        assert_eq!(result.chain.shield_used, n(1));
        assert_eq!(result.chain.to_hp, n(1));
        assert_eq!(result.outcome.state.hp(), n(9));
    }

    #[test]
    fn damage_with_chain_dispatches_hp_changed_with_the_landed_amount() {
        // 「生命值变动」派发用**扣到生命值**的数值（走完链之后），不是基础伤害。
        use crate::families::character::damage_chain::statuses::SHIELD_COMBAT;
        use crate::families::character::zones::combat_status::CombatStatusTriggers;
        use crate::rules::ops::order::EffectSource;
        use crate::timeline::{Event, Trigger};

        fn noop(_state: &mut CharacterState, _event: &Event, _owner: EntityId) {}

        let rules: [ReactionRule; 0] = [];
        let registry = ReactionRegistry::new();
        let mut triggers = CharacterTriggers::new();
        triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            n(1),
            EffectSource::CharacterCard,
            n(0),
            noop,
        ));
        let combat_status =
            crate::families::character::zones::combat_status::CombatStatusZone::new()
                .attach_with_triggers(
                    n(2),
                    &SHIELD_COMBAT,
                    Some(n(2)),
                    None,
                    &CombatStatusTriggers::new(),
                )
                .zone;
        let states = crate::families::action_card::status::StateList::new();
        let chain_env = DamageChainEnv::new(&combat_status, &states);
        let state = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(&state, n(1), n(2), n(5), DamageType::Physical, &env);
        // 5 点伤害、盾 2 -> 扣血 3；派发的事件数值 = 3。
        assert_eq!(result.pending.len(), 1);
        assert_eq!(result.pending.first().expect("一条").event.amount, n(3));
    }

    #[test]
    fn damage_with_chain_is_pure_on_the_character_state() {
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let (combat_status, states) = empty_chain_state();
        let chain_env = DamageChainEnv::new(&combat_status, &states);
        let triggers = CharacterTriggers::new();
        let state = with_attachments(&[Element::Cryo]);
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let _ = damage_with_chain(
            &state,
            n(1),
            n(2),
            n(2),
            DamageType::elemental(Element::Pyro),
            &env,
        );
        assert_eq!(state.hp(), n(10));
        assert_eq!(state.attachment.elements(), vec![Element::Cryo]);
    }

    #[test]
    fn type_override_turns_physical_into_hydro_and_vaporizes() {
        // 卡面「使角色造成的物理伤害变为水元素伤害」：覆盖先于反应判定。
        // 物理 1 点被覆盖成水，打火附着 -> 蒸发（本伤害 +2）-> 扣 3。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let (combat_status, states) = empty_chain_state();
        let chain_env = DamageChainEnv::new(&combat_status, &states)
            .with_type_override(Some(DamageType::elemental(Element::Hydro)));
        let triggers = CharacterTriggers::new();
        let state = with_attachments(&[Element::Pyro]);
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(&state, n(1), n(2), n(1), DamageType::Physical, &env);
        let hit = result.reaction.expect("覆盖为水后应判定蒸发");
        assert_eq!(hit.reaction.name(), "蒸发");
        assert_eq!(hit.covered, Element::Pyro);
        assert_eq!(hit.attack, Element::Hydro, "反应判定用覆盖后的类型");
        assert_eq!(result.chain.to_hp, n(3), "蒸发 +2 -> 1 + 2 = 3");
        assert_eq!(result.chain.input.ty, DamageType::elemental(Element::Hydro));
        assert_eq!(result.outcome.state.hp(), n(7));
        // 反应消耗双方元素：火附着被消耗掉。
        assert!(result.outcome.state.attachment.elements().is_empty());
    }

    #[test]
    fn type_override_turns_physical_into_hydro_and_electro_charges() {
        // 物理 -> 水，打雷附着 -> 感电（本伤害 +1）-> 扣 2。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let (combat_status, states) = empty_chain_state();
        let chain_env = DamageChainEnv::new(&combat_status, &states)
            .with_type_override(Some(DamageType::elemental(Element::Hydro)));
        let triggers = CharacterTriggers::new();
        let state = with_attachments(&[Element::Electro]);
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(&state, n(1), n(2), n(1), DamageType::Physical, &env);
        let hit = result.reaction.expect("覆盖为水后应判定感电");
        assert_eq!(hit.reaction.name(), "感电");
        assert_eq!(result.chain.to_hp, n(2), "感电 +1 -> 1 + 1 = 2");
        assert_eq!(result.outcome.state.hp(), n(8));
    }

    #[test]
    fn unoverridden_physical_damage_still_does_not_react() {
        // 默认（无 override）：物理打火附着不反应，链数值逐字节不变（默认等价对照）。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let (combat_status, states) = empty_chain_state();
        let chain_env = DamageChainEnv::new(&combat_status, &states);
        let triggers = CharacterTriggers::new();
        let state = with_attachments(&[Element::Pyro]);
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let result = damage_with_chain(&state, n(1), n(2), n(1), DamageType::Physical, &env);
        assert!(result.reaction.is_none(), "物理伤害不判反应");
        assert_eq!(result.chain.to_hp, n(1));
        assert_eq!(result.outcome.state.hp(), n(9));
        // 火附着没有被消耗。
        assert_eq!(result.outcome.state.attachment.elements(), vec![Element::Pyro]);
    }

    #[test]
    fn recorded_chain_shows_original_type_then_override_then_reaction() {
        use crate::families::character::damage_chain::record::DamageChainRecorder;

        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let (combat_status, states) = empty_chain_state();
        let chain_env = DamageChainEnv::new(&combat_status, &states)
            .with_type_override(Some(DamageType::elemental(Element::Hydro)));
        let triggers = CharacterTriggers::new();
        let state = with_attachments(&[Element::Pyro]);
        let env = ChainDamageEnv {
            damage: &env(&rules, &registry),
            chain: &chain_env,
            bonus: DamageBonus::NONE,
            context: &EffectContext::no_hands(true),
            triggers: &triggers,
        };
        let mut recorder = DamageChainRecorder::new();
        let mut recorded = RecordedStep::new(n(3), &mut recorder);
        let _ = damage_with_chain_recorded(
            &state,
            n(1),
            n(2),
            n(1),
            DamageType::Physical,
            &env,
            &mut recorded,
        );
        let kinds: Vec<&'static str> = recorder
            .entries()
            .iter()
            .map(|record| match record.content {
                DamageChainRecord::Hit { ty, .. } => {
                    assert_eq!(ty, DamageType::Physical, "Hit 记原始类型");
                    "原始"
                }
                DamageChainRecord::TypeOverride { original, actual } => {
                    assert_eq!(original, DamageType::Physical);
                    assert_eq!(actual, DamageType::elemental(Element::Hydro));
                    "覆盖"
                }
                DamageChainRecord::Reaction { name, .. } => {
                    assert_eq!(name, "蒸发");
                    "反应"
                }
                _ => "其它",
            })
            .collect();
        // 顺序：Hit(原始) -> TypeOverride(原始->实际) -> 链内各段 -> Reaction(蒸发)。
        assert_eq!(kinds[0], "原始");
        assert_eq!(kinds[1], "覆盖");
        let reaction_index = kinds
            .iter()
            .position(|kind| *kind == "反应")
            .expect("记录了反应");
        assert!(reaction_index > 1, "反应记录在类型覆盖之后");
        assert!(recorder.entries().iter().all(|record| record.step == n(3)));
    }
}
