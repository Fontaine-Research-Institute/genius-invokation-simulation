//! **一次「使用技能」端到端**（D184 第 2 步的串联层；D227：效果体默认走关键字一步）。
//!
//! can_use -> 派发「使用技能时」-> 支付（骰子 + 充能）-> 执行「使用技能时」-> 效果体 ->
//! 步末 flush -> 派发「使用技能后」（用 timeline::Resolution 的 insert 插在**步边界**）->
//! 新 DefState + 剩余待执行。
//!
//! ## 依据（唯一权威）
//!
//! - 规则书·进行行动 战斗行动【使用技能】：支付费用 -> 出战角色使用技能攻击对方出战角色；
//!   普通攻击 / 元素战技后获得 1 点充能；元素爆发花费所有充能（卡面数据里 energy = 充能上限）；
//! - 规则书·元素反应·规则：元素伤害 -> 附着 -> 判定反应 -> 反应效果；
//! - 结算顺序.md §三：造成伤害之前先「受伤时」修正（护盾抵消 -> 减伤扣减，穿透不吃），
//!   扣血后「受伤后」，元素伤害「附着 -> 判定反应 -> 结算反应效果 -> 继续原步骤」；
//!   §二：插入式结算（不是栈）——「使用技能后」作为子结算插到当前位置；
//! - D186：派发只排序、不执行；本层是「调用方」，执行仍走各区的 execute_*（pending.rs 统一分派）；
//! - D204 / D208 / D227：效果体是关键字层**一步**（步内禁止插入）；待执行在**步末**统一
//!   flush（本层是 execute_step 的调用方）；「使用技能后」只在步边界插入；
//! - 规则书·其他规则 **结算顺序**（S7.5）：「1、先结算当前牌手的所有卡牌效果，再结算对方
//!   牌手的卡牌效果。2、同属于同一位牌手的卡牌效果…角色牌、角色装备和状态、阵营出战状态、
//!   召唤物、支援牌。3、同属于同一位牌手的同类型卡牌效果…进场的先后顺序」。
//!   所以「使用技能时 / 后」必须对**双方**注册表派发：当前牌手（使用方）的全部先结算，
//!   再结算对方的；同一侧内再按第 2 / 3 条排序（复用 timeline::resolve + rulebook 顺序表）。
//!
//! ## 两条效果体路径（D227）
//!
//! - [SkillEffectBody::Step]（默认 / 新路径）：关键字层一步，用 keywords::step 的
//!   execute_step_me / execute_step_opp 执行；步末 pending 由本层 flush（execute_pending）；
//!   一条一步 = 一个不可中断的原子结算单位，插入只能发生在步与步之间；
//! - [SkillEffectBody::Plan]（旧路径 / 兼容）：D184 的效果体计划（单段伤害），
//!   行为与旧版完全一致（伤害 -> 反应效果执行器 -> 生命值变动待执行），不经过关键字层。
//!
//! ## 为什么返回分支而不是单一局面（D190 / D191）
//!
//! 关键字一步 / 反应效果都可能带随机（月感电 / 月绽放给手牌上状态、随机目标等），
//! 执行器会把它们展开成「操作清单 + 精确概率」的多支（D190：不采样）。每支都会在
//! 同一份输入上独立执行，得到一份完整局面 + 概率；本层原样带回（不做归一化、不丢支）。
//! 确定性技能 / 无随机 -> 恰好一支、概率 1（用 SkillOutcome::single 取）。
//!
//! ## 下落攻击（S8.10；Task 3 / L6）
//!
//! 标记的**持有区 = [crate::state::PlayerDefs::plunging]**（每方一个，根状态持有）：
//!
//! - [use_skill_me] / [use_skill_opp] 读**使用方当前标记**（`state.me()/opp().plunging()`）；
//! - 带显式参数时用 [use_skill_me_with_plunging] / [use_skill_opp_with_plunging]（参数应与
//!   根状态里的标记一致；本层以参数做生效形态判定，结束时仍按根状态清除）；
//! - 判定在 `skill_flow::use_skill_with_plunging` 内做：标记待消耗且本次是普通攻击 ->
//!   生效形态视为下落攻击（优先于重击）；判定取样 = 支付前骰池总数（S8.9 / S8.10）；
//! - 本层把生效形态带出（[SkillOutcome::effective_kind]）；
//! - **战斗行动结束**：每支分支都调 `state::flows::plunging::battle_action_end_me/opp`
//!   清除使用方标记并写回根状态（[SkillBranch::state] 已带）；
//! - [SkillOutcome::plunging] 保留为**观测值**（= 输入标记 after_combat_action，当前实现为 NONE）。
//!
//! 待实测（规则书未写清）：本次战斗行动**不是普通攻击**时标记是否保留 ——
//! `attack_form::PlungingMark::after_combat_action` 当前按「一次战斗行动结束即清除」。
//!
//! ## 终局判定（Task 2 方案 A）
//!
//! [SkillBranch::outcome] = 只读 DefState 的 [crate::flow::game_end::settlement_outcome]。
//! 每步结算（旧路径：伤害 / 反应效果执行后；新路径：步末 flush 后）先给一次判定；
//! **必须在调用方完成 `state::flows::defeat` 清理后再复判**（HP 归 0 但角色仍在角色区 ->
//! 如实 Ongoing，见 `state/terminal.rs` 的调用时点）。本层不改 `state/**`、也不做击倒清理。
//!
//! ## 待接入 / 待复核
//!
//! - **反应效果的执行（R4 / R2）**：关键字一步内部已在**步边界**把本步判出的反应子结算
//!   插入执行（`keywords::reaction::settle_inserted`；D204：步内不插，一步仍是原子单位），
//!   反应派发的待执行接在本步待执行之前、由本层步末 flush 统一执行；旧路径（Plan）仍用
//!   D184 的反应效果执行器。两条路径的「反应效果 vs 生命值变动」先后仍待实测（见下一条）；
//! - 「使用技能时」在支付之后执行：skill_flow::use_skill 的顺序是「派发 before -> 支付 ->
//!   派发 after」，但派发不执行，所以本层只能在支付写回后执行 before；费用修正链
//!   （少花费 1 骰）接在支付之前时要重新排（待接入）；
//! - 「生命值变动」与反应效果的先后：结算顺序.md 只分别列出「扣血后」与「附着 -> 反应效果」；
//!   旧路径本层按「反应效果 -> 生命值变动」执行；新路径（Step）由关键字一步在步边界插入反应
//!   子结算、反应待执行排在本步待执行之前（`keywords/step.rs` 口径）；两者相对次序待实测；
//! - 反应效果给手牌加牌后的费用表同步：缺 base_cost 回调，待接入；
//! - 「使用技能时 / 后」的两侧注册表：本层已按 S7.5 第 1 条做「当前牌手（使用方）先、
//!   对方后」；同一侧内的第 2 / 3 条（来源类别 -> 进场先后）由 timeline 的顺序表保证；
//!   三个技能作用域时机（任意 / 某一类 / 具体技能）在每一侧内重新归并排序，见
//!   dispatch_skill_two_sided（待复核：三档作用域之间的并列次序沿用 skill_flow 的宽到窄约定）；
//! - 目标选择：旧路径固定「对方出战角色」为目标；新路径由关键字的目标选择器决定
//!   （D25 / D26 的选择策略不在这里）。

use crate::families::character::attachment::{AttachmentAllowSet, ReactionRegistry};
use crate::families::character::damage::{
    damage_with_chain, ChainDamageEnv, DamageEnv, ReactionRule,
};
use crate::families::character::damage_chain::bonus::{DamageBonus, ReactionHit};
use crate::families::character::damage_chain::chain::DamageChainEnv;
use crate::families::character::damage_chain::executor::{
    execute_resolved, EntityAlloc, ExecutionEnv, ReactionTriggers, ReactionZones, SidePending,
    SideZones,
};
use crate::families::character::skill::SkillDef;
use crate::families::character::skill_flow::{
    skill_event, skill_timings, use_skill_with_plunging, CannotUse, PlungingMark, PreparedSkills,
    SkillFrames, SkillPending, SkillPhase, SkillTriggers, SkillUseContext, SkillUsePolicy,
};
use crate::families::character::trigger::{execute_character, CharacterPending};
use crate::families::character::types::{DamageType, SkillKind};
use crate::flow::game_end::{settlement_outcome, Outcome};
use crate::flow::hooks::{apply_defeat_cleanup, DefeatCleanup};
use crate::flow::pending::{execute_pending, FlowPending, PendingSet};
use crate::flow::record::FlowRecord;
use crate::keywords::effect::{EffectEnv, TriggerBook};
use crate::keywords::step::{execute_step_me, execute_step_opp, Step};
use crate::reactions::effect::EffectContext;
use crate::rules::ops::order::resolves_before;
use crate::state::flows::plunging::{battle_action_end_me, battle_action_end_opp};
use crate::state::{DefState, PlayerDefs};
use crate::timeline::{dispatch, resolve, Event, Pending};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, EntityId, Nat};
use std::cmp::Ordering;

/// 一次技能使用的**失败原因**。
///
/// 所有失败都保证**入参一个字节都不改**（内部在克隆上做，Err 时克隆丢弃）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkillUseError {
    /// 技能本身不可用（沿用 skill_flow 的判定原因）。
    CannotUse(CannotUse),
    /// 使用方没有出战角色（使用技能必须是出战角色，规则书·进行行动 使用技能）。
    NoActiveCharacter,
    /// 旧路径的伤害型效果体需要对方出战角色，但对方没有。
    NoTargetCharacter,
}

impl From<CannotUse> for SkillUseError {
    fn from(reason: CannotUse) -> SkillUseError {
        SkillUseError::CannotUse(reason)
    }
}

/// 技能的**旧效果体计划**（D162 占位；D227 起只作**兼容路径**保留，行为不变）。
///
/// 旧路径只串「最常见的单段伤害」：具体技能的多段 / 状态 / 召唤等效果由卡牌代码在
/// 「使用技能后」之前自己执行（本层不写具体效果）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SkillEffectPlan {
    /// 没有效果体（占位：例如测试 / 只验证流程）。
    None,
    /// 单段伤害：对**对方出战角色**造成 amount 点 ty 类型伤害（走伤修链 + 反应）。
    Damage {
        /// 基础伤害。
        amount: Nat,
        /// 伤害类型（元素 / 物理 / 穿透）。
        ty: DamageType,
    },
}

/// 技能的**效果体**（D227：默认走关键字层的一步；旧路径保留为兼容）。
#[derive(Clone, Copy, Debug)]
pub enum SkillEffectBody<'a> {
    /// 旧路径（兼容；行为不变）。
    Plan(SkillEffectPlan),
    /// 新路径：关键字层一步。
    Step(&'a Step),
}

impl<'a> From<SkillEffectPlan> for SkillEffectBody<'a> {
    fn from(plan: SkillEffectPlan) -> SkillEffectBody<'a> {
        SkillEffectBody::Plan(plan)
    }
}

impl<'a> SkillEffectBody<'a> {
    /// 这条效果体是不是「伤害型计划」（旧路径需要对方出战角色做目标）。
    fn plan_is_damage(self) -> bool {
        matches!(self, SkillEffectBody::Plan(SkillEffectPlan::Damage { .. }))
    }

    /// 效果体路径的短描述（记录 / 观测用）。
    pub fn description(self) -> &'static str {
        match self {
            SkillEffectBody::Plan(_) => "旧路径（效果体计划）",
            SkillEffectBody::Step(_) => "关键字一步",
        }
    }

    /// 一条一步（旧路径自动迁移；新路径原样）。
    pub fn step(self) -> Step {
        match self {
            SkillEffectBody::Plan(plan) => plan.to_step(),
            SkillEffectBody::Step(step) => step.clone(),
        }
    }
}

/// **两位牌手的角色族注册表**（S7.5 第 1 条：当前牌手优先需要两张表，而不是一张）。
///
/// 两张表是**两个具名实例**（D34：不传敌我编号 / 下标）；谁是「当前牌手」由成对入口
/// （use_skill_me / use_skill_opp）绑定，本结构只保存两张表。
#[derive(Clone, Copy, Debug)]
pub struct SkillTriggerBook<'a> {
    /// 我方的角色族注册表（「使用技能时 / 后」/「生命值变动」）。
    pub me: &'a SkillTriggers,
    /// 敌方的角色族注册表（同上）。
    pub opp: &'a SkillTriggers,
}

impl<'a> SkillTriggerBook<'a> {
    /// 两位牌手的注册表。
    pub const fn new(me: &'a SkillTriggers, opp: &'a SkillTriggers) -> SkillTriggerBook<'a> {
        SkillTriggerBook { me, opp }
    }

    /// **当前牌手**（使用技能的一方）的注册表：me_is_current = true 表示我方是当前牌手。
    fn current(self, me_is_current: bool) -> &'a SkillTriggers {
        if me_is_current {
            self.me
        } else {
            self.opp
        }
    }

    /// **对方牌手**（非当前牌手）的注册表。
    fn other(self, me_is_current: bool) -> &'a SkillTriggers {
        if me_is_current {
            self.opp
        } else {
            self.me
        }
    }
}

/// 技能使用流程的**外部依赖**（只借不拥有；由调用方按局面给）。
pub struct SkillFlowEnv<'a> {
    /// 两位牌手的角色族注册表（S7.5：当前牌手优先；单侧派发已不再使用）。
    pub triggers: SkillTriggerBook<'a>,
    /// 关键字层（一步）用的双方触发集合（区触发 / 角色族触发 / 支援 / 装备）。
    ///
    /// 只在效果体是 [SkillEffectBody::Step] 时用；旧路径不需要它（传 TriggerBook::empty()）。
    pub keyword_triggers: TriggerBook<'a>,
    /// **击倒清理回调**（D255：结算 -> 清理 -> 判终局）。
    ///
    /// 上层（cards）注入真实实现（state::flows::defeat 需要按角色给 DefeatHpReset）；
    /// 不注入时传 NO_DEFEAT_CLEANUP（行为与旧版完全一致）。
    pub defeat_cleanup: DefeatCleanup,
    /// 允许附着的元素集合（角色 / 卡牌定义给，D130）。
    pub allow: AttachmentAllowSet,
    /// 反应规则表（具体反应表由 reactions 块给；顺序 = 判定顺序）。
    pub rules: &'a [ReactionRule],
    /// 「反应时」触发注册表（D127 / D136）。
    pub reaction_registry: &'a ReactionRegistry,
    /// 反应效果执行器用的双方触发注册表。
    pub reaction_triggers: &'a ReactionTriggers,
    /// 触发方元素精通（伤害链的反应加伤 + 执行器强化反应效果）。
    pub mastery: Nat,
    /// 通用增伤（卡面「造成的伤害+N」；顺序由调用方按结算顺序排好）。
    pub increases: &'a [Nat],
    /// 反应效果函数的局面摘要（是否出战角色 / 双方手牌数）。
    pub effect_context: &'a EffectContext,
    /// 反应效果生成物的实体发号器（只发号，实体表在状态层）。
    pub alloc: EntityAlloc,
    /// 使用技能策略开关（被动 / 准备技能阻塞）。
    pub policy: SkillUsePolicy,
    /// 「获得充能」的获得量修正（energy::energy_gain 的 modifiers）。
    pub energy_gain_bonus: &'a [Nat],
    /// 同色费用支付时选的颜色（SameColor 费用需要）。
    pub same_color: Option<crate::base::Element>,
}

/// 一次技能使用的**请求**（本层不替你选技能 / 目标范围）。
pub struct SkillUseRequest<'a> {
    /// 技能定义。
    pub skill: &'a SkillDef,
    /// 使用方出战角色身上的准备技能槽（K53）。
    pub prepared: &'a PreparedSkills,
    /// 效果体（默认走关键字一步；旧的效果体计划保留为兼容）。
    pub effect: SkillEffectBody<'a>,
}

/// 伤害观测摘要（纯数据；旧路径才有：伤害发生一次，所有反应分支共享同一份基础伤害）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SkillDamageSummary {
    /// 实际扣到生命值的伤害（走完伤修链）。
    pub to_hp: Nat,
    /// 被护盾吸掉的点数。
    pub shield_used: Nat,
    /// 加伤总计（通用增伤 + 反应加伤 + 元素精通）。
    pub bonus_total: Nat,
    /// 本次命中 / 预测命中的反应（没有 -> None）。
    pub reaction: Option<ReactionHit>,
    /// 本次元素伤害的附着是否落地。
    pub attachment_landed: bool,
}

/// **一支技能结算分支**（D190：每支都带精确概率）。
#[derive(Clone, Debug)]
pub struct SkillBranch {
    /// 这一支结算后的完整局面。
    pub state: DefState,
    /// 这一支的精确概率。
    pub probability: Fraction,
    /// 这一支之后还要执行的待执行（旧路径：反应效果派发出来的；新路径在步末已 flush，恒空）。
    pub pending: PendingSet,
    /// 这一支的终局判定（只读；调用时点见 [settlement_outcome]：每步结算 + defeat 清理之后）。
    pub outcome: Outcome,
}

/// 一次技能使用的结果（确定性技能 -> branches 恰好一支）。
#[derive(Clone, Debug)]
pub struct SkillOutcome {
    /// 结算分支（D190：反应效果 / 随机效果可能多支）。
    pub branches: Vec<SkillBranch>,
    /// 伤害观测摘要（旧路径的伤害计划才有；新路径为 None）。
    pub damage: Option<SkillDamageSummary>,
    /// 本次命中的反应（旧路径；新路径为 None，见模块「待接入」）。
    pub reaction: Option<ReactionHit>,
    /// 「使用技能时」的待执行（**已由本层执行**；保留供观测 / 测试）。
    pub before: Vec<SkillPending>,
    /// 结算后的实体发号器（一步里生成过召唤物 / 卡牌时前移；旧路径 = env.alloc）。
    pub alloc: EntityAlloc,
    /// 本次技能的**生效形态**（S8.9 / S8.10：普通攻击可能被视为重击 / 下落攻击）。
    ///
    /// 由 `skill_flow::use_skill_with_plunging` 在**支付前**取样骰总数判定；本层只把结果带出，
    /// 供调用方派发「普通攻击后」/ 加伤 / 观测用。
    pub effective_kind: SkillKind,
    /// 本次战斗行动**结束之后**的下落攻击标记（S8.10；观测值）。
    ///
    /// 输入标记来自使用方根状态（`PlayerDefs::plunging`）或 [use_skill_me_with_plunging] 的参数；
    /// 这里是 [PlungingMark::after_combat_action] 后的新值（一次战斗行动结束 -> 清除）。
    /// 清除结果**已写回**每支 [SkillBranch::state]；本字段保留供观测 / 调用方对齐。
    pub plunging: PlungingMark,
    /// 流程级记录（D225 / D227：使用技能 / 一步 / 步末 flush / 步边界插入）。
    ///
    /// 本层只产出内容（不持有记录器）：由 flow/recorded.rs 的入口写进调用方给的 Recorder。
    pub events: Vec<FlowRecord>,
}

impl SkillOutcome {
    /// 确定性技能（或没有效果体）时唯一的那一支；多支时返回 None（请改用 branches）。
    pub fn single(&self) -> Option<&SkillBranch> {
        if self.branches.len() == 1 {
            self.branches.first()
        } else {
            None
        }
    }
}

// ======================================================================
// 内部结果（未绑定敌我；由两个公开入口各自组装成 DefState）
// ======================================================================

/// 一侧玩家的「原始分支」（user = 使用方、target = 对方）。
struct RawBranch {
    user: PlayerDefs,
    target: PlayerDefs,
    probability: Fraction,
    source_pending: SidePending,
    target_pending: SidePending,
}

/// run 的内部结果。
struct RunOutput {
    branches: Vec<RawBranch>,
    damage: Option<SkillDamageSummary>,
    reaction: Option<ReactionHit>,
    before: Vec<SkillPending>,
    alloc: EntityAlloc,
    effective_kind: SkillKind,
    events: Vec<FlowRecord>,
}

impl RunOutput {
    /// 组装成公开结果：me_is_user = true 表示「我方是使用方」。
    fn assemble(
        self,
        state: &DefState,
        me_is_user: bool,
        plunging: PlungingMark,
        defeat_cleanup: DefeatCleanup,
    ) -> SkillOutcome {
        let branches: Vec<SkillBranch> = self
            .branches
            .into_iter()
            .map(|branch| {
                let (me, opp, me_pending, opp_pending) = if me_is_user {
                    (
                        branch.user,
                        branch.target,
                        branch.source_pending,
                        branch.target_pending,
                    )
                } else {
                    (
                        branch.target,
                        branch.user,
                        branch.target_pending,
                        branch.source_pending,
                    )
                };
                let mut pending = PendingSet::new();
                pending.me = flow_pending_from_side(me_pending);
                pending.opp = flow_pending_from_side(opp_pending);
                let branch_state = state.clone().with_me(me).with_opp(opp);
                // S8.10：一次战斗行动结束 -> 清除**使用方**的下落攻击标记并写回根状态
                // （state::flows::plunging::battle_action_end_me/opp，配对入口 D35）。
                let branch_state = if me_is_user {
                    battle_action_end_me(&branch_state)
                } else {
                    battle_action_end_opp(&branch_state)
                };
                // D255：结算 -> 击倒清理 -> 判终局。清理派发出来的待执行并进分支 pending。
                let branch_state =
                    apply_defeat_cleanup(&branch_state, defeat_cleanup, &mut pending);
                let outcome = settlement_outcome(&branch_state);
                SkillBranch {
                    state: branch_state,
                    probability: branch.probability,
                    pending,
                    outcome,
                }
            })
            .collect();
        SkillOutcome {
            branches: fold_skill_branches(branches),
            damage: self.damage,
            reaction: self.reaction,
            before: self.before,
            alloc: self.alloc,
            effective_kind: self.effective_kind,
            plunging: plunging.after_combat_action(),
            events: self.events,
        }
    }
}

/// **使用技能**（我方使用；下落标记读根状态）。
///
/// - 成功 -> 结算分支（确定性技能恰好一支）；settlement 会被**插入**「使用技能后」的子结算
///   （D196：插到当前 cursor 位置 = 步边界；调用方之后用 pending::execute_frames 执行完）；
/// - 失败 -> Err，state / settlement 一个字节都不改；
/// - 下落攻击标记（S8.10）：读 `state.me().plunging()` 判生效形态；战斗行动结束时按
///   `battle_action_end_me` 清除并写回根状态。需要显式传标记时用
///   [use_skill_me_with_plunging]。
pub fn use_skill_me(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
) -> Result<SkillOutcome, SkillUseError> {
    // 标记的持有区在根状态（PlayerDefs::plunging）：旧入口读使用方当前标记。
    use_skill_me_with_plunging(state, env, request, settlement, state.me().plunging())
}

/// **使用技能（我方；带下落攻击标记）**（S8.10）。
///
/// 与 [use_skill_me] 相同，只是**显式**传入 [PlungingMark]（参数应与根状态里的标记一致）：
/// 标记待消耗且本次是普通攻击 -> 生效形态为下落攻击（判定在
/// `skill_flow::use_skill_with_plunging` 内做，支付前取样骰总数）。战斗行动结束时本层仍按
/// [crate::state::flows::plunging::battle_action_end_me] 清除使用方标记并写回根状态；
/// [SkillOutcome::plunging] 保留为观测值。
pub fn use_skill_me_with_plunging(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
    plunging: PlungingMark,
) -> Result<SkillOutcome, SkillUseError> {
    let output = run(
        state,
        RunSide {
            user: state.me(),
            target: state.opp(),
            me_is_user: true,
            plunging,
        },
        env,
        request,
        settlement,
    )?;
    Ok(output.assemble(state, true, plunging, env.defeat_cleanup))
}

/// **使用技能**（敌方使用；与 use_skill_me 共用一份实现，D35）。
pub fn use_skill_opp(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
) -> Result<SkillOutcome, SkillUseError> {
    // 标记的持有区在根状态（PlayerDefs::plunging）：旧入口读使用方当前标记。
    use_skill_opp_with_plunging(state, env, request, settlement, state.opp().plunging())
}

/// **使用技能（敌方；带下落攻击标记）**（与 [use_skill_me_with_plunging] 共用一份实现，D35）。
pub fn use_skill_opp_with_plunging(
    state: &DefState,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
    plunging: PlungingMark,
) -> Result<SkillOutcome, SkillUseError> {
    let output = run(
        state,
        RunSide {
            user: state.opp(),
            target: state.me(),
            me_is_user: false,
            plunging,
        },
        env,
        request,
        settlement,
    )?;
    Ok(output.assemble(state, false, plunging, env.defeat_cleanup))
}

/// 一步用的执行上下文（从技能流程的依赖建；视角方由 execute_step_me / execute_step_opp 绑定）。
fn keyword_effect_env<'a>(env: &SkillFlowEnv<'a>, subject: EntityId) -> EffectEnv<'a> {
    EffectEnv::new(
        subject,
        crate::keywords::target::Perspective::Me,
        DamageEnv {
            allow: env.allow,
            rules: env.rules,
            registry: env.reaction_registry,
        },
    )
    .with_alloc(env.alloc)
    .with_triggers(env.keyword_triggers)
    .with_mastery(env.mastery)
    .with_increases(env.increases)
    .with_effect_context(env.effect_context)
}

/// **两侧派发**「使用技能时 / 后」（规则书·其他规则 结算顺序 S7.5）。
///
/// 语义（逐条对应 S7.5）：
/// 1. **当前牌手优先**：先排出使用方（current）命中的全部同时机效果，再排对方（other）的；
///    实现上对三个作用域时机分别调 timeline::resolve（它本身按「current 表 -> other 表」拼），
///    再把 current 的部分归到前段、other 的部分归到后段 —— 否则跨三个时机的拼接会把
///    对方的效果插到当前牌手前面；
/// 2. **同一牌手内按来源类别**（角色牌 -> 装备和状态 -> 出战状态 -> 召唤物 -> 支援牌）；
/// 3. **同类别按进场先后**；
/// 4. 完全同序 -> 保持稳定顺序（= 宽到窄的作用域趟序，沿用 skill_flow 的约定，待复核）。
///
/// 只派发、不执行（D186）。
fn dispatch_skill_two_sided(
    current: &SkillTriggers,
    other: &SkillTriggers,
    phase: SkillPhase,
    skill: &SkillDef,
    event: &Event,
) -> Vec<SkillPending> {
    let mut current_side: Vec<SkillPending> = Vec::new();
    let mut other_side: Vec<SkillPending> = Vec::new();
    for timing in skill_timings(phase, skill) {
        // 当前牌手在这一个时机命中的条数：resolve 的结果是「current 表在前、other 表在后」，
        // 用同一张 current 表的 dispatch 条数切分（dispatch 不执行、纯只读）。
        let current_count = dispatch(current, timing, event).len();
        let resolved: Vec<SkillPending> = resolve(current, other, timing, event)
            .into_iter()
            .map(Pending::into_queued)
            .collect();
        for (index, item) in resolved.into_iter().enumerate() {
            if index < current_count {
                current_side.push(item);
            } else {
                other_side.push(item);
            }
        }
    }
    current_side.sort_by(by_resolution_order);
    other_side.sort_by(by_resolution_order);
    current_side.append(&mut other_side);
    current_side
}

/// 同一牌手内的结算先后：先来源类别（S7.5 第 2 条），再进场先后（第 3 条）。
///
/// 来源类别顺序复用 rules::ops::order::resolves_before（唯一一份顺序表）。
fn by_resolution_order(left: &SkillPending, right: &SkillPending) -> Ordering {
    if left.source_kind == right.source_kind {
        return left.entry.cmp(&right.entry);
    }
    if resolves_before(left.source_kind, right.source_kind) {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

/// [run] 的「使用方 / 对方 / 敌我绑定 / 下落标记」上下文（打包避免参数过多）。
///
/// 语义与顺序契约不变：user = 使用方、target = 对方；me_is_user = 我方是否使用方（只用于把
/// 中间局面组装成 DefState / 绑定一步的视角方）；plunging = 调用方持有的下落攻击标记（S8.10）。
struct RunSide<'a> {
    user: &'a PlayerDefs,
    target: &'a PlayerDefs,
    me_is_user: bool,
    plunging: PlungingMark,
}

/// 共用实现：state = 当前一局；side = 使用方 / 对方 / 敌我绑定 / 下落标记。
fn run(
    state: &DefState,
    side: RunSide<'_>,
    env: &SkillFlowEnv<'_>,
    request: &SkillUseRequest<'_>,
    settlement: &mut SkillFrames,
) -> Result<RunOutput, SkillUseError> {
    let RunSide {
        user,
        target,
        me_is_user,
        plunging,
    } = side;
    // 1) 使用方出战角色 + 对方出战角色。
    let who = user
        .characters()
        .active_entity()
        .ok_or(SkillUseError::NoActiveCharacter)?;
    let user_state = user
        .characters()
        .slot_of(who)
        .map(|slot| slot.state.clone())
        .ok_or(SkillUseError::NoActiveCharacter)?;
    let target_entity = target.characters().active_entity();
    if request.effect.plan_is_damage() && target_entity.is_none() {
        return Err(SkillUseError::NoTargetCharacter);
    }

    // 2) can_use -> 派发 before -> 支付 -> 充能 -> 派发 after -> 记次数
    //    （全部在 skill_flow::use_skill 里；失败 -> Err，不改任何状态）。
    let context = SkillUseContext {
        skill: request.skill,
        character: &user_state,
        dice: user.dice(),
        uses: user.skill_uses(),
        prepared: request.prepared,
        policy: env.policy,
        energy_gain_bonus: env.energy_gain_bonus,
    };
    // 当前牌手 = 使用方；两张注册表按 S7.5 第 1 条取「当前 -> 对方」。
    let current_triggers = env.triggers.current(me_is_user);
    let other_triggers = env.triggers.other(me_is_user);
    let event = skill_event(who, target_entity);
    // 支付 / 充能 / 使用次数沿用 skill_flow::use_skill；它内部的单侧 before/after 只派发不执行，
    // 这里丢弃其结果，按**两侧**注册表重新派发（S7.5 第 1 条）。
    let used = use_skill_with_plunging(
        current_triggers,
        &context,
        who,
        target_entity,
        env.same_color,
        plunging,
    )?;
    let before = dispatch_skill_two_sided(
        current_triggers,
        other_triggers,
        SkillPhase::Before,
        request.skill,
        &event,
    );
    let after = dispatch_skill_two_sided(
        current_triggers,
        other_triggers,
        SkillPhase::After,
        request.skill,
        &event,
    );

    // 3) 支付结果写回使用方（骰池 / 出战角色状态 / 使用计数）。
    let mut used_character = used.character.clone();
    // 4) 「使用技能时」的待执行：本层是调用方，直接执行（D186：执行不属于派发）。
    execute_character(&before, &mut used_character);
    let user_after = user
        .clone()
        .with_dice(used.dice.clone())
        .with_characters(
            user.characters()
                .with_state(who, used_character)
                .unwrap_or_else(|| user.characters().clone()),
        )
        .with_skill_uses(used.uses.clone());

    // 5) 效果体：两条路径（D227：默认走关键字一步；旧效果体计划保留为兼容）。
    let mut damage = None;
    let mut reaction = None;
    let mut alloc = env.alloc;
    let mut events: Vec<FlowRecord> = Vec::new();

    let branches: Vec<RawBranch> = match request.effect {
        SkillEffectBody::Plan(plan) => {
            // ---- 旧路径（D184；行为不变）：单段伤害 -> 伤修链 -> 反应效果执行器 ----
            let mut target_after = target.clone();
            let mut hp_pending: Vec<CharacterPending> = Vec::new();
            if let SkillEffectPlan::Damage { amount, ty } = plan {
                let target_entity = target_entity.ok_or(SkillUseError::NoTargetCharacter)?;
                let target_slot = target
                    .characters()
                    .slot_of(target_entity)
                    .ok_or(SkillUseError::NoTargetCharacter)?;
                let target_character = target_slot.state.clone();
                let damage_env = DamageEnv {
                    allow: env.allow,
                    rules: env.rules,
                    registry: env.reaction_registry,
                };
                let chain_env =
                    DamageChainEnv::new(target.combat_status(), &target_character.states);
                let bonus = DamageBonus {
                    increases: env.increases,
                    mastery: env.mastery,
                    reaction_bonus: Nat::ZERO,
                    reacted: false,
                };
                let chained = damage_with_chain(
                    &target_character,
                    who,
                    target_entity,
                    amount,
                    ty,
                    &ChainDamageEnv {
                        damage: &damage_env,
                        chain: &chain_env,
                        bonus,
                        context: env.effect_context,
                        triggers: env.triggers.other(me_is_user),
                    },
                );
                reaction = chained.reaction;
                damage = Some(SkillDamageSummary {
                    to_hp: chained.chain.to_hp,
                    shield_used: chained.chain.shield_used,
                    bonus_total: chained.chain.bonus_total,
                    reaction: chained.reaction,
                    attachment_landed: chained.outcome.attachment_landed,
                });
                hp_pending = chained.pending;
                if let Some(zone) = target
                    .characters()
                    .with_state(target_entity, chained.outcome.state)
                {
                    target_after = target.clone().with_characters(zone);
                }
            }

            // 旧路径的反应效果执行器（逐支展开，D190）。
            match reaction {
                None => vec![raw_branch(
                    &user_after,
                    &target_after,
                    Fraction::ONE,
                    SidePending::default(),
                    SidePending::default(),
                    target_entity,
                    &hp_pending,
                )],
                Some(hit) => {
                    let zones = ReactionZones {
                        source: side_zones(&user_after),
                        target: side_zones(&target_after),
                    };
                    let execution_env = ExecutionEnv::new(
                        DamageEnv {
                            allow: env.allow,
                            rules: env.rules,
                            registry: env.reaction_registry,
                        },
                        env.mastery,
                    );
                    match execute_resolved(
                        &hit.event(),
                        env.effect_context,
                        &zones,
                        env.reaction_triggers,
                        &execution_env,
                        env.alloc,
                    ) {
                        None => vec![raw_branch(
                            &user_after,
                            &target_after,
                            Fraction::ONE,
                            SidePending::default(),
                            SidePending::default(),
                            target_entity,
                            &hp_pending,
                        )],
                        Some(execution) => execution
                            .branches
                            .into_iter()
                            .map(|branch| {
                                let source = player_from_zones(&user_after, &branch.zones.source);
                                let target = player_from_zones(&target_after, &branch.zones.target);
                                raw_branch(
                                    &source,
                                    &target,
                                    branch.probability,
                                    branch.pending.source,
                                    branch.pending.target,
                                    target_entity,
                                    &hp_pending,
                                )
                            })
                            .collect(),
                    }
                }
            }
        }
        SkillEffectBody::Step(step) => {
            // ---- 新路径（D227）：关键字层一步；步内不插入，步末 flush ----
            let base_state = if me_is_user {
                state
                    .clone()
                    .with_me(user_after.clone())
                    .with_opp(target.clone())
            } else {
                state
                    .clone()
                    .with_me(target.clone())
                    .with_opp(user_after.clone())
            };
            let effect_env = keyword_effect_env(env, who);
            let stepped = if me_is_user {
                execute_step_me(&base_state, step, &effect_env)
            } else {
                execute_step_opp(&base_state, step, &effect_env)
            };
            alloc = stepped.alloc;
            events.push(FlowRecord::Step {
                index: Nat::ZERO,
                ops: step.len(),
                branches: Nat::new(stepped.branches.len() as _),
            });
            let mut pending_total = Nat::ZERO;
            let mut raw: Vec<RawBranch> = Vec::new();
            for branch in stepped.branches {
                pending_total = nadd!(pending_total, branch.pending.len());
                // 步末 flush（D204：插入只能发生在步与步之间；本层是 execute_step 的调用方）。
                let flushed = execute_pending(&branch.state, &branch.pending);
                let (user_player, target_player) = if me_is_user {
                    (flushed.me().clone(), flushed.opp().clone())
                } else {
                    (flushed.opp().clone(), flushed.me().clone())
                };
                raw.push(RawBranch {
                    user: user_player,
                    target: target_player,
                    probability: branch.probability,
                    source_pending: SidePending::default(),
                    target_pending: SidePending::default(),
                });
            }
            events.push(FlowRecord::Flush {
                after_step: Nat::ZERO,
                pending: pending_total,
            });
            raw
        }
    };

    // 6) 「使用技能后」：插入调用方当前结算位置（D196：只插在步边界；不是 LIFO 触发栈）。
    if !after.is_empty() {
        events.push(FlowRecord::Insert {
            after_step: Nat::ZERO,
            pending: Nat::new(after.len() as _),
            at: "使用技能后",
        });
    }
    settlement.insert(after.clone());

    // 7) 汇总记录（分支数在结算后才能知道，所以放最后）。
    events.push(FlowRecord::UseSkill {
        skill: request.skill.id,
        action: if me_is_user {
            "使用技能（我方）"
        } else {
            "使用技能（敌方）"
        },
        effect: request.effect.description(),
        branches: Nat::new(branches.len() as _),
    });

    Ok(RunOutput {
        branches,
        damage,
        reaction,
        before,
        alloc,
        effective_kind: used.effective_kind,
        events,
    })
}

/// 组一支原始分支：把「生命值变动」待执行作用在被打的角色上（反应效果之后）。
fn raw_branch(
    user: &PlayerDefs,
    target: &PlayerDefs,
    probability: Fraction,
    source_pending: SidePending,
    target_pending: SidePending,
    target_entity: Option<EntityId>,
    hp_pending: &[CharacterPending],
) -> RawBranch {
    let mut target = target.clone();
    if let Some(target_entity) = target_entity {
        apply_character_pending(&mut target, target_entity, hp_pending);
    }
    RawBranch {
        user: user.clone(),
        target,
        probability,
        source_pending,
        target_pending,
    }
}

/// 把一条角色族待执行作用在某个角色的状态上（纯函数式写回）。
fn apply_character_pending(
    player: &mut PlayerDefs,
    character: EntityId,
    pending: &[CharacterPending],
) {
    if pending.is_empty() {
        return;
    }
    let Some(slot) = player.characters().slot_of(character) else {
        return;
    };
    let mut state = slot.state.clone();
    execute_character(pending, &mut state);
    if let Some(zone) = player.characters().with_state(character, state) {
        let next = player.clone().with_characters(zone);
        *player = next;
    }
}

/// 一方的区快照（反应执行器需要；按值收进 / 交出，D182）。
fn side_zones(player: &PlayerDefs) -> SideZones {
    SideZones {
        characters: player.characters().clone(),
        combat_status: player.combat_status().clone(),
        summons: player.summons().clone(),
        hand: player.hand().clone(),
        hand_states: player.hand_states().clone(),
    }
}

/// 把反应执行器交出的区快照写回一方（其余区不动）。
fn player_from_zones(base: &PlayerDefs, zones: &SideZones) -> PlayerDefs {
    base.clone()
        .with_characters(zones.characters.clone())
        .with_combat_status(zones.combat_status.clone())
        .with_summons(zones.summons.clone())
        .with_hand(zones.hand.clone())
        .with_hand_states(zones.hand_states.clone())
}

/// 把执行器的 SidePending 摊平成统一待执行（顺序 = 结算顺序.md §一 第 2 条的来源类别：
/// 角色牌 -> 出战状态 -> 召唤物 -> 手牌状态；支援区没有 pending 类型）。
fn flow_pending_from_side(side: SidePending) -> Vec<FlowPending> {
    let mut pending: Vec<FlowPending> = Vec::new();
    for item in side.characters {
        pending.push(FlowPending::CharacterZone(item));
    }
    for item in side.combat_status {
        pending.push(FlowPending::CombatStatus(item));
    }
    for item in side.summons {
        pending.push(FlowPending::Summon(item));
    }
    for item in side.hand_states {
        pending.push(FlowPending::HandCardState(item));
    }
    pending
}

/// 折叠回（D191）：只在「待执行为空」的分支上做（Queued 没有 PartialEq）。
fn fold_skill_branches(branches: Vec<SkillBranch>) -> Vec<SkillBranch> {
    let mut folded: Vec<SkillBranch> = Vec::new();
    for branch in branches {
        if branch.pending.is_empty() {
            if let Some(existing) = folded
                .iter_mut()
                .find(|existing| existing.pending.is_empty() && existing.state == branch.state)
            {
                existing.probability = existing.probability.exact_add(branch.probability);
                continue;
            }
        }
        folded.push(branch);
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{Element, SkillId};
    use crate::families::character::attachment::Attachment;
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage_chain::statuses::SHIELD_COMBAT;
    use crate::families::character::skill::{SkillCost, SkillDef};
    use crate::families::character::skill_flow::{
        CannotUse, PreparedSkills, SkillFrames, SkillTriggers, AFTER_ANY, BEFORE_ANY,
    };
    use crate::families::character::trigger::CharacterTiming;
    use crate::families::character::types::SkillKind;
    use crate::families::character::zones::combat_status::{
        CombatStatusTriggers, CombatStatusZone,
    };
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::families::character::zones::field::{
        CharacterSlot as SkillCharacterSlot, CharacterZone as SkillCharacterZone,
    };
    use crate::flow::game_end::Outcome;
    use crate::flow::pending::execute_frames;
    use crate::flow::pending::PendingSet;
    use crate::generated::Localized;
    use crate::keywords::effect::TriggerBook;
    use crate::keywords::op::{DamageArgs, DamageSourceKind, KeywordOp};
    use crate::keywords::step::Step;
    use crate::keywords::target::Perspective;
    use crate::keywords::target::TargetSelector;
    use crate::resources::dice::{CostComponent, DicePool, DieFace};
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::n;
    use crate::timeline::{Event, Queued, Resolution, Trigger};

    fn noop() {}

    static THREE_HYDRO: [CostComponent; 1] =
        [CostComponent::SpecificElement(Element::Hydro, Nat::new(3))];

    /// 元素战技：3 水，造成效果体伤害（效果体由 SkillEffectPlan 给）。
    static SKILL: SkillDef = SkillDef {
        id: SkillId::new(12162),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::ElementalSkill,
        cost: SkillCost::from_dice(&THREE_HYDRO),
        effect: noop,
    };

    /// 无消耗技能（只验证流程 / 时机）。
    static FREE_SKILL: SkillDef = SkillDef {
        id: SkillId::new(12161),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::normal_attack(),
        cost: SkillCost::FREE,
        effect: noop,
    };

    /// 被动技能（不可主动使用）。
    static PASSIVE_SKILL: SkillDef = SkillDef {
        id: SkillId::new(12164),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: SkillKind::Passive,
        cost: SkillCost::FREE,
        effect: noop,
    };

    /// 流程依赖的「静态数据」（测试里自己兜住，不用真实反应表以外的状态）。
    struct EnvBundle {
        triggers_me: SkillTriggers,
        triggers_opp: SkillTriggers,
        /// 关键字层：我方角色族触发（一步路径用）。
        keyword_me_triggers: SkillTriggers,
        /// 关键字层：敌方角色族触发（一步路径用）。
        keyword_opp_triggers: SkillTriggers,
        reaction_triggers: ReactionTriggers,
        rules: Vec<ReactionRule>,
        registry: ReactionRegistry,
        context: EffectContext,
        increases: [Nat; 0],
        energy_bonus: [Nat; 0],
    }

    impl EnvBundle {
        fn new() -> EnvBundle {
            EnvBundle {
                triggers_me: SkillTriggers::new(),
                triggers_opp: SkillTriggers::new(),
                keyword_me_triggers: SkillTriggers::new(),
                keyword_opp_triggers: SkillTriggers::new(),
                reaction_triggers: ReactionTriggers::none(),
                rules: crate::reactions::table::base_rules(),
                registry: ReactionRegistry::new(),
                context: EffectContext::new(true, Nat::ZERO, Nat::ZERO),
                increases: [],
                energy_bonus: [],
            }
        }

        fn env(&self) -> SkillFlowEnv<'_> {
            let mut keyword_triggers = TriggerBook::empty();
            keyword_triggers.me_characters = &self.keyword_me_triggers;
            keyword_triggers.opp_characters = &self.keyword_opp_triggers;
            SkillFlowEnv {
                triggers: SkillTriggerBook {
                    me: &self.triggers_me,
                    opp: &self.triggers_opp,
                },
                keyword_triggers,
                allow: AttachmentAllowSet::STANDARD,
                rules: &self.rules,
                reaction_registry: &self.registry,
                reaction_triggers: &self.reaction_triggers,
                mastery: Nat::ZERO,
                increases: &self.increases,
                effect_context: &self.context,
                alloc: EntityAlloc::starting_at(Nat::new(1000)),
                policy: SkillUsePolicy::DEFAULT,
                energy_gain_bonus: &self.energy_bonus,
                same_color: None,
                defeat_cleanup: crate::flow::hooks::NO_DEFEAT_CLEANUP,
            }
        }
    }

    /// 造骰池（按骰面累加）。
    fn pool(dice: &[(DieFace, Nat)]) -> DicePool {
        let mut built = DicePool::empty();
        for (face, count) in dice {
            built = built.add(*face, *count);
        }
        built
    }

    /// 我方的使用角色：单个出战角色（生命 10 / 充能上限 2）。
    fn me(dice: DicePool) -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(
                CharacterZone::try_from_slots(vec![CharacterSlot::new(
                    n(201),
                    CharacterState::new(Nat::new(10), Nat::new(10), Nat::ZERO, Nat::new(2)),
                )])
                .expect("合法槽位"),
            )
            .with_dice(dice)
    }

    /// 我方「顺序记号」角色：生命 0 / 上限 MAX（记号 hp * 10 + digit 不受钳制）。
    fn me_marking(dice: DicePool) -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(
                CharacterZone::try_from_slots(vec![CharacterSlot::new(
                    n(201),
                    CharacterState::new(Nat::ZERO, Nat::MAX, Nat::ZERO, Nat::new(2)),
                )])
                .expect("合法槽位"),
            )
            .with_dice(dice)
    }

    /// 对方的承接角色（可带元素附着）。
    fn opp(hp: Nat, attachment: Attachment) -> PlayerDefs {
        let mut state = CharacterState::new(hp, Nat::new(10), Nat::ZERO, Nat::new(2));
        state.attachment = attachment;
        PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(n(401), state)])
                .expect("合法槽位"),
        )
    }

    /// 对方「顺序记号」角色：生命 0 / 上限 MAX（记号 hp * 10 + digit 不受钳制）。
    fn opp_marking() -> PlayerDefs {
        PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(
                n(401),
                CharacterState::new(Nat::ZERO, Nat::MAX, Nat::ZERO, Nat::new(2)),
            )])
            .expect("合法槽位"),
        )
    }

    fn three_hydro() -> DicePool {
        pool(&[(DieFace::Hydro, Nat::new(3))])
    }

    /// 顺序记号效果：hp = hp * 10 + digit。

    #[test]
    fn defeat_cleanup_runs_before_the_terminal_judgement() {
        // D255：结算 -> 击倒清理 -> 判终局。对方 1 HP，旧路径 2 点物理伤害后 HP=0，
        // 未清理时角色仍在角色区 -> Ongoing；注入清理后角色离场 -> MeWins。
        fn cleanup_ko(state: &DefState, side: Perspective) -> (DefState, PendingSet) {
            let player = if side == Perspective::Me {
                state.me().clone()
            } else {
                state.opp().clone()
            };
            let kept: Vec<SkillCharacterSlot> = player
                .characters()
                .slots()
                .iter()
                .filter(|slot| slot.state.hp() != Nat::ZERO)
                .cloned()
                .collect();
            let zone = SkillCharacterZone::try_from_slots(kept).unwrap_or_default();
            let player = player.with_characters(zone);
            let next = if side == Perspective::Me {
                state.clone().with_me(player)
            } else {
                state.clone().with_opp(player)
            };
            (next, PendingSet::new())
        }
        let before =
            DefState::from_players(me(three_hydro()), opp(Nat::new(1), Attachment::none()))
                .with_round(n(1));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::Damage {
                amount: Nat::new(2),
                ty: DamageType::Physical,
            }),
        };
        // 未注入清理：HP=0 但角色仍在角色区 -> 判 Ongoing。
        let mut settlement = empty_frames();
        let plain = use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("够付");
        assert_eq!(plain.single().expect("一支").outcome, Outcome::Ongoing);
        assert_eq!(
            plain
                .single()
                .expect("一支")
                .state
                .opp()
                .characters()
                .active()
                .expect("还在角色区")
                .state
                .hp(),
            Nat::ZERO
        );
        // 注入假清理：清理后角色离场 -> 判 MeWins。
        let mut env = bundle.env();
        env.defeat_cleanup = cleanup_ko;
        let mut settlement = empty_frames();
        let cleaned = use_skill_me(&before, &env, &request, &mut settlement).expect("够付");
        let branch = cleaned.single().expect("一支");
        assert!(branch.state.opp().characters().is_empty());
        assert_eq!(branch.outcome, Outcome::MeWins);
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

    fn queued(
        effect: fn(&mut CharacterState, &Event, EntityId),
    ) -> Queued<fn(&mut CharacterState, &Event, EntityId)> {
        Queued {
            effect,
            event: Event::of(n(201)),
            owner: n(201),
            source_kind: EffectSource::CharacterCard,
            entry: Nat::ZERO,
        }
    }

    fn empty_frames() -> SkillFrames {
        Resolution::new(Vec::new())
    }

    #[test]
    fn skill_end_to_end_pays_damages_and_runs_reaction() {
        let before = DefState::from_players(
            me(three_hydro()),
            opp(Nat::new(10), Attachment::with(Element::Cryo)),
        );
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::Damage {
                amount: Nat::new(2),
                ty: DamageType::elemental(Element::Pyro),
            }),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("够付");
        assert_eq!(outcome.branches.len(), 1);
        let branch = outcome.single().expect("一支");
        assert!(branch.probability.is_one());
        assert_eq!(
            outcome.reaction.map(|hit| hit.reaction.name()),
            Some("融化")
        );
        let damage = outcome.damage.expect("有伤害");
        assert_eq!(damage.bonus_total, Nat::new(2));
        assert_eq!(damage.to_hp, Nat::new(4));
        let after = &branch.state;
        assert_eq!(
            after.opp().characters().active().expect("有").state.hp(),
            Nat::new(6)
        );
        assert_eq!(after.me().dice().total(), Nat::ZERO);
        assert_eq!(
            after.me().characters().active().expect("有").state.energy(),
            Nat::new(1)
        );
        assert_eq!(after.me().skill_uses().used_count(SKILL.id), Nat::new(1));
        assert!(after
            .opp()
            .characters()
            .active()
            .expect("有")
            .state
            .attachment
            .is_empty());
        assert_eq!(before.me().dice().total(), Nat::new(3));
        assert_eq!(
            before.opp().characters().active().expect("有").state.hp(),
            Nat::new(10)
        );
    }

    #[test]
    fn skill_fails_without_enough_dice_and_changes_nothing() {
        let before = DefState::from_players(
            me(pool(&[(DieFace::Hydro, Nat::new(2))])),
            opp(Nat::new(10), Attachment::none()),
        );
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let error =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect_err("不够付");
        assert_eq!(error, SkillUseError::CannotUse(CannotUse::NotEnoughDice));
        assert_eq!(before.me().dice().total(), Nat::new(2));
        assert!(settlement.is_finished());
    }

    #[test]
    fn skill_pays_energy_for_burst_style_effects() {
        static THREE_HYDRO_BURST: [CostComponent; 1] =
            [CostComponent::SpecificElement(Element::Hydro, Nat::new(3))];
        static BURST: SkillDef = SkillDef {
            id: SkillId::new(12163),
            name: Localized::EMPTY,
            description: Localized::EMPTY,
            kind: SkillKind::ElementalBurst,
            cost: SkillCost::from_dice_and_energy(&THREE_HYDRO_BURST, Nat::new(2)),
            effect: noop,
        };
        let before = DefState::from_players(
            {
                let zone = CharacterZone::try_from_slots(vec![CharacterSlot::new(
                    n(201),
                    CharacterState::new(Nat::new(10), Nat::new(10), Nat::new(2), Nat::new(2)),
                )])
                .expect("合法槽位");
                PlayerDefs::new()
                    .with_characters(zone)
                    .with_dice(three_hydro())
            },
            opp(Nat::new(10), Attachment::none()),
        );
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &BURST,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("够付");
        let after = &outcome.single().expect("一支").state;
        assert_eq!(after.me().dice().total(), Nat::ZERO);
        assert_eq!(
            after.me().characters().active().expect("有").state.energy(),
            Nat::ZERO
        );
    }

    #[test]
    fn skill_damage_goes_through_shield_and_reduction() {
        let shield_zone = CombatStatusZone::new()
            .attach_with_triggers(
                n(401),
                &SHIELD_COMBAT,
                Some(Nat::new(1)),
                None,
                &CombatStatusTriggers::new(),
            )
            .zone;
        let before = DefState::from_players(
            me(three_hydro()),
            opp(Nat::new(10), Attachment::none()).with_combat_status(shield_zone),
        );
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::Damage {
                amount: Nat::new(2),
                ty: DamageType::Physical,
            }),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("够付");
        let damage = outcome.damage.expect("有伤害");
        assert_eq!(damage.shield_used, Nat::new(1));
        assert_eq!(damage.to_hp, Nat::new(1));
        assert!(damage.reaction.is_none());
        assert_eq!(
            outcome
                .single()
                .expect("一支")
                .state
                .opp()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(9)
        );
    }

    #[test]
    fn after_skill_pending_is_inserted_at_the_current_settlement_position() {
        let before = DefState::from_players(
            me_marking(DicePool::empty()),
            opp(Nat::new(10), Attachment::none()),
        );
        let mut bundle = EnvBundle::new();
        bundle.triggers_me.register(Trigger::new(
            AFTER_ANY,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        bundle.triggers_me.register(Trigger::new(
            AFTER_ANY,
            n(201),
            EffectSource::CharacterCard,
            Nat::new(1),
            mark_two,
        ));
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = Resolution::new(vec![queued(mark_three)]);
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        assert!(outcome.before.is_empty());
        assert_eq!(settlement.depth(), Nat::new(2));
        let after_state = execute_frames(&before, n(201), &mut settlement).expect("角色在区里");
        assert_eq!(
            after_state
                .me()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(123)
        );
        assert!(settlement.is_finished());
    }

    #[test]
    fn before_skill_pending_is_dispatched_and_executed_before_the_effect_body() {
        let before = DefState::from_players(
            me_marking(DicePool::empty()),
            opp(Nat::new(10), Attachment::none()),
        );
        let mut bundle = EnvBundle::new();
        bundle.triggers_me.register(Trigger::new(
            BEFORE_ANY,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        assert_eq!(outcome.before.len(), 1);
        assert_eq!(
            outcome
                .single()
                .expect("一支")
                .state
                .me()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(1)
        );
    }

    #[test]
    fn passive_skill_cannot_be_used_and_changes_nothing() {
        let before =
            DefState::from_players(me(DicePool::empty()), opp(Nat::new(10), Attachment::none()));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &PASSIVE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let error = use_skill_me(&before, &bundle.env(), &request, &mut settlement)
            .expect_err("被动不可主动使用");
        assert_eq!(error, SkillUseError::CannotUse(CannotUse::PassiveSkill));
        assert_eq!(
            before,
            DefState::from_players(me(DicePool::empty()), opp(Nat::new(10), Attachment::none()))
        );
    }

    #[test]
    fn damage_plan_without_enemy_active_character_is_rejected() {
        let before = DefState::from_players(me(three_hydro()), PlayerDefs::new());
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::Damage {
                amount: Nat::new(1),
                ty: DamageType::Physical,
            }),
        };
        let mut settlement = empty_frames();
        let error =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect_err("没有目标");
        assert_eq!(error, SkillUseError::NoTargetCharacter);
        assert_eq!(before.me().dice().total(), Nat::new(3));
    }

    #[test]
    fn opp_entry_uses_the_same_pipeline_and_binds_sides_correctly() {
        let before =
            DefState::from_players(opp(Nat::new(10), Attachment::none()), me(three_hydro()));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::Damage {
                amount: Nat::new(1),
                ty: DamageType::Physical,
            }),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_opp(&before, &bundle.env(), &request, &mut settlement).expect("够付");
        let after = &outcome.single().expect("一支").state;
        assert_eq!(after.opp().dice().total(), Nat::ZERO);
        assert_eq!(
            after.me().characters().active().expect("有").state.hp(),
            Nat::new(9)
        );
    }

    #[test]
    fn step_effect_body_matches_the_plan_path_on_the_same_damage() {
        let before = DefState::from_players(
            me(three_hydro()),
            opp(Nat::new(10), Attachment::with(Element::Cryo)),
        );
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let plan_request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::Damage {
                amount: Nat::new(2),
                ty: DamageType::elemental(Element::Pyro),
            }),
        };
        let mut plan_settlement = empty_frames();
        let plan = use_skill_me(&before, &bundle.env(), &plan_request, &mut plan_settlement)
            .expect("够付");
        let step = plan_request.effect.step();
        let step_request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Step(&step),
        };
        let mut step_settlement = empty_frames();
        let stepped = use_skill_me(&before, &bundle.env(), &step_request, &mut step_settlement)
            .expect("够付");
        let plan_state = &plan.single().expect("一支").state;
        let step_state = &stepped.single().expect("一支").state;
        assert_eq!(step_state.me(), plan_state.me());
        assert_eq!(step_state.opp(), plan_state.opp());
        assert!(plan.damage.is_some());
        assert!(stepped.damage.is_none());
        assert_eq!(plan.reaction.map(|hit| hit.reaction.name()), Some("融化"));
        assert!(stepped.reaction.is_none());
        assert!(stepped
            .events
            .iter()
            .any(|event| matches!(event, FlowRecord::Step { .. })));
        assert!(stepped
            .events
            .iter()
            .any(|event| matches!(event, FlowRecord::Flush { .. })));
        assert!(!plan
            .events
            .iter()
            .any(|event| matches!(event, FlowRecord::Step { .. })));
    }

    #[test]
    fn step_pending_is_flushed_at_the_step_boundary_before_the_after_insertion() {
        fn heal_five(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
            *state = state.hp_add(Nat::new(5));
        }
        let mut bundle = EnvBundle::new();
        bundle.keyword_opp_triggers.register(Trigger::new(
            CharacterTiming::HpChanged,
            n(401),
            EffectSource::CharacterCard,
            Nat::ZERO,
            heal_five,
        ));
        bundle.triggers_me.register(Trigger::new(
            AFTER_ANY,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        let before = DefState::from_players(
            me_marking(DicePool::empty()),
            opp(Nat::new(10), Attachment::none()),
        );
        let prepared = PreparedSkills::new();
        let step = Step::new(vec![KeywordOp::Damage(DamageArgs::new(
            TargetSelector::opponent_active(),
            Nat::new(5),
            DamageType::Physical,
            DamageSourceKind::Other,
        ))]);
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Step(&step),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        let branch = outcome.single().expect("一支");
        assert_eq!(
            branch
                .state
                .opp()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(10)
        );
        let after = execute_frames(&branch.state, n(201), &mut settlement).expect("角色在区里");
        assert_eq!(
            after.me().characters().active().expect("有").state.hp(),
            Nat::new(1)
        );
        let kinds: Vec<&'static str> = outcome
            .events
            .iter()
            .map(|event| match event {
                FlowRecord::Step { .. } => "一步",
                FlowRecord::Flush { .. } => "flush",
                FlowRecord::Insert { .. } => "插入",
                FlowRecord::UseSkill { .. } => "使用技能",
                _ => "其它",
            })
            .collect();
        assert_eq!(kinds, vec!["一步", "flush", "插入", "使用技能"]);
    }

    #[test]
    fn step_path_handles_an_empty_target_without_error() {
        let before = DefState::from_players(me_marking(DicePool::empty()), PlayerDefs::new());
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let step = Step::new(vec![KeywordOp::Damage(DamageArgs::new(
            TargetSelector::opponent_active(),
            Nat::new(3),
            DamageType::Physical,
            DamageSourceKind::Other,
        ))]);
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Step(&step),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        assert_eq!(outcome.branches.len(), 1);
        assert_eq!(outcome.single().expect("一支").state.opp(), before.opp());
    }

    #[test]
    fn after_skill_triggers_follow_current_player_first_when_me_uses() {
        let before = DefState::from_players(me_marking(DicePool::empty()), opp_marking());
        let mut bundle = EnvBundle::new();
        bundle.triggers_me.register(Trigger::new(
            AFTER_ANY,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        bundle.triggers_opp.register(Trigger::new(
            AFTER_ANY,
            n(401),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_two,
        ));
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        let branch = outcome.single().expect("一支");
        let after = execute_frames(&branch.state, n(201), &mut settlement).expect("角色在区里");
        assert_eq!(
            after.me().characters().active().expect("有").state.hp(),
            Nat::new(12)
        );
    }

    #[test]
    fn after_skill_triggers_follow_current_player_first_when_opp_uses() {
        let before = DefState::from_players(me_marking(DicePool::empty()), opp_marking());
        let mut bundle = EnvBundle::new();
        bundle.triggers_me.register(Trigger::new(
            AFTER_ANY,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        bundle.triggers_opp.register(Trigger::new(
            AFTER_ANY,
            n(401),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_two,
        ));
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_opp(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        let branch = outcome.single().expect("一支");
        let after = execute_frames(&branch.state, n(201), &mut settlement).expect("角色在区里");
        assert_eq!(
            after.me().characters().active().expect("有").state.hp(),
            Nat::new(21)
        );
    }

    #[test]
    fn before_skill_triggers_follow_current_player_first() {
        let before = DefState::from_players(me_marking(DicePool::empty()), opp_marking());
        let mut bundle = EnvBundle::new();
        bundle.triggers_me.register(Trigger::new(
            BEFORE_ANY,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_one,
        ));
        bundle.triggers_opp.register(Trigger::new(
            BEFORE_ANY,
            n(401),
            EffectSource::CharacterCard,
            Nat::ZERO,
            mark_two,
        ));
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        assert_eq!(
            outcome
                .single()
                .expect("一支")
                .state
                .me()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(12)
        );
        let mut settlement = empty_frames();
        let outcome =
            use_skill_opp(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        assert_eq!(
            outcome
                .single()
                .expect("一支")
                .state
                .opp()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(21)
        );
    }

    // ------------------------------------------------------------------
    // 下落攻击（S8.10）/ L6 读根状态 + 清除写回 / 分支终局
    // （2026-10-19 defs-integration 整文件写事故后由 defs-flow 补回）
    // ------------------------------------------------------------------

    #[test]
    fn plunging_mark_turns_the_next_normal_attack_into_plunging_and_is_cleared() {
        let before =
            DefState::from_players(me(DicePool::empty()), opp(Nat::new(10), Attachment::none()));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome = use_skill_me_with_plunging(
            &before,
            &bundle.env(),
            &request,
            &mut settlement,
            PlungingMark::new().armed(),
        )
        .expect("无消耗");
        // 标记 armed + 本次普通攻击 -> 生效形态为下落攻击（S8.10）。
        assert_eq!(outcome.effective_kind, SkillKind::plunging_attack());
        // 返回的是「战斗行动结束后」的标记：已清除；分支状态同样已清除。
        assert_eq!(outcome.plunging, PlungingMark::NONE);
        assert!(!outcome
            .single()
            .expect("一支")
            .state
            .me()
            .plunging()
            .is_pending());
        // 纯函数：入参不变。
        assert_eq!(
            before.me().characters().active().expect("有").state.hp(),
            Nat::new(10)
        );
    }

    #[test]
    fn normal_attack_without_plunging_still_follows_the_charged_attack_rule() {
        // S8.9：行动开始前骰总数为偶数（0 颗也算偶数）-> 普通攻击视为重击；无标记时不会变下落。
        let before =
            DefState::from_players(me(DicePool::empty()), opp(Nat::new(10), Attachment::none()));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        assert_eq!(outcome.effective_kind, SkillKind::charged_attack());
        assert_eq!(outcome.plunging, PlungingMark::NONE);
    }

    #[test]
    fn plunging_mark_does_not_turn_a_non_normal_attack_and_is_cleared_after_the_action() {
        // S8.10 只说「下一个战斗行动**若为普通攻击**」；战技 / 爆发原样。
        let before =
            DefState::from_players(me(three_hydro()), opp(Nat::new(10), Attachment::none()));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome = use_skill_me_with_plunging(
            &before,
            &bundle.env(),
            &request,
            &mut settlement,
            PlungingMark::new().armed(),
        )
        .expect("够付");
        assert_eq!(outcome.effective_kind, SkillKind::ElementalSkill);
        assert_eq!(outcome.plunging, PlungingMark::NONE);
    }

    #[test]
    fn opp_plunging_mark_applies_to_the_opponent_side() {
        let before =
            DefState::from_players(me(DicePool::empty()), opp(Nat::new(10), Attachment::none()));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome = use_skill_opp_with_plunging(
            &before,
            &bundle.env(),
            &request,
            &mut settlement,
            PlungingMark::new().armed(),
        )
        .expect("无消耗");
        assert_eq!(outcome.effective_kind, SkillKind::plunging_attack());
        assert_eq!(outcome.plunging, PlungingMark::NONE);
    }

    #[test]
    fn use_skill_reads_the_plunging_mark_from_state_and_clears_it_after_the_action() {
        // 持有区在根状态：普通入口自己读使用方标记（无需显式参数）。
        let before = DefState::from_players(
            me(DicePool::empty()).with_plunging(PlungingMark::new().armed()),
            opp(Nat::new(10), Attachment::none()),
        );
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        // S8.10：标记被读到 -> 生效形态为下落攻击。
        assert_eq!(outcome.effective_kind, SkillKind::plunging_attack());
        // 战斗行动结束 -> 清除并写回根状态；观测值同样是清除后的标记。
        let branch = outcome.single().expect("一支");
        assert!(!branch.state.me().plunging().is_pending());
        assert_eq!(outcome.plunging, PlungingMark::NONE);
        // 纯函数：入参状态仍带标记。
        assert!(before.me().plunging().is_pending());
    }

    #[test]
    fn opp_skill_use_clears_only_the_opponent_mark() {
        let before = DefState::from_players(
            me(DicePool::empty()).with_plunging(PlungingMark::new().armed()),
            opp(Nat::new(10), Attachment::none()).with_plunging(PlungingMark::new().armed()),
        );
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &FREE_SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::None),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_opp(&before, &bundle.env(), &request, &mut settlement).expect("无消耗");
        assert_eq!(outcome.effective_kind, SkillKind::plunging_attack());
        let branch = outcome.single().expect("一支");
        // 只清除使用方（敌方）的标记；我方标记保留。
        assert!(!branch.state.opp().plunging().is_pending());
        assert!(branch.state.me().plunging().is_pending());
    }

    #[test]
    fn skill_branch_outcome_waits_for_defeat_cleanup() {
        // 旧路径打 10 点把对方唯一角色 HP 打到 0：击倒清理（state::flows::defeat）还没跑，
        // 角色仍在角色区 -> 分支的终局判定如实是 Ongoing（S7.3：结算完成后才击倒）。
        let before =
            DefState::from_players(me(three_hydro()), opp(Nat::new(10), Attachment::none()))
                .with_round(n(1));
        let bundle = EnvBundle::new();
        let prepared = PreparedSkills::new();
        let request = SkillUseRequest {
            skill: &SKILL,
            prepared: &prepared,
            effect: SkillEffectBody::Plan(SkillEffectPlan::Damage {
                amount: Nat::new(10),
                ty: DamageType::Physical,
            }),
        };
        let mut settlement = empty_frames();
        let outcome =
            use_skill_me(&before, &bundle.env(), &request, &mut settlement).expect("够付");
        let branch = outcome.single().expect("一支");
        assert_eq!(
            branch
                .state
                .opp()
                .characters()
                .active()
                .expect("还在区里")
                .state
                .hp(),
            Nat::ZERO
        );
        assert_eq!(branch.outcome, Outcome::Ongoing);
    }
}
