//! **插入式结算的回放轨迹**（D221）：按「一步」记录每一步内部**每个关键字操作**做了什么。
//!
//! ## 口径（Lead 拍板 D221）
//!
//! - **只读、纯数据、可导出**：轨迹是结算的**观测**，不影响结算结果；与
//!   timeline::Resolution::snapshot 并存 —— snapshot 看「还要执行什么」，trace 看「刚才做了什么」；
//! - **按步记录**：每一步一条 [StepTrace]，步内每个关键字操作一条 [OpTrace]；
//! - **解析后的实际入参**：目标选择器解析出的实体列表、伤害类型 / 数值 / 来源属性、
//!   随机分支概率（Fraction）；
//! - **结果复用既有形状**：伤害条目直接用 damage_chain::executor::DamageEntry，
//!   无法执行用 damage_chain::executor::Skip；
//! - **步内禁止插入**（D204）：[StepTrace::intra_step_insertions] 正常**恒为空**；
//!   步间的插入记在 [StepTrace::insertions]（发生在哪一步之后、插入了多少条待执行）；
//! - **纯函数边界**：traced 入口吃 &DefState，返回新分支 + 轨迹（入参一个字节都不改）。
//!
//! ## 与结算的关系
//!
//! traced 入口自己按步执行（顺序 = Step 的操作顺序、分支按精确概率展开），执行内容与
//! [super::step] 完全一致；差别只在**执行后把观测记下来**。因此轨迹可以直接和
//! [super::step::execute_step_me] 的结果对照（同一个输入应得到同样的局面）。
//!
//! ## 反应子结算（R4 / R2）
//!
//! traced 路径与 [super::step::execute_step_me] **同口径**：一步的全部操作做完后，在**步边界**
//! 把本步判定出的反应用 [super::reaction::settle_inserted] 插入执行，并对每条反应记一条
//! [ReactionTrace]（反应名 / 元素 / 双方 / 是否非伤害 / 精确概率 / 派发 / 前后 diff 结果）。
//! 因此 traced 得到的局面与 execute_step_me 一致（有单测对照）。
//!
//! ## 待复核
//!
//! - 结果的推导是**前后局面 diff**（不重写关键字函数）：护盾吸收 / 加伤等细节在 diff 里
//!   看不到（DamageEntry 的 shield_used / bonus 恒填 0，reacted 恒填 false）；
//!   需要精确数值时以关键字层返回的数据为准（**待接入**：关键字函数回报 outcome）；
//! - 伤害导致的元素附着 / 反应不在 outcome 里单列（附着变化可在 Attached 里看到）；
//! - 重复（Repeat）内部的操作挂在 [OpTrace::children]（同一操作位置的子动作，仍属一步）。

use crate::families::character::damage_chain::executor::{DamageEntry, EntityAlloc, Skip};
use crate::families::character::types::DamageType;
use crate::base::Element;
use crate::flow::pending::{execute_pending, FlowPending, PendingSet};
use crate::rules::ops::order::EffectSource;
use crate::state::DefState;
use crate::timeline::Event;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, nsub, EntityId, Nat};

use super::effect::EffectEnv;
use super::op::{
    CardStatusArgs, ConsumeEnergyArgs, ConvertDiceArgs, CreateCardArgs, DamageArgs, DiscardArgs,
    DrawCardsArgs, EnergyAmount, ForceSwitchArgs, GainDiceArgs, HealArgs, KeywordOp, RepeatArgs,
    StatusArgs, StatusPlace, SummonArgs, SwitchCharacterArgs,
};
use super::reaction::{settle_inserted, ReactionRequest};
use super::step::{execute_op, Step, StepOutcome};
use super::target::{resolve, Perspective, SideScope, TargetSelector};

// ======================================================================
// 轨迹数据结构（只读、纯数据）
// ======================================================================

/// 一次结算的**完整回放轨迹**（按步记录）。
#[derive(Clone, Debug)]
pub struct Trace {
    /// 各步的轨迹（顺序 = 结算顺序）。
    pub steps: Vec<StepTrace>,
}

impl Default for Trace {
    fn default() -> Trace {
        Trace::new()
    }
}

impl Trace {
    /// 空轨迹。
    pub fn new() -> Trace {
        Trace { steps: Vec::new() }
    }

    /// 步数。
    pub fn step_count(&self) -> Nat {
        Nat::new(self.steps.len() as _)
    }

    /// **导出为纯文本**（无外部依赖；只读、不改轨迹）。
    pub fn render(&self) -> String {
        let mut text = String::new();
        for step in &self.steps {
            text.push_str(&format!(
                "== 步 {}（{} 支分支，步内插入 {} 次）==\n",
                step.index.get(),
                step.branches.len(),
                step.intra_step_insertions.len()
            ));
            for (branch_index, branch) in step.branches.iter().enumerate() {
                text.push_str(&format!(
                    "  分支 {}（概率 {} / {}）\n",
                    branch_index,
                    branch.probability.num().get(),
                    branch.probability.den().get()
                ));
                for op in &branch.ops {
                    op.render_into(&mut text, 4);
                }
                for pending in &branch.pending {
                    text.push_str(&format!(
                        "    pending: {}（owner {}，来源 {:?}，进场 {}）\n",
                        pending.zone, pending.owner.get(), pending.source_kind, pending.entry.get()
                    ));
                }
                for reaction in &branch.reactions {
                    reaction.render_into(&mut text, 4);
                }
            }
            for insertion in &step.insertions {
                text.push_str(&format!(
                    "  步 {} 之后插入：执行 {} 条待执行\n",
                    insertion.after_step.get(),
                    insertion.executed.get()
                ));
                for pending in &insertion.pending {
                    text.push_str(&format!(
                        "    插入 pending: {}（owner {}，来源 {:?}）\n",
                        pending.zone, pending.owner.get(), pending.source_kind
                    ));
                }
            }
        }
        text
    }
}

/// 一步的轨迹。
#[derive(Clone, Debug)]
pub struct StepTrace {
    /// 步号（0 起；顺序 = 结算顺序）。
    pub index: Nat,
    /// 这一步的各结算分支。
    pub branches: Vec<BranchTrace>,
    /// **步与步之间**的插入（D204：插入只能在这里发生）。
    pub insertions: Vec<InsertionTrace>,
    /// **步内插入**（D204：正常恒为空；保留字段用来显式表示「步内没有插入」）。
    pub intra_step_insertions: Vec<InsertionTrace>,
}

/// 一步里一支结算分支的轨迹。
#[derive(Clone, Debug)]
pub struct BranchTrace {
    /// 这一支的精确概率。
    pub probability: Fraction,
    /// 步内按顺序执行的关键字操作。
    pub ops: Vec<OpTrace>,
    /// 这一步全部操作累计派发的待执行（步末统一执行）。
    pub pending: Vec<PendingTrace>,
    /// **步边界插入执行的反应子结算**（R4 / R2）：每条一条记录（执行完回到原步骤）。
    pub reactions: Vec<ReactionTrace>,
}

/// 一条**反应子结算**的轨迹（R4 / R2；步边界插入执行）。
#[derive(Clone, Debug)]
pub struct ReactionTrace {
    /// 产生顺序（本步内 0 起）。
    pub index: Nat,
    /// 反应名（例如「超载」「冻结」）。
    pub reaction: &'static str,
    /// 命中的已覆盖元素。
    pub covered: Element,
    /// 本次施加 / 攻击的元素。
    pub attack: Element,
    /// 触发方实体。
    pub source: EntityId,
    /// 承载方实体。
    pub target: EntityId,
    /// 是不是非伤害形式的附着引发的反应（真 = 结算时忽略所有伤害类效果）。
    pub non_damage: bool,
    /// 这一支的精确概率（相对本步分支）。
    pub probability: Fraction,
    /// 反应效果执行派发出来的待执行。
    pub pending: Vec<PendingTrace>,
    /// 反应效果对各区的影响（前后局面 diff；无差异则空）。
    pub outcomes: Vec<OpOutcome>,
}

impl ReactionTrace {
    fn render_into(&self, text: &mut String, indent: usize) {
        let padding = " ".repeat(indent);
        text.push_str(&format!(
            "{}反应 {}（已覆盖 {:?} + 施加 {:?}{}，概率 {} / {}）\n",
            padding,
            self.reaction,
            self.covered,
            self.attack,
            if self.non_damage { "，非伤害（忽略伤害类效果）" } else { "" },
            self.probability.num().get(),
            self.probability.den().get()
        ));
        text.push_str(&format!(
            "{}  触发方 {} -> 承载方 {}\n",
            padding,
            self.source.get(),
            self.target.get()
        ));
        for outcome in &self.outcomes {
            text.push_str(&format!("{}  结果: {:?}\n", padding, outcome));
        }
        for pending in &self.pending {
            text.push_str(&format!(
                "{}  派发: {}（owner {}，来源 {:?}）\n",
                padding,
                pending.zone,
                pending.owner.get(),
                pending.source_kind
            ));
        }
    }
}

/// 一个关键字操作的轨迹。
#[derive(Clone, Debug)]
pub struct OpTrace {
    /// 步内下标（0 起）。
    pub index: Nat,
    /// 操作种类（中文名）。
    pub kind: &'static str,
    /// 目标选择器解析出的实体列表。
    pub targets: Vec<TargetTrace>,
    /// 解析后的实际入参（名字 + 文本值）。
    pub inputs: Vec<OpInput>,
    /// 走到这一步分支的精确概率（进入本操作前累计 × 本操作分支）。
    pub probability: Fraction,
    /// 执行结果（各区发生了什么）。
    pub outcomes: Vec<OpOutcome>,
    /// 本操作派发出来的待执行（不执行，步末统一做）。
    pub pending: Vec<PendingTrace>,
    /// 重复（Repeat）内部的子操作轨迹（仍属同一步、同一操作位置）。
    pub children: Vec<OpTrace>,
}

impl OpTrace {
    fn render_into(&self, text: &mut String, indent: usize) {
        let padding = " ".repeat(indent);
        text.push_str(&format!(
            "{}{} #{}（概率 {} / {}）\n",
            padding,
            self.kind,
            self.index.get(),
            self.probability.num().get(),
            self.probability.den().get()
        ));
        if !self.targets.is_empty() {
            let targets: Vec<String> = self
                .targets
                .iter()
                .map(|target| format!("{:?}:{}", target.side, target.entity.get()))
                .collect();
            text.push_str(&format!("{}  目标: {}\n", padding, targets.join(", ")));
        }
        for input in &self.inputs {
            text.push_str(&format!("{}  {}: {}\n", padding, input.name, input.value));
        }
        for outcome in &self.outcomes {
            text.push_str(&format!("{}  结果: {:?}\n", padding, outcome));
        }
        for pending in &self.pending {
            text.push_str(&format!(
                "{}  派发: {}（owner {}，来源 {:?}）\n",
                padding,
                pending.zone,
                pending.owner.get(),
                pending.source_kind
            ));
        }
        for child in &self.children {
            child.render_into(text, indent.saturating_add(4));
        }
    }
}

/// 一个解析后的实体目标。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetTrace {
    /// 目标属于哪一侧（两个具名实例之一）。
    pub side: Perspective,
    /// 实体号。
    pub entity: EntityId,
}

/// 一条解析后的入参（名字 + 文本值；零浮点）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpInput {
    /// 入参名（中文）。
    pub name: &'static str,
    /// 文本值（数值用 Nat / 枚举用 Debug 文本）。
    pub value: String,
}

/// 一条待执行的轨迹（哪个区、挂在谁、来源类别、进场先后、事件）。
#[derive(Clone, Copy, Debug)]
pub struct PendingTrace {
    /// 区名（角色 / 角色区 / 出战状态 / 装备 / 召唤物 / 支援 / 手牌状态）。
    pub zone: &'static str,
    /// 触发挂在谁身上。
    pub owner: EntityId,
    /// 来源类别（结算顺序第 2 条）。
    pub source_kind: EffectSource,
    /// 进场先后（结算顺序第 3 条）。
    pub entry: Nat,
    /// 触发它的事件。
    pub event: Event,
}

/// 一次步间插入的轨迹。
#[derive(Clone, Debug)]
pub struct InsertionTrace {
    /// 发生在第几步之后（步号；D204）。
    pub after_step: Nat,
    /// 插入并执行的待执行。
    pub pending: Vec<PendingTrace>,
    /// 执行的条数。
    pub executed: Nat,
}

/// 一个关键字操作的**执行结果**（各区发生了什么）。
///
/// 伤害条目直接复用 damage_chain::executor::DamageEntry；无法执行复用 Skip。
#[derive(Clone, Debug)]
pub enum OpOutcome {
    /// 某角色掉血（to_hp = 前后生命差；shield_used / bonus 见模块文档待复核）。
    Damage(Vec<DamageEntry>),
    /// 某角色回血。
    Healed {
        /// 被治疗的角色。
        target: EntityId,
        /// 实际回血量。
        amount: Nat,
    },
    /// 某角色新增元素附着。
    Attached {
        /// 目标角色。
        target: EntityId,
        /// 新增的元素。
        element: Element,
    },
    /// 某处挂上状态。
    Status {
        /// 挂载位置。
        place: StatusPlace,
        /// 承载实体（角色 / 手牌实体）。
        target: EntityId,
        /// 层数（布尔状态 = None）。
        layers: Option<Nat>,
    },
    /// 生成召唤物。
    Summon {
        /// 哪一方。
        side: Perspective,
        /// 召唤物实体。
        entity: EntityId,
    },
    /// 抽到一张牌（每张一条）。
    Drawn {
        /// 哪一方。
        side: Perspective,
        /// 抽到的实体。
        entity: EntityId,
    },
    /// 弃置一个实体。
    Discarded {
        /// 哪一方。
        side: Perspective,
        /// 被弃置的实体。
        entity: EntityId,
        /// 在哪个区（角色装备 / 手牌 / 召唤物 / 支援）。
        zone: &'static str,
    },
    /// 骰子数量变化（获得 / 转化）。
    Dice {
        /// 哪一方。
        side: Perspective,
        /// 变化的数量。
        count: Nat,
    },
    /// 生成一张卡加入手牌。
    CardAdded {
        /// 哪一方。
        side: Perspective,
        /// 新牌实体。
        entity: EntityId,
    },
    /// 给手牌 / 牌组里的卡赋予状态。
    CardStatus {
        /// 哪一方。
        side: Perspective,
        /// 被赋予的卡实体。
        entity: EntityId,
        /// 在牌组里（false = 在手牌里）。
        on_pile: bool,
    },
    /// 切换出战角色。
    Switched {
        /// 哪一方。
        side: Perspective,
        /// 原出战角色。
        from: EntityId,
        /// 新出战角色。
        to: EntityId,
    },
    /// 消耗充能。
    EnergyConsumed {
        /// 目标角色。
        target: EntityId,
        /// 实际消耗量。
        amount: Nat,
    },
    /// 手牌置底。
    HandToPile {
        /// 哪一方。
        side: Perspective,
        /// 移动的张数。
        moved: Nat,
    },
    /// 重复包装（子动作见 OpTrace::children）。
    Repeated {
        /// 执行次数。
        count: Nat,
    },
    /// 无法执行（跳过）。
    Skipped(Skip),
}

// ======================================================================
// 对外入口
// ======================================================================

/// traced 一步的结果：结算结果 + 轨迹。
#[derive(Clone, Debug)]
pub struct TracedStep {
    /// 与 [super::step::execute_step_me] 同口径的结算结果。
    pub outcome: StepOutcome,
    /// 回放轨迹。
    pub trace: Trace,
}

/// **执行一步（我方视角）并记录轨迹**（D221）。
pub fn execute_step_me_traced(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> TracedStep {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_traced_step_entry(state, step, &scoped)
}

/// **执行一步（敌方视角）并记录轨迹**。
pub fn execute_step_opp_traced(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> TracedStep {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Opp);
    run_traced_step_entry(state, step, &scoped)
}

/// **顺序执行多步（我方视角）并记录轨迹**：步间的插入记在 StepTrace::insertions。
pub fn execute_steps_me_traced(
    state: &DefState,
    steps: &[Step],
    env: &EffectEnv<'_>,
) -> TracedStep {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_traced_sequence(state, steps, &scoped)
}

/// **顺序执行多步（敌方视角）并记录轨迹**。
pub fn execute_steps_opp_traced(
    state: &DefState,
    steps: &[Step],
    env: &EffectEnv<'_>,
) -> TracedStep {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Opp);
    run_traced_sequence(state, steps, &scoped)
}

// ======================================================================
// 内部：带轨迹的步执行
// ======================================================================

/// 步内工作分支。
struct Work {
    state: DefState,
    probability: Fraction,
    pending: PendingSet,
    /// 本步已判定、待步边界插入执行的反应子结算（D204）。
    reactions: Vec<ReactionRequest>,
    alloc: EntityAlloc,
    /// 步内每个关键字操作的轨迹。
    traces: Vec<OpTrace>,
    /// 步边界插入执行后的反应轨迹（R4 / R2）。
    reaction_traces: Vec<ReactionTrace>,
}

/// 一次操作在某支局面上的执行 + 轨迹。
struct OpRun {
    state: DefState,
    branch_probability: Fraction,
    pending: PendingSet,
    /// 本操作触发的反应子结算（待步边界执行）。
    reactions: Vec<ReactionRequest>,
    alloc: EntityAlloc,
    trace: OpTrace,
}

fn run_traced_step_entry(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> TracedStep {
    let work = run_traced_step(state, step, env);
    let mut alloc = env.alloc();
    let mut branches = Vec::new();
    for branch in &work {
        if branch.alloc.next() > alloc.next() {
            alloc = EntityAlloc::starting_at(branch.alloc.next());
        }
    }
    let mut branch_traces = Vec::new();
    for branch in work {
        branch_traces.push(BranchTrace {
            probability: branch.probability,
            ops: branch.traces,
            pending: pending_traces(&branch.pending),
            reactions: branch.reaction_traces,
        });
        branches.push(super::op::KeywordBranch::new(
            branch.state,
            branch.probability,
            branch.pending,
        ));
    }
    TracedStep {
        outcome: StepOutcome {
            branches,
            alloc: EntityAlloc::starting_at(alloc.next()),
        },
        trace: Trace {
            steps: vec![StepTrace {
                index: Nat::ZERO,
                branches: branch_traces,
                insertions: Vec::new(),
                intra_step_insertions: Vec::new(),
            }],
        },
    }
}

/// 执行一步（步内不插入、不执行待执行，D204）。
fn run_traced_step(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> Vec<Work> {
    let env = env
        .fork(env.alloc())
        .with_step_start_hands(state.me().hand().len(), state.opp().hand().len());
    let mut work = vec![Work {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: PendingSet::new(),
        reactions: Vec::new(),
        alloc: env.alloc(),
        traces: Vec::new(),
        reaction_traces: Vec::new(),
    }];
    for (index, op) in step.ops().iter().enumerate() {
        let mut next: Vec<Work> = Vec::new();
        for branch in work {
            let runs = trace_op(
                &branch.state,
                &env.fork(branch.alloc),
                op,
                Nat::new(index as _),
                branch.probability,
            );
            for run in runs {
                let mut traces = branch.traces.clone();
                traces.push(run.trace);
                let mut pending = branch.pending.clone();
                for item in run.pending.me {
                    pending.push_me(item);
                }
                for item in run.pending.opp {
                    pending.push_opp(item);
                }
                let mut reactions = branch.reactions.clone();
                reactions.extend(run.reactions.iter().copied());
                next.push(Work {
                    state: run.state,
                    probability: branch.probability.exact_mul(run.branch_probability),
                    pending,
                    reactions,
                    alloc: run.alloc,
                    traces,
                    reaction_traces: Vec::new(),
                });
            }
        }
        work = next;
    }
    // 步边界：把本步判定出的反应作为插入式子结算执行（R4 / R2；D204 步内不插）。
    settle_step_reactions(work, &env)
}

/// 反应子结算的工作分支（步边界插入执行）。
struct ReactionWork {
    state: DefState,
    probability: Fraction,
    pending: PendingSet,
    alloc: EntityAlloc,
    reactions: Vec<ReactionTrace>,
}

/// **步边界插入执行反应子结算**（与 step::run_step 同口径；D204 步内不插）。
fn settle_step_reactions(work: Vec<Work>, env: &EffectEnv<'_>) -> Vec<Work> {
    let mut settled = Vec::new();
    for branch in work {
        for outcome in settle_traced_reactions(&branch.state, &branch.reactions, env) {
            let probability = branch.probability.exact_mul(outcome.probability);
            // 反应效果派发的待执行接在本步待执行之前（结算顺序.md §三 的元素伤害段落）。
            let mut pending = outcome.pending;
            append_pending(&mut pending, branch.pending.clone());
            settled.push(Work {
                state: outcome.state,
                probability,
                pending,
                reactions: Vec::new(),
                alloc: outcome.alloc,
                traces: branch.traces.clone(),
                reaction_traces: outcome.reactions,
            });
        }
    }
    settled
}

/// 按顺序结算反应请求并逐条记录（每条可能展开成多支；概率精确相乘）。
fn settle_traced_reactions(
    state: &DefState,
    requests: &[ReactionRequest],
    env: &EffectEnv<'_>,
) -> Vec<ReactionWork> {
    let mut work = vec![ReactionWork {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: PendingSet::new(),
        alloc: env.alloc(),
        reactions: Vec::new(),
    }];
    for (order, request) in requests.iter().enumerate() {
        let mut next: Vec<ReactionWork> = Vec::new();
        for branch in work {
            let before = Snapshot::of(&branch.state);
            let settled = settle_inserted(&branch.state, &[*request], &env.fork(branch.alloc));
            for settled_branch in settled.branches {
                let after = Snapshot::of(&settled_branch.state);
                let probability = branch.probability.exact_mul(settled_branch.probability);
                let mut pending = settled_branch.pending.clone();
                append_pending(&mut pending, branch.pending.clone());
                let mut reactions = branch.reactions.clone();
                reactions.push(ReactionTrace {
                    index: Nat::new(order as _),
                    reaction: request.hit.reaction.name(),
                    covered: request.hit.covered,
                    attack: request.hit.attack,
                    source: request.hit.source,
                    target: request.hit.target,
                    non_damage: request.non_damage,
                    probability,
                    pending: pending_traces(&settled_branch.pending),
                    outcomes: reaction_outcomes(&before, &after),
                });
                next.push(ReactionWork {
                    state: settled_branch.state,
                    probability,
                    pending,
                    alloc: settled.alloc,
                    reactions,
                });
            }
        }
        work = next;
    }
    work
}

/// 追加待执行（保持顺序：前面的先执行）。
fn append_pending(into: &mut PendingSet, from: PendingSet) {
    for item in from.me {
        into.push_me(item);
    }
    for item in from.opp {
        into.push_opp(item);
    }
}

/// 反应效果对各区的影响（前后 diff；复用操作结果的既有形状）。
fn reaction_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    outcomes.extend(damage_outcomes(before, after));
    outcomes.extend(heal_outcomes(before, after));
    outcomes.extend(switch_outcomes(before, after));
    outcomes.extend(energy_outcomes(before, after));
    outcomes.retain(|outcome| !matches!(outcome, OpOutcome::Skipped(_)));
    outcomes
}

/// 执行一条操作并逐分支生成轨迹（递归处理 Repeat）。
fn trace_op(
    state: &DefState,
    env: &EffectEnv<'_>,
    op: &KeywordOp,
    index: Nat,
    entering: Fraction,
) -> Vec<OpRun> {
    if let KeywordOp::Repeat(args) = op {
        return trace_repeat(state, env, args, index, entering);
    }
    let before = Snapshot::of(state);
    let targets = resolved_targets(state, env, op);
    let inputs = inputs_of(op);
    let kind = kind_of(op);
    let result = execute_op(state, env, op);
    let alloc = result.alloc;
    let mut runs = Vec::new();
    for branch in result.branches {
        let after = Snapshot::of(&branch.state);
        let outcomes = outcomes_of(op, env, &targets, &before, &after);
        let pending = pending_traces(&branch.pending);
        let reactions = branch.reactions.clone();
        runs.push(OpRun {
            state: branch.state,
            branch_probability: branch.probability,
            pending: branch.pending,
            reactions,
            alloc,
            trace: OpTrace {
                index,
                kind,
                targets: targets.clone(),
                inputs: inputs.clone(),
                probability: entering.exact_mul(branch.probability),
                outcomes,
                pending,
                children: Vec::new(),
            },
        });
    }
    runs
}

/// 重复包装：子动作逐次执行、挂在 children 里；仍属同一步。
fn trace_repeat(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &RepeatArgs,
    index: Nat,
    entering: Fraction,
) -> Vec<OpRun> {
    let mut work: Vec<Work> = vec![Work {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: PendingSet::new(),
        reactions: Vec::new(),
        alloc: env.alloc(),
        traces: Vec::new(),
        reaction_traces: Vec::new(),
    }];
    let mut iteration = Nat::ZERO;
    while iteration < args.count {
        let ops = (args.plan)(iteration);
        for inner in &ops {
            let mut next: Vec<Work> = Vec::new();
            for branch in work {
                let runs = trace_op(
                    &branch.state,
                    &env.fork(branch.alloc),
                    inner,
                    iteration,
                    Fraction::ONE,
                );
                for run in runs {
                    let mut children = branch.traces.clone();
                    children.push(run.trace);
                    let mut pending = branch.pending.clone();
                    for item in run.pending.me {
                        pending.push_me(item);
                    }
                    for item in run.pending.opp {
                        pending.push_opp(item);
                    }
                    let mut reactions = branch.reactions.clone();
                    reactions.extend(run.reactions.iter().copied());
                    next.push(Work {
                        state: run.state,
                        probability: branch.probability.exact_mul(run.branch_probability),
                        pending,
                        reactions,
                        alloc: run.alloc,
                        traces: children,
                        reaction_traces: Vec::new(),
                    });
                }
            }
            work = next;
        }
        iteration = nadd!(iteration, Nat::new(1));
    }
    work.into_iter()
        .map(|branch| OpRun {
            state: branch.state,
            branch_probability: branch.probability,
            pending: branch.pending,
            reactions: branch.reactions,
            alloc: branch.alloc,
            trace: OpTrace {
                index,
                kind: "重复",
                targets: Vec::new(),
                inputs: vec![OpInput {
                    name: "次数",
                    value: args.count.get().to_string(),
                }],
                probability: entering.exact_mul(branch.probability),
                outcomes: vec![OpOutcome::Repeated { count: args.count }],
                pending: Vec::new(),
                children: branch.traces,
            },
        })
        .collect()
}

/// 顺序执行多步并记录轨迹（步间插入 = 执行上一步的待执行）。
fn run_traced_sequence(state: &DefState, steps: &[Step], env: &EffectEnv<'_>) -> TracedStep {
    let mut work: Vec<Work> = vec![Work {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: PendingSet::new(),
        reactions: Vec::new(),
        alloc: env.alloc(),
        traces: Vec::new(),
        reaction_traces: Vec::new(),
    }];
    let mut step_traces: Vec<StepTrace> = Vec::new();
    for (step_index, step) in steps.iter().enumerate() {
        let mut next: Vec<Work> = Vec::new();
        let mut branches: Vec<BranchTrace> = Vec::new();
        let mut insertions: Vec<InsertionTrace> = Vec::new();
        for branch in work {
            let stepped = run_traced_step(&branch.state, step, &env.fork(branch.alloc));
            for run in stepped {
                let probability = branch.probability.exact_mul(run.probability);
                branches.push(BranchTrace {
                    probability,
                    ops: run.traces,
                    pending: pending_traces(&run.pending),
                    reactions: run.reaction_traces,
                });
                let insertion = InsertionTrace {
                    after_step: Nat::new(step_index as _),
                    pending: pending_traces(&run.pending),
                    executed: run.pending.len(),
                };
                let flushed = execute_pending(&run.state, &run.pending);
                insertions.push(insertion);
                next.push(Work {
                    state: flushed,
                    probability,
                    pending: PendingSet::new(),
                    reactions: Vec::new(),
                    alloc: run.alloc,
                    traces: Vec::new(),
                    reaction_traces: Vec::new(),
                });
            }
        }
        step_traces.push(StepTrace {
            index: Nat::new(step_index as _),
            branches,
            insertions,
            intra_step_insertions: Vec::new(),
        });
        work = next;
    }
    let mut alloc = env.alloc();
    for branch in &work {
        if branch.alloc.next() > alloc.next() {
            alloc = EntityAlloc::starting_at(branch.alloc.next());
        }
    }
    let mut branches = Vec::new();
    for branch in work {
        branches.push(super::op::KeywordBranch::new(
            branch.state,
            branch.probability,
            branch.pending,
        ));
    }
    TracedStep {
        outcome: StepOutcome {
            branches,
            alloc: EntityAlloc::starting_at(alloc.next()),
        },
        trace: Trace { steps: step_traces },
    }
}

// ======================================================================
// 观测：入参 / 目标 / 结果
// ======================================================================

/// 操作种类（中文名）。
fn kind_of(op: &KeywordOp) -> &'static str {
    match op {
        KeywordOp::Damage(_) => "造成伤害",
        KeywordOp::Heal(_) => "治疗",
        KeywordOp::AttachElement(_) => "附着元素",
        KeywordOp::Status(_) => "施加状态",
        KeywordOp::Summon(_) => "召唤",
        KeywordOp::DrawCards(_) => "抽牌",
        KeywordOp::Discard(_) => "弃置",
        KeywordOp::GainDice(_) => "获得骰子",
        KeywordOp::ConvertDice(_) => "转化骰",
        KeywordOp::CreateCard(_) => "生成卡入手",
        KeywordOp::SwitchCharacter(_) => "切换角色",
        KeywordOp::ForceSwitch(_) => "强制切换",
        KeywordOp::ConsumeEnergy(_) => "消耗充能",
        KeywordOp::CardStatus(_) => "赋予卡牌状态",
        KeywordOp::PutHandToPileBottom(_) => "手牌置底",
        KeywordOp::Repeat(_) => "重复",
    }
}

/// 操作的目标选择器（没有目标的返回 None）。
fn selector_of(op: &KeywordOp) -> Option<TargetSelector> {
    match op {
        KeywordOp::Damage(args) => Some(args.targets),
        KeywordOp::Heal(args) => Some(args.targets),
        KeywordOp::AttachElement(args) => Some(args.targets),
        KeywordOp::Status(args) => Some(args.targets),
        KeywordOp::Discard(args) => Some(args.targets),
        KeywordOp::SwitchCharacter(args) => Some(args.to),
        KeywordOp::ForceSwitch(args) => Some(args.targets),
        KeywordOp::ConsumeEnergy(args) => Some(args.targets),
        KeywordOp::CardStatus(args) => Some(args.targets),
        KeywordOp::Summon(_)
        | KeywordOp::DrawCards(_)
        | KeywordOp::GainDice(_)
        | KeywordOp::ConvertDice(_)
        | KeywordOp::CreateCard(_)
        | KeywordOp::PutHandToPileBottom(_)
        | KeywordOp::Repeat(_) => None,
    }
}

/// 解析目标选择器（只读；不采样）。
fn resolved_targets(state: &DefState, env: &EffectEnv<'_>, op: &KeywordOp) -> Vec<TargetTrace> {
    let Some(selector) = selector_of(op) else {
        return Vec::new();
    };
    let targets = resolve(state, selector, &env.resolve_context());
    targets
        .iter()
        .map(|(side, entity)| TargetTrace { side, entity })
        .collect()
}

/// 解析后的实际入参（文本；零浮点）。
fn inputs_of(op: &KeywordOp) -> Vec<OpInput> {
    match op {
        KeywordOp::Damage(DamageArgs {
            amount,
            ty,
            source_kind,
            pick,
            ..
        }) => vec![
            OpInput {
                name: "数值",
                value: amount.get().to_string(),
            },
            OpInput {
                name: "类型",
                value: damage_type_text(*ty),
            },
            OpInput {
                name: "来源",
                value: format!("{source_kind:?}"),
            },
            OpInput {
                name: "选取",
                value: format!("{pick:?}"),
            },
        ],
        KeywordOp::Heal(HealArgs { amount, .. }) => vec![OpInput {
            name: "数值",
            value: amount.get().to_string(),
        }],
        KeywordOp::AttachElement(args) => vec![OpInput {
            name: "元素",
            value: format!("{:?}", args.element),
        }],
        KeywordOp::Status(StatusArgs {
            place,
            card,
            layers,
            duration,
            stack_cap,
            ..
        }) => vec![
            OpInput {
                name: "位置",
                value: format!("{place:?}"),
            },
            OpInput {
                name: "状态卡",
                value: card.0.get().to_string(),
            },
            OpInput {
                name: "层数",
                value: optional_nat_text(*layers),
            },
            OpInput {
                name: "持续回合",
                value: optional_nat_text(*duration),
            },
            OpInput {
                name: "叠加上限",
                value: optional_nat_text(*stack_cap),
            },
        ],
        KeywordOp::Summon(SummonArgs {
            card,
            uses,
            stack_limit,
            ..
        }) => vec![
            OpInput {
                name: "召唤物卡",
                value: card.0.get().to_string(),
            },
            OpInput {
                name: "可用次数",
                value: optional_nat_text(*uses),
            },
            OpInput {
                name: "叠加上限",
                value: optional_nat_text(*stack_limit),
            },
        ],
        KeywordOp::DrawCards(DrawCardsArgs { count, .. }) => vec![OpInput {
            name: "张数",
            value: format!("{count:?}"),
        }],
        KeywordOp::GainDice(GainDiceArgs { face, count, .. }) => vec![
            OpInput {
                name: "骰面",
                value: format!("{face:?}"),
            },
            OpInput {
                name: "数量",
                value: count.get().to_string(),
            },
        ],
        KeywordOp::ConvertDice(ConvertDiceArgs {
            source,
            count,
            target,
            ..
        }) => vec![
            OpInput {
                name: "来源骰",
                value: format!("{source:?}"),
            },
            OpInput {
                name: "目标骰",
                value: format!("{target:?}"),
            },
            OpInput {
                name: "数量",
                value: count.get().to_string(),
            },
        ],
        KeywordOp::CreateCard(CreateCardArgs { card, .. }) => vec![OpInput {
            name: "卡牌",
            value: card.0.get().to_string(),
        }],
        KeywordOp::ConsumeEnergy(ConsumeEnergyArgs { amount, .. }) => vec![OpInput {
            name: "充能",
            value: match amount {
                EnergyAmount::All => "全部".to_string(),
                EnergyAmount::Fixed(value) => value.get().to_string(),
            },
        }],
        KeywordOp::CardStatus(CardStatusArgs { status, pick, .. }) => vec![
            OpInput {
                name: "卡牌状态",
                value: format!("{status:?}"),
            },
            OpInput {
                name: "选取",
                value: format!("{pick:?}"),
            },
        ],
        KeywordOp::Discard(DiscardArgs { .. }) => Vec::new(),
        KeywordOp::SwitchCharacter(SwitchCharacterArgs { .. }) => Vec::new(),
        KeywordOp::ForceSwitch(ForceSwitchArgs { .. }) => Vec::new(),
        KeywordOp::PutHandToPileBottom(args) => vec![
            OpInput {
                name: "一方",
                value: format!("{:?}", args.side),
            },
            OpInput {
                name: "顺序函数",
                value: format!("0x{:x}", args.order as usize),
            },
        ],
        KeywordOp::Repeat(args) => vec![OpInput {
            name: "次数",
            value: args.count.get().to_string(),
        }],
    }
}

fn optional_nat_text(value: Option<Nat>) -> String {
    match value {
        Some(value) => value.get().to_string(),
        None => "无".to_string(),
    }
}

fn damage_type_text(ty: DamageType) -> String {
    match ty {
        DamageType::Elemental(element) => format!("元素({element:?})"),
        DamageType::Physical => "物理".to_string(),
        DamageType::Piercing => "穿透".to_string(),
    }
}

/// 结果推导：前后局面 diff（见模块文档待复核）。
fn outcomes_of(
    op: &KeywordOp,
    env: &EffectEnv<'_>,
    targets: &[TargetTrace],
    before: &Snapshot,
    after: &Snapshot,
) -> Vec<OpOutcome> {
    match op {
        KeywordOp::Damage(_) => damage_outcomes(before, after),
        KeywordOp::Heal(_) => heal_outcomes(before, after),
        KeywordOp::AttachElement(args) => attach_outcomes(before, after, args.element),
        KeywordOp::Status(args) => status_outcomes(args, targets),
        KeywordOp::Summon(_) => summon_outcomes(before, after),
        KeywordOp::DrawCards(_) => drawn_outcomes(before, after),
        KeywordOp::Discard(_) => discard_outcomes(before, after),
        KeywordOp::GainDice(args) => dice_outcomes(env, args.side, before, after, args.count),
        KeywordOp::ConvertDice(args) => dice_outcomes(env, args.side, before, after, args.count),
        KeywordOp::CreateCard(_) => added_outcomes(before, after),
        KeywordOp::SwitchCharacter(_) | KeywordOp::ForceSwitch(_) => switch_outcomes(before, after),
        KeywordOp::ConsumeEnergy(_) => energy_outcomes(before, after),
        KeywordOp::CardStatus(_) => card_status_outcomes(targets, before, after),
        KeywordOp::PutHandToPileBottom(_) => hand_to_pile_outcomes(before, after),
        KeywordOp::Repeat(args) => vec![OpOutcome::Repeated { count: args.count }],
    }
}

fn damage_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        let before_side = before.side(side);
        let after_side = after.side(side);
        for slot in &before_side.characters {
            let Some(after_slot) = after_side.character(slot.entity) else {
                continue;
            };
            if slot.hp > after_slot.hp {
                outcomes.push(OpOutcome::Damage(vec![DamageEntry {
                    target: slot.entity,
                    to_hp: nsub!(slot.hp, after_slot.hp),
                    shield_used: Nat::ZERO,
                    bonus: Nat::ZERO,
                    reacted: false,
                }]));
            }
        }
    }
    outcomes
}

fn heal_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        for slot in &before.side(side).characters {
            let Some(after_slot) = after.side(side).character(slot.entity) else {
                continue;
            };
            if after_slot.hp > slot.hp {
                outcomes.push(OpOutcome::Healed {
                    target: slot.entity,
                    amount: nsub!(after_slot.hp, slot.hp),
                });
            }
        }
    }
    outcomes
}

fn attach_outcomes(before: &Snapshot, after: &Snapshot, element: Element) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        for slot in &before.side(side).characters {
            let Some(after_slot) = after.side(side).character(slot.entity) else {
                continue;
            };
            let had = slot.attachment.contains(&element);
            let has = after_slot.attachment.contains(&element);
            if !had && has {
                outcomes.push(OpOutcome::Attached {
                    target: slot.entity,
                    element,
                });
            }
        }
    }
    outcomes
}

fn status_outcomes(args: &StatusArgs, targets: &[TargetTrace]) -> Vec<OpOutcome> {
    targets
        .iter()
        .map(|target| OpOutcome::Status {
            place: args.place,
            target: target.entity,
            layers: args.layers,
        })
        .collect()
}

fn summon_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        for entry in after.side(side).summons.iter() {
            if !before.side(side).summons.iter().any(|old| old == entry) {
                outcomes.push(OpOutcome::Summon { side, entity: *entry });
            }
        }
    }
    outcomes
}

fn drawn_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        for entity in after.side(side).hand.iter() {
            if !before.side(side).hand.contains(entity) {
                outcomes.push(OpOutcome::Drawn {
                    side,
                    entity: *entity,
                });
            }
        }
    }
    outcomes
}

fn discard_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        let before_side = before.side(side);
        let after_side = after.side(side);
        for entity in removed(&before_side.equipment, &after_side.equipment) {
            outcomes.push(OpOutcome::Discarded {
                side,
                entity,
                zone: "角色装备",
            });
        }
        for entity in removed(&before_side.hand, &after_side.hand) {
            outcomes.push(OpOutcome::Discarded {
                side,
                entity,
                zone: "手牌",
            });
        }
        for entity in removed(&before_side.summons, &after_side.summons) {
            outcomes.push(OpOutcome::Discarded {
                side,
                entity,
                zone: "召唤物",
            });
        }
        for entity in removed(&before_side.supports, &after_side.supports) {
            outcomes.push(OpOutcome::Discarded {
                side,
                entity,
                zone: "支援",
            });
        }
    }
    outcomes
}

fn removed(before: &[EntityId], after: &[EntityId]) -> Vec<EntityId> {
    before
        .iter()
        .copied()
        .filter(|entity| !after.contains(entity))
        .collect()
}

/// 骰子变化：按操作影响的一侧记录（获得 = 总数差；转化 = 操作数量）。
fn dice_outcomes(
    env: &EffectEnv<'_>,
    scope: SideScope,
    before: &Snapshot,
    after: &Snapshot,
    fallback: Nat,
) -> Vec<OpOutcome> {
    let (sides, count) = super::target::sides_of(scope, env.viewer());
    let mut outcomes = Vec::new();
    for side in sides.iter().take(count) {
        let before_dice = before.side(*side).dice;
        let after_dice = after.side(*side).dice;
        let delta = if after_dice > before_dice {
            nsub!(after_dice, before_dice)
        } else {
            fallback
        };
        outcomes.push(OpOutcome::Dice {
            side: *side,
            count: delta,
        });
    }
    outcomes
}

fn added_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        for entity in after.side(side).hand.iter() {
            if !before.side(side).hand.contains(entity) {
                outcomes.push(OpOutcome::CardAdded {
                    side,
                    entity: *entity,
                });
            }
        }
    }
    outcomes
}

fn switch_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        let before_active = before.side(side).active;
        let after_active = after.side(side).active;
        if let (Some(from), Some(to)) = (before_active, after_active) {
            if from != to {
                outcomes.push(OpOutcome::Switched { side, from, to });
            }
        }
    }
    outcomes
}

fn energy_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        for slot in &before.side(side).characters {
            let Some(after_slot) = after.side(side).character(slot.entity) else {
                continue;
            };
            if slot.energy > after_slot.energy {
                outcomes.push(OpOutcome::EnergyConsumed {
                    target: slot.entity,
                    amount: nsub!(slot.energy, after_slot.energy),
                });
            }
        }
    }
    outcomes
}

fn card_status_outcomes(
    targets: &[TargetTrace],
    before: &Snapshot,
    after: &Snapshot,
) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for target in targets {
        let before_side = before.side(target.side);
        let after_side = after.side(target.side);
        if let Some(before_state) = before_side.hand_state(target.entity) {
            let after_state = after_side
                .hand_state(target.entity)
                .map(|entry| entry.1)
                .unwrap_or(Nat::ZERO);
            if after_state > before_state.1 {
                outcomes.push(OpOutcome::CardStatus {
                    side: target.side,
                    entity: target.entity,
                    on_pile: false,
                });
            }
        }
        if let Some(before_marks) = before_side.pile_mark(target.entity) {
            let after_marks = after_side
                .pile_mark(target.entity)
                .map(|entry| entry.1)
                .unwrap_or(Nat::ZERO);
            if after_marks > before_marks.1 {
                outcomes.push(OpOutcome::CardStatus {
                    side: target.side,
                    entity: target.entity,
                    on_pile: true,
                });
            }
        }
    }
    outcomes
}

fn hand_to_pile_outcomes(before: &Snapshot, after: &Snapshot) -> Vec<OpOutcome> {
    let mut outcomes = Vec::new();
    for side in [Perspective::Me, Perspective::Opp] {
        let before_len = Nat::new(before.side(side).hand.len() as _);
        let after_len = Nat::new(after.side(side).hand.len() as _);
        if before_len > after_len {
            outcomes.push(OpOutcome::HandToPile {
                side,
                moved: nsub!(before_len, after_len),
            });
        }
    }
    outcomes
}

// ======================================================================
// 待执行轨迹
// ======================================================================

fn pending_traces(pending: &PendingSet) -> Vec<PendingTrace> {
    let mut traces = Vec::new();
    for item in &pending.me {
        traces.push(flow_pending_trace(item));
    }
    for item in &pending.opp {
        traces.push(flow_pending_trace(item));
    }
    traces
}

fn flow_pending_trace(item: &FlowPending) -> PendingTrace {
    match item {
        FlowPending::Character { pending, .. } => PendingTrace {
            zone: "角色",
            owner: pending.owner,
            source_kind: pending.source_kind,
            entry: pending.entry,
            event: pending.event,
        },
        FlowPending::CharacterZone(pending) => PendingTrace {
            zone: "角色区",
            owner: pending.owner,
            source_kind: pending.source_kind,
            entry: pending.entry,
            event: pending.event,
        },
        FlowPending::CombatStatus(pending) => PendingTrace {
            zone: "出战状态",
            owner: pending.owner,
            source_kind: pending.source_kind,
            entry: pending.entry,
            event: pending.event,
        },
        FlowPending::Equipment { pending, .. } => PendingTrace {
            zone: "角色装备",
            owner: pending.owner,
            source_kind: pending.source_kind,
            entry: pending.entry,
            event: pending.event,
        },
        FlowPending::Summon(pending) => PendingTrace {
            zone: "召唤物",
            owner: pending.owner,
            source_kind: pending.source_kind,
            entry: pending.entry,
            event: pending.event,
        },
        FlowPending::Support(pending) => PendingTrace {
            zone: "支援",
            owner: pending.owner,
            source_kind: pending.source_kind,
            entry: pending.entry,
            event: pending.event,
        },
        FlowPending::HandCardState(pending) => PendingTrace {
            zone: "手牌状态",
            owner: pending.owner,
            source_kind: pending.source_kind,
            entry: pending.entry,
            event: pending.event,
        },
    }
}

// ======================================================================
// 局面快照（只读；diff 用）
// ======================================================================

/// 整局快照（双方各一份；只读）。
struct Snapshot {
    me: SideSnapshot,
    opp: SideSnapshot,
}

impl Snapshot {
    fn of(state: &DefState) -> Snapshot {
        Snapshot {
            me: SideSnapshot::of(state.me()),
            opp: SideSnapshot::of(state.opp()),
        }
    }

    fn side(&self, side: Perspective) -> &SideSnapshot {
        match side {
            Perspective::Me => &self.me,
            Perspective::Opp => &self.opp,
        }
    }
}

/// 一方的快照。
struct SideSnapshot {
    characters: Vec<CharacterSnapshot>,
    dice: Nat,
    hand: Vec<EntityId>,
    hand_states: Vec<(EntityId, Nat)>,
    equipment: Vec<EntityId>,
    summons: Vec<EntityId>,
    supports: Vec<EntityId>,
    pile_marks: Vec<(EntityId, Nat)>,
    active: Option<EntityId>,
}

impl SideSnapshot {
    fn of(player: &crate::state::PlayerDefs) -> SideSnapshot {
        SideSnapshot {
            characters: player
                .characters()
                .slots()
                .iter()
                .map(|slot| CharacterSnapshot {
                    entity: slot.character,
                    hp: slot.state.hp(),
                    energy: slot.state.energy(),
                    attachment: slot.state.attachment.elements(),
                })
                .collect(),
            dice: player.dice().total(),
            hand: player
                .hand()
                .entries()
                .iter()
                .map(|instance| instance.entity)
                .collect(),
            hand_states: player
                .hand_states()
                .entries()
                .iter()
                .map(|entry| (entry.entity, entry.states.len()))
                .collect(),
            equipment: player
                .equipment()
                .iter()
                .flat_map(|zone| zone.entries().iter().map(|entry| entry.equip))
                .collect(),
            summons: player
                .summons()
                .entries()
                .iter()
                .map(|entry| entry.summon)
                .collect(),
            supports: player
                .supports()
                .entries()
                .iter()
                .map(|entry| entry.entity)
                .collect(),
            pile_marks: player
                .pile()
                .entries()
                .iter()
                .filter_map(|entry| {
                    let position = player.pile().position_of(entry.entity)?;
                    let marks = player.pile().marks_at(position).len();
                    if marks == 0 {
                        None
                    } else {
                        Some((entry.entity, Nat::new(marks as _)))
                    }
                })
                .collect(),
            active: player.characters().active_entity(),
        }
    }

    fn character(&self, entity: EntityId) -> Option<&CharacterSnapshot> {
        self.characters.iter().find(|slot| slot.entity == entity)
    }

    fn hand_state(&self, entity: EntityId) -> Option<(EntityId, Nat)> {
        self.hand_states
            .iter()
            .copied()
            .find(|entry| entry.0 == entity)
    }

    fn pile_mark(&self, entity: EntityId) -> Option<(EntityId, Nat)> {
        self.pile_marks
            .iter()
            .copied()
            .find(|entry| entry.0 == entity)
    }
}

/// 一个角色的快照。
struct CharacterSnapshot {
    entity: EntityId,
    hp: Nat,
    energy: Nat,
    attachment: Vec<Element>,
}

// ======================================================================
// 测试（D221）
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::families::character::attachment::{
        Attachment, AttachmentAllowSet, ReactionRegistry,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::{DamageEnv, ReactionRule};
    use crate::reactions::effect::EffectContext;
    use crate::families::character::trigger::{CharacterEffect, CharacterTiming, CharacterTriggers};
    use crate::families::character::types::DamageType;
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::keywords::effect::TriggerBook;
    use crate::keywords::op::{HealArgs, KeywordOp, RepeatArgs};
    use crate::keywords::step::Step;
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

    /// 带反应规则 / 局面摘要的上下文（R4 测试用；允许附着 = 标准集合）。
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

    fn damage_op(targets: TargetSelector, amount: Nat) -> KeywordOp {
        KeywordOp::Damage(super::super::op::DamageArgs::new(
            targets,
            amount,
            DamageType::Physical,
            super::super::op::DamageSourceKind::Other,
        ))
    }

    fn duel_with_me_hp(me_hp: u32) -> DefState {
        let me = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(
                n(101),
                CharacterState::new(n(me_hp), n(10), Nat::ZERO, n(2)),
            )])
            .expect("合法角色区"),
        );
        DefState::from_players(me, PlayerDefs::new().with_characters(characters(&[301])))
    }

    fn hp_of(state: &DefState, side: Perspective, character: EntityId) -> Nat {
        super::super::target::player_of(state, side)
            .characters()
            .slot_of(character)
            .expect("角色在区里")
            .state
            .hp()
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

    #[test]
    fn trace_records_ops_in_order_with_resolved_inputs() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel_with_me_hp(4);
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![
            damage_op(TargetSelector::opponent_active(), n(3)),
            KeywordOp::Heal(HealArgs::new(TargetSelector::Subject, n(2))),
        ]);
        let traced = execute_step_me_traced(&state, &step, &effect);
        // 结算结果与正常执行一致。
        assert_eq!(traced.outcome.branches.len(), 1);
        assert_eq!(traced.trace.steps.len(), 1);
        let step_trace = &traced.trace.steps[0];
        assert_eq!(step_trace.index, Nat::ZERO);
        assert!(step_trace.intra_step_insertions.is_empty(), "D204：步内无插入");
        assert_eq!(step_trace.branches.len(), 1);
        let branch = &step_trace.branches[0];
        assert_eq!(branch.ops.len(), 2);
        // 第一条：造成伤害（目标解析正确、入参正确、结果正确）。
        let damage = &branch.ops[0];
        assert_eq!(damage.index, Nat::ZERO);
        assert_eq!(damage.kind, "造成伤害");
        assert_eq!(
            damage.targets,
            vec![TargetTrace {
                side: Perspective::Opp,
                entity: n(301)
            }]
        );
        assert!(damage
            .inputs
            .iter()
            .any(|input| input.name == "数值" && input.value == "3"));
        assert!(damage
            .inputs
            .iter()
            .any(|input| input.name == "类型" && input.value == "物理"));
        assert!(matches!(damage.outcomes[0], OpOutcome::Damage(_)));
        // 第二条：治疗自己（低血 -> 回血 2）。
        let healed = &branch.ops[1];
        assert_eq!(healed.kind, "治疗");
        assert!(matches!(
            healed.outcomes[0],
            OpOutcome::Healed {
                target,
                amount
            } if target == n(101) && amount == n(2)
        ));
        // 入参局面一个字节都没改。
        assert_eq!(hp_of(&state, Perspective::Opp, n(301)), n(10));
        assert_eq!(hp_of(&state, Perspective::Me, n(101)), n(4));
    }

    #[test]
    fn trace_records_insertions_only_between_steps() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = triggers_with(heal_and_count);
        let state = duel_with_me_hp(10);
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let steps = vec![
            Step::new(vec![damage_op(TargetSelector::opponent_active(), n(5))]),
            Step::new(vec![damage_op(TargetSelector::opponent_active(), n(5))]),
        ];
        let traced = execute_steps_me_traced(&state, &steps, &effect);
        assert_eq!(traced.trace.steps.len(), 2);
        for (index, step) in traced.trace.steps.iter().enumerate() {
            assert_eq!(step.index, Nat::new(index as _));
            assert!(step.intra_step_insertions.is_empty());
            assert_eq!(step.insertions.len(), 1, "每步末各一次插入");
            assert_eq!(step.insertions[0].after_step, Nat::new(index as _));
            assert_eq!(step.insertions[0].executed, n(1));
        }
        // 两次伤害之间插入了治疗：最终 10 血。
        assert_eq!(
            hp_of(
                &traced.outcome.single().expect("单支").state,
                Perspective::Opp,
                n(301)
            ),
            n(10)
        );
    }

    #[test]
    fn empty_step_trace_has_one_branch_and_no_ops() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel_with_me_hp(10);
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let traced = execute_step_me_traced(&state, &Step::empty(), &effect);
        let step = &traced.trace.steps[0];
        assert_eq!(step.branches.len(), 1);
        assert!(step.branches[0].ops.is_empty());
        assert!(step.branches[0].pending.is_empty());
        assert!(step.insertions.is_empty());
    }

    #[test]
    fn trace_is_read_only_and_repeatable() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel_with_me_hp(10);
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![damage_op(TargetSelector::opponent_active(), n(2))]);
        let first = execute_step_me_traced(&state, &step, &effect);
        let second = execute_step_me_traced(&state, &step, &effect);
        // 同一输入 -> 同一轨迹（可复现）；入参局面不变。
        assert_eq!(first.trace.render(), second.trace.render());
        assert!(!first.trace.render().is_empty());
        assert_eq!(hp_of(&state, Perspective::Opp, n(301)), n(10));
        assert_eq!(first.trace.step_count(), n(1));
    }

    #[test]
    fn repeat_trace_keeps_children_inside_the_same_step() {
        fn one_damage(_index: Nat) -> Vec<KeywordOp> {
            vec![damage_op(TargetSelector::opponent_active(), n(1))]
        }
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel_with_me_hp(10);
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![KeywordOp::Repeat(RepeatArgs::indexed(
            n(3),
            one_damage,
        ))]);
        let traced = execute_step_me_traced(&state, &step, &effect);
        let op = &traced.trace.steps[0].branches[0].ops[0];
        assert_eq!(op.kind, "重复");
        assert_eq!(op.children.len(), 3, "重复的子动作仍属同一步");
        assert!(matches!(
            op.outcomes[0],
            OpOutcome::Repeated { count } if count == n(3)
        ));
        assert_eq!(
            hp_of(
                &traced.outcome.single().expect("单支").state,
                Perspective::Opp,
                n(301)
            ),
            n(7)
        );
    }

    #[test]
    fn traced_path_settles_and_records_reactions() {
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
        let step = Step::new(vec![KeywordOp::Damage(
            super::super::op::DamageArgs::new(
                TargetSelector::opponent_active(),
                n(2),
                DamageType::Elemental(Element::Pyro),
                super::super::op::DamageSourceKind::Other,
            ),
        )]);
        let traced = execute_step_me_traced(&state, &step, &effect);
        let plain = crate::keywords::step::execute_step_me(&state, &step, &effect);
        // R4：traced 路径也在步边界结算反应效果 -> 与 execute_step_me 局面一致。
        assert_eq!(
            traced.outcome.single().expect("单支").state,
            plain.single().expect("单支").state
        );
        let branch = traced.outcome.single().expect("单支");
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(401)), n(6));
        assert_eq!(
            branch.state.opp().characters().active_entity(),
            Some(n(402)),
            "超载的强制切换已执行"
        );
        // 轨迹里有反应条目（反应名 / 结果 / 可导出文本）。
        let branch_trace = &traced.trace.steps[0].branches[0];
        assert_eq!(branch_trace.reactions.len(), 1);
        assert_eq!(branch_trace.reactions[0].reaction, "超载");
        assert!(!branch_trace.reactions[0].non_damage);
        assert!(!branch_trace.reactions[0].outcomes.is_empty());
        assert!(traced.trace.render().contains("反应 超载"));
    }
}
