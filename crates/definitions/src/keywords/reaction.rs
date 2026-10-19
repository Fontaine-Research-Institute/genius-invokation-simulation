//! **反应子结算**（R4 / R2）：关键字层的「判定反应 -> 执行反应效果 -> 回到原步骤」。
//!
//! ## 规则书依据（唯一权威）
//!
//! - 规则书·元素反应·规则（S5.2）：
//!   1. 对已经具有「元素附着」的目标，造成特定类型的「元素伤害」时会引发「元素反应」；
//!   2. **以非伤害的形式施加特定类型的「元素附着」时也会引发「元素反应」；结算这类
//!      「元素反应」效果时，会忽略所有伤害类效果**。
//! - 结算顺序.md §三：元素伤害「附着 -> 判定反应 -> 结算反应效果 -> 继续原步骤」；
//!   §二：插入式结算（不是栈）—— 反应效果作为子结算插到当前位置，做完回到原位置。
//!
//! ## 与 D204（一步原子）的关系
//!
//! 一步内部**不允许插入**（D204）：关键字函数只把「本次触发的反应」记进
//! [ReactionRequest]，一步的全部操作按书写顺序做完后，在**步边界**用
//! [ReactionFrames] 把这些反应作为子结算插入执行（做完回到原步骤 = 调用方继续下一步）。
//!
//! 为什么不用 timeline::Resolution 直接装：Resolution 的待执行项是
//! Queued<E> = 函数指针 + 事件 + owner / 来源 / 进场，只能编码「角色族效果函数」；
//! 反应效果的执行器（damage_chain::executor::ReactionExecutor）需要整份双方区快照 +
//! 反应事件 + 效果分支，放不进 Queued<E>。所以本模块给出**同语义的专用帧**
//! [ReactionFrames]（insert / 内层先做完再回到插入点 / 不是 LIFO 重排），
//! 并把「执行一条反应」交给既有执行器（**不重写**执行器语义）。
//!
//! ## 实现要点
//!
//! - 判定反应：伤害路径由 damage_with_chain 预测（DamageOutcome.reaction / ReactionHit），
//!   非伤害附着路径用 bonus::predict_reaction（与 damage() 同一个 matching_covered 口径）；
//! - 执行效果：reactions::table::resolve 取「加伤 + 分支」，非伤害反应先做
//!   ReactionEffectResult::without_damage（忽略伤害类效果），再交给
//!   damage_chain::executor::execute_reaction 逐支执行到各区；
//! - 结果：每支一个 [KeywordBranch]（局面 + 精确概率 + 待执行），待执行由**步末**统一做；
//! - 发号器：执行器会为召唤物 / 生成卡发新号，逐支取 BranchExecution.alloc。
//!
//! ## 待接入 / 待实测
//!
//! - **反应效果造成的伤害是否再引发反应**：执行器内部用 damage()（会判反应、不执行效果），
//!   本模块不递归执行第二层反应效果 —— **待复核**（damage_chain/executor 原有待复核条目）；
//! - 反应效果给手牌加牌 / 加状态后，手牌费用表没有同步（缺 base_cost 回调）—— 待接入；
//! - 非伤害附着的「反应后附着怎么消耗」规则书未写清；这里按 D167 的同口径
//!   （移除命中的已覆盖元素 + 本次施加元素，本次元素不再附着）—— **待实测**；
//! - 风 / 岩天生不可附着：非伤害附着不落地也不判反应（伤害路径仍按 D134 判反应）—— 待复核。

use crate::families::character::attachment::ReactionEvent;
use crate::families::character::damage::DamageEnv;
use crate::families::character::damage_chain::bonus::ReactionHit;
use crate::families::character::damage_chain::executor::{
    execute_reaction, EntityAlloc, ExecutionEnv, ReactionTriggers, ReactionZones, SidePending,
    SideZones,
};
use crate::flow::pending::{FlowPending, PendingSet};
use crate::reactions::table;
use crate::state::{DefState, PlayerDefs};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::Nat;

use super::effect::EffectEnv;
use super::keyword::{push_pending, set_player};
use super::op::{KeywordBranch, KeywordResult};
use super::target::{player_of, Perspective};

/// **一次待执行的插入式子结算：一条已判定的反应**。
///
/// 关键字函数只产出它（纯数据、Copy），一步在步边界统一结算（D204：步内不插）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReactionRequest {
    /// 已判定的反应（damage_chain::bonus::ReactionHit；四要素与 ReactionEvent 一致）。
    pub hit: ReactionHit,
    /// 触发方（相对侧；= 反应事件 Source 所在的那一方）。
    pub source: Perspective,
    /// 承载方（反应作用在其角色身上的一方）。
    pub target: Perspective,
    /// 是不是「非伤害形式的附着」引发的反应（真 = 结算时忽略所有伤害类效果）。
    pub non_damage: bool,
}

impl ReactionRequest {
    /// 建一条反应子结算请求。
    pub const fn new(
        hit: ReactionHit,
        source: Perspective,
        target: Perspective,
        non_damage: bool,
    ) -> ReactionRequest {
        ReactionRequest {
            hit,
            source,
            target,
            non_damage,
        }
    }

    /// 反应事件（交给执行器 / 注册表用）。
    pub fn event(self) -> ReactionEvent {
        self.hit.event()
    }
}

/// **插入式子结算帧**（与 timeline::Resolution 同语义：insert 插入子结算，内层做完回到插入点；
/// 不是 LIFO 触发栈、不重排已有帧）。
///
/// 帧的内容 = 一组 [ReactionRequest]（帧内按生成顺序结算）。用专用帧而不是 Resolution 的原因
/// 见模块文档（Queued<E> 装不下反应效果执行所需的整份区快照）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReactionFrames {
    frames: Vec<Vec<ReactionRequest>>,
}

impl ReactionFrames {
    /// 空帧栈。
    pub fn new() -> ReactionFrames {
        ReactionFrames { frames: Vec::new() }
    }

    /// **插入子结算**：压入一帧（空帧 = 不做任何事）。
    pub fn insert(&mut self, requests: Vec<ReactionRequest>) {
        if requests.is_empty() {
            return;
        }
        self.frames.push(requests);
    }

    /// 取下一帧（内层先做；做完回到外层）。没有帧 -> None。
    pub fn next_frame(&mut self) -> Option<Vec<ReactionRequest>> {
        self.frames.pop()
    }

    /// 是否已经全部做完。
    pub fn is_finished(&self) -> bool {
        self.frames.is_empty()
    }

    /// 嵌套层数。
    pub fn depth(&self) -> Nat {
        Nat::new(self.frames.len() as _)
    }
}

/// 反应结算的一步分支（带发号器）。
struct Work {
    state: DefState,
    probability: Fraction,
    pending: PendingSet,
    alloc: EntityAlloc,
}

/// **执行一条反应子结算**：返回每支（局面 + 精确概率 + 待执行 + 发号器）。
///
/// 没登记效果的反应 -> 恰好一支、局面不变（不 panic）。
pub fn settle_reaction(
    state: &DefState,
    request: &ReactionRequest,
    env: &EffectEnv<'_>,
) -> Vec<(KeywordBranch, EntityAlloc)> {
    let event = request.event();
    let Some(effect) = table::resolve(&event, env.effect_context) else {
        return vec![(KeywordBranch::determined(state.clone()), env.alloc())];
    };
    let effect = if request.non_damage {
        effect.without_damage()
    } else {
        effect
    };
    let source_player = player_of(state, request.source);
    let target_player = player_of(state, request.target);
    let zones = ReactionZones {
        source: side_zones(source_player),
        target: side_zones(target_player),
    };
    let triggers = ReactionTriggers {
        source: (*env.side_triggers(request.source)).clone(),
        target: (*env.side_triggers(request.target)).clone(),
    };
    let execution_env = ExecutionEnv::new(
        DamageEnv {
            allow: env.damage.allow,
            rules: env.damage.rules,
            registry: env.damage.registry,
        },
        env.mastery,
    )
    .with_modifiers(env.reaction_modifiers());
    let execution = execute_reaction(
        &event,
        &effect,
        &zones,
        &triggers,
        &execution_env,
        env.alloc(),
    );
    execution
        .branches
        .into_iter()
        .map(|branch| {
            let source_after = player_from_zones(source_player, &branch.zones.source);
            let target_after = player_from_zones(target_player, &branch.zones.target);
            let mut next = set_player(state, request.source, source_after);
            next = set_player(&next, request.target, target_after);
            let mut pending = PendingSet::new();
            for item in flow_pending_from_side(branch.pending.source) {
                push_pending(&mut pending, request.source, item);
            }
            for item in flow_pending_from_side(branch.pending.target) {
                push_pending(&mut pending, request.target, item);
            }
            (
                KeywordBranch::new(next, branch.probability, pending),
                branch.alloc,
            )
        })
        .collect()
}

/// **按顺序结算一组反应请求**（每条可能展开成多支；概率精确相乘；不折叠）。
pub fn settle_reactions(
    state: &DefState,
    requests: &[ReactionRequest],
    env: &EffectEnv<'_>,
) -> KeywordResult {
    let mut work = vec![Work {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: PendingSet::new(),
        alloc: env.alloc(),
    }];
    for request in requests {
        let mut next: Vec<Work> = Vec::new();
        for branch in work {
            let scoped = env.fork(branch.alloc);
            for (settled, alloc) in settle_reaction(&branch.state, request, &scoped) {
                let mut pending = branch.pending.clone();
                append_pending(&mut pending, settled.pending.clone());
                next.push(Work {
                    state: settled.state,
                    probability: branch.probability.exact_mul(settled.probability),
                    pending,
                    alloc,
                });
            }
        }
        work = next;
    }
    finish(work, env.alloc())
}

/// **插入式结算**（R4 的「回到原步骤」）：把一批反应请求作为子结算插入当前位置执行。
///
/// 步边界调用：本步的全部动作已经做完（D204 步内不插），这里把反应效果插入执行，
/// 做完回到调用方的下一步。
pub fn settle_inserted(
    state: &DefState,
    requests: &[ReactionRequest],
    env: &EffectEnv<'_>,
) -> KeywordResult {
    let mut frames = ReactionFrames::new();
    frames.insert(requests.to_vec());
    let mut work = vec![Work {
        state: state.clone(),
        probability: Fraction::ONE,
        pending: PendingSet::new(),
        alloc: env.alloc(),
    }];
    while let Some(frame) = frames.next_frame() {
        for request in &frame {
            let mut next: Vec<Work> = Vec::new();
            for branch in work {
                let scoped = env.fork(branch.alloc);
                for (settled, alloc) in settle_reaction(&branch.state, request, &scoped) {
                    let mut pending = branch.pending.clone();
                    append_pending(&mut pending, settled.pending.clone());
                    next.push(Work {
                        state: settled.state,
                        probability: branch.probability.exact_mul(settled.probability),
                        pending,
                        alloc,
                    });
                }
            }
            work = next;
        }
    }
    finish(work, env.alloc())
}

/// 把工作分支收成 KeywordResult（发号器取各支最大值）。
fn finish(work: Vec<Work>, fallback: EntityAlloc) -> KeywordResult {
    let mut alloc = fallback;
    for branch in &work {
        if branch.alloc.next() > alloc.next() {
            alloc = branch.alloc;
        }
    }
    KeywordResult {
        branches: work
            .into_iter()
            .map(|branch| KeywordBranch::new(branch.state, branch.probability, branch.pending))
            .collect(),
        alloc,
    }
}

/// 追加待执行（保持顺序；用于把反应子结算派发的待执行接在前面）。
fn append_pending(into: &mut PendingSet, from: PendingSet) {
    for item in from.me {
        into.push_me(item);
    }
    for item in from.opp {
        into.push_opp(item);
    }
}

/// 一方的区快照（执行器按值收进 / 交出；与 flow/skill_use 同口径）。
fn side_zones(player: &PlayerDefs) -> SideZones {
    SideZones {
        characters: player.characters().clone(),
        combat_status: player.combat_status().clone(),
        summons: player.summons().clone(),
        hand: player.hand().clone(),
        hand_states: player.hand_states().clone(),
    }
}

/// 把执行器交出的区快照写回一方（其余区不动；手牌费用表同步待接入）。
fn player_from_zones(base: &PlayerDefs, zones: &SideZones) -> PlayerDefs {
    base.clone()
        .with_characters(zones.characters.clone())
        .with_combat_status(zones.combat_status.clone())
        .with_summons(zones.summons.clone())
        .with_hand(zones.hand.clone())
        .with_hand_states(zones.hand_states.clone())
}

/// 执行器的 SidePending -> 统一待执行（顺序 = 结算顺序 §一 第 2 条的来源类别：
/// 角色牌 -> 出战状态 -> 召唤物 -> 手牌状态）。
fn flow_pending_from_side(pending: SidePending) -> Vec<FlowPending> {
    let mut list = Vec::new();
    for item in pending.characters {
        list.push(FlowPending::CharacterZone(item));
    }
    for item in pending.combat_status {
        list.push(FlowPending::CombatStatus(item));
    }
    for item in pending.summons {
        list.push(FlowPending::Summon(item));
    }
    for item in pending.hand_states {
        list.push(FlowPending::HandCardState(item));
    }
    list
}

// ======================================================================
// 测试（R2 / R4）
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::families::character::attachment::{
        Attachment, AttachmentAllowSet, ReactionRegistry,
    };
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::DamageEnv;
    use crate::families::character::damage_chain::bonus::{AmountOp, DamageModifiers};
    use crate::families::character::zones::field::{
        CharacterSlot, CharacterZone,
    };
    use crate::reactions::effect::EffectContext;
    use crate::reactions::superconduct::SUPERCONDUCT;
    use crate::reactions::swirl::SWIRL;
    use crate::state::testing::{characters, n};
    use crate::state::PlayerDefs;
    use gi_genius_invokation_engine::EntityId;

    fn opp() -> PlayerDefs {
        PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![
                CharacterSlot::new(n(401), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
                CharacterSlot::new(n(402), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
                CharacterSlot::new(n(403), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
            ])
            .expect("合法角色区"),
        )
    }

    fn state() -> DefState {
        DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            opp(),
        )
    }

    fn hit() -> ReactionHit {
        ReactionHit {
            reaction: &SUPERCONDUCT,
            covered: Element::Electro,
            attack: Element::Cryo,
            source: n(101),
            target: n(401),
        }
    }

    fn env<'a>(
        registry: &'a ReactionRegistry,
        rules: &'a [crate::families::character::damage::ReactionRule],
        context: &'a EffectContext,
    ) -> EffectEnv<'a> {
        let damage = DamageEnv {
            allow: AttachmentAllowSet::STANDARD,
            rules,
            registry,
        };
        EffectEnv::new(n(101), Perspective::Me, damage).with_effect_context(context)
    }

    fn hp_of(state: &DefState, entity: EntityId) -> Nat {
        state
            .opp()
            .characters()
            .slot_of(entity)
            .expect("角色在区里")
            .state
            .hp()
    }

    #[test]
    fn reaction_frames_insert_and_return_like_resolution() {
        let mut frames = ReactionFrames::new();
        assert!(frames.is_finished());
        let request = ReactionRequest::new(hit(), Perspective::Me, Perspective::Opp, false);
        frames.insert(vec![request]);
        assert_eq!(frames.depth(), n(1));
        let frame = frames.next_frame().expect("有帧");
        assert_eq!(frame, vec![request]);
        assert!(frames.is_finished(), "取完即空（做完回到原步骤）");
        // 空插入不做任何事。
        frames.insert(Vec::new());
        assert!(frames.is_finished());
    }

    #[test]
    fn non_damage_reaction_ignores_all_damage_effects() {
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let effect = env(&registry, &rules, &context);
        let before = state();
        // 非伤害：超导的「1 点穿透伤害」被忽略 -> 全场不掉血。
        let request = ReactionRequest::new(hit(), Perspective::Me, Perspective::Opp, true);
        let settled = settle_reaction(&before, &request, &effect);
        assert_eq!(settled.len(), 1);
        let (branch, _alloc) = &settled[0];
        assert_eq!(branch.probability, Fraction::ONE);
        for entity in [n(401), n(402), n(403)] {
            assert_eq!(hp_of(&branch.state, entity), n(10));
        }
        // 对照：伤害型反应照常对目标以外角色造成 1 点穿透。
        let request = ReactionRequest::new(hit(), Perspective::Me, Perspective::Opp, false);
        let settled = settle_reaction(&before, &request, &effect);
        let (branch, _alloc) = &settled[0];
        assert_eq!(hp_of(&branch.state, n(402)), n(9));
        assert_eq!(hp_of(&branch.state, n(403)), n(9));
        // 承载角色本身不被「目标以外」的伤害命中。
        assert_eq!(hp_of(&branch.state, n(401)), n(10));
    }

    #[test]
    fn settle_reactions_keeps_probability_and_order() {
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let effect = env(&registry, &rules, &context);
        let before = state();
        let requests = vec![
            ReactionRequest::new(hit(), Perspective::Me, Perspective::Opp, true),
            ReactionRequest::new(hit(), Perspective::Me, Perspective::Opp, true),
        ];
        let settled = settle_reactions(&before, &requests, &effect);
        assert_eq!(settled.branches.len(), 1);
        assert_eq!(settled.probability_total(), Fraction::ONE);
        assert_eq!(hp_of(&settled.branches[0].state, n(401)), n(10));
    }

    #[test]
    fn reaction_damage_uses_keyword_reaction_modifiers() {
        // 扩散：对目标以外角色 1 点元素伤害；关键字上下文挂「翻倍」乘算来源后应打 2 点。
        let rules = crate::reactions::table::base_rules();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let effect = env(&registry, &rules, &context);
        let before = state();
        let request = ReactionRequest::new(
            ReactionHit {
                reaction: &SWIRL,
                covered: Element::Cryo,
                attack: Element::Anemo,
                source: n(101),
                target: n(401),
            },
            Perspective::Me,
            Perspective::Opp,
            false,
        );
        let plain = settle_reaction(&before, &request, &effect);
        let (branch, _alloc) = &plain[0];
        assert_eq!(hp_of(&branch.state, n(402)), n(9));
        assert_eq!(hp_of(&branch.state, n(403)), n(9));
        // 关键字上下文给反应效果伤害挂乘算来源：×2 -> 2 点。
        let multipliers = [AmountOp::mul_zone(n(2))];
        let boosted = effect.with_reaction_modifiers(DamageModifiers {
            ops: &multipliers,
            rounding: None,
        });
        let settled = settle_reaction(&before, &request, &boosted);
        let (branch, _alloc) = &settled[0];
        assert_eq!(hp_of(&branch.state, n(402)), n(8));
        assert_eq!(hp_of(&branch.state, n(403)), n(8));
        // 承载角色本身不受「目标以外」伤害影响。
        assert_eq!(hp_of(&branch.state, n(401)), n(10));
    }

    #[test]
    fn attach_helper_imports_are_used() {
        // 只为让 Attachment / ReactionHit 的 Debug/Clone 形状在测试里被引用（不新增语义）。
        let attachment = Attachment::with(Element::Hydro);
        assert!(attachment.has(Element::Hydro));
        assert_eq!(hit(), hit());
    }
}