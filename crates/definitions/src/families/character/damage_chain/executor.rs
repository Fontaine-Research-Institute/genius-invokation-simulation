//! **反应效果执行器**：把 `reactions::effect::EffectOp` 接到各区的操作上。
//!
//! ## 依据（只读规则书 + `结算顺序.md`）
//!
//! - 规则书·元素反应·规则 / 元素反应·效果：反应的效果 = 「本伤害+N」+ 若干**具体操作**
//!   （护盾 / 状态 / 召唤物 / 强制切换 / 加手牌 / 手牌状态）；
//! - `结算顺序.md` §三 第 3 行：「附着 -> 判定反应 -> **反应效果** -> 继续原步骤」——所以本执行器
//!   是在 `damage()` 返回「命中反应」之后插入的那一段子结算（D196）；
//! - `结算顺序.md` §一 第 2/3 条：同一时机内按「来源类别 -> 进场先后」。一个反应的效果条目
//!   由规则书条文顺序给定（`ops` 向量顺序：超载 = 先 +2 伤害再强制切换），本层**不重排**；
//!   唯一需要排序的是「同一 `TargetScope` 展开出的多个角色」——按**角色区槽位顺序**
//!   （= 进场先后，§一 第 3 条）依次执行。
//!
//! ## 执行模型（纯函数 + 区自持状态）
//!
//! - 各区**自持状态**（D182），执行器按值收进 [`ReactionZones`]、按值交出新区
//!   （与各区「吃 `&self`、返回新状态」的纯函数风格拼接）；**不持有全局状态、不建区表**；
//! - 每个 `*_with_triggers` 派发出来的**待执行**原样收进 [`ReactionPending`]，本层**不执行**
//!   （D186：派发只排序、不执行）；
//! - 生成物（召唤物实体 / 新加手牌的实体）的实体号由 [`EntityAlloc`] 发号：**只发号**，
//!   实体表在状态层（D29 / D34）；
//! - 本体（反应）作用的「触发方 / 承载方」用相对 [`Side`] 表达，两侧具名实例由调用方绑定。
//!
//! ## 概率（D190 / D191）
//!
//! `table::resolve` 给出的每个分支（操作清单 + `Fraction` 精确概率）**逐支执行**：
//! 一支 = 从**同一份输入**出发的一份完整新区，概率原样带回；**不采样、不归一化、不折叠**。
//! 折叠回（等价分支合并）由调用方在观测点做（D191）。
//!
//! ## 待接入 / 待实测
//!
//! - **`EffectOp::Status` 的 `uses` 字段**：出战状态区 / 角色状态列表都没有「可用次数」字段
//!   （可用次数目前只在召唤物区），本层执行时忽略它（`待接入`：状态区补可用次数字段）；
//! - **`EffectOp::Summon` 的 `stack_limit` 字段**：召唤物区的同名合并是「次数 / 效果量取最大」，
//!   没有独立叠加上限，本层忽略它（`待接入`）；
//! - **强制切换**的「下一个角色」：规则书只说「强制切换到下一个角色」，没定义「下一个」；
//!   本层按**角色区槽位顺序**取下一个（末尾回绕），单一角色时无法切换 -> `待实测`；
//! - **元素精通强化反应效果**：规则书只给了一个例子（结晶护盾每点 +3 层），本层按它做
//!   「护盾类 Status 的层数 += 精通 × 3」；其它反应效果的强化方式未知 -> `待实测`；
//! - 反应效果造成的**伤害**也走伤修链（护盾 / 减伤 / 穿透规则一致），但**不再享受**本次反应的
//!   「本伤害+N」（那一份已经加在触发伤害上）—— `待复核`。

use crate::base::CardId;
use crate::families::action_card::areas::hand_state::{
    HandCardStatePending, HandCardStateTriggers, HandCardStates,
};
use crate::families::action_card::status::{StateIdentity, StateList};
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::CardInstance;
use crate::families::character::attachment::ReactionEvent;
use crate::families::character::damage::damage;
use crate::families::character::damage::DamageEnv;
use crate::families::character::damage_chain::bonus::{
    DamageBonus, DamageModifiers, MASTERY_SHIELD_PER_POINT,
};
use crate::families::character::damage_chain::chain::{
    settle, ChainOutcome, DamageChainEnv, DamageInput,
};
use crate::families::character::damage_chain::statuses;
use crate::families::character::types::DamageType;
use crate::families::character::zones::combat_status::{
    CombatStatusPending, CombatStatusTriggers, CombatStatusZone,
};
use crate::families::character::zones::field::{
    CharacterZone, CharacterZonePending, CharacterZoneTriggers,
};
use crate::families::summon::zones::{
    SummonEntry, SummonOutcome, SummonPending, SummonTriggers, SummonZone,
};
use crate::reactions::effect::{
    EffectContext, EffectOp, HandCardStatus, ReactionBranch, ReactionEffectResult, Side,
    StatusTarget, TargetScope,
};
use crate::reactions::table;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, nmul, nsub, EntityId, Nat};

/// 角色状态列表里状态条目的位置（**角色身上的状态没有位置概念**，统一用 0）。
///
/// 与手牌卡状态（`HAND_STATE_POS` = 0）同一个口径：`StateList` 的 `pos` 只在
/// 「一个列表装多个位置」时有语义；角色 / 手牌都没有位置，所以恒 0。**待复核**。
pub const CHARACTER_STATE_POS: Nat = Nat::ZERO;

// ======================================================================
// 区快照（每个区自持状态；执行器按值收进 / 交出）
// ======================================================================

/// **一方的区快照**：角色区 / 出战状态区 / 召唤物区 / 手牌 / 手牌卡状态。
#[derive(Clone, Debug, Default)]
pub struct SideZones {
    /// 角色区（角色槽位 + 出战指针 + 每个角色的运行期状态）。
    pub characters: CharacterZone,
    /// 出战状态区。
    pub combat_status: CombatStatusZone,
    /// 召唤物区。
    pub summons: SummonZone,
    /// 手牌区。
    pub hand: HandState,
    /// 手牌卡状态表。
    pub hand_states: HandCardStates,
}

impl SideZones {
    /// 空的一侧（没有任何角色 / 状态 / 召唤物 / 手牌）。
    pub const fn new() -> SideZones {
        SideZones {
            characters: CharacterZone::new(),
            combat_status: CombatStatusZone::new(),
            summons: SummonZone::new(),
            hand: HandState::new(),
            hand_states: HandCardStates::new(),
        }
    }
}

/// **双方区快照**（触发方 / 承载方；引擎不分敌我，D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct ReactionZones {
    /// 触发方（造成这次元素伤害、引发反应的一方；规则书里的「我方」）。
    pub source: SideZones,
    /// 承载方（身上有附着、反应作用在其角色身上的一方；规则书里的「敌方」）。
    pub target: SideZones,
}

impl ReactionZones {
    /// 双方都空。
    pub const fn new() -> ReactionZones {
        ReactionZones {
            source: SideZones::new(),
            target: SideZones::new(),
        }
    }

    /// 按相对侧取一侧（只读）。
    pub fn side(&self, side: Side) -> &SideZones {
        match side {
            Side::Source => &self.source,
            Side::Target => &self.target,
        }
    }

    /// 按相对侧取一侧（可写）。
    pub fn side_mut(&mut self, side: Side) -> &mut SideZones {
        match side {
            Side::Source => &mut self.source,
            Side::Target => &mut self.target,
        }
    }
}

// ======================================================================
// 触发注册表快照（每个区一张，每个牌手一个具名实例；本层只借用）
// ======================================================================

/// **一方的触发注册表**（各区各一张；执行器只把它们传给 `*_with_triggers`）。
#[derive(Clone, Debug, Default)]
pub struct SideTriggers {
    /// 角色区触发。
    pub characters: CharacterZoneTriggers,
    /// 出战状态区触发。
    pub combat_status: CombatStatusTriggers,
    /// 召唤物区触发。
    pub summons: SummonTriggers,
    /// 手牌卡状态触发。
    pub hand_states: HandCardStateTriggers,
}

impl SideTriggers {
    /// 没有任何登记。
    pub const fn new() -> SideTriggers {
        SideTriggers {
            characters: CharacterZoneTriggers::new(),
            combat_status: CombatStatusTriggers::new(),
            summons: SummonTriggers::new(),
            hand_states: HandCardStateTriggers::new(),
        }
    }
}

/// **双方的触发注册表**。
#[derive(Clone, Debug, Default)]
pub struct ReactionTriggers {
    /// 触发方。
    pub source: SideTriggers,
    /// 承载方。
    pub target: SideTriggers,
}

impl ReactionTriggers {
    /// 双方都没有任何登记（测试 / 纯执行用）。
    pub const fn none() -> ReactionTriggers {
        ReactionTriggers {
            source: SideTriggers::new(),
            target: SideTriggers::new(),
        }
    }

    /// 按相对侧取一侧。
    pub fn side(&self, side: Side) -> &SideTriggers {
        match side {
            Side::Source => &self.source,
            Side::Target => &self.target,
        }
    }
}

// ======================================================================
// 待执行（各区派发出来；本层不执行）
// ======================================================================

/// **一方的待执行**：四个区各自派发出来的效果函数指针 + 事件。
#[derive(Clone, Debug, Default)]
pub struct SidePending {
    /// 角色区待执行。
    pub characters: Vec<CharacterZonePending>,
    /// 出战状态区待执行。
    pub combat_status: Vec<CombatStatusPending>,
    /// 召唤物区待执行。
    pub summons: Vec<SummonPending>,
    /// 手牌卡状态待执行。
    pub hand_states: Vec<HandCardStatePending>,
}

/// **双方的待执行**（由调用方执行；D186：派发永不执行）。
#[derive(Clone, Debug, Default)]
pub struct ReactionPending {
    /// 触发方。
    pub source: SidePending,
    /// 承载方。
    pub target: SidePending,
}

impl ReactionPending {
    /// 按相对侧取一侧。
    pub fn side_mut(&mut self, side: Side) -> &mut SidePending {
        match side {
            Side::Source => &mut self.source,
            Side::Target => &mut self.target,
        }
    }
}

// ======================================================================
// 实体发号（只发号，不建实体表）
// ======================================================================

/// **生成物实体号分配器**（纯值；每执行一条生成操作前进一格）。
///
/// 引擎的实体表 / 号段归状态层（D29 / D34）；本层只保证「同一次执行里不撞号」，
/// 起始值由调用方给（一般 = 当前最大实体号 + 1）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntityAlloc {
    next: EntityId,
}

impl EntityAlloc {
    /// 从 `next` 开始发号。
    pub const fn starting_at(next: EntityId) -> EntityAlloc {
        EntityAlloc { next }
    }

    /// 下一个将要发出的号（只读）。
    pub fn next(&self) -> EntityId {
        self.next
    }

    /// 发一个号（内部用）。
    fn take(&mut self) -> EntityId {
        let entity = self.next;
        self.next = nadd!(self.next, Nat::new(1));
        entity
    }
}

// ======================================================================
// 执行结果（观测 / 测试用；纯数据）
// ======================================================================

/// 一个目标身上的伤害落地结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DamageEntry {
    /// 目标角色实体。
    pub target: EntityId,
    /// 本次扣到生命值的伤害（走完伤修链之后）。
    pub to_hp: Nat,
    /// 被护盾吸掉的点数。
    pub shield_used: Nat,
    /// 加伤点数（反应效果造成的伤害一般没有加伤）。
    pub bonus: Nat,
    /// 是否命中了元素反应（反应效果造成的伤害 **待复核**：是否会再引发反应）。
    pub reacted: bool,
}

/// 状态挂到了哪一处承载。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusPlace {
    /// 角色状态列表（`CharacterState::states`）。
    CharacterState,
    /// 出战状态区（`CombatStatusZone`）。
    CombatStatus,
}

/// 一条操作**无法执行**的原因（不改任何状态，不 panic）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skip {
    /// `TargetScope` 解析不到任何实体（该方没有出战角色 / 实体已被击倒）。
    NoTarget,
    /// 实体不在对应的区里。
    NotInZone,
    /// 没有「下一个角色」可强制切换（只有一个角色）。
    NoNextCharacter,
    /// 状态卡 id 没有登记状态身份（`statuses::status_of_card` 返回 None）。
    UnknownStatusCard,
    /// 手牌下标越界（分支展开后手牌被改过）。
    HandIndexOutOfRange,
    /// 手牌已满 / 实体重复，加牌失败。
    HandFull,
    /// 召唤物区已满且没有同名可合并。
    SummonZoneFull,
    /// 叠加上限已到（结晶护盾「最多 2 点」这类）。
    StackCapped,
}

/// 一条操作的执行结果。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum OpResult {
    /// 造成伤害（每个目标一条；顺序 = 解析出的目标顺序）。
    Damage(Vec<DamageEntry>),
    /// 附属 / 叠加状态。
    Status {
        /// 挂到哪一处承载。
        place: StatusPlace,
        /// 承载实体（角色状态 = 角色实体；出战状态 = 当前出战角色实体）。
        target: EntityId,
        /// 实际加上的层数（经叠加上限 / 元素精通强化之后）。
        stacks: Nat,
    },
    /// 生成 / 合并召唤物。
    Summon {
        /// 生成在哪一方。
        side: Side,
        /// 实际保留 / 新生成的召唤物实体。
        entity: EntityId,
        /// 生成结果（新增 / 合并）。
        outcome: SummonOutcome,
    },
    /// 强制切换出战角色。
    Switch {
        /// 哪一方被切换。
        side: Side,
        /// 原出战角色。
        from: EntityId,
        /// 新出战角色。
        to: EntityId,
    },
    /// 生成一张牌加入手牌。
    CardAdded {
        /// 哪一方的手牌。
        side: Side,
        /// 新牌的实体。
        entity: EntityId,
        /// 是否真的加入（手牌满 / 重复 -> false）。
        added: bool,
    },
    /// 给一张手牌赋予状态。
    HandStatus {
        /// 哪一方的手牌。
        side: Side,
        /// 被赋予状态的手牌实体。
        entity: EntityId,
        /// 赋予的状态。
        status: HandCardStatus,
    },
    /// 无法执行（不改状态）。
    Skipped(Skip),
}

/// 一条操作 + 它的执行结果。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct OpRecord {
    /// 原始操作请求。
    pub op: EffectOp,
    /// 执行结果。
    pub result: OpResult,
}

// ======================================================================
// 执行依赖
// ======================================================================

/// 执行器的**外部依赖**（只借不拥有）。
pub struct ExecutionEnv<'a> {
    /// 伤害入口的依赖（允许附着集合 + 反应规则表 + 反应注册表）——**直接复用**
    /// `families::character::damage::DamageEnv`，不另造一套。
    pub damage: DamageEnv<'a>,
    /// 触发方**元素精通**（规则书·元素精通：强化反应效果；结晶护盾每点 +3 层）。
    pub mastery: Nat,
    /// **反应效果造成的伤害**的乘算 / 除算来源（D9 四段模型；默认 NONE）。
    ///
    /// 为什么需要单独来源：反应效果里的 EffectOp::Damage 是「反应效果造成的伤害」，
    /// 不是本次触发伤害本身 —— 触发伤害的 modifiers 在关键字层给触发伤害；反应效果
    /// 自己的乘算由调用方（卡牌 / 关键字）通过 ExecutionEnv::with_modifiers 传入。
    pub modifiers: DamageModifiers<'a>,
}

impl<'a> ExecutionEnv<'a> {
    /// 建执行依赖（反应效果伤害默认无乘算 / 除算 -> DamageModifiers::NONE）。
    pub fn new(damage: DamageEnv<'a>, mastery: Nat) -> ExecutionEnv<'a> {
        ExecutionEnv {
            damage,
            mastery,
            modifiers: DamageModifiers::NONE,
        }
    }

    /// 给**反应效果造成的伤害**挂乘算 / 除算来源（纯值，不改既有语义）。
    pub fn with_modifiers(mut self, modifiers: DamageModifiers<'a>) -> ExecutionEnv<'a> {
        self.modifiers = modifiers;
        self
    }
}

// ======================================================================
// 执行结果：分支
// ======================================================================

/// **一条已展开分支**的执行结果（D190：分支 = 操作清单 + 精确概率，不采样）。
#[derive(Clone, Debug)]
pub struct BranchExecution {
    /// 该分支的精确概率（原样带回，不改写）。
    pub probability: Fraction,
    /// 执行后的双方区。
    pub zones: ReactionZones,
    /// 执行后的发号器。
    pub alloc: EntityAlloc,
    /// 各区派发出来的待执行（本层不执行）。
    pub pending: ReactionPending,
    /// 每条操作的执行记录（顺序 = 分支里的操作顺序）。
    pub records: Vec<OpRecord>,
}

/// **整个反应效果**的执行结果：本伤害加成 + 全部分支。
#[derive(Clone, Debug)]
pub struct ReactionExecution {
    /// 规则书「本伤害 +N」（加在**触发伤害**上；由伤修链消费）。
    pub damage_bonus: Nat,
    /// 全部分支（每个分支都是从同一份输入独立执行）。
    pub branches: Vec<BranchExecution>,
}

impl ReactionExecution {
    /// 分支概率之和（精确分数；用于断言概率守恒，D150 / D158）。
    pub fn probability_total(&self) -> Fraction {
        self.branches.iter().fold(Fraction::ZERO, |sum, branch| {
            sum.exact_add(branch.probability)
        })
    }

    /// 分支条数。
    pub fn branch_count(&self) -> Nat {
        Nat::new(self.branches.len() as _)
    }
}

// ======================================================================
// 执行器
// ======================================================================

/// **反应效果执行器**：事件 + 双方触发注册表 + 执行依赖。
pub struct ReactionExecutor<'e, 'd> {
    /// 本次反应事件（谁触发 / 作用在谁身上 / 命中的元素）。
    pub event: &'e ReactionEvent,
    /// 双方触发注册表（只借）。
    pub triggers: &'e ReactionTriggers,
    /// 执行依赖（伤害入口 + 元素精通）。
    pub env: &'e ExecutionEnv<'d>,
}

impl<'e, 'd> ReactionExecutor<'e, 'd> {
    /// 建执行器。
    pub fn new(
        event: &'e ReactionEvent,
        triggers: &'e ReactionTriggers,
        env: &'e ExecutionEnv<'d>,
    ) -> ReactionExecutor<'e, 'd> {
        ReactionExecutor {
            event,
            triggers,
            env,
        }
    }

    /// **执行整个反应效果**：每个分支各跑一遍（D190：不采样、不折叠）。
    pub fn execute_effect(
        &self,
        effect: &ReactionEffectResult,
        zones: &ReactionZones,
        alloc: EntityAlloc,
    ) -> ReactionExecution {
        let branches = effect
            .branches
            .iter()
            .map(|branch| self.execute_branch(branch, zones, alloc))
            .collect();
        ReactionExecution {
            damage_bonus: effect.damage_bonus,
            branches,
        }
    }

    /// **执行一条分支**（操作按分支里的顺序依次执行，不重排）。
    pub fn execute_branch(
        &self,
        branch: &ReactionBranch,
        zones: &ReactionZones,
        alloc: EntityAlloc,
    ) -> BranchExecution {
        let mut zones = zones.clone();
        let mut alloc = alloc;
        let mut pending = ReactionPending::default();
        let mut records = Vec::with_capacity(branch.ops.len());
        for op in &branch.ops {
            let result = self.apply_op(op, &mut zones, &mut alloc, &mut pending);
            records.push(OpRecord { op: *op, result });
        }
        BranchExecution {
            probability: branch.probability,
            zones,
            alloc,
            pending,
            records,
        }
    }

    /// 执行一条操作（分发到各区操作；每条都纯函数式回写）。
    fn apply_op(
        &self,
        op: &EffectOp,
        zones: &mut ReactionZones,
        alloc: &mut EntityAlloc,
        pending: &mut ReactionPending,
    ) -> OpResult {
        match *op {
            EffectOp::Damage { amount, ty, target } => self.apply_damage(amount, ty, target, zones),
            EffectOp::Status {
                card,
                target,
                stacks,
                uses: _,
                stack_cap,
            } => self.apply_status(card, target, stacks, stack_cap, zones, pending),
            EffectOp::Summon {
                card,
                side,
                uses,
                stack_limit: _,
            } => self.apply_summon(card, side, uses, zones, alloc, pending),
            EffectOp::ForceSwitch { target } => self.apply_force_switch(target, zones, pending),
            EffectOp::AddCardToHand { card, side } => self.apply_add_card(card, side, zones, alloc),
            EffectOp::HandCardStatus {
                status,
                side,
                index,
            } => self.apply_hand_status(status, side, index, zones, pending),
        }
    }

    // ------------------------------------------------------------------
    // 伤害
    // ------------------------------------------------------------------

    /// `EffectOp::Damage` -> `damage::damage`（先走伤修链再扣血）。
    ///
    /// 反应效果造成的伤害也吃护盾 / 减伤（规则书：护盾抵消一切伤害；穿透不吃），
    /// 但**不吃**本次反应的「本伤害 +N」（那一份已经加在触发伤害上）。
    fn apply_damage(
        &self,
        amount: Nat,
        ty: DamageType,
        scope: TargetScope,
        zones: &mut ReactionZones,
    ) -> OpResult {
        let side = scope_side(scope);
        let entities = self.resolve_scope(scope, zones);
        let mut entries = Vec::new();
        for entity in entities {
            let side_zones = zones.side_mut(side);
            let Some(chain) =
                damage_chain_points(side_zones, entity, amount, ty, self.env.modifiers)
            else {
                continue;
            };
            let Some(slot) = side_zones.characters.slot_of(entity) else {
                continue;
            };
            let outcome = damage(
                &slot.state,
                self.event.source,
                entity,
                chain.to_hp,
                ty,
                &self.env.damage,
            );
            let reacted = outcome.reaction.is_some();
            if let Some(next) = side_zones.characters.with_state(entity, outcome.state) {
                side_zones.characters = next;
            }
            entries.push(DamageEntry {
                target: entity,
                to_hp: chain.to_hp,
                shield_used: chain.shield_used,
                bonus: chain.bonus_total,
                reacted,
            });
        }
        if entries.is_empty() {
            OpResult::Skipped(Skip::NoTarget)
        } else {
            OpResult::Damage(entries)
        }
    }

    // ------------------------------------------------------------------
    // 状态
    // ------------------------------------------------------------------

    /// `EffectOp::Status` -> 角色状态列表 `StateList::add` / 出战状态区
    /// `CombatStatusZone::attach_with_triggers`。
    fn apply_status(
        &self,
        card: CardId,
        target: StatusTarget,
        stacks: Nat,
        stack_cap: Option<Nat>,
        zones: &mut ReactionZones,
        pending: &mut ReactionPending,
    ) -> OpResult {
        let Some(identity) = statuses::status_of_card(card) else {
            return OpResult::Skipped(Skip::UnknownStatusCard);
        };
        // 元素精通强化反应效果：护盾类状态每点精通 +3 层（规则书·元素精通原文例）。
        let stacks = shield_stacks_with_mastery(identity, stacks, self.env.mastery);
        match target {
            StatusTarget::Character(scope) => {
                self.apply_character_status(identity, scope, stacks, stack_cap, zones)
            }
            StatusTarget::Side(side) => {
                self.apply_combat_status(identity, side, stacks, stack_cap, zones, pending)
            }
        }
    }

    /// 角色状态：写进该角色 `CharacterState::states`（`pos` = 0）。
    fn apply_character_status(
        &self,
        identity: &'static dyn StateIdentity,
        scope: TargetScope,
        stacks: Nat,
        stack_cap: Option<Nat>,
        zones: &mut ReactionZones,
    ) -> OpResult {
        let side = scope_side(scope);
        let entities = self.resolve_scope(scope, zones);
        let mut last_target = None;
        let mut applied = Nat::ZERO;
        for entity in entities {
            let Some(slot) = zones.side(side).characters.slot_of(entity) else {
                continue;
            };
            let mut state = slot.state.clone();
            let existing = character_state_layers(&state.states, identity);
            let add = capped_stacks(existing, stacks, stack_cap);
            last_target = Some(entity);
            if add == Nat::ZERO {
                continue;
            }
            state.states.add(CHARACTER_STATE_POS, identity, Some(add));
            let side_zones = zones.side_mut(side);
            if let Some(next) = side_zones.characters.with_state(entity, state) {
                side_zones.characters = next;
            }
            applied = nadd!(applied, add);
        }
        match last_target {
            None => OpResult::Skipped(Skip::NoTarget),
            Some(_) if applied == Nat::ZERO => OpResult::Skipped(Skip::StackCapped),
            Some(target) => OpResult::Status {
                place: StatusPlace::CharacterState,
                target,
                stacks: applied,
            },
        }
    }

    /// 出战状态：写进该方 `CombatStatusZone`（挂在该方出战角色上）。
    fn apply_combat_status(
        &self,
        identity: &'static dyn StateIdentity,
        side: Side,
        stacks: Nat,
        stack_cap: Option<Nat>,
        zones: &mut ReactionZones,
        pending: &mut ReactionPending,
    ) -> OpResult {
        let Some(active) = zones.side(side).characters.active_entity() else {
            return OpResult::Skipped(Skip::NoTarget);
        };
        let side_zones = zones.side_mut(side);
        let existing = combat_status_layers(&side_zones.combat_status, identity);
        let add = capped_stacks(existing, stacks, stack_cap);
        if add == Nat::ZERO {
            return OpResult::Skipped(Skip::StackCapped);
        }
        let triggers = self.triggers.side(side);
        let resolution = side_zones.combat_status.attach_with_triggers(
            active,
            identity,
            Some(add),
            None,
            &triggers.combat_status,
        );
        side_zones.combat_status = resolution.zone;
        pending
            .side_mut(side)
            .combat_status
            .extend(resolution.pending);
        OpResult::Status {
            place: StatusPlace::CombatStatus,
            target: active,
            stacks: add,
        }
    }

    // ------------------------------------------------------------------
    // 召唤物
    // ------------------------------------------------------------------

    /// `EffectOp::Summon` -> `SummonZone::summon_with_triggers`（同名自动合并）。
    fn apply_summon(
        &self,
        card: CardId,
        side: Side,
        uses: Nat,
        zones: &mut ReactionZones,
        alloc: &mut EntityAlloc,
        pending: &mut ReactionPending,
    ) -> OpResult {
        let entity = alloc.take();
        let entry = SummonEntry::new(entity, card).with_uses(Some(uses));
        let triggers = self.triggers.side(side);
        let side_zones = zones.side_mut(side);
        let resolution = side_zones
            .summons
            .summon_with_triggers(entry, &triggers.summons);
        side_zones.summons = resolution.zone;
        pending.side_mut(side).summons.extend(resolution.pending);
        match resolution.outcome {
            SummonOutcome::RejectedFull => OpResult::Skipped(Skip::SummonZoneFull),
            outcome => {
                let kept = match outcome {
                    SummonOutcome::Merged { kept, .. } => kept,
                    _ => entity,
                };
                OpResult::Summon {
                    side,
                    entity: kept,
                    outcome,
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // 强制切换
    // ------------------------------------------------------------------

    /// `EffectOp::ForceSwitch` -> `CharacterZone::switch_active_with_triggers`。
    ///
    /// 「下一个角色」= 角色区槽位顺序的下一个（末尾回绕输入出角色）；只有一个角色 -> 无法切换。
    fn apply_force_switch(
        &self,
        scope: TargetScope,
        zones: &mut ReactionZones,
        pending: &mut ReactionPending,
    ) -> OpResult {
        let side = scope_side(scope);
        let side_zones = zones.side_mut(side);
        let Some(from) = side_zones.characters.active_entity() else {
            return OpResult::Skipped(Skip::NoTarget);
        };
        let Some(to) = next_character(&side_zones.characters, from) else {
            return OpResult::Skipped(Skip::NoNextCharacter);
        };
        let triggers = self.triggers.side(side);
        let Some(resolution) = side_zones
            .characters
            .switch_active_with_triggers(to, &triggers.characters)
        else {
            return OpResult::Skipped(Skip::NoNextCharacter);
        };
        side_zones.characters = resolution.zone;
        pending.side_mut(side).characters.extend(resolution.pending);
        OpResult::Switch { side, from, to }
    }

    // ------------------------------------------------------------------
    // 手牌
    // ------------------------------------------------------------------

    /// `EffectOp::AddCardToHand` -> `HandState::add`。
    fn apply_add_card(
        &self,
        card: CardId,
        side: Side,
        zones: &mut ReactionZones,
        alloc: &mut EntityAlloc,
    ) -> OpResult {
        let entity = alloc.take();
        let side_zones = zones.side_mut(side);
        let (hand, added) = side_zones.hand.add(CardInstance::new(entity, card));
        side_zones.hand = hand;
        OpResult::CardAdded {
            side,
            entity,
            added,
        }
    }

    /// `EffectOp::HandCardStatus` -> `HandCardStates::add_state_with_triggers`。
    ///
    /// 下标 -> 具体牌用**手牌区只读指针**（D183：`HandState::entries()`）。
    fn apply_hand_status(
        &self,
        status: HandCardStatus,
        side: Side,
        index: Nat,
        zones: &mut ReactionZones,
        pending: &mut ReactionPending,
    ) -> OpResult {
        let Some(entry) = zones
            .side(side)
            .hand
            .entries()
            .get(index.get() as usize)
            .copied()
        else {
            return OpResult::Skipped(Skip::HandIndexOutOfRange);
        };
        let identity = statuses::hand_status_identity(status);
        let triggers = self.triggers.side(side);
        let side_zones = zones.side_mut(side);
        let resolution = side_zones.hand_states.add_state_with_triggers(
            entry.entity,
            identity,
            None,
            &triggers.hand_states,
        );
        side_zones.hand_states = resolution.states;
        pending
            .side_mut(side)
            .hand_states
            .extend(resolution.pending);
        OpResult::HandStatus {
            side,
            entity: entry.entity,
            status,
        }
    }

    // ------------------------------------------------------------------
    // TargetScope 解析
    // ------------------------------------------------------------------

    /// 把 `TargetScope` 解析成实体列表（**按角色区槽位顺序 = 进场先后**，结算顺序 §一 第 3 条）。
    ///
    /// - `Reacted`：反应事件里的承载角色；
    /// - `SourceActive` / `TargetActive`：对应方的出战角色；
    /// - `TargetOthers`：承载方**除承载角色外**的全部角色（槽位顺序）。
    fn resolve_scope(&self, scope: TargetScope, zones: &ReactionZones) -> Vec<EntityId> {
        match scope {
            TargetScope::Reacted => vec![self.event.target],
            TargetScope::SourceActive => zones
                .source
                .characters
                .active_entity()
                .into_iter()
                .collect(),
            TargetScope::TargetActive => zones
                .target
                .characters
                .active_entity()
                .into_iter()
                .collect(),
            TargetScope::TargetOthers => zones
                .target
                .characters
                .slots()
                .iter()
                .map(|slot| slot.character)
                .filter(|character| *character != self.event.target)
                .collect(),
        }
    }
}

// ======================================================================
// 自由函数（纯函数；测试 / 外部复用）
// ======================================================================

/// 目标范围属于哪一方（相对侧）。
pub fn scope_side(scope: TargetScope) -> Side {
    match scope {
        TargetScope::SourceActive => Side::Source,
        TargetScope::Reacted | TargetScope::TargetActive | TargetScope::TargetOthers => {
            Side::Target
        }
    }
}

/// **槽位顺序的下一个角色**（结算顺序 §一 第 3 条 = 进场先后；末尾回绕）。
///
/// 只有一个角色 / 不在区里 -> `None`（无法强制切换）。
pub fn next_character(characters: &CharacterZone, active: EntityId) -> Option<EntityId> {
    let slots = characters.slots();
    let position = slots.iter().position(|slot| slot.character == active)?;
    if slots.len() <= 1 {
        return None;
    }
    let index = Nat::new(position as _);
    let len = Nat::new(slots.len() as _);
    let next = nadd!(index, Nat::new(1));
    let next = if next >= len { Nat::ZERO } else { next };
    if next == index {
        return None;
    }
    slots.get(next.get() as usize).map(|slot| slot.character)
}

/// 叠加上限：先按上限钳制「还能加多少」（`None` = 不设上限）。
pub fn capped_stacks(existing: Nat, stacks: Nat, cap: Option<Nat>) -> Nat {
    let Some(cap) = cap else {
        return stacks;
    };
    if existing >= cap {
        return Nat::ZERO;
    }
    let room = nsub!(cap, existing);
    if stacks > room {
        room
    } else {
        stacks
    }
}

/// 元素精通对**护盾类状态**的强化：每点精通 +3 层（规则书·元素精通）。
fn shield_stacks_with_mastery(
    identity: &'static dyn StateIdentity,
    stacks: Nat,
    mastery: Nat,
) -> Nat {
    if mastery == Nat::ZERO || !statuses::identity_in(identity, statuses::SHIELD_STATUSES) {
        return stacks;
    }
    nadd!(stacks, nmul!(mastery, MASTERY_SHIELD_PER_POINT))
}

/// 角色状态列表里某个身份的现有总层数（布尔类按 1 层）。
///
/// **按身份数据地址比较**（`statuses::same_identity` → `StateIdentity::identity_address`）：
/// ZST 身份没有独立地址，统一取「指向定义数据的指针」；这里保持同一口径直接遍历。
fn character_state_layers(states: &StateList, identity: &'static dyn StateIdentity) -> Nat {
    let mut total = Nat::ZERO;
    for entry in states.entries() {
        if entry.pos == CHARACTER_STATE_POS && statuses::same_identity(entry.state, identity) {
            total = nadd!(total, entry.layers.unwrap_or(Nat::new(1)));
        }
    }
    total
}

/// 出战状态区里某个身份的总层数（布尔类按 1 层）。
fn combat_status_layers(zone: &CombatStatusZone, identity: &'static dyn StateIdentity) -> Nat {
    let mut total = Nat::ZERO;
    for entry in zone.entries() {
        if statuses::same_identity(entry.status, identity) {
            total = nadd!(total, entry.layers.unwrap_or(Nat::new(1)));
        }
    }
    total
}

/// 对某个角色算「伤修链」的结果（只读区；不改状态）。
///
/// modifiers = 反应效果伤害自己的乘算 / 除算来源（默认 NONE；调用方给）。
fn damage_chain_points(
    side_zones: &SideZones,
    entity: EntityId,
    amount: Nat,
    ty: DamageType,
    modifiers: DamageModifiers<'_>,
) -> Option<ChainOutcome> {
    let slot = side_zones.characters.slot_of(entity)?;
    let env = DamageChainEnv::new(&side_zones.combat_status, &slot.state.states)
        .with_modifiers(modifiers);
    Some(settle(
        DamageInput::new(amount, ty),
        &env,
        &DamageBonus::NONE,
    ))
}

/// 执行器入口的便捷函数（不持有执行器时的最短路径）。
pub fn execute_reaction(
    event: &ReactionEvent,
    effect: &ReactionEffectResult,
    zones: &ReactionZones,
    triggers: &ReactionTriggers,
    env: &ExecutionEnv<'_>,
    alloc: EntityAlloc,
) -> ReactionExecution {
    ReactionExecutor::new(event, triggers, env).execute_effect(effect, zones, alloc)
}

/// **从反应事件直接执行**：内部调 `reactions::table::resolve` 取「加伤 + 分支」，
/// 再逐支执行到各区（反应表 -> 执行器的完整闭环）。
///
/// - 没登记效果的反应 -> `None`（不改状态；表里 14 个身份都登记了，正常不会发生）；
/// - `context` 决定**分支展开**（手牌数 / 是否出战），不影响加伤（见 `bonus`）。
pub fn execute_resolved(
    event: &ReactionEvent,
    context: &EffectContext,
    zones: &ReactionZones,
    triggers: &ReactionTriggers,
    env: &ExecutionEnv<'_>,
    alloc: EntityAlloc,
) -> Option<ReactionExecution> {
    let effect = table::resolve(event, context)?;
    Some(execute_reaction(
        event, &effect, zones, triggers, env, alloc,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::StateList;
    use crate::families::character::attachment::{
        AttachmentAllowSet, ReactionCondition, ReactionRegistry,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::zones::field::CharacterSlot;
    use crate::reactions::effect::EffectContext;
    use crate::families::character::damage_chain::bonus::AmountOp;
    use crate::reactions::{crystallize, entities, lunar_charged, vaporize};
    use gi_genius_invokation_engine::nat::ExactNat128;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    /// 角色运行期状态（生命 `hp` / 上限 10 / 充能 0-2）。
    fn character(hp: u32) -> CharacterState {
        CharacterState::new(n(hp), n(10), Nat::ZERO, n(2))
    }

    fn slot(entity: u32, hp: u32) -> CharacterSlot {
        CharacterSlot::new(n(entity), character(hp))
    }

    /// 双方角色区：第一个槽位默认是出战角色（`try_from_slots` 的既定行为）。
    fn zones_with(source: Vec<CharacterSlot>, target: Vec<CharacterSlot>) -> ReactionZones {
        let mut zones = ReactionZones::new();
        zones.source.characters = CharacterZone::try_from_slots(source).expect("合法槽位");
        zones.target.characters = CharacterZone::try_from_slots(target).expect("合法槽位");
        zones
    }

    /// 双手牌（每张一个实体号，便于观察下标 -> 实体）。
    fn hand_with(count: u32) -> HandState {
        let mut hand = HandState::new();
        let mut entity = n(200);
        let mut index = Nat::ZERO;
        while index < n(count) {
            let (next, added) = hand.add(CardInstance::new(entity, CardId::new(entity.get())));
            assert!(added);
            hand = next;
            entity = nadd!(entity, Nat::new(1));
            index = nadd!(index, Nat::new(1));
        }
        hand
    }

    /// 本次反应事件：源 = 触发方 1 号，承载 = 承载方 3 号（蒸发）。
    fn event() -> ReactionEvent {
        ReactionEvent::new(
            &vaporize::VAPORIZE,
            crate::base::Element::Hydro,
            crate::base::Element::Pyro,
            n(1),
            n(3),
        )
    }

    /// 执行一条分支（空触发注册表；伤害入口用基础反应表）。
    fn execute(ops: Vec<EffectOp>, zones: &ReactionZones, mastery: Nat) -> BranchExecution {
        let event = event();
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let damage_env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let env = ExecutionEnv::new(damage_env, mastery);
        let triggers = ReactionTriggers::none();
        let executor = ReactionExecutor::new(&event, &triggers, &env);
        let branch = ReactionBranch {
            ops,
            probability: Fraction::ONE,
        };
        executor.execute_branch(&branch, zones, EntityAlloc::starting_at(n(100)))
    }

    /// 执行一条分支，并给**反应效果造成的伤害**挂乘算 / 除算来源。
    fn execute_with_modifiers(
        ops: Vec<EffectOp>,
        zones: &ReactionZones,
        modifiers: DamageModifiers<'_>,
    ) -> BranchExecution {
        let event = event();
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let damage_env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let env = ExecutionEnv::new(damage_env, Nat::ZERO).with_modifiers(modifiers);
        let triggers = ReactionTriggers::none();
        let executor = ReactionExecutor::new(&event, &triggers, &env);
        let branch = ReactionBranch {
            ops,
            probability: Fraction::ONE,
        };
        executor.execute_branch(&branch, zones, EntityAlloc::starting_at(n(100)))
    }

    /// 状态列表里有没有这个身份（**按身份数据地址比较**，`identity_address`）。
    fn has_state(list: &StateList, identity: &'static dyn StateIdentity) -> bool {
        list.entries()
            .iter()
            .any(|entry| statuses::same_identity(entry.state, identity))
    }

    /// 某张手牌上有没有这个状态。
    fn hand_has(
        states: &HandCardStates,
        entity: EntityId,
        identity: &'static dyn StateIdentity,
    ) -> bool {
        states
            .states_of(entity)
            .is_some_and(|list| has_state(list, identity))
    }

    fn damage_op(amount: u32, ty: DamageType, target: TargetScope) -> EffectOp {
        EffectOp::Damage {
            amount: n(amount),
            ty,
            target,
        }
    }

    fn summon_op(card: CardId, side: Side) -> EffectOp {
        EffectOp::Summon {
            card,
            side,
            uses: n(1),
            stack_limit: None,
        }
    }

    // ------------------------------------------------------------------
    // EffectOp::Damage
    // ------------------------------------------------------------------

    #[test]
    fn damage_op_hits_the_target_active_character() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10), slot(4, 10)]);
        let result = execute(
            vec![damage_op(
                2,
                DamageType::Physical,
                TargetScope::TargetActive,
            )],
            &zones,
            Nat::ZERO,
        );
        assert_eq!(result.records.len(), 1);
        match &result.records[0].result {
            OpResult::Damage(entries) => {
                assert_eq!(entries.len(), 1);
                assert_eq!(entries[0].target, n(3));
                assert_eq!(entries[0].to_hp, n(2));
            }
            other => panic!("应为伤害结果，实际 {other:?}"),
        }
        assert_eq!(
            result
                .zones
                .target
                .characters
                .slot_of(n(3))
                .expect("3 号")
                .state
                .hp(),
            n(8)
        );
        // 后台角色不动。
        assert_eq!(
            result
                .zones
                .target
                .characters
                .slot_of(n(4))
                .expect("4 号")
                .state
                .hp(),
            n(10)
        );
    }

    #[test]
    fn damage_op_reads_the_shield_zone_through_the_chain() {
        // 出战状态区挂 2 点护盾：4 点伤害只扣 2 点血（链路：区 -> 伤修链 -> 扣血）。
        let mut zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        zones.target.combat_status = CombatStatusZone::new()
            .attach_with_triggers(
                n(3),
                &statuses::SHIELD_COMBAT,
                Some(n(2)),
                None,
                &CombatStatusTriggers::new(),
            )
            .zone;
        let result = execute(
            vec![damage_op(
                4,
                DamageType::Physical,
                TargetScope::TargetActive,
            )],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Damage(entries) => {
                assert_eq!(entries[0].shield_used, n(2));
                assert_eq!(entries[0].to_hp, n(2));
            }
            other => panic!("应为伤害结果，实际 {other:?}"),
        }
        assert_eq!(
            result
                .zones
                .target
                .characters
                .slot_of(n(3))
                .expect("3 号")
                .state
                .hp(),
            n(8)
        );
    }

    #[test]
    fn damage_op_piercing_ignores_the_shield() {
        let mut zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        zones.target.combat_status = CombatStatusZone::new()
            .attach_with_triggers(
                n(3),
                &statuses::SHIELD_COMBAT,
                Some(n(9)),
                None,
                &CombatStatusTriggers::new(),
            )
            .zone;
        let result = execute(
            vec![damage_op(
                3,
                DamageType::Piercing,
                TargetScope::TargetActive,
            )],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Damage(entries) => {
                assert_eq!(entries[0].shield_used, Nat::ZERO);
                assert_eq!(entries[0].to_hp, n(3));
            }
            other => panic!("应为伤害结果，实际 {other:?}"),
        }
    }

    #[test]
    fn target_others_resolves_in_slot_order_and_skips_the_reacted_one() {
        let zones = zones_with(
            vec![slot(1, 10)],
            vec![slot(3, 10), slot(4, 10), slot(5, 10)],
        );
        let result = execute(
            vec![damage_op(
                1,
                DamageType::Piercing,
                TargetScope::TargetOthers,
            )],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Damage(entries) => {
                assert_eq!(
                    entries.iter().map(|entry| entry.target).collect::<Vec<_>>(),
                    vec![n(4), n(5)],
                    "承载角色（3 号）被排除，其余按槽位（进场）顺序"
                );
            }
            other => panic!("应为伤害结果，实际 {other:?}"),
        }
    }

    #[test]
    fn damage_op_without_any_target_is_skipped() {
        // 承载方没有角色 -> TargetActive 解析不到 -> 跳过（不改状态）。
        let zones = zones_with(vec![slot(1, 10)], Vec::new());
        let result = execute(
            vec![damage_op(
                2,
                DamageType::Physical,
                TargetScope::TargetActive,
            )],
            &zones,
            Nat::ZERO,
        );
        assert_eq!(result.records[0].result, OpResult::Skipped(Skip::NoTarget));
    }

    // ------------------------------------------------------------------
    // EffectOp::Status
    // ------------------------------------------------------------------

    #[test]
    fn status_op_character_scope_writes_the_character_state_list() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(
            vec![EffectOp::Status {
                card: entities::FROZEN,
                target: StatusTarget::Character(TargetScope::TargetActive),
                stacks: n(1),
                uses: None,
                stack_cap: None,
            }],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Status {
                place,
                target,
                stacks,
            } => {
                assert_eq!(*place, StatusPlace::CharacterState);
                assert_eq!(*target, n(3));
                assert_eq!(*stacks, n(1));
            }
            other => panic!("应为状态结果，实际 {other:?}"),
        }
        let states = &result
            .zones
            .target
            .characters
            .slot_of(n(3))
            .expect("3 号")
            .state
            .states;
        assert!(has_state(states, &statuses::FROZEN));
    }

    #[test]
    fn status_op_side_scope_writes_the_combat_status_zone() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(
            vec![EffectOp::Status {
                card: entities::DENDRO_CORE,
                target: StatusTarget::Side(Side::Target),
                stacks: n(1),
                uses: None,
                stack_cap: None,
            }],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Status { place, target, .. } => {
                assert_eq!(*place, StatusPlace::CombatStatus);
                assert_eq!(*target, n(3), "出战状态挂在当前出战角色上");
            }
            other => panic!("应为状态结果，实际 {other:?}"),
        }
        assert_eq!(result.zones.target.combat_status.entries().len(), 1);
        assert!(statuses::same_identity(
            result.zones.target.combat_status.entries()[0].status,
            &statuses::DENDRO_CORE
        ));
    }

    #[test]
    fn reaction_damage_gets_multiplier_modifiers() {
        // 超导 / 扩散类「对目标以外角色」的伤害也走伤修链：乘算来源接进执行器后，
        // 同一份反应伤害应吃到四段模型的乘算（×2）。
        let zones = zones_with(
            vec![slot(1, 10)],
            vec![slot(3, 10), slot(4, 10), slot(5, 10)],
        );
        let ops = vec![EffectOp::Damage {
            amount: n(1),
            ty: DamageType::Physical,
            target: TargetScope::TargetOthers,
        }];
        let hp = |branch: &BranchExecution, entity: u32| {
            branch
                .zones
                .target
                .characters
                .slot_of(n(entity))
                .expect("角色在区里")
                .state
                .hp()
        };
        // 默认（NONE）：1 点伤害。
        let plain = execute(ops.clone(), &zones, Nat::ZERO);
        assert_eq!(hp(&plain, 4), n(9));
        assert_eq!(hp(&plain, 5), n(9));
        // 挂上「翻倍」乘算来源：2 点伤害。
        let multipliers = [AmountOp::mul_zone(n(2))];
        let boosted = execute_with_modifiers(
            ops,
            &zones,
            DamageModifiers {
                ops: &multipliers,
                rounding: None,
            },
        );
        assert_eq!(hp(&boosted, 4), n(8));
        assert_eq!(hp(&boosted, 5), n(8));
        // 被反应的角色本身不在 TargetOthers 里，不受影响。
        assert_eq!(hp(&boosted, 3), n(10));
    }

    #[test]
    fn status_op_respects_the_stack_cap() {
        // 结晶护盾「最多 2 点」：已叠 2 -> 再叠 1 被上限吃掉（跳过，不改状态）。
        let mut zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        zones.target.combat_status = CombatStatusZone::new()
            .attach_with_triggers(
                n(3),
                &statuses::SHIELD_COMBAT,
                Some(n(2)),
                None,
                &CombatStatusTriggers::new(),
            )
            .zone;
        let result = execute(
            vec![EffectOp::Status {
                card: entities::SHIELD,
                target: StatusTarget::Side(Side::Target),
                stacks: n(1),
                uses: None,
                stack_cap: Some(n(2)),
            }],
            &zones,
            Nat::ZERO,
        );
        assert_eq!(
            result.records[0].result,
            OpResult::Skipped(Skip::StackCapped)
        );
    }

    #[test]
    fn status_op_unknown_card_is_skipped() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(
            vec![EffectOp::Status {
                card: CardId::new(987_654),
                target: StatusTarget::Side(Side::Target),
                stacks: n(1),
                uses: None,
                stack_cap: None,
            }],
            &zones,
            Nat::ZERO,
        );
        assert_eq!(
            result.records[0].result,
            OpResult::Skipped(Skip::UnknownStatusCard)
        );
    }

    #[test]
    fn mastery_boosts_shield_status_stacks() {
        // 规则书·元素精通例：每 1 点元素精通，结晶护盾 +3 层 -> 1 + 2 × 3 = 7。
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(
            vec![EffectOp::Status {
                card: entities::SHIELD,
                target: StatusTarget::Character(TargetScope::TargetActive),
                stacks: n(1),
                uses: None,
                stack_cap: None,
            }],
            &zones,
            n(2),
        );
        match &result.records[0].result {
            OpResult::Status { stacks, .. } => assert_eq!(*stacks, n(7)),
            other => panic!("应为状态结果，实际 {other:?}"),
        }
        // 非护盾状态不被精通强化。
        let frozen = execute(
            vec![EffectOp::Status {
                card: entities::FROZEN,
                target: StatusTarget::Character(TargetScope::TargetActive),
                stacks: n(1),
                uses: None,
                stack_cap: None,
            }],
            &zones,
            n(2),
        );
        match &frozen.records[0].result {
            OpResult::Status { stacks, .. } => assert_eq!(*stacks, n(1)),
            other => panic!("应为状态结果，实际 {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // EffectOp::Summon
    // ------------------------------------------------------------------

    #[test]
    fn summon_op_generates_a_summon_with_an_allocated_entity() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(
            vec![summon_op(entities::THUNDERCLOUD, Side::Source)],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Summon {
                side,
                entity,
                outcome,
            } => {
                assert_eq!(*side, Side::Source);
                assert_eq!(*entity, n(100), "实体号由 EntityAlloc 发");
                assert_eq!(*outcome, SummonOutcome::Summoned);
            }
            other => panic!("应为召唤结果，实际 {other:?}"),
        }
        assert_eq!(result.zones.source.summons.entries().len(), 1);
        assert_eq!(result.alloc.next(), n(101), "发号器已前进");
        // 承载方召唤物区不受影响。
        assert!(result.zones.target.summons.is_empty());
    }

    #[test]
    fn summon_op_merges_same_card_and_keeps_original_entity() {
        let mut zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        zones.source.summons = SummonZone::new()
            .summon_with_triggers(
                SummonEntry::new(n(50), entities::THUNDERCLOUD).with_uses(Some(n(2))),
                &SummonTriggers::new(),
            )
            .zone;
        let result = execute(
            vec![summon_op(entities::THUNDERCLOUD, Side::Source)],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Summon {
                entity, outcome, ..
            } => {
                assert_eq!(*entity, n(50), "同名合并保留原召唤物实体");
                assert!(matches!(outcome, SummonOutcome::Merged { kept, .. } if *kept == n(50)));
            }
            other => panic!("应为召唤结果，实际 {other:?}"),
        }
        assert_eq!(result.zones.source.summons.entries().len(), 1);
    }

    // ------------------------------------------------------------------
    // EffectOp::ForceSwitch
    // ------------------------------------------------------------------

    #[test]
    fn force_switch_moves_the_target_to_the_next_character() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10), slot(4, 10)]);
        let result = execute(
            vec![EffectOp::ForceSwitch {
                target: TargetScope::Reacted,
            }],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::Switch { side, from, to } => {
                assert_eq!(*side, Side::Target);
                assert_eq!(*from, n(3));
                assert_eq!(*to, n(4));
            }
            other => panic!("应为切换结果，实际 {other:?}"),
        }
        assert_eq!(result.zones.target.characters.active_entity(), Some(n(4)));
        // 触发方出战角色不动。
        assert_eq!(result.zones.source.characters.active_entity(), Some(n(1)));
    }

    #[test]
    fn force_switch_with_a_single_character_is_skipped() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(
            vec![EffectOp::ForceSwitch {
                target: TargetScope::Reacted,
            }],
            &zones,
            Nat::ZERO,
        );
        assert_eq!(
            result.records[0].result,
            OpResult::Skipped(Skip::NoNextCharacter)
        );
        assert_eq!(result.zones.target.characters.active_entity(), Some(n(3)));
    }

    // ------------------------------------------------------------------
    // EffectOp::AddCardToHand
    // ------------------------------------------------------------------

    #[test]
    fn add_card_op_puts_a_new_card_into_the_hand() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(
            vec![EffectOp::AddCardToHand {
                card: entities::MOONDRIFT_HARMONY,
                side: Side::Source,
            }],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::CardAdded {
                side,
                entity,
                added,
            } => {
                assert_eq!(*side, Side::Source);
                assert_eq!(*entity, n(100));
                assert!(*added);
            }
            other => panic!("应为加牌结果，实际 {other:?}"),
        }
        assert_eq!(result.zones.source.hand.len(), n(1));
        assert_eq!(
            result.zones.source.hand.card_of(n(100)),
            Some(entities::MOONDRIFT_HARMONY)
        );
    }

    // ------------------------------------------------------------------
    // EffectOp::HandCardStatus
    // ------------------------------------------------------------------

    #[test]
    fn hand_status_op_applies_to_the_indexed_hand_card() {
        let mut zones = zones_with(vec![slot(1, 10), slot(2, 10)], vec![slot(3, 10)]);
        zones.source.hand = hand_with(2);
        let result = execute(
            vec![EffectOp::HandCardStatus {
                status: HandCardStatus::CostReduction,
                side: Side::Source,
                index: n(1),
            }],
            &zones,
            Nat::ZERO,
        );
        match &result.records[0].result {
            OpResult::HandStatus {
                side,
                entity,
                status,
            } => {
                assert_eq!(*side, Side::Source);
                assert_eq!(*entity, n(201), "下标 1 = 实体 201（手牌区只读指针）");
                assert_eq!(*status, HandCardStatus::CostReduction);
            }
            other => panic!("应为手牌状态结果，实际 {other:?}"),
        }
        assert!(hand_has(
            &result.zones.source.hand_states,
            n(201),
            &statuses::COST_REDUCTION
        ));
    }

    #[test]
    fn hand_status_op_out_of_range_index_is_skipped() {
        let mut zones = zones_with(vec![slot(1, 10), slot(2, 10)], vec![slot(3, 10)]);
        zones.source.hand = hand_with(1);
        let result = execute(
            vec![EffectOp::HandCardStatus {
                status: HandCardStatus::ElectricShock,
                side: Side::Source,
                index: n(5),
            }],
            &zones,
            Nat::ZERO,
        );
        assert_eq!(
            result.records[0].result,
            OpResult::Skipped(Skip::HandIndexOutOfRange)
        );
    }

    // ------------------------------------------------------------------
    // 分支 / 概率 / 顺序
    // ------------------------------------------------------------------

    #[test]
    fn branch_probabilities_are_kept_and_sum_to_one_without_sampling() {
        // 月感电：3 张手牌 -> 3 支（每支 1/3），执行器逐支执行、原样带回概率。
        let mut zones = zones_with(vec![slot(1, 10), slot(2, 10)], vec![slot(3, 10)]);
        zones.target.hand = hand_with(3);
        let event = event();
        let rules: [crate::families::character::damage::ReactionRule; 0] = [];
        let registry = ReactionRegistry::new();
        let damage_env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let env = ExecutionEnv::new(damage_env, Nat::ZERO);
        let triggers = ReactionTriggers::none();
        let executor = ReactionExecutor::new(&event, &triggers, &env);
        let effect = lunar_charged::effect(&event, &EffectContext::new(true, n(0), n(3)));
        let execution = executor.execute_effect(&effect, &zones, EntityAlloc::starting_at(n(100)));
        assert_eq!(execution.branch_count(), n(3));
        assert!(execution.probability_total().is_one(), "分支概率和必须 = 1");
        let third = Fraction::new(ExactNat128::new(1), ExactNat128::new(3));
        for (index, branch) in execution.branches.iter().enumerate() {
            assert_eq!(branch.probability, third);
            // 每支独立执行：都生成雷暴云 + 给对应下标的手牌[电击]。
            assert_eq!(branch.zones.source.summons.entries().len(), 1);
            assert!(hand_has(
                &branch.zones.target.hand_states,
                n(200 + index as u32),
                &statuses::ELECTRIC_SHOCK
            ));
        }
        // 输入区没有被改（执行器纯函数式收进 / 交出）。
        assert!(zones.source.summons.is_empty());
        assert!(zones.target.hand_states.is_empty());
    }

    #[test]
    fn ops_execute_in_branch_order() {
        // 分支里的操作按给定顺序执行（超载式：先伤害再切换）。
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10), slot(4, 10)]);
        let result = execute(
            vec![
                damage_op(2, DamageType::Physical, TargetScope::Reacted),
                EffectOp::ForceSwitch {
                    target: TargetScope::Reacted,
                },
            ],
            &zones,
            Nat::ZERO,
        );
        assert_eq!(result.records.len(), 2);
        assert!(matches!(result.records[0].result, OpResult::Damage(_)));
        assert!(matches!(result.records[1].result, OpResult::Switch { .. }));
        // 3 号被扣血、随后被切换。
        assert_eq!(
            result
                .zones
                .target
                .characters
                .slot_of(n(3))
                .expect("3 号")
                .state
                .hp(),
            n(8)
        );
        assert_eq!(result.zones.target.characters.active_entity(), Some(n(4)));
    }

    #[test]
    fn empty_branch_is_allowed_and_changes_nothing() {
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let result = execute(Vec::new(), &zones, Nat::ZERO);
        assert!(result.records.is_empty());
        assert_eq!(
            result.zones.source.characters.len(),
            zones.source.characters.len()
        );
        assert_eq!(result.zones.target.characters.active_entity(), Some(n(3)));
    }

    // ------------------------------------------------------------------
    // 纯函数（自由函数）
    // ------------------------------------------------------------------

    #[test]
    fn next_character_wraps_around_and_rejects_single_character_zone() {
        let zone = CharacterZone::try_from_slots(vec![slot(3, 10), slot(4, 10), slot(5, 10)])
            .expect("合法");
        assert_eq!(next_character(&zone, n(3)), Some(n(4)));
        assert_eq!(next_character(&zone, n(5)), Some(n(3)), "末尾回绕");
        assert_eq!(next_character(&zone, n(9)), None, "不在区里");
        let single = CharacterZone::try_from_slots(vec![slot(3, 10)]).expect("合法");
        assert_eq!(next_character(&single, n(3)), None);
    }

    #[test]
    fn capped_stacks_clamps_to_the_remaining_room() {
        assert_eq!(capped_stacks(n(0), n(1), Some(n(2))), n(1));
        assert_eq!(capped_stacks(n(1), n(1), Some(n(2))), n(1));
        assert_eq!(capped_stacks(n(2), n(1), Some(n(2))), Nat::ZERO);
        assert_eq!(capped_stacks(n(5), n(1), Some(n(2))), Nat::ZERO);
        assert_eq!(capped_stacks(n(5), n(9), None), n(9), "无上限");
    }

    #[test]
    fn scope_side_maps_every_target_scope() {
        assert_eq!(scope_side(TargetScope::SourceActive), Side::Source);
        assert_eq!(scope_side(TargetScope::TargetActive), Side::Target);
        assert_eq!(scope_side(TargetScope::Reacted), Side::Target);
        assert_eq!(scope_side(TargetScope::TargetOthers), Side::Target);
    }

    #[test]
    fn entity_alloc_starts_where_asked_and_advances_by_one() {
        let mut alloc = EntityAlloc::starting_at(n(7));
        assert_eq!(alloc.next(), n(7));
        assert_eq!(alloc.take(), n(7));
        assert_eq!(alloc.next(), n(8));
    }

    #[test]
    fn side_zone_accessors_reach_both_sides() {
        let mut zones = ReactionZones::new();
        zones.side_mut(Side::Source).hand = hand_with(1);
        assert_eq!(zones.side(Side::Source).hand.len(), n(1));
        assert!(zones.side(Side::Target).hand.is_empty());
        let pending = ReactionPending::default();
        assert!(pending.source.hand_states.is_empty());
        assert!(pending.target.summons.is_empty());
    }

    #[test]
    fn status_identity_table_covers_every_reaction_status_card() {
        // 执行器用到的状态身份都来自 statuses 表（不在别处另存一份）。
        for card in [
            entities::FROZEN,
            entities::SHIELD,
            entities::DENDRO_CORE,
            entities::CATALYZING_FIELD,
        ] {
            assert!(statuses::status_of_card(card).is_some());
        }
        // 手牌状态两种身份都能取到。
        for status in [HandCardStatus::ElectricShock, HandCardStatus::CostReduction] {
            let _ = statuses::hand_status_identity(status);
        }
        // 共享口径的公共类型（StateList / ReactionCondition）仍被引用（import 不过期）。
        let _ = StateList::new();
        let _ = ReactionCondition::new(crate::base::Element::Hydro, crate::base::Element::Pyro);
    }

    #[test]
    fn resolved_effect_from_the_table_executes_through_the_executor() {
        // 反应表 -> 执行器的完整闭环：结晶 = 本伤害+1 + 给触发方出战角色 1 点护盾。
        let zones = zones_with(vec![slot(1, 10)], vec![slot(3, 10)]);
        let event = ReactionEvent::new(
            &crystallize::CRYSTALLIZE,
            crate::base::Element::Hydro,
            crate::base::Element::Geo,
            n(1),
            n(3),
        );
        let rules: [crate::families::character::damage::ReactionRule; 0] = [];
        let registry = ReactionRegistry::new();
        let damage_env = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules: &rules,
            registry: &registry,
        };
        let env = ExecutionEnv::new(damage_env, Nat::ZERO);
        let triggers = ReactionTriggers::none();
        let execution = execute_resolved(
            &event,
            &EffectContext::no_hands(false),
            &zones,
            &triggers,
            &env,
            EntityAlloc::starting_at(n(100)),
        )
        .expect("结晶已登记");
        assert_eq!(execution.damage_bonus, n(1));
        assert_eq!(execution.branch_count(), n(1));
        assert!(execution.probability_total().is_one());
        let branch = &execution.branches[0];
        assert!(matches!(
            branch.records[0].result,
            OpResult::Status {
                place: StatusPlace::CharacterState,
                target,
                ..
            } if target == n(1)
        ));
        // 护盾挂在**触发方**出战角色（1 号）身上。
        let states = &branch
            .zones
            .source
            .characters
            .slot_of(n(1))
            .expect("1 号")
            .state
            .states;
        assert!(has_state(states, &statuses::SHIELD_COMBAT));
    }
}
