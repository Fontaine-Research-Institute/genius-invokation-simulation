//! **关键字函数**：每个关键字一个纯函数，签名统一：
//!
//! ```text
//! pub fn 关键字(state: &DefState, env: &EffectEnv, args: &关键字Args) -> KeywordResult
//! ```
//!
//! ## 口径
//!
//! - **纯函数**：不吃 &mut，不改 env；返回结算分支（局面 + 精确概率 + 待执行）；
//! - **内部调用已实现的底层操作**，不重写规则：
//!   - 伤害 -> families::character::damage::damage_with_chain（伤修链 + 附着 / 反应）；
//!   - 治疗 / 生命变动 -> families::character::trigger::heal_with_triggers（派发「生命值变动 / 治疗后」）；
//!   - 状态 / 召唤 / 弃置 / 切换 -> 各区自己的 *_with_triggers（派发只产出待执行，不执行）；
//!   - 骰子 -> resources::dice::DicePool 的 add / convert；
//!   - 抽牌 -> PileState::draw_many（折叠 + 精确概率分支原样带回）；
//! - **不采样**（D190）：随机取一张 = 展开成「每支概率 1 / 候选数」的分支；
//! - **不在一步内执行待执行**（D204）：所有 *_with_triggers 派发出来的待执行原样装进
//!   PendingSet 返回，由步执行器在**步末**统一执行；
//! - **反应也不在一步内执行**（R4 / R2）：伤害 / 非伤害附着判出的反应只记成
//!   [super::reaction::ReactionRequest]；步执行器在**步边界**把它作为插入式子结算执行
//!   （做完回到原步骤）。非伤害附着引发的反应执行时忽略所有伤害类效果（规则书 S5.2 第 2 条）；
//! - **实体号从上下文来**（D206）：自己 / 我方 / 敌方都写选择器，不写实体号。
//!
//! ## 依据
//!
//! 规则书·进行行动（使用技能 / 切换角色）、游戏区域（手牌 / 支援区 / 召唤物）、
//! 特殊词条（护盾 / 穿透 / 可用次数）、元素反应·效果；结算顺序.md §三；
//! D204 / D205 / D206 / D207 / D208（Lead 拍板）。

use crate::base::CardId;
use crate::families::action_card::status::{StateForm, StateIdentity};
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::pile::PileState;
use crate::families::action_card::zones::CardInstance;
use crate::families::character::character::CharacterState;
use crate::families::character::damage::{damage_with_chain, ChainDamageEnv};
use crate::families::character::damage_chain::bonus::{predict_reaction, DamageBonus};
use crate::families::character::damage_chain::chain::DamageChainEnv;
use crate::families::character::damage_chain::executor::{
    next_character, EntityAlloc, CHARACTER_STATE_POS,
};
use crate::families::character::damage_chain::statuses;
use crate::families::character::trigger::heal_with_triggers;
use crate::families::summon::zones::SummonEntry;
use crate::flow::pending::{FlowPending, PendingSet};
use crate::state::{DefState, PlayerDefs};
use gi_genius_invokation_engine::nat::ExactNat128;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, nsub, EntityId, Nat};

use super::effect::EffectEnv;
use super::op::{
    AttachElementArgs, CardStatusArgs, ConsumeEnergyArgs, ConvertDiceArgs, CreateCardArgs,
    DamageArgs, DiscardArgs, DrawCardsArgs, DrawCount, EnergyAmount, ForceSwitchArgs,
    GainDiceArgs, HealArgs, KeywordBranch, KeywordResult, PutHandToPileBottomArgs, StatusArgs,
    StatusPlace, SummonArgs, SwitchCharacterArgs, TargetPick,
};
use super::reaction::ReactionRequest;
use super::target::{player_of, resolve, Perspective};

// ======================================================================
// 小工具
// ======================================================================

/// 换掉某一方的全部区（纯函数）。
pub(crate) fn set_player(state: &DefState, side: Perspective, player: PlayerDefs) -> DefState {
    match side {
        Perspective::Me => state.clone().with_me(player),
        Perspective::Opp => state.clone().with_opp(player),
    }
}

/// 读某个角色的运行期状态（不在角色区 -> None）。
fn character_state(
    state: &DefState,
    side: Perspective,
    character: EntityId,
) -> Option<CharacterState> {
    player_of(state, side)
        .characters()
        .slot_of(character)
        .map(|slot| slot.state.clone())
}

/// 写回某个角色的运行期状态（不在角色区 -> 局面不变）。
fn with_character(
    state: &DefState,
    side: Perspective,
    character: EntityId,
    next_state: CharacterState,
) -> DefState {
    let player = player_of(state, side);
    match player.characters().with_state(character, next_state) {
        Some(zone) => set_player(state, side, player.clone().with_characters(zone)),
        None => state.clone(),
    }
}

/// 在目标集合里只留**角色**（其它实体对角色类关键字是空目标）。
fn character_targets(state: &DefState, targets: &super::target::TargetSet) -> Vec<(Perspective, EntityId)> {
    targets
        .iter()
        .filter(|(side, entity)| player_of(state, *side).characters().contains(*entity))
        .collect()
}

/// 往某一侧的待执行列表里放一条。
pub(crate) fn push_pending(set: &mut PendingSet, side: Perspective, item: FlowPending) {
    match side {
        Perspective::Me => set.push_me(item),
        Perspective::Opp => set.push_opp(item),
    }
}

/// 合并待执行（保持 from 的顺序，接在 into 后面）。
fn merge_pending(into: &mut PendingSet, from: PendingSet) {
    for item in from.me {
        into.push_me(item);
    }
    for item in from.opp {
        into.push_opp(item);
    }
}

/// **叠加上限下的本次实际层数**（Z15）：返回 None = 本次不添加（已达上限）。
///
/// - 布尔类状态：幂等，与上限无关，原样交给各区；
/// - 层数类状态：现有层数 + 本次请求层数，**截断到上限**（规则书没写清超限处理，
///   本实现按卡面「最多」的常见解释截断；TODO / 待实测）；
/// - cap = None 表示没有上限，原样交给各区（层数相加由区自己完成）。
fn capped_layers(
    identity: &'static dyn StateIdentity,
    existing: Option<Option<Nat>>,
    requested: Option<Nat>,
    cap: Option<Nat>,
) -> Option<Option<Nat>> {
    if identity.form() == StateForm::Boolean {
        return Some(requested);
    }
    let current = match existing {
        None => Nat::ZERO,
        Some(None) => Nat::new(1),
        Some(Some(layers)) => layers,
    };
    let want = requested.unwrap_or(Nat::new(1));
    match cap {
        None => Some(requested),
        Some(cap) => {
            let allowed = if cap > current {
                nsub!(cap, current)
            } else {
                Nat::ZERO
            };
            let add = if want < allowed { want } else { allowed };
            if add == Nat::ZERO {
                None
            } else {
                Some(Some(add))
            }
        }
    }
}

/// 召唤物的**叠加上限**（Z15）：按可用次数截断（None = 没有上限）。
fn capped_uses(uses: Option<Nat>, cap: Option<Nat>) -> Option<Nat> {
    match (uses, cap) {
        (Some(uses), Some(cap)) => Some(if uses < cap { uses } else { cap }),
        _ => uses,
    }
}

/// 生成物发号器前进一格。
fn alloc_after_one(alloc: EntityId) -> EntityId {
    nadd!(alloc, Nat::new(1))
}

/// 在候选集合上按选取策略执行（All = 一支；OneUniform = 每候选一支、概率 1 / 候选数）。
///
/// 每个目标返回 (新局局面, 本目标的待执行, 本目标的反应子结算)；反应子结算由步边界
/// 统一插入执行（R4 / R2；D204 步内不插）。
fn fan_out<F>(
    state: &DefState,
    env: &EffectEnv<'_>,
    candidates: Vec<(Perspective, EntityId)>,
    pick: TargetPick,
    mut apply: F,
) -> KeywordResult
where
    F: FnMut(
        &DefState,
        Perspective,
        EntityId,
    ) -> Option<(DefState, PendingSet, Vec<ReactionRequest>)>,
{
    if candidates.is_empty() {
        return KeywordResult::unchanged(state, env.alloc());
    }
    match pick {
        TargetPick::All => {
            let mut current = state.clone();
            let mut pending = PendingSet::new();
            let mut reactions: Vec<ReactionRequest> = Vec::new();
            for (side, entity) in candidates {
                if let Some((next, extra, mut extra_reactions)) = apply(&current, side, entity) {
                    current = next;
                    merge_pending(&mut pending, extra);
                    reactions.append(&mut extra_reactions);
                }
            }
            let branch = KeywordBranch::determined(current)
                .with_pending(pending)
                .with_reactions(reactions);
            KeywordResult {
                branches: vec![branch],
                alloc: env.alloc(),
            }
        }
        TargetPick::OneUniform => {
            let count = candidates.len();
            let probability = Fraction::new(ExactNat128::new(1), ExactNat128::new(count as u128));
            let branches = candidates
                .into_iter()
                .map(|(side, entity)| match apply(state, side, entity) {
                    Some((next, pending, reactions)) => KeywordBranch::new(next, probability, pending)
                        .with_reactions(reactions),
                    None => KeywordBranch::determined(state.clone()),
                })
                .collect();
            KeywordResult {
                branches,
                alloc: env.alloc(),
            }
        }
    }
}

// ======================================================================
// 造成伤害
// ======================================================================

/// **造成伤害**：对选择器解析出的每个角色走伤修链（加伤 -> 护盾 -> 减伤 -> 扣血 / 附着 / 反应）。
///
/// 每次伤害都派发「生命值变动」待执行（不执行，步末统一做，D204）。
pub fn cause_damage(state: &DefState, env: &EffectEnv<'_>, args: &DamageArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    fan_out(state, env, candidates, args.pick, |current, side, target| {
        let target_state = character_state(current, side, target)?;
        let player = player_of(current, side);
        // 乘算 / 除算修饰随参数进来（卡面「此次伤害翻倍」等）；默认 NONE。
        let chain_env = DamageChainEnv::new(player.combat_status(), &target_state.states)
            .with_modifiers(args.modifiers);
        let chained = damage_with_chain(
            &target_state,
            env.subject(),
            target,
            args.amount,
            args.ty,
            &ChainDamageEnv {
                damage: &env.damage,
                chain: &chain_env,
                bonus: DamageBonus {
                    increases: env.increases,
                    mastery: env.mastery,
                    reaction_bonus: Nat::ZERO,
                    reacted: false,
                },
                context: env.effect_context,
                triggers: env.character_triggers(side),
            },
        );
        let next = with_character(current, side, target, chained.outcome.state);
        let mut pending = PendingSet::new();
        for item in chained.pending {
            push_pending(
                &mut pending,
                side,
                FlowPending::Character {
                    character: target,
                    pending: item,
                },
            );
        }
        // R4：伤害路径判出的反应不在这里执行（D204 步内不插）；记成反应子结算，
        // 由步执行器在步边界插入执行（做完回到原步骤）。
        let mut reactions = Vec::new();
        if let Some(hit) = chained.reaction {
            reactions.push(ReactionRequest::new(hit, env.viewer(), side, false));
        }
        Some((next, pending, reactions))
    })
}

// ======================================================================
// 治疗 / 附着 / 状态
// ======================================================================

/// **治疗**：对每个目标角色 hp_add 并派发「生命值变动 -> 治疗后」。
pub fn heal(state: &DefState, env: &EffectEnv<'_>, args: &HealArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    fan_out(state, env, candidates, TargetPick::All, |current, side, target| {
        let target_state = character_state(current, side, target)?;
        let resolution = heal_with_triggers(
            &target_state,
            target,
            args.amount,
            Some(env.subject()),
            env.character_triggers(side),
        );
        let next = with_character(current, side, target, resolution.state);
        let mut pending = PendingSet::new();
        for item in resolution.pending {
            push_pending(
                &mut pending,
                side,
                FlowPending::Character {
                    character: target,
                    pending: item,
                },
            );
        }
        Some((next, pending, Vec::new()))
    })
}

/// **附着元素**（非伤害形式）：改元素附着集合，**并且也判反应**（R2 / 规则书 S5.2 第 2 条）。
///
/// 规则书·元素反应·规则 第（2）条：「以非伤害的形式施加特定类型的『元素附着』时也会引发
/// 『元素反应』。结算这类『元素反应』效果时，会忽略所有伤害类效果。」
///
/// 顺序（结算顺序.md §三）：附着 -> 判定反应 -> 结算反应效果 -> 继续原步骤。
/// 本关键字只做到「判定 + 记反应子结算」：反应效果由步边界插入执行（D204 步内不插），
/// 执行时由 [super::reaction::ReactionRequest::non_damage] 标记「忽略所有伤害类效果」。
///
/// 反应后的附着消耗规则书未写清；这里按 D167 同一口径（移除命中的已覆盖元素 + 本次施加
/// 元素，本次元素不再附着）—— **待实测**。
pub fn attach_element(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &AttachElementArgs,
) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    fan_out(state, env, candidates, TargetPick::All, |current, side, target| {
        let mut target_state = character_state(current, side, target)?;
        let before = target_state.attachment;
        // 风 / 岩天生不可附着：非伤害附着不落地、也就不引发反应（伤害路径仍按 D134 判反应）。
        // 待复核：规则书只写了「施加元素附着」，没单独说风 / 岩的非伤害形态。
        if !env.damage.allow.allows(args.element) {
            return Some((current.clone(), PendingSet::new(), Vec::new()));
        }
        // R2：非伤害形式的附着也判反应（判定口径与 damage() 相同：matching_covered）。
        if let Some(hit) = predict_reaction(
            before,
            args.element,
            env.damage.rules,
            env.subject(),
            target,
        ) {
            target_state.attachment = before.remove_all(&[hit.covered, args.element]);
            let next = with_character(current, side, target, target_state);
            let request = ReactionRequest::new(hit, env.viewer(), side, true);
            return Some((next, PendingSet::new(), vec![request]));
        }
        let (attachment, _landed) = before.attach(args.element, env.damage.allow);
        target_state.attachment = attachment;
        Some((
            with_character(current, side, target, target_state),
            PendingSet::new(),
            Vec::new(),
        ))
    })
}

/// **施加状态**：角色状态 / 出战状态 / 手牌状态三种承载。
///
/// - 角色状态：直接写 CharacterState::states（位置 = CHARACTER_STATE_POS；与反应执行器同口径）；
/// - 出战状态：CombatStatusZone::attach_with_triggers（派发待执行，不执行）；
/// - 手牌状态：HandCardStates::add_state_with_triggers（派发待执行，不执行）。
///
/// 状态身份由 statuses::status_of_card(card) 给；未知卡 -> 不做任何事（不 panic）。
pub fn apply_status(state: &DefState, env: &EffectEnv<'_>, args: &StatusArgs) -> KeywordResult {
    let Some(identity) = statuses::status_of_card(args.card) else {
        return KeywordResult::unchanged(state, env.alloc());
    };
    let targets = resolve(state, args.targets, &env.resolve_context());
    let mut current = state.clone();
    let mut pending = PendingSet::new();
    for (side, entity) in targets.iter() {
        match args.place {
            StatusPlace::Character => {
                if let Some(mut target_state) = character_state(&current, side, entity) {
                    // Z15：叠加上限（同身份同承载）；达到上限 -> 本次不再叠加。
                    let existing = target_state
                        .states
                        .layers(CHARACTER_STATE_POS, identity);
                    if let Some(layers) =
                        capped_layers(identity, existing, args.layers, args.stack_cap)
                    {
                        target_state
                            .states
                            .add(CHARACTER_STATE_POS, identity, layers);
                        current = with_character(&current, side, entity, target_state);
                    }
                }
            }
            StatusPlace::CombatStatus => {
                let player = player_of(&current, side);
                if player.characters().active_entity() != Some(entity) {
                    continue;
                }
                // Z15：叠加上限（同身份同承载）；达到上限 -> 本次不再叠加。
                let existing = player.combat_status().layers(identity);
                let Some(layers) =
                    capped_layers(identity, existing, args.layers, args.stack_cap)
                else {
                    continue;
                };
                let resolution = player.combat_status().attach_with_triggers(
                    entity,
                    identity,
                    layers,
                    args.duration,
                    &env.side_triggers(side).combat_status,
                );
                current = set_player(
                    &current,
                    side,
                    player.clone().with_combat_status(resolution.zone),
                );
                for item in resolution.pending {
                    push_pending(&mut pending, side, FlowPending::CombatStatus(item));
                }
            }
            StatusPlace::HandCard => {
                let player = player_of(&current, side);
                if !player.hand().contains(entity) {
                    continue;
                }
                // Z15：叠加上限（同身份同承载）；达到上限 -> 本次不再叠加。
                let existing = player.hand_states().layers(entity, identity);
                let Some(layers) =
                    capped_layers(identity, existing, args.layers, args.stack_cap)
                else {
                    continue;
                };
                let resolution = player.hand_states().add_state_with_triggers(
                    entity,
                    identity,
                    layers,
                    &env.side_triggers(side).hand_states,
                );
                current = set_player(
                    &current,
                    side,
                    player.clone().with_hand_states(resolution.states),
                );
                for item in resolution.pending {
                    push_pending(&mut pending, side, FlowPending::HandCardState(item));
                }
            }
        }
    }
    KeywordResult::single(current, pending, env.alloc())
}

// ======================================================================
// 召唤 / 抽牌 / 弃置 / 骰子 / 生成卡
// ======================================================================

/// **召唤 / 生成召唤物**（同名合并、满 4 拒绝由召唤物区自持；本层只写回）。
pub fn summon(state: &DefState, env: &EffectEnv<'_>, args: &SummonArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    let mut pending = PendingSet::new();
    let mut alloc = env.alloc();
    for side in sides.iter().take(count) {
        let entity = alloc.next();
        alloc = EntityAlloc::starting_at(alloc_after_one(entity));
        // Z15：同名叠加的可用次数上限（None = 没有上限）。
        let uses = capped_uses(args.uses, args.stack_limit);
        let entry = SummonEntry::new(entity, args.card)
            .with_uses(uses)
            .with_points(args.points)
            .with_effect_kind(args.effect_kind);
        let player = player_of(&current, *side);
        let resolution = player
            .summons()
            .summon_with_triggers(entry, &env.side_triggers(*side).summons);
        current = set_player(
            &current,
            *side,
            player.clone().with_summons(resolution.zone),
        );
        for item in resolution.pending {
            push_pending(&mut pending, *side, FlowPending::Summon(item));
        }
    }
    KeywordResult::single(current, pending, alloc)
}

/// **抽牌**：每张抽取的折叠分支原样展开（PileState::draw_many 的精确概率）。
pub fn draw_cards(state: &DefState, env: &EffectEnv<'_>, args: &DrawCardsArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut branches: Vec<KeywordBranch> = vec![KeywordBranch::determined(state.clone())];
    for side in sides.iter().take(count) {
        let draw_count = match args.count {
            DrawCount::Fixed(value) => value,
            DrawCount::HandAtStepStartPlus(extra) => nadd!(env.step_start_hand(*side), extra),
        };
        if draw_count == Nat::ZERO {
            continue;
        }
        let mut next_branches: Vec<KeywordBranch> = Vec::new();
        for branch in &branches {
            let player = player_of(&branch.state, *side);
            let draws = player.pile().draw_many(player.hand(), draw_count);
            if draws.is_empty() {
                next_branches.push(branch.clone());
                continue;
            }
            for (draw, probability) in draws {
                let next_player = player.clone().with_pile(draw.pile).with_hand(draw.hand);
                next_branches.push(KeywordBranch::new(
                    set_player(&branch.state, *side, next_player),
                    branch.probability.exact_mul(probability),
                    branch.pending.clone(),
                ));
            }
        }
        branches = next_branches;
    }
    KeywordResult {
        branches,
        alloc: env.alloc(),
    }
}

/// **弃置**：按「支援区 -> 手牌 -> 召唤物 -> 装备栏」找到实体所在的区并弃置。
///
/// 支援区弃置自己 = 目标选择器 Subject（D205）。
pub fn discard(state: &DefState, env: &EffectEnv<'_>, args: &DiscardArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let mut current = state.clone();
    let mut pending = PendingSet::new();
    for (side, entity) in targets.iter() {
        let player = player_of(&current, side);
        if let Some(resolution) = player
            .supports()
            .discard_with_triggers(entity, env.support_triggers(side))
        {
            current = set_player(
                &current,
                side,
                player.clone().with_supports(resolution.zone),
            );
            for item in resolution.pending {
                push_pending(&mut pending, side, FlowPending::Support(item));
            }
            continue;
        }
        if let Some(hand) = player.hand().discard(entity) {
            current = set_player(&current, side, player.clone().with_hand(hand));
            continue;
        }
        if let Some(resolution) = player
            .summons()
            .discard_with_triggers(entity, &env.side_triggers(side).summons)
        {
            current = set_player(
                &current,
                side,
                player.clone().with_summons(resolution.zone),
            );
            for item in resolution.pending {
                push_pending(&mut pending, side, FlowPending::Summon(item));
            }
            continue;
        }
        let detached = player.equipment().iter().find_map(|zone| {
            zone.detach_with_triggers(entity, env.equipment_triggers(side))
                .map(|resolution| (zone.owner(), resolution))
        });
        if let Some((owner, resolution)) = detached {
            current = set_player(
                &current,
                side,
                player.clone().with_equipment_zone(resolution.zone),
            );
            for item in resolution.pending {
                push_pending(&mut pending, side, FlowPending::Equipment { owner, pending: item });
            }
        }
    }
    KeywordResult::single(current, pending, env.alloc())
}

/// **获得骰子**。
pub fn gain_dice(state: &DefState, env: &EffectEnv<'_>, args: &GainDiceArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    for side in sides.iter().take(count) {
        let player = player_of(&current, *side);
        let dice = player.dice().add(args.face, args.count);
        current = set_player(&current, *side, player.clone().with_dice(dice));
    }
    KeywordResult::single(current, PendingSet::new(), env.alloc())
}

/// **转化骰**（来源不够 -> 不做任何事）。
pub fn convert_dice(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &ConvertDiceArgs,
) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    for side in sides.iter().take(count) {
        let player = player_of(&current, *side);
        if let Some(dice) = player.dice().convert(args.source, args.count, args.target) {
            current = set_player(&current, *side, player.clone().with_dice(dice));
        }
    }
    KeywordResult::single(current, PendingSet::new(), env.alloc())
}

/// **生成一张卡入手**（手牌满 / 实体重复 -> 不加入；费用表同步**待接入**）。
pub fn create_card(state: &DefState, env: &EffectEnv<'_>, args: &CreateCardArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    let mut alloc = env.alloc();
    for side in sides.iter().take(count) {
        let entity = alloc.next();
        alloc = EntityAlloc::starting_at(alloc_after_one(entity));
        let player = player_of(&current, *side);
        let (hand, added) = player.hand().add(CardInstance::new(entity, args.card));
        if added {
            current = set_player(&current, *side, player.clone().with_hand(hand));
        }
    }
    KeywordResult::single(current, PendingSet::new(), alloc)
}

// ======================================================================
// 切换 / 充能
// ======================================================================

/// **切换角色**（把选择器解析到的我方后台角色切为出战角色；不付骰子费用）。
///
/// 费用（规则书：切换角色支付 1 个任意元素骰）归骰池 / 调用方，本关键字只做区与时机。
pub fn switch_character(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &SwitchCharacterArgs,
) -> KeywordResult {
    let targets = resolve(state, args.to, &env.resolve_context());
    let mut current = state.clone();
    let mut pending = PendingSet::new();
    for (side, entity) in targets.iter() {
        if side != env.viewer() {
            continue;
        }
        let player = player_of(&current, side);
        if let Some(resolution) = player
            .characters()
            .switch_active_with_triggers(entity, &env.side_triggers(side).characters)
        {
            current = set_player(
                &current,
                side,
                player.clone().with_characters(resolution.zone),
            );
            for item in resolution.pending {
                push_pending(&mut pending, side, FlowPending::CharacterZone(item));
            }
        }
    }
    KeywordResult::single(current, pending, env.alloc())
}

/// **强制切换**：把解析到的出战角色强制换成角色区里的下一个后台角色（超载类效果）。
pub fn force_switch(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &ForceSwitchArgs,
) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let mut current = state.clone();
    let mut pending = PendingSet::new();
    for (side, entity) in targets.iter() {
        let player = player_of(&current, side);
        let zone = player.characters();
        if zone.active_entity() != Some(entity) {
            continue;
        }
        let Some(next) = next_character(zone, entity) else {
            continue;
        };
        if let Some(resolution) =
            zone.switch_active_with_triggers(next, &env.side_triggers(side).characters)
        {
            current = set_player(
                &current,
                side,
                player.clone().with_characters(resolution.zone),
            );
            for item in resolution.pending {
                push_pending(&mut pending, side, FlowPending::CharacterZone(item));
            }
        }
    }
    KeywordResult::single(current, pending, env.alloc())
}

/// **消耗充能**（元素爆发：全部；其它：固定点数）。
pub fn consume_energy(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &ConsumeEnergyArgs,
) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let mut current = state.clone();
    for (side, entity) in targets.iter() {
        if let Some(target_state) = character_state(&current, side, entity) {
            let amount = match args.amount {
                EnergyAmount::All => target_state.energy(),
                EnergyAmount::Fixed(value) => value,
            };
            let next_state = target_state.energy_sub(amount);
            current = with_character(&current, side, entity, next_state);
        }
    }
    KeywordResult::single(current, PendingSet::new(), env.alloc())
}

// ======================================================================
// 卡牌状态 / 手牌置底
// ======================================================================

/// **赋予卡牌状态**：目标在手牌 -> 手牌卡状态表；目标在牌组 -> 牌库位置状态（mark）。
///
/// 月兆·满辉（D208）：随机 1 张手牌 + 牌组顶各一次 -> 调用两次本关键字（同一步）。
pub fn apply_card_status(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &CardStatusArgs,
) -> KeywordResult {
    let identity = statuses::hand_status_identity(args.status);
    let targets = resolve(state, args.targets, &env.resolve_context());
    fan_out(state, env, targets.iter().collect(), args.pick, |current, side, entity| {
        let player = player_of(current, side);
        if player.hand().contains(entity) {
            // Z15：手牌卡状态的叠加上限（同身份同卡）；达到上限 -> 本次不再叠加。
            let existing = player.hand_states().layers(entity, identity);
            let Some(layers) = capped_layers(identity, existing, None, args.stack_cap) else {
                return Some((current.clone(), PendingSet::new(), Vec::new()));
            };
            let resolution = player.hand_states().add_state_with_triggers(
                entity,
                identity,
                layers,
                &env.side_triggers(side).hand_states,
            );
            let next = set_player(
                current,
                side,
                player.clone().with_hand_states(resolution.states),
            );
            let mut pending = PendingSet::new();
            for item in resolution.pending {
                push_pending(&mut pending, side, FlowPending::HandCardState(item));
            }
            return Some((next, pending, Vec::new()));
        }
        if let Some(position) = player.pile().position_of(entity) {
            // Z15：牌组位置状态（mark）同样受叠加上限约束。
            let existing = player.pile().mark_layers(position, identity);
            let Some(layers) = capped_layers(identity, existing, None, args.stack_cap) else {
                return Some((current.clone(), PendingSet::new(), Vec::new()));
            };
            let pile = player.pile().add_mark(position, identity, layers)?;
            return Some((
                set_player(current, side, player.clone().with_pile(pile)),
                PendingSet::new(),
                Vec::new(),
            ));
        }
        None
    })
}

/// **把某一方全部手牌置于牌组底**（沙中遗事；D208）。
///
/// 顺序口径：**后处理的牌在牌组底更上面**。实现上按手牌容器顺序**从后往前**追加到
/// 牌库末尾（末尾 = 底），于是容器里靠后的手牌最终落在底区更上面。
///
/// **待接入 / 待实测**：
/// - 牌库区没有「把不在牌库的牌插到底部」的公开原语（families 本块只读），这里用
///   PileState::from_fixed 重建整副牌库 —— 会丢掉随机段 / 公共状态列表；
///   需要牌库区补一个 insert_bottom_with(instance) 原语；
/// - 手牌容器声明无序，这里只按容器顺序（可复现，不承诺是玩家选择顺序）。
pub fn put_hand_to_pile_bottom(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &PutHandToPileBottomArgs,
) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    for side in sides.iter().take(count) {
        let player = player_of(&current, *side);
        let hand_entries = player.hand().entries();
        let total = Nat::new(hand_entries.len() as _);
        if total == Nat::ZERO {
            continue;
        }
        let mut instances: Vec<(EntityId, CardId)> = player
            .pile()
            .entries()
            .iter()
            .map(|entry| (entry.entity, entry.card))
            .collect();
        // 顺序由调用方注入（D209）；默认「从后往前」，于是后处理的牌在底区更上面。
        let mut processed = Nat::ZERO;
        while processed < total {
            let index = (args.order)(processed, total);
            if let Some(instance) = hand_entries.get(index.get() as usize) {
                instances.push((instance.entity, instance.card));
            }
            processed = nadd!(processed, Nat::new(1));
        }
        let pile = PileState::from_fixed(instances);
        let next_player = player
            .clone()
            .with_pile(pile)
            .with_hand(HandState::new());
        current = set_player(&current, *side, next_player);
    }
    KeywordResult::single(current, PendingSet::new(), env.alloc())
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::{CardId, Element};
    use crate::families::action_card::areas::support::{
        SupportEntry, SupportTriggers, SupportValue, SupportZone,
    };
    use crate::families::character::attachment::{AttachmentAllowSet, ReactionRegistry};
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::DamageEnv;
    use crate::families::character::damage_chain::bonus::{AmountOp, DamageModifiers};
    use crate::families::character::types::DamageType;
    use crate::reactions::effect::HandCardStatus;
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::resources::dice::{DicePool, DieFace};
    use crate::keywords::target::TargetSelector;
    use crate::state::testing::{characters, hand, n, pile, CARD_A};
    use crate::state::PlayerDefs;

    static NO_ELEMENTS: [Element; 0] = [];

    fn damage_env(registry: &ReactionRegistry) -> DamageEnv<'_> {
        DamageEnv {
            allow: AttachmentAllowSet::new(&NO_ELEMENTS),
            rules: &[],
            registry,
        }
    }

    fn env<'a>(
        subject: EntityId,
        viewer: Perspective,
        registry: &'a ReactionRegistry,
    ) -> EffectEnv<'a> {
        EffectEnv::new(subject, viewer, damage_env(registry))
    }

    fn single(outcome: &KeywordResult) -> &DefState {
        assert_eq!(outcome.branches.len(), 1, "确定性关键字应恰好一支");
        &outcome.branches[0].state
    }

    /// 我方 2 个角色（第 1 个出战）+ 2 张手牌；敌方 3 个角色。
    fn sample() -> DefState {
        DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[101, 102]))
                .with_hand(hand(&[201, 202])),
            PlayerDefs::new().with_characters(characters(&[301, 302, 303])),
        )
    }

    fn hp_of(state: &DefState, side: Perspective, character: EntityId) -> Nat {
        player_of(state, side)
            .characters()
            .slot_of(character)
            .expect("角色在区里")
            .state
            .hp()
    }

    #[test]
    fn damage_self_hurts_only_the_subject() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let outcome = cause_damage(
            &state,
            &env(n(101), Perspective::Me, &registry),
            &DamageArgs::new(
                TargetSelector::Subject,
                n(3),
                DamageType::Physical,
                crate::keywords::op::DamageSourceKind::Other,
            ),
        );
        let after = single(&outcome);
        assert_eq!(hp_of(after, Perspective::Me, n(101)), n(7));
        assert_eq!(hp_of(after, Perspective::Me, n(102)), n(10));
        // 同一局面里「敌方」不动。
        assert_eq!(hp_of(after, Perspective::Opp, n(301)), n(10));
    }

    /// 卡面「此次伤害翻倍」的一种写法：倍数区 ×2。
    static DOUBLE_DAMAGE_OPS: [AmountOp; 1] = [AmountOp::mul_zone(Nat::new(2))];

    #[test]
    fn damage_args_multipliers_reach_the_damage_chain() {
        // 卡面「此次伤害翻倍」：DamageArgs.modifiers 的倍数区 ×2 -> 2 点变 4 点。
        let registry = ReactionRegistry::new();
        let state = sample();
        let args = DamageArgs::new(
            TargetSelector::Subject,
            n(2),
            DamageType::Physical,
            crate::keywords::op::DamageSourceKind::NormalAttack,
        )
        .with_modifiers(DamageModifiers {
            ops: &DOUBLE_DAMAGE_OPS,
            rounding: None,
        });
        let outcome = cause_damage(&state, &env(n(101), Perspective::Me, &registry), &args);
        let after = single(&outcome);
        // 10 - (2 × 2) = 6。
        assert_eq!(hp_of(after, Perspective::Me, n(101)), n(6));
    }

    #[test]
    fn heal_self_refills_only_the_subject() {
        let registry = ReactionRegistry::new();
        let low = CharacterZone::try_from_slots(vec![
            CharacterSlot::new(n(101), CharacterState::new(n(4), n(10), Nat::ZERO, n(2))),
            CharacterSlot::new(n(102), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
        ])
        .expect("合法角色区");
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(low),
            PlayerDefs::new().with_characters(characters(&[301])),
        );
        let outcome = heal(
            &state,
            &env(n(101), Perspective::Me, &registry),
            &HealArgs::new(TargetSelector::Subject, n(3)),
        );
        let after = single(&outcome);
        assert_eq!(hp_of(after, Perspective::Me, n(101)), n(7));
        assert_eq!(hp_of(after, Perspective::Me, n(102)), n(10));
    }

    #[test]
    fn attach_element_writes_the_attachment_set() {
        static ALLOW: [Element; 1] = [Element::Hydro];
        let registry = ReactionRegistry::new();
        let state = sample();
        let damage = DamageEnv {
            allow: AttachmentAllowSet::new(&ALLOW),
            rules: &[],
            registry: &registry,
        };
        let effect = EffectEnv::new(n(101), Perspective::Me, damage);
        let outcome = attach_element(
            &state,
            &effect,
            &AttachElementArgs {
                targets: TargetSelector::opponent_active(),
                element: Element::Hydro,
            },
        );
        let after = single(&outcome);
        let attached = after
            .opp()
            .characters()
            .slot_of(n(301))
            .expect("有")
            .state
            .attachment;
        assert!(attached.has(Element::Hydro));
        assert!(outcome.branches[0].pending.is_empty(), "附着不派发待执行");
    }

    #[test]
    fn status_combat_and_hand_places_write_their_own_zones() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let effect = env(n(101), Perspective::Me, &registry);
        // 出战状态（卡 203 = 出战护盾）。
        let combat = apply_status(
            &state,
            &effect,
            &StatusArgs {
                targets: TargetSelector::opponent_active(),
                place: StatusPlace::CombatStatus,
                card: CardId::new(203),
                layers: Some(n(2)),
                duration: Some(n(1)),
                stack_cap: None,
            },
        );
        let with_combat = single(&combat);
        assert_eq!(with_combat.opp().combat_status().len(), n(1));
        // 手牌状态（卡 134 的身份也能挂到手牌上，本层只看挂载位置）。
        let hand_status = apply_status(
            &state,
            &effect,
            &StatusArgs {
                targets: TargetSelector::HandCards(crate::keywords::target::SideScope::Own),
                place: StatusPlace::HandCard,
                card: CardId::new(134),
                layers: Some(n(1)),
                duration: None,
                stack_cap: None,
            },
        );
        let with_hand = single(&hand_status);
        assert!(with_hand.me().hand_states().contains_state(n(201), identity_134()));
        // 角色状态不写进出战状态区 / 手牌状态表。
        assert!(with_hand.opp().combat_status().is_empty(), "手牌状态不碰出战状态");
    }

    fn identity_134() -> &'static dyn crate::families::action_card::status::StateIdentity {
        crate::families::character::damage_chain::statuses::status_of_card(CardId::new(134))
            .expect("134 有登记")
    }

    #[test]
    fn damage_random_one_target_expands_to_exact_branches() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let outcome = cause_damage(
            &state,
            &env(n(101), Perspective::Me, &registry),
            &DamageArgs {
                targets: TargetSelector::all_opponent_characters(),
                amount: n(4),
                ty: DamageType::Physical,
                source_kind: crate::keywords::op::DamageSourceKind::Other,
                pick: TargetPick::OneUniform,
                modifiers: crate::families::character::damage_chain::bonus::DamageModifiers::NONE,
            },
        );
        assert_eq!(outcome.branches.len(), 3);
        assert!(outcome.probability_total().is_one());
        for branch in &outcome.branches {
            let damaged = [n(301), n(302), n(303)]
                .into_iter()
                .filter(|entity| {
                    crate::keywords::target::player_of(&branch.state, Perspective::Opp)
                        .characters()
                        .slot_of(*entity)
                        .map(|slot| slot.state.hp() == n(6))
                        .unwrap_or(false)
                })
                .count();
            assert_eq!(damaged, 1, "每支只随机命中一个目标");
        }
    }

    #[test]
    fn discard_self_removes_only_the_subject_support() {
        let registry = ReactionRegistry::new();
        let triggers = SupportTriggers::new();
        let mut zone = SupportZone::new();
        for entity in [n(701), n(702)] {
            let added = zone.add_with_triggers(
                SupportEntry::new(CARD_A, entity, SupportValue::None),
                &triggers,
            );
            zone = added[0].0.zone.clone();
        }
        assert_eq!(zone.len(), n(2));
        let state = DefState::from_players(
            PlayerDefs::new().with_supports(zone),
            PlayerDefs::new().with_characters(characters(&[301])),
        );
        let outcome = discard(
            &state,
            &env(n(701), Perspective::Me, &registry),
            &DiscardArgs {
                targets: TargetSelector::Subject,
            },
        );
        let after = single(&outcome);
        assert_eq!(after.me().supports().len(), n(1));
        assert!(after.me().supports().contains(n(702)));
        assert!(!after.me().supports().contains(n(701)));
        // 入参不变。
        assert_eq!(state.me().supports().len(), n(2));
    }

    #[test]
    fn consume_all_energy_and_switch_to_next_character() {
        let registry = ReactionRegistry::new();
        let charged = CharacterZone::try_from_slots(vec![
            CharacterSlot::new(n(101), CharacterState::new(n(10), n(10), n(4), n(4))),
            CharacterSlot::new(n(102), CharacterState::new(n(10), n(10), Nat::ZERO, n(2))),
        ])
        .expect("合法角色区");
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(charged),
            PlayerDefs::new().with_characters(characters(&[301, 302])),
        );
        let effect = env(n(101), Perspective::Me, &registry);
        let drained = consume_energy(
            &state,
            &effect,
            &ConsumeEnergyArgs {
                targets: TargetSelector::Subject,
                amount: EnergyAmount::All,
            },
        );
        let after = single(&drained);
        assert_eq!(
            after
                .me()
                .characters()
                .slot_of(n(101))
                .expect("有")
                .state
                .energy(),
            Nat::ZERO
        );
        // 切换角色：我方后台第 1 个 = 102。
        let switched = switch_character(
            single(&drained),
            &effect,
            &SwitchCharacterArgs {
                to: TargetSelector::AfterActive {
                    side: crate::keywords::target::SideScope::Own,
                    offset: n(1),
                },
            },
        );
        assert_eq!(
            single(&switched).me().characters().active_entity(),
            Some(n(102))
        );
    }

    #[test]
    fn force_switch_rotates_opponent_active() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let outcome = force_switch(
            &state,
            &env(n(101), Perspective::Me, &registry),
            &ForceSwitchArgs {
                targets: TargetSelector::opponent_active(),
            },
        );
        assert_eq!(
            single(&outcome).opp().characters().active_entity(),
            Some(n(302))
        );
    }

    #[test]
    fn dice_and_draw_keywords_use_the_zone_primitives() {
        let registry = ReactionRegistry::new();
        let state = DefState::from_players(
            PlayerDefs::new()
                .with_dice(DicePool::empty().add(DieFace::Omni, n(2)))
                .with_pile(pile(&[601, 602, 603])),
            PlayerDefs::new().with_characters(characters(&[301])),
        );
        let effect = env(n(101), Perspective::Me, &registry);
        let gained = single(&gain_dice(
            &state,
            &effect,
            &GainDiceArgs {
                side: crate::keywords::target::SideScope::Own,
                face: DieFace::Pyro,
                count: n(2),
            },
        ))
        .clone();
        assert_eq!(gained.me().dice().total(), n(4));
        let converted = single(&convert_dice(
            &gained,
            &effect,
            &ConvertDiceArgs {
                side: crate::keywords::target::SideScope::Own,
                source: Some(DieFace::Pyro),
                count: n(1),
                target: DieFace::Omni,
            },
        ))
        .clone();
        assert_eq!(converted.me().dice().total(), n(4));
        let drawn = single(&draw_cards(
            &state,
            &effect,
            &DrawCardsArgs {
                side: crate::keywords::target::SideScope::Own,
                count: DrawCount::Fixed(n(2)),
            },
        ))
        .clone();
        assert_eq!(drawn.me().hand().len(), n(2));
        assert_eq!(drawn.me().pile().len(), n(1));
    }

    #[test]
    fn summon_status_and_create_card_apply_through_zones() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let effect = env(n(101), Perspective::Me, &registry);
        let summoned_result = summon(
            &state,
            &effect,
            &SummonArgs {
                side: crate::keywords::target::SideScope::Own,
                card: CardId::new(9001),
                uses: Some(n(2)),
                points: None,
                effect_kind: None,
                stack_limit: None,
            },
        );
        let summoned = single(&summoned_result).clone();
        assert_eq!(summoned.me().summons().len(), n(1));
        // 发号器前进一格。
        assert_eq!(summoned_result.alloc.next(), n(1));
        // 角色状态（卡 134 = 角色护盾）。
        let status = apply_status(
            &state,
            &effect,
            &StatusArgs {
                targets: TargetSelector::Subject,
                place: StatusPlace::Character,
                card: CardId::new(134),
                layers: Some(n(2)),
                duration: None,
                stack_cap: None,
            },
        );
        let with_status = single(&status);
        assert!(with_status
            .me()
            .characters()
            .slot_of(n(101))
            .expect("有")
            .state
            .states
            .entries()
            .iter()
            .any(|entry| {
                crate::families::character::damage_chain::statuses::same_identity(
                    entry.state,
                    status_identity_134(),
                )
            }));
        // 生成一张卡入手（发号器从 900 开始）。
        let created = create_card(
            &state,
            &effect.fork(EntityAlloc::starting_at(n(900))),
            &CreateCardArgs {
                side: crate::keywords::target::SideScope::Own,
                card: CARD_A,
            },
        );
        let after = single(&created);
        assert_eq!(after.me().hand().len(), n(3));
        assert!(after.me().hand().contains(n(900)));
    }

    fn status_identity_134() -> &'static dyn crate::families::action_card::status::StateIdentity {
        crate::families::character::damage_chain::statuses::status_of_card(CardId::new(134))
            .expect("134 有登记")
    }

    #[test]
    fn put_hand_to_pile_bottom_keeps_later_cards_above() {
        let registry = ReactionRegistry::new();
        let state = DefState::from_players(
            PlayerDefs::new()
                .with_hand(hand(&[201, 202, 203]))
                .with_pile(pile(&[601, 602])),
            PlayerDefs::new().with_characters(characters(&[301])),
        );
        let outcome = put_hand_to_pile_bottom(
            &state,
            &env(n(201), Perspective::Me, &registry),
            &PutHandToPileBottomArgs::new(crate::keywords::target::SideScope::Own),
        );
        let after = single(&outcome);
        assert!(after.me().hand().is_empty());
        assert_eq!(after.me().pile().len(), n(5));
        // 牌组顶 -> 底：601, 602, 203, 202, 201（后处理的牌在底区更上面）。
        let order: Vec<EntityId> = after
            .me()
            .pile()
            .entries()
            .iter()
            .map(|entry| entry.entity)
            .collect();
        assert_eq!(order, vec![n(601), n(602), n(203), n(202), n(201)]);
    }

    #[test]
    fn injected_order_changes_the_bottom_order() {
        fn forward(index: Nat, _total: Nat) -> Nat {
            index
        }
        let registry = ReactionRegistry::new();
        let state = DefState::from_players(
            PlayerDefs::new()
                .with_hand(hand(&[201, 202]))
                .with_pile(pile(&[601])),
            PlayerDefs::new(),
        );
        let outcome = put_hand_to_pile_bottom(
            &state,
            &env(n(201), Perspective::Me, &registry),
            &PutHandToPileBottomArgs {
                side: crate::keywords::target::SideScope::Own,
                order: forward,
            },
        );
        let order: Vec<EntityId> = single(&outcome)
            .me()
            .pile()
            .entries()
            .iter()
            .map(|entry| entry.entity)
            .collect();
        // 正序处理 -> 后处理的牌（202）在底区更下面。
        assert_eq!(order, vec![n(601), n(201), n(202)]);
    }

    #[test]
    fn card_status_random_hand_card_expands_to_exact_probability_branches() {
        let registry = ReactionRegistry::new();
        let state = DefState::from_players(
            PlayerDefs::new().with_hand(hand(&[201, 202])),
            PlayerDefs::new(),
        );
        let outcome = apply_card_status(
            &state,
            &env(n(201), Perspective::Me, &registry),
            &CardStatusArgs {
                targets: TargetSelector::HandCards(crate::keywords::target::SideScope::Own),
                status: HandCardStatus::CostReduction,
                pick: TargetPick::OneUniform,
                stack_cap: None,
            },
        );
        assert_eq!(outcome.branches.len(), 2);
        assert!(outcome.probability_total().is_one());
        for branch in &outcome.branches {
            let states = branch.state.me().hand_states();
            let marked: Vec<EntityId> = [n(201), n(202)]
                .into_iter()
                .filter(|entity| {
                    states.contains_state(
                        *entity,
                        crate::families::character::damage_chain::statuses::hand_status_identity(
                            HandCardStatus::CostReduction,
                        ),
                    )
                })
                .collect();
            assert_eq!(marked.len(), 1);
        }
    }

    // ------------------------------------------------------------------
    // Z15：叠加上限（stack_cap / stack_limit）
    // ------------------------------------------------------------------

    fn character_layers(
        state: &DefState,
        side: Perspective,
        entity: EntityId,
        identity: &'static dyn StateIdentity,
    ) -> Option<Option<Nat>> {
        player_of(state, side)
            .characters()
            .slot_of(entity)
            .and_then(|slot| slot.state.states.layers(CHARACTER_STATE_POS, identity))
    }

    fn character_status_args(stack_cap: Option<Nat>) -> StatusArgs {
        StatusArgs {
            targets: TargetSelector::Subject,
            place: StatusPlace::Character,
            card: CardId::new(134),
            layers: Some(n(2)),
            duration: None,
            stack_cap,
        }
    }

    #[test]
    fn stack_cap_clamps_layered_character_status() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let effect = env(n(101), Perspective::Me, &registry);
        let identity = statuses::status_of_card(CardId::new(134)).expect("134 = 角色护盾");
        // 上限 2：第一次叠 2 层，第二次再叠 2 层 -> 截断到 2（不会变成 4）。
        let first = apply_status(&state, &effect, &character_status_args(Some(n(2))));
        let after_first = single(&first).clone();
        assert_eq!(
            character_layers(&after_first, Perspective::Me, n(101), identity),
            Some(Some(n(2)))
        );
        let second = apply_status(&after_first, &effect, &character_status_args(Some(n(2))));
        let after_second = single(&second).clone();
        assert_eq!(
            character_layers(&after_second, Perspective::Me, n(101), identity),
            Some(Some(n(2))),
            "达到叠加上限后不再叠加"
        );
        // 对照：没有上限（stack_cap = None）时正常相加 2 + 2 = 4。
        let unbounded = apply_status(&after_first, &effect, &character_status_args(None));
        assert_eq!(
            character_layers(single(&unbounded), Perspective::Me, n(101), identity),
            Some(Some(n(4)))
        );
    }

    #[test]
    fn stack_cap_is_partial_when_remaining_room_is_smaller() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let effect = env(n(101), Perspective::Me, &registry);
        let identity = statuses::status_of_card(CardId::new(134)).expect("134 = 角色护盾");
        // 先叠 1 层（无上限），再用「最多 2 层」叠 2 层 -> 只剩 1 层空间，总层数 = 2。
        let first = apply_status(&state, &effect, &StatusArgs {
            layers: Some(n(1)),
            ..character_status_args(None)
        });
        let after_first = single(&first).clone();
        let second = apply_status(&after_first, &effect, &character_status_args(Some(n(2))));
        assert_eq!(
            character_layers(single(&second), Perspective::Me, n(101), identity),
            Some(Some(n(2)))
        );
    }

    #[test]
    fn stack_cap_applies_to_combat_and_hand_card_statuses() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let effect = env(n(101), Perspective::Me, &registry);
        // 出战状态（卡 203 = 出战护盾）：上限 2，连续两次各 1 层 -> 2。
        let identity = statuses::status_of_card(CardId::new(203)).expect("203 = 出战护盾");
        let mut current = state.clone();
        for _ in 0..3 {
            let outcome = apply_status(&current, &effect, &StatusArgs {
                targets: TargetSelector::opponent_active(),
                place: StatusPlace::CombatStatus,
                card: CardId::new(203),
                layers: Some(n(1)),
                duration: None,
                stack_cap: Some(n(2)),
            });
            current = single(&outcome).clone();
        }
        assert_eq!(
            player_of(&current, Perspective::Opp)
                .combat_status()
                .layers(identity),
            Some(Some(n(2))),
            "出战状态也受叠加上限约束"
        );
        // 手牌状态（卡 134 身份挂手牌）：上限 1，叠 2 层 -> 1。
        let hand_identity = statuses::status_of_card(CardId::new(134)).expect("134 = 角色护盾");
        let outcome = apply_status(&state, &effect, &StatusArgs {
            targets: TargetSelector::HandCards(crate::keywords::target::SideScope::Own),
            place: StatusPlace::HandCard,
            card: CardId::new(134),
            layers: Some(n(2)),
            duration: None,
            stack_cap: Some(n(1)),
        });
        assert_eq!(
            player_of(single(&outcome), Perspective::Me)
                .hand_states()
                .layers(n(201), hand_identity),
            Some(Some(n(1)))
        );
    }

    #[test]
    fn summon_stack_limit_clamps_uses() {
        let registry = ReactionRegistry::new();
        let state = sample();
        let effect = env(n(101), Perspective::Me, &registry);
        let card = CardId::new(9001);
        let args = SummonArgs {
            side: crate::keywords::target::SideScope::Own,
            card,
            uses: Some(n(3)),
            points: None,
            effect_kind: None,
            stack_limit: Some(n(2)),
        };
        // 「最多叠加到 2」：初始可用次数就截断到 2。
        let first = summon(&state, &effect, &args);
        let after_first = single(&first).clone();
        assert_eq!(
            player_of(&after_first, Perspective::Me)
                .summons()
                .entry_of_card(card)
                .and_then(|entry| entry.uses),
            Some(n(2))
        );
        // 再召唤一次（还是 3、上限 2）-> 同名合并后仍是 2，不会超过上限。
        let second = summon(&after_first, &effect, &args);
        assert_eq!(
            player_of(single(&second), Perspective::Me)
                .summons()
                .entry_of_card(card)
                .and_then(|entry| entry.uses),
            Some(n(2))
        );
        // 对照：没有上限时按区自持的「取较大可用次数」合并（3）。
        let unbounded = SummonArgs {
            stack_limit: None,
            ..args
        };
        let third = summon(&state, &effect, &unbounded);
        assert_eq!(
            player_of(single(&third), Perspective::Me)
                .summons()
                .entry_of_card(card)
                .and_then(|entry| entry.uses),
            Some(n(3))
        );
    }

    #[test]
    fn card_status_stack_cap_clamps_hand_and_pile_states() {
        let registry = ReactionRegistry::new();
        let state = DefState::from_players(
            PlayerDefs::new()
                .with_hand(hand(&[201]))
                .with_pile(pile(&[601])),
            PlayerDefs::new(),
        );
        let effect = env(n(201), Perspective::Me, &registry);
        let identity = statuses::hand_status_identity(HandCardStatus::CostReduction);
        let hand_args = CardStatusArgs {
            targets: TargetSelector::HandCards(crate::keywords::target::SideScope::Own),
            status: HandCardStatus::CostReduction,
            pick: TargetPick::All,
            stack_cap: Some(n(1)),
        };
        // 手牌状态：上限 1，连叠两次仍为 1。
        let first = apply_card_status(&state, &effect, &hand_args);
        let once = single(&first).clone();
        assert_eq!(
            player_of(&once, Perspective::Me)
                .hand_states()
                .layers(n(201), identity),
            Some(Some(n(1)))
        );
        let second = apply_card_status(&once, &effect, &hand_args);
        assert_eq!(
            player_of(single(&second), Perspective::Me)
                .hand_states()
                .layers(n(201), identity),
            Some(Some(n(1))),
            "达到上限后不再叠加"
        );
        // 对照：没有上限时叠两次 = 2 层。
        let unbounded = CardStatusArgs {
            stack_cap: None,
            ..hand_args
        };
        let two = apply_card_status(&state, &effect, &unbounded);
        let twice = apply_card_status(single(&two), &effect, &unbounded);
        assert_eq!(
            player_of(single(&twice), Perspective::Me)
                .hand_states()
                .layers(n(201), identity),
            Some(Some(n(2)))
        );
        // 牌组位置状态（mark）同样受叠加上限约束。
        let pile_args = CardStatusArgs {
            targets: TargetSelector::PileTop(crate::keywords::target::SideScope::Own),
            status: HandCardStatus::CostReduction,
            pick: TargetPick::All,
            stack_cap: Some(n(1)),
        };
        let first = apply_card_status(&state, &effect, &pile_args);
        let once = single(&first).clone();
        let second = apply_card_status(&once, &effect, &pile_args);
        assert_eq!(
            player_of(single(&second), Perspective::Me)
                .pile()
                .mark_layers(Nat::ZERO, identity),
            Some(Some(n(1)))
        );
    }
}
