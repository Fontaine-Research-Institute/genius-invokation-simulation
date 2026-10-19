//! **一步（Step）**：一个可观察效果内的全部动作（D204 / D208）。
//!
//! ## 语义（Lead 拍板 D204 / D208）
//!
//! - 步**可以**包含多个关键字操作；**步内按书写顺序依次执行**；
//! - **步内禁止插入任何外来结算**（触发、其它效果、子结算都不行）：
//!   execute_step 只调用关键字函数，**不做任何 insert / execute_pending**；
//! - 触发 / 其它效果**只在步末统一派发**：关键字函数把各区的待执行装进
//!   PendingSet 返回；步执行器把它们原样带回，由调用方在**步与步之间**执行 / 插入
//!   （D196 的 insert 只能用在步边界）；
//! - 所以一步同时是**显示单位**和**不可中断的原子结算单位**；
//! - **重复（Repeat）**（D208 / D209）：一个 Repeat 操作在执行一步时循环执行内部操作，
//!   仍然属于**同一步**、不拆步、不插入；
//! - 步与步**顺序执行**：[execute_steps_me] / [execute_steps_opp] 在每步末尾执行上一步
//!   派发出来的待执行（= 插入发生在步边界）；
//! - **反应效果**（R4 / R2）：一步内判出的反应不立即执行；本步全部操作做完后，在**步边界**
//!   用 [super::reaction::settle_inserted] 把反应子结算插入执行（做完回到原步骤），
//!   反应派发的待执行接在本步待执行之前。因此一步仍是不可中断的原子单位（D204）。
//!
//! 「执行步(state, step, env) -> 分支」在本层是**成对入口**（D29 / D34：不传侧参数）：
//! [execute_step_me] / [execute_step_opp]，共用一份实现 [run_step]。
//!
//! ## 与 D190 / D191 的关系
//!
//! 步执行本身是展开点：一个操作可能展开成多支（例如「随机 1 张手牌」），多支之间概率
//! 精确相乘；步执行器**不折叠**（折叠回由调用方在需要时做，见 flow::fold_states）。

use crate::families::character::damage_chain::executor::EntityAlloc;
use crate::flow::pending::execute_pending;
use crate::state::DefState;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, Nat};

use super::effect::EffectEnv;
use super::keyword::{
    apply_card_status, apply_status, attach_element, cause_damage, consume_energy, convert_dice,
    create_card, discard, draw_cards, force_switch, gain_dice, heal, put_hand_to_pile_bottom,
    summon, switch_character,
};
use super::op::{KeywordBranch, KeywordOp, KeywordResult, RepeatArgs};
use super::reaction::{settle_inserted, ReactionRequest};
use super::target::Perspective;

/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}

impl Step {
    /// 用一组操作建一步。
    pub fn new(ops: Vec<KeywordOp>) -> Step {
        Step { ops }
    }

    /// 没有操作的一步。
    pub fn empty() -> Step {
        Step { ops: Vec::new() }
    }

    /// 操作清单（只读）。
    pub fn ops(&self) -> &[KeywordOp] {
        &self.ops
    }

    /// 操作条数。
    pub fn len(&self) -> Nat {
        Nat::new(self.ops.len() as _)
    }

    /// 是否没有操作。
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// **一步的执行结果**：结算分支 + 推进后的发号器。
///
/// 分支里的待执行**还没有执行**（D204：步内不插入；调用方在步边界统一执行）。
#[derive(Clone, Debug)]
pub struct StepOutcome {
    /// 结算分支（每支 = 局面 + 精确概率 + 待执行）。
    pub branches: Vec<KeywordBranch>,
    /// 推进后的实体发号器。
    pub alloc: EntityAlloc,
}

impl StepOutcome {
    /// 确定性（只有一支）时取出那一支；多支时 None。
    pub fn single(&self) -> Option<&KeywordBranch> {
        if self.branches.len() == 1 {
            self.branches.first()
        } else {
            None
        }
    }

    /// 各分支概率之和（精确分数；用于断言概率守恒）。
    pub fn probability_total(&self) -> Fraction {
        self.branches
            .iter()
            .fold(Fraction::ZERO, |sum, branch| sum.exact_add(branch.probability))
    }
}

/// 步内的工作分支（带发号器的内部形态）。
struct BranchWork {
    state: DefState,
    probability: Fraction,
    pending: crate::flow::pending::PendingSet,
    /// 本步已判定、待步边界插入执行的反应子结算（D204：步内不插）。
    reactions: Vec<ReactionRequest>,
    alloc: EntityAlloc,
}

/// **执行一步**（我方视角）。
///
/// 只调用关键字函数：**不做任何 insert / execute_pending**（D204）；返回的待执行由调用方
/// 在步边界执行。
pub fn execute_step_me(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_step(state, step, &scoped)
}

/// **执行一步**（敌方视角；与 execute_step_me 共用一份实现）。
pub fn execute_step_opp(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Opp);
    run_step(state, step, &scoped)
}

/// 共用实现：视角方已绑定在 env 里。
///
/// 一步的全部关键字操作按书写顺序执行完之后，在**步边界**把本步产生的反应子结算
/// 插入执行（R4 / R2；D204 步内不插）；反应效果派发的待执行接在本步待执行之前
/// （结算顺序.md §三 的元素伤害段落：反应效果先于「扣血后」类触发）。
fn run_step(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> StepOutcome {
    let env = env
        .fork(env.alloc())
        .with_step_start_hands(state.me().hand().len(), state.opp().hand().len());
    let work = run_ops(state, &env, step.ops());
    let mut next_alloc = env.alloc();
    let mut branches: Vec<KeywordBranch> = Vec::new();
    for branch in work {
        // 步边界插入：一步内的动作已经全部做完；反应效果作为子结算执行完回到原步骤。
        let settled = settle_inserted(&branch.state, &branch.reactions, &env.fork(branch.alloc));
        for settled_branch in settled.branches {
            let mut pending = settled_branch.pending;
            for item in branch.pending.me.iter() {
                pending.push_me(item.clone());
            }
            for item in branch.pending.opp.iter() {
                pending.push_opp(item.clone());
            }
            branches.push(
                KeywordBranch::new(
                    settled_branch.state,
                    branch.probability.exact_mul(settled_branch.probability),
                    pending,
                )
                .with_reactions(settled_branch.reactions),
            );
        }
        if settled.alloc.next() > next_alloc.next() {
            next_alloc = settled.alloc;
        }
    }
    StepOutcome {
        branches,
        alloc: next_alloc,
    }
}

/// **顺序执行多步**（我方视角）：每步之间执行上一步派发出来的待执行（插入只在步边界，D204）。
pub fn execute_steps_me(state: &DefState, steps: &[Step], env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_steps(state, steps, &scoped)
}

/// **顺序执行多步**（敌方视角）。
pub fn execute_steps_opp(state: &DefState, steps: &[Step], env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Opp);
    run_steps(state, steps, &scoped)
}

/// 共用实现：逐步骤执行；每步末尾把待执行执行掉（= 在步与步之间插入）。
fn run_steps(state: &DefState, steps: &[Step], env: &EffectEnv<'_>) -> StepOutcome {
    let mut work = vec![BranchWork {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: crate::flow::pending::PendingSet::new(),
        reactions: Vec::new(),
        alloc: env.alloc(),
    }];
    for step in steps {
        let mut next: Vec<BranchWork> = Vec::new();
        for branch in work {
            let stepped = run_step(&branch.state, step, &env.fork(branch.alloc));
            let alloc = stepped.alloc;
            for result in stepped.branches {
                // 步末统一派发 / 执行（D204：插入只能发生在步与步之间）。
                let flushed = execute_pending(&result.state, &result.pending);
                next.push(BranchWork {
                    state: flushed,
                    probability: branch.probability.exact_mul(result.probability),
                    pending: crate::flow::pending::PendingSet::new(),
                    reactions: Vec::new(),
                    alloc,
                });
            }
        }
        work = next;
    }
    let mut next_alloc = env.alloc();
    for branch in &work {
        if branch.alloc.next() > next_alloc.next() {
            next_alloc = branch.alloc;
        }
    }
    StepOutcome {
        branches: work
            .into_iter()
            .map(|branch| KeywordBranch::new(branch.state, branch.probability, branch.pending))
            .collect(),
        alloc: next_alloc,
    }
}

/// 顺序执行一组操作（一步内；不插入、不派发，D204）。
fn run_ops(state: &DefState, env: &EffectEnv<'_>, ops: &[KeywordOp]) -> Vec<BranchWork> {
    let mut work = vec![BranchWork {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: crate::flow::pending::PendingSet::new(),
        reactions: Vec::new(),
        alloc: env.alloc(),
    }];
    for op in ops {
        let mut next: Vec<BranchWork> = Vec::new();
        for branch in work {
            let result = execute_op(&branch.state, &env.fork(branch.alloc), op);
            let alloc = result.alloc;
            for result_branch in result.branches {
                let mut pending = branch.pending.clone();
                for item in result_branch.pending.me {
                    pending.push_me(item);
                }
                for item in result_branch.pending.opp {
                    pending.push_opp(item);
                }
                let mut reactions = branch.reactions.clone();
                reactions.extend(result_branch.reactions.iter().copied());
                next.push(BranchWork {
                    state: result_branch.state,
                    probability: branch.probability.exact_mul(result_branch.probability),
                    pending,
                    reactions,
                    alloc,
                });
            }
        }
        work = next;
    }
    work
}

/// 执行一条关键字操作（分发到关键字函数；trace 模块复用）。
pub(crate) fn execute_op(state: &DefState, env: &EffectEnv<'_>, op: &KeywordOp) -> KeywordResult {
    match op {
        KeywordOp::Damage(args) => cause_damage(state, env, args),
        KeywordOp::Heal(args) => heal(state, env, args),
        KeywordOp::AttachElement(args) => attach_element(state, env, args),
        KeywordOp::Status(args) => apply_status(state, env, args),
        KeywordOp::Summon(args) => summon(state, env, args),
        KeywordOp::DrawCards(args) => draw_cards(state, env, args),
        KeywordOp::Discard(args) => discard(state, env, args),
        KeywordOp::GainDice(args) => gain_dice(state, env, args),
        KeywordOp::ConvertDice(args) => convert_dice(state, env, args),
        KeywordOp::CreateCard(args) => create_card(state, env, args),
        KeywordOp::SwitchCharacter(args) => switch_character(state, env, args),
        KeywordOp::ForceSwitch(args) => force_switch(state, env, args),
        KeywordOp::ConsumeEnergy(args) => consume_energy(state, env, args),
        KeywordOp::CardStatus(args) => apply_card_status(state, env, args),
        KeywordOp::PutHandToPileBottom(args) => put_hand_to_pile_bottom(state, env, args),
        KeywordOp::Repeat(args) => repeat(state, env, args),
    }
}

/// **重复**（D208 / D209）：在同一操作位置按顺序执行 n 次，每次的序号传给 plan。
fn repeat(state: &DefState, env: &EffectEnv<'_>, args: &RepeatArgs) -> KeywordResult {
    let mut work = vec![BranchWork {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: crate::flow::pending::PendingSet::new(),
        reactions: Vec::new(),
        alloc: env.alloc(),
    }];
    let mut index = Nat::ZERO;
    while index < args.count {
        let mut next: Vec<BranchWork> = Vec::new();
        for branch in work {
            let ops = (args.plan)(index);
            let scoped = env.fork(branch.alloc);
            let inner = run_ops(&branch.state, &scoped, &ops);
            for result in inner {
                let mut pending = branch.pending.clone();
                for item in result.pending.me {
                    pending.push_me(item);
                }
                for item in result.pending.opp {
                    pending.push_opp(item);
                }
                let mut reactions = branch.reactions.clone();
                reactions.extend(result.reactions.iter().copied());
                next.push(BranchWork {
                    state: result.state,
                    probability: branch.probability.exact_mul(result.probability),
                    pending,
                    reactions,
                    alloc: result.alloc,
                });
            }
        }
        work = next;
        index = nadd!(index, Nat::new(1));
    }
    let mut alloc = env.alloc();
    for branch in &work {
        if branch.alloc.next() > alloc.next() {
            alloc = branch.alloc;
        }
    }
    KeywordResult {
        branches: work
            .into_iter()
            .map(|branch| {
                KeywordBranch::new(branch.state, branch.probability, branch.pending)
                    .with_reactions(branch.reactions)
            })
            .collect(),
        alloc,
    }
}

// ======================================================================
// 测试（D204 / D207 / D208 / D209）
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{CardId, Element};
    use crate::families::character::attachment::{Attachment, AttachmentAllowSet, ReactionRegistry};
    use crate::families::character::damage::ReactionRule;
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::DamageEnv;
    use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
    use crate::families::character::damage_chain::statuses;
    use crate::families::character::trigger::{
        CharacterEffect, CharacterTiming, CharacterTriggers,
    };
    use crate::families::character::types::DamageType;
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{characters, hand, n, pile};
    use crate::state::PlayerDefs;
    use crate::timeline::{Event, Trigger};
    use gi_genius_invokation_engine::EntityId;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    use crate::keywords::op::{
        CardStatusArgs, ConsumeEnergyArgs, DamageArgs, DamageSourceKind, DrawCardsArgs, DrawCount,
        EnergyAmount, KeywordOp, PutHandToPileBottomArgs, RepeatArgs,
    };
    use crate::keywords::target::{SideScope, TargetSelector};
    use crate::keywords::op::AttachElementArgs;
    use crate::reactions::effect::{EffectContext, HandCardStatus};

    static NO_ELEMENTS: [Element; 0] = [];

    use crate::keywords::effect::TriggerBook;

    /// 插入探针：每执行一次 HpChanged 效果 +1（用来证明步内不执行、步末才派发）。
    static PROBE: AtomicUsize = AtomicUsize::new(0);

    /// 记录 Repeat 的序号（D209）。
    static INDEX_LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());

    /// 共享静态探针的测试串行锁（测试默认并行，探针会互相污染）。
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn probe_effect(_state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        PROBE.fetch_add(1, Ordering::SeqCst);
    }

    fn heal_and_count(state: &mut CharacterState, _event: &Event, _owner: EntityId) {
        PROBE.fetch_add(1, Ordering::SeqCst);
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

    /// 带反应规则 / 局面摘要的上下文（R4 / R2 测试用；允许附着 = 标准集合）。
    fn env_with_reactions<'a>(
        subject: EntityId,
        viewer: Perspective,
        registry: &'a ReactionRegistry,
        rules: &'a [ReactionRule],
        context: &'a EffectContext,
        me_characters: &'a CharacterTriggers,
        opp_characters: &'a CharacterTriggers,
    ) -> EffectEnv<'a> {
        let damage = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules,
            registry,
        };
        let mut book: TriggerBook<'a> = TriggerBook::empty();
        book.me_characters = me_characters;
        book.opp_characters = opp_characters;
        EffectEnv::new(subject, viewer, damage)
            .with_triggers(book)
            .with_effect_context(context)
    }

    /// 敌方 3 个角色：出战角色带指定元素附着（401 出战，402 / 403 后台）。
    fn opp_with_attachment(element: Element) -> PlayerDefs {
        let mut active = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        active.attachment = Attachment::with(element);
        PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![
                CharacterSlot::new(n(401), active),
                CharacterSlot::new(n(402), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
                CharacterSlot::new(n(403), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
            ])
            .expect("合法角色区"),
        )
    }

    fn duel_three() -> DefState {
        DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            opp_with_attachment(Element::Electro),
        )
    }

    fn damage_op(targets: TargetSelector, amount: Nat, ty: DamageType) -> KeywordOp {
        KeywordOp::Damage(DamageArgs::new(
            targets,
            amount,
            ty,
            DamageSourceKind::Other,
        ))
    }

    fn hp_of(state: &DefState, side: Perspective, character: EntityId) -> Nat {
        crate::keywords::target::player_of(state, side)
            .characters()
            .slot_of(character)
            .expect("角色在区里")
            .state
            .hp()
    }

    /// 我方 1 个角色 / 敌方 1 个角色（默认 10 血）。
    fn duel() -> DefState {
        DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(characters(&[301])),
        )
    }

    // ------------------------------------------------------------------
    // D204：一步原子（步内不插入、按顺序执行）
    // ------------------------------------------------------------------

    #[test]
    fn one_step_runs_ops_in_order_without_any_insertion() {
        let _guard = TEST_LOCK.lock().expect("串行锁");
        PROBE.store(0, Ordering::SeqCst);
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = triggers_with(heal_and_count);
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![
            damage_op(TargetSelector::opponent_active(), n(5), DamageType::Physical),
            damage_op(TargetSelector::opponent_active(), n(5), DamageType::Physical),
        ]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("确定性一步");
        // 两次伤害**都先落地**：没有中间插入（否则第一次伤害后就被治疗，血量不会是 0）。
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(301)), Nat::ZERO);
        // 触发还没执行（探针 0）；待执行 2 条（每次伤害一条 HpChanged）。
        assert_eq!(PROBE.load(Ordering::SeqCst), 0);
        assert_eq!(branch.pending.len(), n(2));
        // 步末统一执行后：两次治疗 +5 +5 -> 回到 10。
        let flushed = execute_pending(&branch.state, &branch.pending);
        assert_eq!(PROBE.load(Ordering::SeqCst), 2);
        assert_eq!(hp_of(&flushed, Perspective::Opp, n(301)), n(10));
        // 入参局面一个字节都没改。
        assert_eq!(hp_of(&state, Perspective::Opp, n(301)), n(10));
    }

    #[test]
    fn insertion_happens_only_between_two_steps() {
        let _guard = TEST_LOCK.lock().expect("串行锁");
        PROBE.store(0, Ordering::SeqCst);
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = triggers_with(heal_and_count);
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let steps = vec![
            Step::new(vec![damage_op(
                TargetSelector::opponent_active(),
                n(5),
                DamageType::Physical,
            )]),
            Step::new(vec![damage_op(
                TargetSelector::opponent_active(),
                n(5),
                DamageType::Physical,
            )]),
        ];
        let outcome = execute_steps_me(&state, &steps, &effect);
        let branch = outcome.single().expect("确定性两步");
        // 第一步伤害 -> 步末治疗 -> 第二步伤害 -> 步末治疗：最终 10。
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(301)), n(10));
        assert_eq!(PROBE.load(Ordering::SeqCst), 2);
        assert!(branch.pending.is_empty(), "多步执行器在步末把待执行清空");
    }

    #[test]
    fn one_step_is_one_display_unit_even_with_many_ops() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![
            damage_op(TargetSelector::opponent_active(), n(3), DamageType::Physical),
            damage_op(TargetSelector::opponent_active(), n(2), DamageType::Physical),
        ]);
        assert_eq!(step.len(), n(2));
        let outcome = execute_step_me(&state, &step, &effect);
        assert_eq!(outcome.branches.len(), 1, "一步只产出一支（显示单位）");
        assert_eq!(
            hp_of(&outcome.single().expect("有").state, Perspective::Opp, n(301)),
            n(5)
        );
    }

    // ------------------------------------------------------------------
    // D207：菲林斯元素爆发 = 一步
    // ------------------------------------------------------------------

    /// 菲林斯（1418）：hp 10 / maxenergy 4；元素爆发 14183：6 雷伤 + 对所有敌方后台 2 穿透。
    fn felis_state() -> DefState {
        let shield = statuses::status_of_card(CardId::new(134)).expect("角色护盾有登记");
        let mut back_two = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        back_two.states.add(CHARACTER_STATE_POS, shield, Some(n(10)));
        let mut back_three = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        back_three.states.add(CHARACTER_STATE_POS, shield, Some(n(10)));
        let me = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(
                n(14181),
                CharacterState::new(n(10), n(10), n(4), n(4)),
            )])
            .expect("合法角色区"),
        );
        let opp = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![
                CharacterSlot::new(n(9001), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
                CharacterSlot::new(n(9002), back_two),
                CharacterSlot::new(n(9003), back_three),
            ])
            .expect("合法角色区"),
        );
        DefState::from_players(me, opp)
    }

    #[test]
    fn felis_burst_is_one_atomic_step() {
        let _guard = TEST_LOCK.lock().expect("串行锁");
        PROBE.store(0, Ordering::SeqCst);
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = triggers_with(probe_effect);
        let state = felis_state();
        let effect = env(
            n(14181),
            Perspective::Me,
            &registry,
            &me_triggers,
            &opp_triggers,
        );
        let step = Step::new(vec![
            KeywordOp::ConsumeEnergy(ConsumeEnergyArgs {
                targets: TargetSelector::Subject,
                amount: EnergyAmount::All,
            }),
            KeywordOp::Damage(DamageArgs::new(
                TargetSelector::opponent_active(),
                n(6),
                DamageType::Elemental(Element::Electro),
                DamageSourceKind::ElementalBurst,
            )),
            KeywordOp::Damage(DamageArgs::new(
                TargetSelector::all_opponent_standby(),
                n(2),
                DamageType::Piercing,
                DamageSourceKind::ElementalBurst,
            )),
        ]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("爆发是确定性一步");
        // 充能归零。
        assert_eq!(
            crate::keywords::target::player_of(&branch.state, Perspective::Me)
                .characters()
                .slot_of(n(14181))
                .expect("有")
                .state
                .energy(),
            Nat::ZERO
        );
        // 出战角色掉 6；后台各掉 2（穿透不吃护盾）。
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(9001)), n(4));
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(9002)), n(8));
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(9003)), n(8));
        // 步内没有任何触发被执行（探针 0）；三次伤害各派发一条 HpChanged。
        assert_eq!(PROBE.load(Ordering::SeqCst), 0);
        assert_eq!(branch.pending.len(), n(3));
        // 步末统一派发。
        let _flushed = execute_pending(&branch.state, &branch.pending);
        assert_eq!(PROBE.load(Ordering::SeqCst), 3);
    }

    // ------------------------------------------------------------------
    // D208 / D209：重复包装
    // ------------------------------------------------------------------

    fn indexed_plan(index: Nat) -> Vec<KeywordOp> {
        INDEX_LOG.lock().expect("锁").push(index.get());
        vec![damage_op(
            TargetSelector::opponent_active(),
            n(1),
            DamageType::Physical,
        )]
    }

    #[test]
    fn repeat_zero_does_nothing() {
        let _guard = TEST_LOCK.lock().expect("串行锁");
        INDEX_LOG.lock().expect("锁").clear();
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![KeywordOp::Repeat(RepeatArgs::indexed(
            Nat::ZERO,
            indexed_plan,
        ))]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("一步");
        assert_eq!(branch.state, state);
        assert!(branch.pending.is_empty());
        assert!(INDEX_LOG.lock().expect("锁").is_empty(), "重复 0 次不调用 plan");
    }

    #[test]
    fn repeat_three_passes_index_and_stays_inside_one_step() {
        let _guard = TEST_LOCK.lock().expect("串行锁");
        INDEX_LOG.lock().expect("锁").clear();
        PROBE.store(0, Ordering::SeqCst);
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = triggers_with(probe_effect);
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![KeywordOp::Repeat(RepeatArgs::indexed(
            n(3),
            indexed_plan,
        ))]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("重复是同一步，不拆分支");
        assert_eq!(*INDEX_LOG.lock().expect("锁"), vec![0, 1, 2]);
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(301)), n(7));
        // 三次伤害都在**同一步**里：返回时一次带出 3 条待执行（没有中途执行）。
        assert_eq!(PROBE.load(Ordering::SeqCst), 0);
        assert_eq!(branch.pending.len(), n(3));
        let _flushed = execute_pending(&branch.state, &branch.pending);
        assert_eq!(PROBE.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn shazhong_yishi_step_puts_hand_under_pile_then_draws_hand_plus_one() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = DefState::from_players(
            PlayerDefs::new()
                .with_hand(hand(&[201, 202, 203]))
                .with_pile(pile(&[601, 602])),
            PlayerDefs::new().with_characters(characters(&[301])),
        );
        let effect = env(n(201), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![
            KeywordOp::PutHandToPileBottom(PutHandToPileBottomArgs::new(SideScope::Own)),
            KeywordOp::DrawCards(DrawCardsArgs {
                side: SideScope::Own,
                count: DrawCount::HandAtStepStartPlus(n(1)),
            }),
        ]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("一步");
        // 原手牌 3 + 1 = 4 张；牌库 5 - 4 = 1 张（最底那张 = 第一张手牌 201）。
        assert_eq!(branch.state.me().hand().len(), n(4));
        assert_eq!(branch.state.me().pile().len(), n(1));
        assert_eq!(
            branch.state.me().pile().entry(Nat::ZERO).map(|entry| entry.entity),
            Some(n(201))
        );
        // 动态张数在**步开始时**取：手牌已经离手也不会算成 1 张。
        assert!(branch.pending.is_empty());
    }

    #[test]
    fn yuezhao_full_glory_two_grants_are_one_step_with_exact_branches() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = DefState::from_players(
            PlayerDefs::new()
                .with_hand(hand(&[201, 202]))
                .with_pile(pile(&[601, 602])),
            PlayerDefs::new(),
        );
        let effect = env(n(201), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![
            KeywordOp::CardStatus(CardStatusArgs {
                targets: TargetSelector::HandCards(SideScope::Own),
                status: HandCardStatus::CostReduction,
                pick: crate::keywords::op::TargetPick::OneUniform,
                stack_cap: None,
            }),
            KeywordOp::CardStatus(CardStatusArgs {
                targets: TargetSelector::PileTop(SideScope::Own),
                status: HandCardStatus::CostReduction,
                pick: crate::keywords::op::TargetPick::All,
                stack_cap: None,
            }),
        ]);
        let outcome = execute_step_me(&state, &step, &effect);
        assert_eq!(outcome.branches.len(), 2, "随机手牌 2 张 -> 2 支");
        assert!(outcome.probability_total().is_one());
        let identity = statuses::hand_status_identity(HandCardStatus::CostReduction);
        for branch in &outcome.branches {
            let hand_states = branch.state.me().hand_states();
            let marked_hand = [n(201), n(202)]
                .into_iter()
                .filter(|entity| hand_states.contains_state(*entity, identity))
                .count();
            assert_eq!(marked_hand, 1, "每支恰好一张手牌被赋予");
            assert!(
                branch.state.me().pile().has_mark(Nat::ZERO, identity),
                "牌组顶也被赋予（同一步的第二次赋予）"
            );
        }
    }

    // ------------------------------------------------------------------
    // 空步 / 一步的重复执行
    // ------------------------------------------------------------------

    #[test]
    fn empty_step_changes_nothing() {
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let outcome = execute_step_me(&state, &Step::empty(), &effect);
        let branch = outcome.single().expect("空步");
        assert_eq!(branch.state, state);
        assert!(branch.pending.is_empty());
    }

    #[test]
    fn nested_repeat_produces_same_effects_without_splitting_steps() {
        let _guard = TEST_LOCK.lock().expect("串行锁");
        INDEX_LOG.lock().expect("锁").clear();
        PROBE.store(0, Ordering::SeqCst);
        let registry = ReactionRegistry::new();
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = triggers_with(probe_effect);
        let state = duel();
        let effect = env(n(101), Perspective::Me, &registry, &me_triggers, &opp_triggers);
        let step = Step::new(vec![KeywordOp::Repeat(RepeatArgs::indexed(
            n(2),
            indexed_plan,
        ))]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("嵌套重复仍是一步");
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(301)), n(8));
        assert_eq!(PROBE.load(Ordering::SeqCst), 0);
        assert_eq!(branch.pending.len(), n(2));
        let _flushed = execute_pending(&branch.state, &branch.pending);
        assert_eq!(PROBE.load(Ordering::SeqCst), 2);
    }

    // ------------------------------------------------------------------
    // R4 / R2：反应效果在步边界插入执行
    // ------------------------------------------------------------------

    #[test]
    fn keyword_damage_settles_overloaded_force_switch_at_step_boundary() {
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel_three();
        let effect = env_with_reactions(
            n(101),
            Perspective::Me,
            &registry,
            &rules,
            &context,
            &me_triggers,
            &opp_triggers,
        );
        let step = Step::new(vec![damage_op(
            TargetSelector::opponent_active(),
            n(2),
            DamageType::Elemental(Element::Pyro),
        )]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("一支");
        // 超载：本伤害 +2 -> 10 - 4 = 6；反应效果（强制切换）在步边界执行 -> 出战变 402。
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(401)), n(6));
        assert_eq!(
            branch.state.opp().characters().active_entity(),
            Some(n(402))
        );
        assert!(branch.reactions.is_empty(), "反应子结算已执行完");
    }

    #[test]
    fn one_step_defers_reaction_effect_to_the_step_boundary() {
        // 两步伤害都是「对敌方出战角色」；超载的强制切换如果在步内插入，
        // 第二次伤害就会打到切换后的角色（402）。D204：步内不插 -> 两次都打 401。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel_three();
        let effect = env_with_reactions(
            n(101),
            Perspective::Me,
            &registry,
            &rules,
            &context,
            &me_triggers,
            &opp_triggers,
        );
        let step = Step::new(vec![
            damage_op(
                TargetSelector::opponent_active(),
                n(1),
                DamageType::Elemental(Element::Pyro),
            ),
            damage_op(
                TargetSelector::opponent_active(),
                n(1),
                DamageType::Elemental(Element::Pyro),
            ),
        ]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("一支");
        // 第一次：超载 +2 -> 3 点；第二次：附着已被反应消耗 -> 1 点；合计 401 掉 4 点。
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(401)), n(6));
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(402)), n(10));
        // 反应效果在步末才执行：切换发生在两步伤害都落地之后。
        assert_eq!(
            branch.state.opp().characters().active_entity(),
            Some(n(402))
        );
    }

    #[test]
    fn keyword_damage_path_runs_vaporize_damage_bonus() {
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            opp_with_attachment(Element::Hydro),
        );
        let effect = env_with_reactions(
            n(101),
            Perspective::Me,
            &registry,
            &rules,
            &context,
            &me_triggers,
            &opp_triggers,
        );
        let step = Step::new(vec![damage_op(
            TargetSelector::opponent_active(),
            n(2),
            DamageType::Elemental(Element::Pyro),
        )]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("一支");
        // 蒸发「本伤害 +2」由伤害链执行：10 - 4 = 6（反应效果本身没有额外操作）。
        assert_eq!(hp_of(&branch.state, Perspective::Opp, n(401)), n(6));
        assert!(branch.reactions.is_empty());
    }

    #[test]
    fn non_damage_attachment_triggers_frozen_reaction() {
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            opp_with_attachment(Element::Hydro),
        );
        let effect = env_with_reactions(
            n(101),
            Perspective::Me,
            &registry,
            &rules,
            &context,
            &me_triggers,
            &opp_triggers,
        );
        let step = Step::new(vec![KeywordOp::AttachElement(AttachElementArgs {
            targets: TargetSelector::opponent_active(),
            element: Element::Cryo,
        })]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("一支");
        let slot = branch
            .state
            .opp()
            .characters()
            .slot_of(n(401))
            .expect("角色在区里");
        // 冻结：非伤害反应忽略 +1 伤害 -> 生命不变；[冻结] 状态仍然附属。
        assert_eq!(slot.state.hp(), n(10));
        let frozen = statuses::status_of_card(CardId::new(106)).expect("106 = 冻结有登记");
        assert!(slot
            .state
            .states
            .entries()
            .iter()
            .any(|entry| statuses::same_identity(entry.state, frozen)));
        // 反应消耗参与元素（与 D167 同口径，待实测）。
        assert!(!slot.state.attachment.has(Element::Hydro));
        assert!(!slot.state.attachment.has(Element::Cryo));
    }

    #[test]
    fn non_damage_attachment_ignores_all_damage_effects() {
        // 超导（雷 + 冰）：效果只有「1 点穿透伤害」-> 非伤害反应应全部忽略、不扣血。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let me_triggers = CharacterTriggers::new();
        let opp_triggers = CharacterTriggers::new();
        let state = duel_three();
        let effect = env_with_reactions(
            n(101),
            Perspective::Me,
            &registry,
            &rules,
            &context,
            &me_triggers,
            &opp_triggers,
        );
        let step = Step::new(vec![KeywordOp::AttachElement(AttachElementArgs {
            targets: TargetSelector::opponent_active(),
            element: Element::Cryo,
        })]);
        let outcome = execute_step_me(&state, &step, &effect);
        let branch = outcome.single().expect("一支");
        for entity in [n(401), n(402), n(403)] {
            assert_eq!(
                hp_of(&branch.state, Perspective::Opp, entity),
                n(10),
                "非伤害反应忽略所有伤害类效果"
            );
        }
        assert_eq!(
            branch.state.opp().characters().active_entity(),
            Some(n(401)),
            "超导没有强制切换"
        );
    }
}
