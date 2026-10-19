# 定义层-Flow

> 中文 · [[定义层-Flow-EN|English]]

*本页范围: `flow/*` 端到端流程 (使用技能 / 打牌 / 回合 / 终局 / 注入点 / 复苏 / 特技 / 幻变) , 含顺序, 分支与待执行语义.*

---

## 1. 职责

`flow/*` 是**端到端流程层**: 把入口请求 (用技能 / 打牌 / 回合阶段 / 复苏 / 特技等) 在一个 `&DefState` 上跑完'校验 -> 支付 -> 派发 / 执行 -> 一步效果 -> 步末 flush -> 步边界插入 -> 击倒清理 -> 终局判定', 返回**分支 + 精确概率 + 待执行 + 终局结果**.本层不拥有持久记录器, 也不替调用方推进行动权.

## 2. 关键概念与类型

| 文件 | 职责 |
|---|---|
| `crates/definitions/src/flow/skill_use.rs` | 使用技能 (`use_skill_me/opp`; 旧 `SkillEffectPlan` / 新 `Step` 两条效果体路径)  |
| `flow/play_card.rs` | 打出行事牌 (普通 / 带时机 / 料理 / 秘传)  |
| `flow/turn.rs` | 回合流程: 回合开始, 投掷 / 重投, 阶段广播, 宣布结束, 结束阶段, 下一回合 |
| `flow/game_end.rs` | 终局判定薄入口 (`settlement_outcome` / `is_terminal`)  |
| `flow/hooks.rs` | 注入点: `DefeatCleanup` (D255) 与 `DrawHooks` (F10 / D188)  |
| `flow/revive.rs` | L2 复苏端到端 (次数判定 + 复苏 + 计数更新)  |
| `flow/technique.rs` | L4 特技端到端 (判定 / 支付卡面费用 / 战斗行动 / 恒空 SkillUsed)  |
| `flow/elemental_transmutation.rs` | L9 元素幻变: 投掷阶段 2+2 转换, 首次生成转换 |
| `flow/pending.rs` | 统一待执行 `FlowPending` / `PendingSet` + 分派执行器 |
| `flow/recorded.rs` / `record.rs` | 记录器路径 (`use_skill_*_recorded`, `play_card_*_recorded`; `FlowRecord`)  |
| `flow/switch_character.rs` / `tuning.rs` | 切换角色 / 元素调和 (支付在 flow; 区操作在 state/flows)  |

### 2.1 分支与待执行

| 概念 | 口径 |
|---|---|
| 分支 (D190)  | 每个入口返回 `Vec<Branch>`; 每支带 `state` + `probability: Fraction` (精确分数, 不采样) ; 确定性入口用 `single()` |
| 待执行 (D186)  | 派发只排序, 不执行; 各区 `*Pending` 归一成 `FlowPending` / `PendingSet` (`me` / `opp`)  |
| 执行器 | `execute_pending(&DefState, &PendingSet) -> DefState` / `execute_pending_mut`, 只按类型送到对应 `execute_*` |
| 谁执行 | 入口内部已执行的在结果里标'已执行, 保留供观测'; 返回的 `pending` 才需调用方在步边界执行 / 插入 (D196 / D204)  |

### 2.2 注入点 `flow/hooks.rs`

| 类型 / 函数 | 文件 | 说明 |
|---|---|---|
| `DefeatCleanup = fn(&DefState, Perspective) -> (DefState, PendingSet)` | `flow/hooks.rs` | D255'结算 -> 清理 -> 判终局'的清理回调; 上层注入真实实现, 默认 `NO_DEFEAT_CLEANUP` 不改任何东西 |
| `apply_defeat_cleanup(state, cleanup, &mut pending)` | `flow/hooks.rs` | 先 me 后 opp; 清理派发的待执行并入 `pending` |
| `DrawHooks { base_cost: Option<&dyn Fn(CardId) -> Vec<CostComponent>> }` | `flow/hooks.rs` | F10 手牌费用表 + D188 `drawn_marks` 物化 |
| `apply_draw_branch(player, &DrawBranch, hooks)` | `flow/hooks.rs` | 手牌 / 牌库落地 -> 费用表同步 (有 `base_cost`) -> `drawn_marks` 物化进手牌卡状态表 (空注册表, 不派发状态触发)  |

### 2.3 主要 Env (只借不拥有) 

| Env | 文件 | 关键字段 |
|---|---|---|
| `SkillFlowEnv` | `flow/skill_use.rs` | `triggers: SkillTriggerBook`, `keyword_triggers: TriggerBook`, `defeat_cleanup`, `allow`, `rules`, `reaction_registry`, `reaction_triggers`, `mastery`, `increases`, `effect_context`, `alloc`, `policy`, `energy_gain_bonus`, `same_color` |
| `PlayCardEnv` | `flow/play_card.rs` | `triggers: PlayTriggerBook`, `support_triggers`, `equipment_triggers`, `keywords: Option<KeywordEnv>`, `defeat_cleanup` |
| `TurnEnv` | `flow/turn.rs` | `triggers` (阶段广播单表) , `combat_status`, `summons`, `supports`, `draw: DrawHooks` |

## 3. 公开入口

### 索引

本小节的源码块为当前仓库的实现源码 (含 `use` / `derive` / `impl` 行, 路径见小节标题) ; 超长函数只列文档注释 + 签名 + 函数体开头几行, `// ...` 表示其余函数体省略.

| 模块 | 说明 |
|---|---|
| 注入点 | `flow/hooks.rs`: `DefeatCleanup` / `NO_DEFEAT_CLEANUP` / `DrawHooks` / `apply_draw_branch` / `apply_defeat_cleanup` |
| 终局 | `flow/game_end.rs`: `settlement_outcome` / `is_terminal` |
| 使用技能 | `flow/skill_use.rs`: `SkillTriggerBook` / `SkillFlowEnv` / `SkillUseRequest` / `SkillEffectBody` / `SkillEffectPlan` / `use_skill_*` (含 `_with_plunging`)  |
| 打牌 | `flow/play_card.rs`: `PlayTriggerBook` / `KeywordEnv` / `PlayCardEnv` / `CardPlacement` / `PlayCardRequest` / `PlayBranch` / `PlayCardOutcome` / `FoodReviveEnv` / `FoodPlayOutcome` / `ArcanePlayOutcome` / `PlayCardError` / `play_card_*` |
| 回合 | `flow/turn.rs`: `TurnTiming` / `TurnEnv` / `TurnEndOutcome` / `TurnEndBranch` / `RoundBeginOutcome` / `RollTransmutation` / `RoundEndState` / `DeclareRoundEnd*` / `EndPhaseError` / `EndPhaseStep` / `round_begin` … `round_end` |
| 复苏 | `flow/revive.rs`: `ReviveEnv` / `ReviveError` / `ReviveOutcome` / `revive_me` / `revive_opp` |
| 特技 | `flow/technique.rs`: `TechniqueEnv` / `TechniqueRequest` / `TechniqueError` / `TechniqueOutcome` / `use_technique_me/opp` |
| 幻变 | `flow/elemental_transmutation.rs`: `transmute_roll_phase_me/opp` / `transmute_opening_me/opp` |
| 规则 op (被引用)  | `rules/ops/stage.rs`: `timing_of_card_play` (打牌 `/play_card_*` 的行动时机)  |

### 源码: `flow/hooks.rs`

```rust
use crate::base::CardId;
use crate::families::action_card::areas::hand_state::HandCardStateTriggers;
use crate::families::action_card::zones::pile::DrawBranch;
use crate::flow::pending::PendingSet;
use crate::keywords::target::Perspective;
use crate::resources::dice::CostComponent;
use crate::state::{DefState, PlayerDefs};

/// **击倒清理回调**（D255：结算 -> 清理 -> 判终局）。
///
/// 入参 = 当前一局 + 要清理哪一方（Perspective::Me / Opp）；返回 = 清理后的局面 +
/// 清理派发出来的待执行（本层会并进分支的 PendingSet，由调用方在边界执行）。
///
/// 上层（cards）注入真实实现：对该方所有 HP=0 的角色调
/// state::flows::defeat::defeat_me/opp（按角色给 DefeatHpReset：定义 max_hp + 成长）。
pub type DefeatCleanup = fn(&DefState, Perspective) -> (DefState, PendingSet);

/// **不清理**：直接交回原局面 + 空待执行（默认；不注入时行为与旧版完全一致）。
pub fn no_defeat_cleanup(state: &DefState, _side: Perspective) -> (DefState, PendingSet) {
    (state.clone(), PendingSet::new())
}

/// 默认的击倒清理回调（= [no_defeat_cleanup]；常量方便 env 默认值使用）。
pub const NO_DEFEAT_CLEANUP: DefeatCleanup = no_defeat_cleanup;

/// 抓牌路径的**注入点**（F10 手牌费用表 + D188 drawn_marks 物化）。
#[derive(Clone, Copy)]
pub struct DrawHooks<'a> {
    /// 卡牌基础费用（卡 id -> 基础费用）：新进手牌按它建 CurrentCost。
    ///
    /// None = 不同步费用表（旧行为；调用方自己保证费用表正确）。
    pub base_cost: Option<&'a dyn Fn(CardId) -> Vec<CostComponent>>,
}

impl<'a> DrawHooks<'a> {
    /// 不注入任何抓牌 hook（旧行为）。
    pub const fn none() -> DrawHooks<'a> {
        DrawHooks { base_cost: None }
    }

    /// 只注入基础费用回调。
    pub const fn with_base_cost(
        base_cost: &'a dyn Fn(CardId) -> Vec<CostComponent>,
    ) -> DrawHooks<'a> {
        DrawHooks {
            base_cost: Some(base_cost),
        }
    }
}

/// **把一条抽牌分支落到一方**（F10 / D188）：
///
/// 1. 手牌 / 牌库换成抽牌后的区；
/// 2. 有 base_cost 回调 -> PlayerDefs::with_synced_hand_costs（新牌按基础费用建 CurrentCost，
///    已有手牌的 delta 保留，离开手牌的删除）；
/// 3. 抽到牌携带的 drawn_marks（D188 展开结果）物化进**手牌卡状态表**：
///    与 drawn 一一对应；因为手牌溢出被弃置的牌不再物化；
///    物化用空触发注册表（只落状态数据；触发派发待接入，见模块 TODO）。
pub fn apply_draw_branch(
    player: &PlayerDefs,
    branch: &DrawBranch,
    hooks: DrawHooks<'_>,
) -> PlayerDefs {
    let mut next = player
        .clone()
        .with_hand(branch.hand.clone())
        .with_pile(branch.pile.clone());

/// **把一条抽牌分支落到一方**（F10 / D188）：
///
/// 1. 手牌 / 牌库换成抽牌后的区；
/// 2. 有 base_cost 回调 -> PlayerDefs::with_synced_hand_costs（新牌按基础费用建 CurrentCost，
///    已有手牌的 delta 保留，离开手牌的删除）；
/// 3. 抽到牌携带的 drawn_marks（D188 展开结果）物化进**手牌卡状态表**：
///    与 drawn 一一对应；因为手牌溢出被弃置的牌不再物化；
///    物化用空触发注册表（只落状态数据；触发派发待接入，见模块 TODO）。
pub fn apply_draw_branch(
    player: &PlayerDefs,
    branch: &DrawBranch,
    hooks: DrawHooks<'_>,
) -> PlayerDefs {
    let mut next = player
        .clone()
        .with_hand(branch.hand.clone())
        .with_pile(branch.pile.clone());
    // ...

/// **执行一次击倒清理**（D255 顺序：结算 -> 清理 -> 判终局）。
///
/// 先我方、再敌方（两方都要清；具体清理哪些角色由注入的实现决定）。清理派发出来的待执行
/// 并进 pending（D186：本层不执行，由调用方在边界执行）。
pub fn apply_defeat_cleanup(
    state: &DefState,
    cleanup: DefeatCleanup,
    pending: &mut PendingSet,
) -> DefState {
    let (state, me_cleanup) = cleanup(state, Perspective::Me);
    merge_pending(pending, me_cleanup);
    let (state, opp_cleanup) = cleanup(&state, Perspective::Opp);
    merge_pending(pending, opp_cleanup);
    state
}
```

### 源码: `flow/game_end.rs`

```rust
use crate::state::terminal::terminal;
use crate::state::DefState;

/// **每步结算 + 击倒清理之后**的终局判定（规则书 S1.4 / S7.7）。
///
/// 纯读取：只借 `&DefState`、不改任何区。调用时点见模块文档（必须先完成结算与
/// `state::flows::defeat` 清理；否则 HP 已 0 但角色仍在区里的局面会被判成 `Ongoing`）。
/// 技能 / 打牌入口已通过 `DefeatCleanup` 注入完成清理；直接调用本函数时由调用方保证顺序。
pub fn settlement_outcome(state: &DefState) -> Outcome {
    terminal(state)
}

/// 这局是否已经终局（[Outcome::Ongoing] 之外都是；含回合上限 / 同时全灭）。
pub fn is_terminal(state: &DefState) -> bool {
    settlement_outcome(state).is_terminal()
}
```

### 源码: `flow/skill_use.rs`

*注释中的关键字代码已替换为名称, 代码行未改*

```rust
use crate::families::character::attachment::{AttachmentAllowSet, ReactionRegistry};
use crate::families::character::damage::{
use crate::families::character::damage_chain::bonus::{DamageBonus, ReactionHit};
use crate::families::character::damage_chain::chain::DamageChainEnv;
use crate::families::character::damage_chain::executor::{
use crate::families::character::skill::SkillDef;
use crate::families::character::skill_flow::{
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
    /// 使用方出战角色身上的准备技能槽（准备技能）。
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
    /// 使用方出战角色身上的准备技能槽（准备技能）。
    pub prepared: &'a PreparedSkills,
    /// 效果体（默认走关键字一步；旧的效果体计划保留为兼容）。
    pub effect: SkillEffectBody<'a>,
}

/// 技能的**效果体**（D227：默认走关键字层的一步；旧路径保留为兼容）。
#[derive(Clone, Copy, Debug)]
pub enum SkillEffectBody<'a> {
    /// 旧路径（兼容；行为不变）。
    Plan(SkillEffectPlan),
    /// 新路径：关键字层一步。
    Step(&'a Step),
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
```

### 源码: `flow/play_card.rs`

*注释中的关键字代码已替换为名称, 代码行未改*

```rust
use crate::base::CardId;
use crate::families::action_card::areas::support::{
use crate::families::action_card::def::{PlayCondition, PlayConditionFn, ViewSide};
use crate::families::action_card::ActionCardType;
use crate::families::character::satiated::{can_apply_food_effect, can_choose_as_food_target};
use crate::families::character::zones::equipment::{
use crate::families::character::zones::field::CharacterZoneTriggers;
use crate::families::status::arcane_edict::ArcaneEdict;
use crate::flow::game_end::{settlement_outcome, Outcome};
use crate::flow::hooks::{apply_defeat_cleanup, DefeatCleanup};
use crate::flow::pending::{execute_pending, FlowPending, PendingSet};
use crate::flow::record::FlowRecord;
use crate::flow::revive::{revive_me, revive_opp, ReviveEnv, ReviveError};
use crate::keywords::effect::{EffectEnv, TriggerBook};
use crate::keywords::step::{execute_step_me, execute_step_opp, Step};
use crate::resources::dice::CostComponent;
use crate::rules::ops::arcane::ArcanePlay;
use crate::rules::ops::equip::{CharacterEquip, EquipReject, SlotKind, WeaponType};
use crate::rules::ops::food::can_revive;
use crate::rules::ops::stage::{timing_of_card_play, Timing};
use crate::state::{DefState, PlayerDefs};
use crate::timeline::{resolve, Event, Pending, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, EntityId, Nat};

/// **两位牌手的行动牌打出注册表**（S7.5 第 1 条：当前牌手优先需要两张表）。
///
/// 两张表是两个具名实例（D34）；谁是当前牌手由 play_card_me / play_card_opp 绑定。
#[derive(Clone, Copy, Debug)]
pub struct PlayTriggerBook<'a> {
    /// 我方的「打出前 / 后」注册表。
    pub me: &'a PlayTriggers,
    /// 敌方的「打出前 / 后」注册表。
    pub opp: &'a PlayTriggers,
}

/// 关键字层（一步）的**外部依赖**（只借不拥有；D227：事件牌效果体走一步时要它）。
///
/// 字段与 keywords::effect::EffectEnv 的构造一致（视角方由 play_card_me / play_card_opp 绑定）。
pub struct KeywordEnv<'a> {
    /// 双方触发集合（区触发 / 角色族触发 / 支援 / 装备）。
    pub triggers: TriggerBook<'a>,
    /// 伤害入口依赖（允许附着集合 + 反应规则表 + 反应注册表）。
    pub damage: crate::families::character::damage::DamageEnv<'a>,
    /// 元素精通。
    pub mastery: Nat,
    /// 通用增伤。
    pub increases: &'a [Nat],
    /// 反应效果函数的局面摘要。
    pub effect_context: &'a crate::reactions::effect::EffectContext,
    /// 实体发号器（生成召唤物 / 卡牌用）。
    pub alloc: crate::families::character::damage_chain::executor::EntityAlloc,
}

/// 打出行动牌的**外部依赖**（只借不拥有）。
pub struct PlayCardEnv<'a> {
    /// 两位牌手的「打出前 / 后」触发注册表（S7.5：当前牌手优先）。
    pub triggers: PlayTriggerBook<'a>,
    /// 支援区触发注册表（进入 / 弃置）。
    pub support_triggers: &'a SupportTriggers,
    /// 装备栏触发注册表（装备 / 弃置）。
    pub equipment_triggers: &'a EquipmentTriggers,
    /// 关键字层依赖（事件牌效果体走一步时必需；没有效果体 / 旧路径传 None）。
    pub keywords: Option<KeywordEnv<'a>>,
    /// **击倒清理回调**（D255：结算 -> 清理 -> 判终局）。
    ///
    /// 上层（cards）注入真实实现（state::flows::defeat 需要按角色给 DefeatHpReset）；
    /// 不注入时传 crate::flow::hooks::NO_DEFEAT_CLEANUP（行为与旧版完全一致）。
    pub defeat_cleanup: DefeatCleanup,
}

/// 行动牌的**落位**（三个变体 = ActionCardType 的三个值；落位数据由调用方按卡牌定义给）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardPlacement<'a> {
    /// 支援牌：进支援区（值三选一，D182 / 分工计划 §4.4.15）。
    Support {
        /// 支援牌的值。
        value: SupportValue,
        /// **持续回合**（Z6 / Z14；S8.6 持续回合）：由卡牌定义给；
        /// 整局用 [SupportDuration::WholeGame]，有限回合用 [SupportDuration::Round]。
        duration: SupportDuration,
    },
    /// 装备牌：挂到指定角色身上（槽位 / 武器门由卡牌标签给）。
    Equipment {
        /// 挂到哪个角色。
        character: EntityId,
        /// 槽位种类。
        slot: SlotKind,
        /// 需要的角色武器类型（武器牌才有）。
        required_weapon: Option<WeaponType>,
        /// 该角色的装备条件（武器类型）。
        character_equip: CharacterEquip,
    },
    /// 事件牌：结算完离场（不进任何区）。
    Event {
        /// 效果体（D227：关键字层一步；None = 无效果体，由调用方在「打出后」之前自己执行）。
        effect: Option<&'a Step>,
    },
}

/// 一次打牌的**请求**（本层不替你选牌 / 目标）。
pub struct PlayCardRequest<'a> {
    /// 手牌实体号（必须真在手牌里）。
    pub entity: EntityId,
    /// 当前费用（由调用方按当前费用表给：基础费用 + 加减费 delta）。
    pub cost: Vec<CostComponent>,
    /// 同色费用支付时选的颜色（SameColor 费用需要）。
    pub same_color: Option<crate::base::Element>,
    /// 落位。
    pub placement: CardPlacement<'a>,
    /// 打出的**条件**（D217；由调用方从 `ActionCardDef::play_condition()` 给，`None` = 无条件）。
    ///
    /// 本层在「手牌里有这张牌」之后、**比较是否付得起**与「打出前」触发**之前**调用它：
    /// 拒绝 -> [PlayCardError::ConditionNotMet]、入参不变。
    pub play_condition: Option<PlayConditionFn>,
}

/// **一支打牌分支**（支援区满 4 时是多支，D190）。
#[derive(Clone, Debug)]
pub struct PlayBranch {
    /// 这一支结算后的完整局面。
    pub state: DefState,
    /// 这一支的精确概率。
    pub probability: Fraction,
    /// 这一支之后还要执行的待执行（支援 / 装备区派发出来的；本层不执行）。
    pub pending: PendingSet,
    /// 这一支的终局判定（只读；调用时点见 [settlement_outcome]：每步结算 + defeat 清理之后）。
    pub outcome: Outcome,
}

/// 一次打牌的结果。
#[derive(Clone, Debug)]
pub struct PlayCardOutcome {
    /// 结算分支（支援区未满 -> 恰好一支）。
    pub branches: Vec<PlayBranch>,
    /// 「打出前」的待执行（**已由本层执行**；保留供观测）。
    pub before: Vec<PlayPending>,
    /// 「打出后」的待执行（**未执行**；由调用方执行）。
    pub after: Vec<PlayPending>,
    /// 本次打牌的**行动时机**（S3.1 行动 + S8.8 战斗行动（行动牌））。
    ///
    /// 由 [crate::rules::ops::stage::timing_of_card_play] 按「这张牌是否带『战斗行动』规则」判定：
    /// 默认快速行动；带规则时改为战斗行动。行动权推进（是否换手）归调用方，本层不切行动方。
    pub timing: Timing,
    /// 结算后的实体发号器（事件牌一步生成过召唤物 / 卡牌时前移；没有一步时 None）。
    pub alloc: Option<crate::families::character::damage_chain::executor::EntityAlloc>,
    /// 流程级记录（D225 / D227：打出牌 / 一步 / 步末 flush / 步边界插入）。
    ///
    /// 本层只产出内容（不持有记录器）：由 flow/recorded.rs 的入口写进调用方给的 Recorder。
    pub events: Vec<FlowRecord>,
}

/// 料理结算的**复苏参数**（L2 + S8.1 第 4 条；目标在击破区时才需要）。
#[derive(Clone, Copy)]
pub struct FoodReviveEnv<'a> {
    /// 角色区触发注册表（复苏会派发「进入角色区」时机）。
    pub characters: &'a CharacterZoneTriggers,
    /// 本回合已经通过料理复苏的角色数（调用方持有；每回合上限见 rules::ops::food）。
    pub revived_this_round: Nat,
    /// 复苏后生命值（卡面给；规则书没写具体值）。
    pub hp: Nat,
    /// 具有「复苏」效果的料理事件牌 id（记录 / 观测用；本层不解释卡牌语义）。
    pub card: CardId,
}

/// 一次料理打牌的**结果**：打牌结果 + 复苏后的本回合计数（非复苏料理 -> None）。
#[derive(Clone, Debug)]
pub struct FoodPlayOutcome {
    /// 普通打牌流程的完整结果（分支 / 前后派发 / 时机 / 记录）。
    pub play: PlayCardOutcome,
    /// 复苏料理：**更新后**的本回合复苏计数（= 传入值 + 1；调用方写回自己的计数）。
    /// 普通料理 -> None。
    pub revived_this_round: Option<Nat>,
}

/// **秘传打出的结果**（Z12 / 秘传）：打牌结果 + **打出后**的秘传揭令容器。
#[derive(Clone, Debug)]
pub struct ArcanePlayOutcome {
    /// 普通打牌流程的完整结果（分支 / 前后派发 / 时机 / 记录）。
    pub play: PlayCardOutcome,
    /// 打出后的秘传揭令容器（已 record_play：本局张数 +1、按秘传规则消耗揭令）。
    pub edict: ArcaneEdict,
}

/// 打牌失败的原因（所有失败都保证入参不变）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayCardError {
    /// 这张牌不在打出方手牌里。
    NotInHand,
    /// 骰子不够付。
    NotEnoughDice,
    /// 装备被拒绝（槽位 / 武器类型门），状态不变。
    CannotEquip(EquipReject),
    /// 落位数据与 ActionCardType 不一致（构造错误；正常不会发生）。
    PlacementMismatch,
    /// 事件牌带了一步效果体，但环境里没有关键字层依赖（[PlayCardEnv::keywords] 为 None）。
    MissingKeywordEnv,
    /// 这张牌的**打出条件**不通过（D217）；带的是调试文案（中文短句，不是给玩家看的本地化文案）。
    ///
    /// 只读检查、**入参不变**；它发生在「比较是否付得起」**之前**，所以同一张牌
    /// 「条件不通过 + 骰子也不够」时，本层报的是这个错误，不是 [PlayCardError::NotEnoughDice]。
    ConditionNotMet(&'static str),
    /// 秘传（Z12 / 秘传）：揭令不存在，或本局已经打出过秘传（每人 1 张）。
    ArcaneNotPlayable(ArcanePlay),
    /// 料理目标不合法（S8.1 第 2 条）：目标角色不在场，或已处于「饱腹」
    /// （不能成为其他料理的目标 / 不受其效果）。只发生在 [play_card_me_food] 等料理入口。
    FoodTargetNotAllowed,
    /// 复苏料理不可用（L2 / S8.1 第 4 条）：次数已用完 / 目标不在击破区 / 角色区已满。
    FoodReviveNotAllowed(ReviveError),
}

/// **打出行动牌**（我方打出；无「战斗行动」规则 = 快速行动）。
///
/// 带「战斗行动（行动牌）」规则的牌用 [play_card_me_with_timing]（S8.8），否则时机会被
/// 判成快速行动。
pub fn play_card_me(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
) -> Result<PlayCardOutcome, PlayCardError> {
    play_card_me_with_timing(state, env, request, false)
}

/// **打出行动牌（我方；按牌是否带「战斗行动」规则判时机）**（S3.1 / S8.8）。
///
/// `has_combat_action_tag`：这张牌是否带「战斗行动（行动牌）」规则 —— 由**卡牌定义 / 标签**
/// 给（flow 不认识具体标签）；判定复用 `rules::ops::stage::timing_of_card_play`，
/// 结果放进 [PlayCardOutcome::timing]（默认快速行动；带规则 -> 战斗行动）。
pub fn play_card_me_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    has_combat_action_tag: bool,
) -> Result<PlayCardOutcome, PlayCardError> {
    play(state, env, request, true, has_combat_action_tag, None)
}

/// **打出行动牌**（敌方打出；与 play_card_me 共用一份实现，D35；无「战斗行动」规则）。
pub fn play_card_opp(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
) -> Result<PlayCardOutcome, PlayCardError> {
    play_card_opp_with_timing(state, env, request, false)
}

/// **打出行动牌（敌方；按牌是否带「战斗行动」规则判时机）**（与 [play_card_me_with_timing]
/// 共用一份实现，D35）。
pub fn play_card_opp_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    has_combat_action_tag: bool,
) -> Result<PlayCardOutcome, PlayCardError> {
    play(state, env, request, false, has_combat_action_tag, None)
}

/// **打出「料理」行动牌**（我方；S8.1 料理）：结果与 [play_card_me] 相同，另外：
///
/// 1. **结算前**校验目标（S8.1 第 2 条）：
///    - 目标在**角色区** -> 普通料理：`can_choose_as_food_target` / `can_apply_food_effect`
///      （= 未饱腹）校验；
///    - 目标在**击破区** -> 复苏料理（L2）：次数用 `rules::ops::food::can_revive` 判
///      （每回合最多 1 个），复苏参数由 [FoodReviveEnv] 给；失败 ->
///      [PlayCardError::FoodReviveNotAllowed]；
/// 2. **结算后**（每支分支）：普通料理给目标加「饱腹」；复苏料理先复活目标、再给它加「饱腹」
///    （S8.1 第 1 / 4 条）。
///
/// 料理牌的**目标实体**由调用方给（flow 不认识卡面 / 选择策略）；目标可以是双方角色
/// （规则书 S8.1 未限制阵营，待实测）。
pub fn play_card_me_food(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_me_food_with_timing(state, env, request, target, revive, false)
}

/// **打出「料理」行动牌（我方；按牌是否带「战斗行动」规则判时机）**（S8.1 / S8.8）。
pub fn play_card_me_food_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
    has_combat_action_tag: bool,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_food(
        state,
        env,
        request,
        target,
        revive,
        true,
        has_combat_action_tag,
    )
}

/// **打出「料理」行动牌**（敌方；与 [play_card_me_food] 共用一份实现，D35）。
pub fn play_card_opp_food(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_opp_food_with_timing(state, env, request, target, revive, false)
}

/// **打出「料理」行动牌（敌方；按牌是否带「战斗行动」规则判时机）**。
pub fn play_card_opp_food_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
    has_combat_action_tag: bool,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_food(
        state,
        env,
        request,
        target,
        revive,
        false,
        has_combat_action_tag,
    )
}

/// **打出「秘传」行动牌**（我方；Z12 / 秘传）。
///
/// - 打牌**前置**：edict.can_play() 必须是 Allowed（揭令仍存在 + 本局未打出过）；
///   否则 Err(ArcaneNotPlayable(原因))、入参不变；
/// - 打牌**后置**：成功结算后 record_play（张数 +1、消耗揭令），新容器放进
///   ArcanePlayOutcome.edict（调用方写回自己持有的揭令状态）。
///
/// 与 play_card_me 的区别只在「秘传判定 + 揭令记账」：普通牌不要走这个入口。
pub fn play_card_me_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, true, false)
}

/// **打出「秘传」行动牌**（我方；按牌是否带「战斗行动」规则判时机；S8.8）。
pub fn play_card_me_arcane_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, true, has_combat_action_tag)
}

/// **打出「秘传」行动牌**（敌方；与 play_card_me_arcane 共用一份实现，D35）。
pub fn play_card_opp_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, false, false)
}

/// **打出「秘传」行动牌**（敌方；按牌是否带「战斗行动」规则判时机）。
pub fn play_card_opp_arcane_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, false, has_combat_action_tag)
}
```

### 源码: `flow/turn.rs`

*注释中的关键字代码已替换为名称, 代码行未改*

```rust
use crate::families::action_card::areas::support::{SupportPending, SupportTriggers};
use crate::families::character::zones::combat_status::{
use crate::families::summon::zones::{execute_summon, SummonPending, SummonTriggers};
use crate::flow::fold_states;
use crate::flow::game_end::{settlement_outcome, Outcome};
use crate::flow::hooks::{apply_draw_branch, DrawHooks};
use crate::flow::pending::FlowPending;
use crate::keywords::target::Perspective;
use crate::resources::dice::{DicePool, DieFace};
use crate::rules::ops::opening::{is_opening_round, ElementalTransmutation};
use crate::rules::ops::order::{EffectSource, SOURCE_ORDER};
use crate::rules::ops::stage::END_PHASE_DRAW;
use crate::state::flows::end_phase::{self, EndPhasePending};
use crate::state::flows::round_begin::begin_round;
use crate::state::{DefState, PlayerDefs};
use crate::timeline::{dispatch, Event, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, nsub, EntityId, Nat};

/// 回合里**需要广播的阶段时机键**（D185：由需要广播的块自己定义）。
///
/// 这是**阶段广播**（「现在到哪个阶段了」），不是具体卡牌时机；具体卡的「结束阶段」
/// 触发登记在各区自己的注册表里、由各区在 tick 时派发（本层只做流程骨架）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TurnTiming {
    /// 回合开始（计数清零之后）。
    RoundBegin,
    /// 投掷阶段开始。
    RollPhase,
    /// 行动阶段开始。
    ActionPhaseStart,
    /// 行动阶段结束（双方都宣布回合结束后）。
    ActionPhaseEnd,
    /// 结束阶段（各区块 tick 之后）。
    EndPhase,
    /// 回合结束（进入下一个回合之前）。
    RoundEnd,
}

/// 回合同步的阶段效果函数指针：fn(&mut 对局, &Event, owner)。
pub type TurnEffect = fn(&mut DefState, &Event, EntityId);

/// 回合同步的阶段触发登记项。
pub type TurnTrigger = Trigger<TurnTiming, TurnEffect>;

/// 回合同步的阶段触发注册表（每个牌手一个具名实例，D34；本层只接收打出方 / 当前方的那一张）。
pub type TurnTriggers = TriggerRegistry<TurnTiming, TurnEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type TurnPending = Queued<TurnEffect>;

/// 回合流程的**外部依赖**（只借不拥有）。
pub struct TurnEnv<'a> {
    /// 阶段广播注册表。
    pub triggers: &'a TurnTriggers,
    /// 结束阶段 tick 用的出战状态区触发注册表。
    pub combat_status: &'a CombatStatusTriggers,
    /// 结束阶段 tick 用的召唤物区触发注册表。
    pub summons: &'a SummonTriggers,
    /// 结束阶段 tick 用的支援区触发注册表（Z6：支援牌持续回合）。
    pub supports: &'a SupportTriggers,
    /// 抓牌路径的**注入点**（F10 手牌费用表 + D188 drawn_marks 物化）。
    ///
    /// 不注入时传 DrawHooks::none()：新抓手牌的费用表不同步；drawn_marks 仍会物化进
    /// 手牌卡状态表（D188）。调用方需要费用表时注入 base_cost 回调（卡牌定义在上层）。
    pub draw: DrawHooks<'a>,
}

/// 结束阶段的结果：**分支列表**（tick 已应用、结束阶段效果已执行、双方各抓 2 张）+
/// 各区 tick / 阶段广播的待执行（保留供观测）。
///
/// **顺序（S2.4）**：第 1 步「从先手牌手开始，处理在结束阶段触发的卡牌效果」由本层作为调用方
/// **先执行**（先手方 -> 另一方；同一侧按 S7.5 来源类别 -> 进场先后）；第 2 步「每位牌手抓 2 张」
/// 在效果全部落地**之后**再做。因此返回的每个分支都已按规则书顺序结算完。
///
/// ## 为什么是分支列表（D190 / D191；F10）
///
/// 规则书·回合流程 结束阶段第 2 步：「每位牌手从自己的牌堆中抓 2 张牌。」抓牌可能遇到
/// 随机段（牌库块把它展开成「抓牌分支 + 精确概率」，不采样）；本层两侧分别展开再**组合**
/// （概率精确相乘），最后用 [fold_states] 折叠相同局面（D191）：
///
/// - 牌库没有牌 -> 这一侧**抓不到牌**（规则书·游戏区域 V：牌库空了不会获得手牌），
///   该侧按「原样一支、概率 1」处理，结束阶段照常进行；
/// - 牌库分支天然多支 -> 交叉组合成多支；局面完全相同的分支折叠、概率精确相加；
/// - 待执行（`end_phase` / `pending`）与分支无关：派发只排序、不执行（D186），每支相同。
///
/// 手牌上限 10 / 溢出自动弃置已在牌库块内处理（规则书·游戏区域 VI）。
pub struct TurnEndOutcome {
    /// 各分支（tick 与抓 2 张都已应用；顺序 = 首次出现顺序）。
    pub branches: Vec<TurnEndBranch>,
    /// 出战状态 / 召唤物 tick 派发出来的待执行（**已由本层按 S2.4/S7.5 顺序执行**；
    /// 保留供观测 / 记录，调用方不要再执行）。
    pub end_phase: EndPhasePending,
    /// 阶段广播（EndPhase）派发出来的待执行（**已由本层执行**；保留供观测，调用方不要再执行）。
    pub pending: Vec<TurnPending>,
    /// 我方支援区 tick（持续回合 -1 / 耗尽弃置）派发出来的待执行（**未执行**；D186）。
    ///
    /// 顺序 = 支援区排列顺序（规则书·游戏区域 III）；先手 / 后手的执行先后由调用方按
    /// 本次结束阶段的先手牌手决定：**先手方的 support 列表先执行**（S7.5 第 1 条）。
    pub support_me: Vec<SupportPending>,
    /// 敌方支援区 tick 派发出来的待执行（**未执行**；与 [TurnEndOutcome::support_me] 同口径）。
    pub support_opp: Vec<SupportPending>,
}

/// 结束阶段的一支分支：tick + 抓牌后的完整局面 + 精确概率 + 终局判定。
#[derive(Clone, Debug)]
pub struct TurnEndBranch {
    /// 这一支的完整局面（tick 与抓 2 张都已应用）。
    pub state: DefState,
    /// 这一支的精确概率。
    pub probability: Fraction,
    /// 这一支的终局判定（只读；调用时点见 [settlement_outcome]）。
    pub outcome: Outcome,
}

/// **回合开始**的结果：新的一局 + 阶段广播待执行 + 终局判定（回合上限）。
#[derive(Clone, Debug)]
pub struct RoundBeginOutcome {
    /// 回合开始后的局面（回合数 +1、双方每回合计数清零）。
    ///
    /// [begin_next_round] 还会把新回合的先手牌手写进 [DefState::first_player]；直接调用
    /// [round_begin] 时沿用入参局面里的值（第一回合由开局流程外部裁定）。
    pub state: DefState,
    /// [TurnTiming::RoundBegin] 派发出来的待执行。
    pub pending: Vec<TurnPending>,
    /// 回合开始后的终局判定（S7.7：第 15 回合开始仍未分胜负 -> 双方都输）。
    pub outcome: Outcome,
}

/// **元素幻变对投掷阶段的接入开关**（卡面 331004–331012「投掷阶段：总是投出 2 个 X
/// 和 2 个 Y」）。
///
/// **显式开关 / 条件**：调用方只在「对应的元素幻变卡在场 + 队伍元素集合恰等于 {X,Y}（队伍元素集合）」时
/// 传 `Some`；`None` = 没有对应卡 / 条件不满足 -> 走**普通投掷**，现有流程完全不变。
#[derive(Clone, Copy, Debug)]
pub struct RollTransmutation {
    /// 卡牌绑定的两种元素（具体卡由卡牌层给；rules 层只做通用判定）。
    pub rule: ElementalTransmutation,
    /// `true` = 第 1 轮「首次生成」语义（卡面顺序：抓 -> 弃 -> 上场后 4 骰转 2+2）；
    /// `false` = 每轮投掷阶段的「总是投出 2+2」。
    ///
    /// 两种语义的**骰子运算相同**（`apply_roll_phase` 即 `convert_opening_dice`），
    /// 但触发 / 时序不同，待实测（留空 #15 / #64）。
    pub opening: bool,
}

/// 行动阶段里「宣布回合结束」的记账：本回合先手 + 双方是否已宣布 + 下回合先手。
///
/// ## 规则书依据
///
/// - 进行行动 战斗行动【宣布结束】：「放弃本回合的行动，『宣布回合结束』。先『宣布回合结束』
///   的牌手会成为『先手牌手』，并在下回合的行动阶段优先行动。这之后，未『宣布回合结束』的
///   牌手可以继续行动，直到『宣布回合结束』。双方都『宣布回合结束』后，行动阶段结束，
///   进入本回合的『结束阶段』。」
/// - 回合流程 行动阶段：「从『先手牌手』开始，双方『轮流进行行动』。」
/// - 回合流程 结束阶段：「1、从『先手牌手』开始，处理在『结束阶段』触发的卡牌效果。」
///
/// ## 为什么由 flow 层承载（TODO）
///
/// 覆盖报告 §4 F4 的落地方案是给 `DefState` 加 `first_player: Perspective` +
/// 每方 `ended_this_round: bool`；但本次任务写范围不含 `state/**`（别的成员在改），
/// 所以先由 flow 层的记账类型承载 —— 形状与「DefState 字段」等价，搬进 `DefState` 时
/// 只需把入口里多出来的 `round` 参数删掉。
///
/// 另见模块文档「宣布回合结束与先手归属」里的两条 TODO（第一回合先手怎么定 /
/// 结束阶段用哪个「先手牌手」）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct RoundEndState {
    /// 本回合的先手牌手（决定本回合行动阶段与结束阶段的处理顺序）。
    ///
    /// 第一回合由外部裁定（TODO：规则书没写先手裁定）；之后由上一回合的先宣布者接任
    /// （[RoundEndState::start_next_round]）。**双方都宣布后**会写进根状态
    /// `DefState::first_player`（[declare_round_end_me] / [declare_round_end_opp]），
    /// 新回合开始时由 [begin_next_round] 再写一次。
    first_player: Option<Perspective>,
    /// 本回合**先**宣布回合结束的牌手（规则书 S3.6：成为下回合的先手牌手）。
    first_declarer: Option<Perspective>,
    /// 我方是否已宣布回合结束。
    me_ended: bool,
    /// 敌方是否已宣布回合结束。
    opp_ended: bool,
}

impl RoundEndState {
    /// 还没有人宣布、也还没有先手牌手（第一回合开局；先手待外部裁定）。
    pub const fn new() -> RoundEndState {
        RoundEndState {
            first_player: None,
            first_declarer: None,
            me_ended: false,
            opp_ended: false,
        }
    }

    /// 带上本回合先手牌手（第一回合由外部裁定；之后一般由上一回合的
    /// [RoundEndState::start_next_round] 给）。
    pub const fn with_first_player(first: Perspective) -> RoundEndState {
        RoundEndState {
            first_player: Some(first),
            first_declarer: None,
            me_ended: false,
            opp_ended: false,
        }
    }

    /// 本回合先手牌手（结束阶段 / 行动阶段的处理顺序起始方）；未知 -> None。
    pub const fn first_player(&self) -> Option<Perspective> {
        self.first_player
    }

    /// 本回合先宣布回合结束的牌手（= 下回合先手牌手，S3.6）；还没人宣布 -> None。
    pub const fn first_declarer(&self) -> Option<Perspective> {
        self.first_declarer
    }

    /// **下回合的行动阶段优先行动方**（S3.6：先宣布者成为先手牌手）；还没人宣布 -> None。
    pub const fn next_first_player(&self) -> Option<Perspective> {
        self.first_declarer
    }

    /// 我方是否已宣布回合结束。
    pub const fn me_ended(&self) -> bool {
        self.me_ended
    }

    /// 敌方是否已宣布回合结束。
    pub const fn opp_ended(&self) -> bool {
        self.opp_ended
    }

    /// 双方是否都已宣布回合结束（= 可以进入结束阶段，S3.6 / S2.4）。
    pub const fn both_ended(&self) -> bool {
        self.me_ended && self.opp_ended
    }

    /// 某一方是否已宣布回合结束。
    pub fn has_ended(&self, side: Perspective) -> bool {
        match side {
            Perspective::Me => self.me_ended,
            Perspective::Opp => self.opp_ended,
        }
    }

    /// **开始下一回合**：清掉本回合的宣布标记，把先宣布者接成下回合先手。
    ///
    /// - 有人先宣布 -> 下回合先手 = 先宣布者（S3.6「并在下回合的行动阶段优先行动」）；
    /// - 没人宣布（异常 / 空局面）-> 保留本回合先手，不让先手丢失。
    pub fn start_next_round(&self) -> RoundEndState {
        RoundEndState {
            first_player: self.first_declarer.or(self.first_player),
            first_declarer: None,
            me_ended: false,
            opp_ended: false,
        }
    }
}

/// 一次「宣布回合结束」的结果。
#[derive(Clone, Debug)]
pub struct DeclareRoundEndOutcome {
    /// 结算后的一局：宣布结束**不改任何区**；双方都宣布后会把本回合先手牌手写进
    /// `DefState::first_player`（= `round.first_player()`；None = 未裁定）。
    pub state: DefState,
    /// 记账后的回合结束状态。
    pub round: RoundEndState,
    /// 双方是否都已宣布（= 行动阶段结束、可以进入结束阶段）。
    pub action_phase_over: bool,
    /// 双方都宣布时派发的 [TurnTiming::ActionPhaseEnd]（只有这一种情况非空）。
    pub pending: Vec<TurnPending>,
}

/// 宣布回合结束失败的原因。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeclareRoundEndError {
    /// 这一方本回合已经宣布过了。
    AlreadyEnded,
}

/// 结束阶段失败的原因。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EndPhaseError {
    /// 还没有双方都宣布回合结束（S3.6：双方都宣布后进入结束阶段）。
    ActionPhaseNotOver,
    /// 不知道本回合先手牌手（第一回合先手由外部裁定，规则书没写；需先
    /// [RoundEndState::with_first_player]）。
    FirstPlayerUnknown,
}

/// 结束阶段里**按先手顺序**展开的一条待执行（哪一方 + 待执行本身）。
#[derive(Clone, Debug)]
pub struct EndPhaseStep {
    /// 这一条属于哪一方（视角方绑定，不是编号，D34）。
    pub side: Perspective,
    /// 待执行本身（出战状态 / 召唤物，统一成 [FlowPending]）。
    pub pending: FlowPending,
}

/// **回合开始**：双方每回合计数清零 + 回合数 +1（state::flows::round_begin）+ 派发 RoundBegin
/// + 终局判定（回合上限，S7.7）。
///
/// 依据：state::flows::round_begin::begin_round（技能使用次数清零、回合数推进）；
/// 规则书·其他规则 对局回合上限（第 15 回合开始即终局）。
pub fn round_begin(state: &DefState, env: &TurnEnv<'_>) -> RoundBeginOutcome {
    let next = begin_round(state);
    let pending = broadcast(env, TurnTiming::RoundBegin);
    let outcome = settlement_outcome(&next);
    RoundBeginOutcome {
        state: next,
        pending,
        outcome,
    }
}

/// **回合开始（下一回合）**：回合数 +1 / 计数清零 + 终局判定（[round_begin]）+ 把本回合
/// 先宣布者接成下回合先手（[RoundEndState::start_next_round]）。
pub fn begin_next_round(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> (RoundBeginOutcome, RoundEndState) {
    let next_round = round.start_next_round();
    let RoundBeginOutcome {
        state: begun,
        pending,
    // ...

/// **投掷阶段开始**：只派发阶段广播（掷骰结果由下面的 roll_me / roll_opp 给）。
pub fn roll_phase(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::RollPhase))
}

/// **我方投掷 8 颗骰**（投掷阶段；D190：返回骰池分支 + 精确概率，不采样）。
///
/// 骰池先清空再投（规则书：每回合的投掷阶段会投掷 8 个元素骰；元素骰不跨回合保留）。
/// 返回空 Vec 只可能出现在骰池已满（理论不可达：先清空了）。
/// **不带元素幻变**；有对应卡时用 [roll_me_with_transmutation]（L9）。
pub fn roll_me(state: &DefState) -> Vec<(DefState, Fraction)> {
    roll_side(state, true)
}

/// **敌方投掷 8 颗骰**（与 roll_me 共用一份实现，D35；不带元素幻变）。
///
/// 带元素幻变开关时用 [roll_opp_with_transmutation]。
pub fn roll_opp(state: &DefState) -> Vec<(DefState, Fraction)> {
    roll_side(state, false)
}

/// **我方投掷 8 颗骰（带元素幻变开关；L9）**。
///
/// `Some` 时按「8 颗里固定 2 个 X + 2 个 Y」生成：等价于「先投 8 颗、再把其中 4 颗转成 2+2」
/// 的分布（骰子独立同分布；折叠组不能直接 `convert`，所以实现取等价构造 = 4 颗随机 + 2+2 固定）。
/// `None` -> 与 [roll_me] 完全一致。
///
/// TODO（规则书 / 卡面没写清，待实测）：
/// - 「投掷 2+2」的时序（重投之前 / 之后）与「首次生成」的时序，见留空 #15 / #64；
/// - 若实测「2+2 是额外追加在 8 颗之外」或「替换的不是 4 颗」，需调整本函数。
pub fn roll_me_with_transmutation(
    state: &DefState,
    transmutation: Option<RollTransmutation>,
) -> Vec<(DefState, Fraction)> {
    roll_side_with_transmutation(state, true, transmutation)
}

/// **敌方投掷 8 颗骰（带元素幻变开关）**（与 [roll_me_with_transmutation] 共用实现，D35）。
pub fn roll_opp_with_transmutation(
    state: &DefState,
    transmutation: Option<RollTransmutation>,
) -> Vec<(DefState, Fraction)> {
    roll_side_with_transmutation(state, false, transmutation)
}

/// **我方重投**：把 rerolled 里列出的骰面各重投几颗（投掷阶段 1 次机会）。
///
/// - rerolled = (骰面, 颗数) 列表，必须都在当前骰池里（否则返回空分支：非法选择）；
/// - 重投的颗数按同样数量重新随机（不限定基础色）；
/// - 保留的骰子不动（观测已落地为具体骰面，D190：观测后展开成确定值）。
pub fn reroll_me(state: &DefState, rerolled: &[(DieFace, Nat)]) -> Vec<(DefState, Fraction)> {
    reroll_side(state, rerolled, true)
}

/// **敌方重投**（与 reroll_me 共用一份实现）。
pub fn reroll_opp(state: &DefState, rerolled: &[(DieFace, Nat)]) -> Vec<(DefState, Fraction)> {
    reroll_side(state, rerolled, false)
}

/// **行动阶段开始**：派发广播（行动权 / 行动决策由调用方驱动）。
pub fn action_phase_start(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::ActionPhaseStart))
}

/// **行动阶段结束**（双方都宣布回合结束之后）：派发广播。
pub fn action_phase_end(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::ActionPhaseEnd))
}

/// **我方宣布回合结束**（战斗行动，S3.6）：置位；先宣布者记为下回合先手。
///
/// 双方都宣布后派发 [TurnTiming::ActionPhaseEnd]、置 `action_phase_over`，并把
/// 本回合先手牌手写进 `DefState::first_player`。
pub fn declare_round_end_me(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> Result<DeclareRoundEndOutcome, DeclareRoundEndError> {
    declare_round_end(state, round, env, Perspective::Me)
}

/// **敌方宣布回合结束**（与 [declare_round_end_me] 共用一份实现，D35）。
pub fn declare_round_end_opp(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> Result<DeclareRoundEndOutcome, DeclareRoundEndError> {
    declare_round_end(state, round, env, Perspective::Opp)
}

/// **结束阶段（我方先手）**：出战状态 tick -> 召唤物 tick（state::flows::end_phase）+
/// 派发 EndPhase 广播 -> **执行结束阶段触发的效果（S2.4 第 1 步）** -> 双方各抓 2 张
/// （S2.4 第 2 步；分支 + 折叠）。
pub fn end_phase_me_first(state: &DefState, env: &TurnEnv<'_>) -> TurnEndOutcome {
    turn_end(
        end_phase::end_phase_me_first(state, env.combat_status, env.summons),
        env,
        Perspective::Me,
    )
}

/// **结束阶段（敌方先手）**（与 end_phase_me_first 共用一份实现）。
pub fn end_phase_opp_first(state: &DefState, env: &TurnEnv<'_>) -> TurnEndOutcome {
    turn_end(
        end_phase::end_phase_opp_first(state, env.combat_status, env.summons),
        env,
        Perspective::Opp,
    )
}

/// **结束阶段**（按 [RoundEndState] 记录的先手牌手选先手方）。
///
/// 规则书·回合流程 结束阶段「1、从『先手牌手』开始……」：先手方先 tick，再另一方。
/// 与 [end_phase_me_first] / [end_phase_opp_first] 的关系：本入口只是**按记账选**其中一个，
/// 不再让调用方硬选；待执行的有序展开见 [TurnEndOutcome::ordered_end_phase]。
pub fn end_phase_by_first_player(
    state: &DefState,
    round: &RoundEndState,
    env: &TurnEnv<'_>,
) -> Result<TurnEndOutcome, EndPhaseError> {
    if !round.both_ended() {
        return Err(EndPhaseError::ActionPhaseNotOver);
    }
    let Some(first) = round.first_player() else {
        return Err(EndPhaseError::FirstPlayerUnknown);
    };
    // 结束阶段分支里的 DefState 也带上先手牌手（调用方拿到即一致）。
    let state = state.clone().with_first_player(round.first_player());
    Ok(match first {
        Perspective::Me => end_phase_me_first(&state, env),
        Perspective::Opp => end_phase_opp_first(&state, env),
    })
}

/// **回合结束**：派发 RoundEnd（进入下一个回合之前的收尾；抓 2 张牌是牌库块的事）。
pub fn round_end(state: &DefState, env: &TurnEnv<'_>) -> (DefState, Vec<TurnPending>) {
    (state.clone(), broadcast(env, TurnTiming::RoundEnd))
}
```

### 源码: `flow/revive.rs`

```rust
use crate::base::CardId;
use crate::families::character::zones::field::{CharacterZonePending, CharacterZoneTriggers};
use crate::rules::ops::food::can_revive;
use crate::state::flows::revive::{
use crate::state::DefState;
use gi_genius_invokation_engine::{nadd, EntityId, Nat};

/// 一次复苏的**外部依赖**（只借不拥有）。
pub struct ReviveEnv<'a> {
    /// 角色区触发注册表（复苏会派发「进入角色区」时机）。
    pub characters: &'a CharacterZoneTriggers,
    /// **本回合**已经通过料理复苏的角色数（调用方持有；每回合上限见 rules::ops::food）。
    pub revived_this_round: Nat,
}

impl<'a> ReviveEnv<'a> {
    /// 建复苏依赖。
    pub const fn new(
        characters: &'a CharacterZoneTriggers,
        revived_this_round: Nat,
    ) -> ReviveEnv<'a> {
        ReviveEnv {
            characters,
            revived_this_round,
        }
    }
}

/// 复苏失败的原因（所有失败都保证入参不变、不消耗次数）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReviveError {
    /// 本回合的复苏次数已经用完（规则书·特殊词条 料理；每回合 1 次）。
    NoReviveLeft,
    /// 这个角色不在击倒区（没被击倒 / 已经复苏过）。
    NotRevivable,
    /// 角色区已满（3 个槽位）：放不回去。
    CharacterZoneFull,
}

/// 一次复苏的结果（纯数据）。
#[derive(Clone, Debug)]
pub struct ReviveOutcome {
    /// 复苏后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 execute_character_zone）。
    pub characters: Vec<CharacterZonePending>,
    /// 这次复苏用的「复苏料理」卡 id（记录 / 观测用；数值来自卡面，由调用方给 hp）。
    pub card: CardId,
    /// **更新后**的本回合复苏计数（= 传入值 + 1；调用方写回自己的计数）。
    pub revived_this_round: Nat,
}

/// **我方角色复苏**（L2）：次数判定 -> 击倒区取人 -> 生命复位 -> 放回角色区 -> 记次数。
///
/// - card：具有复苏效果的料理事件牌 id（只做记录 / 观测；本层不解释卡牌语义）；
/// - character：要复苏的角色实体（必须在我方击倒区）；
/// - hp：复苏后生命值（卡面给；规则书没写具体值）；
/// - 失败 -> Err、入参一个字节都不改、**不消耗**本回合复苏次数。
pub fn revive_me(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
) -> Result<ReviveOutcome, ReviveError> {
    revive_side(state, env, card, character, hp, true)
}

/// **敌方角色复苏**（与 revive_me 共用一份实现，D35）。
pub fn revive_opp(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
) -> Result<ReviveOutcome, ReviveError> {
    revive_side(state, env, card, character, hp, false)
}

/// 共用实现：on_me = true 表示复苏我方角色。
fn revive_side(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
    on_me: bool,
) -> Result<ReviveOutcome, ReviveError> {
    // 1) 次数：每回合最多通过料理复苏 1 个角色（规则书·特殊词条 料理）。
    if !can_revive(env.revived_this_round) {
        return Err(ReviveError::NoReviveLeft);

/// 复苏失败的原因（所有失败都保证入参不变、不消耗次数）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReviveError {
    /// 本回合的复苏次数已经用完（规则书·特殊词条 料理；每回合 1 次）。
    NoReviveLeft,
    /// 这个角色不在击倒区（没被击倒 / 已经复苏过）。
    NotRevivable,
    /// 角色区已满（3 个槽位）：放不回去。
    CharacterZoneFull,
}

/// 一次复苏的结果（纯数据）。
#[derive(Clone, Debug)]
pub struct ReviveOutcome {
    /// 复苏后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 execute_character_zone）。
    pub characters: Vec<CharacterZonePending>,
    /// 这次复苏用的「复苏料理」卡 id（记录 / 观测用；数值来自卡面，由调用方给 hp）。
    pub card: CardId,
    /// **更新后**的本回合复苏计数（= 传入值 + 1；调用方写回自己的计数）。
    pub revived_this_round: Nat,
}

/// **我方角色复苏**（L2）：次数判定 -> 击倒区取人 -> 生命复位 -> 放回角色区 -> 记次数。
///
/// - card：具有复苏效果的料理事件牌 id（只做记录 / 观测；本层不解释卡牌语义）；
/// - character：要复苏的角色实体（必须在我方击倒区）；
/// - hp：复苏后生命值（卡面给；规则书没写具体值）；
/// - 失败 -> Err、入参一个字节都不改、**不消耗**本回合复苏次数。
pub fn revive_me(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
) -> Result<ReviveOutcome, ReviveError> {
    revive_side(state, env, card, character, hp, true)
}

/// **敌方角色复苏**（与 revive_me 共用一份实现，D35）。
pub fn revive_opp(
    state: &DefState,
    env: &ReviveEnv<'_>,
    card: CardId,
    character: EntityId,
    hp: Nat,
) -> Result<ReviveOutcome, ReviveError> {
    revive_side(state, env, card, character, hp, false)
}
```

### 源码: `flow/technique.rs`

*注释中的关键字代码已替换为名称, 代码行未改*

```rust
use crate::families::character::skill_flow::technique::{
use crate::families::character::skill_flow::{SkillPending, SkillTriggers};
use crate::families::character::zones::equipment::EquipmentZone;
use crate::resources::dice::CostComponent;
use crate::rules::ops::stage::Timing;
use crate::state::DefState;
use gi_genius_invokation_engine::EntityId;

/// 特技流程的**外部依赖**（只借不拥有）。
pub struct TechniqueEnv<'a> {
    /// 「使用技能时 / 后」的注册表 —— 只用来调用 dispatch_technique 证明**恒空**（特技）。
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
    /// 特技第 4 条：特技不派发「使用技能时 / 后」-> **恒空**（可观测 / 可测试）。
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
    /// 特技第 4 条：特技不派发「使用技能时 / 后」-> **恒空**（可观测 / 可测试）。
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
```

### 源码: `flow/elemental_transmutation.rs`

*注释中的关键字代码已替换为名称, 代码行未改*

```rust
use crate::rules::ops::opening::{is_opening_round, ElementalTransmutation};
use crate::state::DefState;

/// **投掷阶段 2+2 转换（我方）**：把 4 颗骰转成该卡两元素各 2 颗（总数不变）。
///
/// 调用时点：`flow::turn::roll_me` 的某一支骰池观测完成之后（本钩子不改 roll 流程）。
/// 骰子不足 4 -> None（不改入参）。
pub fn transmute_roll_phase_me(state: &DefState, rule: ElementalTransmutation) -> Option<DefState> {
    let converted = rule.apply_roll_phase(&dice_of(state, true))?;
    Some(with_dice(state, true, converted))
}

/// **投掷阶段 2+2 转换（敌方）**（与 [transmute_roll_phase_me] 共用一份实现，D35）。
pub fn transmute_roll_phase_opp(
    state: &DefState,
    rule: ElementalTransmutation,
) -> Option<DefState> {
    let converted = rule.apply_roll_phase(&dice_of(state, false))?;
    Some(with_dice(state, false, converted))
}

/// **「首次生成」4 骰 -> 2+2（我方）**：只在**第 1 轮**生效（留空 #64 的判据）。
///
/// 调用时点：第 1 轮行动阶段开始时（具体在抓牌 / 弃牌 / 上场前后的哪一步待实测，见模块 TODO）。
/// 非第 1 轮 / 骰子不足 4 -> None（不改入参）。
pub fn transmute_opening_me(state: &DefState, rule: ElementalTransmutation) -> Option<DefState> {
    if !is_opening_round(state.round()) {
        return None;
    }
    let converted = rule.convert_opening_dice(&dice_of(state, true))?;
    Some(with_dice(state, true, converted))
}

/// **「首次生成」4 骰 -> 2+2（敌方）**（与 [transmute_opening_me] 共用一份实现，D35）。
pub fn transmute_opening_opp(state: &DefState, rule: ElementalTransmutation) -> Option<DefState> {
    if !is_opening_round(state.round()) {
        return None;
    }
    let converted = rule.convert_opening_dice(&dice_of(state, false))?;
    Some(with_dice(state, false, converted))
}
```

### 源码: `rules/ops/stage.rs` (`play_card_*` 引用的行动时机判定) 

```rust
use gi_genius_invokation_engine::Nat;

/// 打出一张**行动牌**的行动权：默认快速行动；带「战斗行动」标签时改为战斗行动
/// （规则书·特殊词条 战斗行动（行动牌））。
pub fn timing_of_card_play(has_combat_action_tag: bool) -> Timing {
    if has_combat_action_tag {
        Timing::Combat
    } else {
        Timing::Fast
    }
}
```

## 4. 不变量与禁止

> [!IMPORTANT]
> 纯函数: 入参 `&DefState` 不变; 失败 `Err` / 无分支时状态不变.
> 所有随机点展开为分支 + 精确 `Fraction`, 不采样; 分支概率之和应为 1.
> 步是原子单位 (D204) : 步内不插入; flow 只在步末 / 步边界 flush / 插入.

> [!WARNING]
> `before` (技能 / 打出前) 已由 flow 执行; `PlayCardOutcome::after`, `support_me/opp` 与分支 `pending` 未执行.
> `TurnEndOutcome::end_phase` / `pending` 已按序执行, 调用方不要再执行.
> 行动权推进不在 flow: `timing` 只报告时机, 不切换行动方.

## 5. 依据 (规则书与 D 编号) 

| 规则 / 决策 | 落点 |
|---|---|
| S3.1 行动 (快速 / 战斗) + S8.8 战斗行动 (行动牌)  | `play_card_*_with_timing` / `timing_of_card_play` (D263)  |
| S2.4 结束阶段'1 效果 -> 2 抓牌' | `turn::turn_end` (D262)  |
| S7.5 结算顺序 (当前牌手 -> 来源类别 -> 进场先后)  | `skill_use` / `play_card` / `turn` 的派发排序 |
| S8.10 下落攻击 | `use_skill_*_with_plunging` + `battle_action_end_*` (D256 / D261)  |
| S8.1 料理 (饱腹 / 每回合复苏 1 次)  | `play_card_*_food` + `flow::revive` (L1 / L2)  |
| 特技 (战斗行动, 不派发 SkillUsed, 伤害不算角色造成)  | `flow::technique` (L4)  |
| Z12 / 秘传 (每位牌手 1 张 / 揭令消耗)  | `play_card_*_arcane` (D264)  |
| Z6 支援牌持续回合 | `CardPlacement::Support{duration}` + support tick |
| L9 元素幻变 | `roll_*_with_transmutation` + `flow::elemental_transmutation` |
| D186 派发只排序, 不执行 | `pending.rs` + 各入口字段口径 |
| D190 / D191 观测展开 + 折叠 | 各分支 + `fold_states` |
| D196 插入式结算 (不是栈)  | `timeline::Resolution` / `SkillFrames` / `ReactionFrames` |
| D204 / D208 一步原子, Repeat 同属一步 | `execute_step_*` / `execute_steps_*` |
| D225 / D227 流程记录 | `FlowRecord` + `flow/recorded.rs` |
| D255 结算 -> 清理 -> 终局 | `DefeatCleanup` 注入 |
| D267 / D268 双负 | `settlement_outcome` -> `Outcome::BothLose` |
| D270 第七轮收口 | 本页对应 1130 tests 基线 |

## 6. 常见误用

> [!WARNING]
> 把 `SkillOutcome::before` / `PlayCardOutcome::before` 再执行一次; 它们已执行, 仅供观测 / 记录.
> 在一步执行器内手动 insert / execute_pending; 插入只能在步边界 (D204) .
> 打秘传牌走普通 `play_card_me`; 普通入口不做揭令判定 / 记账 (Z12 专门入口) .

> [!TIP]
> 终结前要清理击倒: 通过 `SkillFlowEnv::defeat_cleanup` / `PlayCardEnv::defeat_cleanup` 注入真实实现 (默认 no-op) .
> 抓牌后要费用表正确: 结束阶段注入 `TurnEnv::draw = DrawHooks::with_base_cost(...)`; 开局抓牌在 `state/flows/opening` 自带 `base_cost`.

- 打料理牌不传 `FoodReviveEnv` 却期望目标在击破区: 会得到 `FoodTargetNotAllowed`.
- 把 `TechniqueOutcome::skill_used` 当成会派发: 它恒空 (特技不触发使用技能时机) .
- 用 `transmute_roll_phase_*` 代替 `roll_*_with_transmutation`: 直投钩子只转 4 颗骰, 不生成 8 颗骰分支.

## 7. 测试

- `flow/skill_use.rs`: `skill_end_to_end_pays_damages_and_runs_reaction`, `skill_fails_without_enough_dice_and_changes_nothing`, `skill_pays_energy_for_burst_style_effects`, `skill_damage_goes_through_shield_and_reduction`, `after_skill_pending_is_inserted_at_the_current_settlement_position`, `before_skill_pending_is_dispatched_and_executed_before_the_effect_body`, `passive_skill_cannot_be_used_and_changes_nothing`, `damage_plan_without_enemy_active_character_is_rejected`, `opp_entry_uses_the_same_pipeline_and_binds_sides_correctly`, `step_effect_body_matches_the_plan_path_on_the_same_damage`, `step_pending_is_flushed_at_the_step_boundary_before_the_after_insertion`, `after_skill_triggers_follow_current_player_first_*`, `plunging_mark_turns_the_next_normal_attack_into_plunging_and_is_cleared`, `use_skill_reads_the_plunging_mark_from_state_and_clears_it_after_the_action`, `defeat_cleanup_runs_before_the_terminal_judgement`, `skill_branch_outcome_waits_for_defeat_cleanup`.
- `flow/play_card.rs`: `support_card_goes_to_the_support_zone_and_pays_dice`, `full_support_zone_returns_one_branch_per_discard_choice`, `equipment_card_goes_to_the_equipment_zone_of_the_target_character`, `event_card_step_effect_runs_at_the_step_boundary`, `before_play_pending_is_executed_and_after_pending_is_returned`, `condition_is_checked_before_the_cost_check`, `play_card_triggers_follow_current_player_first_with_two_registries`, `card_play_timing_is_fast_unless_the_card_has_the_combat_action_rule`, `arcane_play_consumes_the_edict_and_rejects_the_second_play`, `food_play_marks_the_target_satiated`, `revive_food_puts_the_target_back_and_marks_it_satiated`, `defeat_cleanup_runs_before_the_terminal_judgement`.
- `flow/turn.rs`: `round_begin_resets_skill_uses_and_dispatches_the_broadcast`, `roll_me_returns_eight_dice_branches_with_probability_one`, `reroll_keeps_dice_count_and_replaces_the_selected_ones`, `end_phase_ticks_combat_status_and_summons`, `end_phase_ticks_support_durations_and_returns_pending_to_callers`, `end_phase_draws_two_cards_for_each_side`, `end_phase_draw_branches_combine_with_exact_probabilities`, `end_phase_branch_carries_the_terminal_outcome`, `declare_round_end_records_first_declarer_and_opens_the_end_phase`, `first_declarer_becomes_the_next_round_first_player`, `ordered_end_phase_follows_the_first_player`, `end_phase_effects_execute_before_the_draw`, `roll_with_transmutation_guarantees_two_plus_two_among_eight_dice`, `opening_transmutation_applies_in_round_one`, `end_phase_draw_syncs_costs_and_materializes_drawn_marks`.
- `flow/game_end.rs`: `settlement_outcome_reads_the_state_without_changing_it`, `unstarted_or_ongoing_games_are_not_terminal`.
- `flow/hooks.rs`: `draw_hooks_sync_the_cost_table_for_new_hand_cards`, `draw_branch_materializes_drawn_marks_into_hand_card_states`, `apply_defeat_cleanup_cleans_both_sides_and_merges_pending`, `default_cleanup_is_a_noop`.
- `flow/revive.rs`: `revive_puts_the_character_back_and_counts_once`, `revive_is_rejected_when_the_round_count_is_used_up`, `revive_rejects_characters_that_are_not_defeated`, `revive_rejects_a_full_character_zone`, `revive_opp_does_not_touch_me`.
- `flow/technique.rs`: `technique_is_a_combat_action_and_dispatches_no_skill_used`, `technique_without_a_card_cost_does_not_pay`, `technique_rejection_reasons_are_surfaced`, `technique_rejects_when_the_card_cost_cannot_be_paid`, `use_technique_opp_binds_the_other_side`.
- `flow/elemental_transmutation.rs`: `roll_phase_hook_turns_four_dice_into_two_plus_two`, `roll_phase_hook_rejects_fewer_than_four_dice_without_changing_state`, `opening_hook_only_applies_in_round_one`, `hook_is_additive_and_does_not_change_existing_flow_results`.

## 8. 待接入与待实测

> [!NOTE]
> 旧留空 #66 / #67 / #68 / #89 / #90 / #97 / #105 / #108 / #110 / #122 多条已被第七轮实现覆盖, 
> 只更新了 §二十二 汇总; 正文措辞待统一 (详见交付消息与留空正文) .

- `DefeatCleanup` 现为整侧纯函数 (非逐角色 + `DefeatHpReset` 形式) ; 只注入 `use_skill_*` / `play_card_*`, 结束阶段分支未注入 (留空 §二十二 #122) .
- `drawn_marks` 物化不派发'手牌卡状态加上'触发 (留空 §二十二 #122) .
- 结束阶段阶段广播只有一张 `TurnTriggers` (对方侧待接入, 留空 #91) .
- 支援牌持续回合递减时机 (留空 #67/#93) 与'支援效果 vs 抓牌'顺序偏差 (留空 #94) .
- L9 幻变时序与触发条件由卡牌层给 (留空 #108/#118/#122) .
- 秘传揭令容器由调用方持有 (`ArcaneEdict`, D264) ; 判定与 D217 条件先后规则书未写 (留空 #99) .
- 复活与料理生效先后 / 复苏后生命值 / 目标阵营限制: 留空 #109/#119.
- 特技是否占每回合使用次数 / 费用规则 / 目标合法性: 留空 #85/#86/#101.
- 记录器路径: `play_card_*_recorded_with_timing` 已有; `use_skill_*_recorded` 无 `_with_plunging` 变体 (待确认) .
- 行动权推进不归 flow (`DefRules::to_move` 仍是占位, 见 [[定义层-State]]) .

---

最后核对: 2026-10-19 (代码: `crates/definitions/src/flow/` 各文件; `flow/hooks.rs`:DefeatCleanup / DrawHooks) 
