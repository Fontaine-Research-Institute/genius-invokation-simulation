# Definitions Layer: State

> [[定义层-State|中文]] · English

*Scope: the pure-data state of a match (`DefState` / `PlayerDefs`), the per-side zone tables, and the read-only terminal check.*

---

## 1. Responsibility

The `state` layer defines the **complete pure-data state of one match** (`DefState` = our `PlayerDefs` + the opponent's `PlayerDefs` + round counter + first player) and exposes a **read-only** terminal check. Every flow (`flow/*`) takes `&DefState` and returns a **new** `DefState`; the input is never mutated.

## 2. Key concepts and types

### 2.1 Root state `DefState`

File: `crates/definitions/src/state/def_state.rs`. All fields are private and read-only.

| Field | Type | Meaning |
|---|---|---|
| `me` / `opp` | `PlayerDefs` | Two named player instances (D34: no side indexes) |
| `round` | `Nat` | Round counter; `0` = first round has not started, +1 at each round start, capped by `ROUND_LIMIT` |
| `first_player` | `Option<Perspective>` | First player of this round (D260); `None` = not decided, not a default of our side |

`Perspective` lives in `crates/definitions/src/base/perspective.rs` (D260: state must not depend on keywords); the old path `keywords::target::Perspective` is still re-exported.

### 2.2 All zones of a player `PlayerDefs`

File: `crates/definitions/src/state/player.rs`. Fields are private and exposed via read-only pointers (D183); they map to the rulebook's Game Zones.

| Zone / table | Type (file) | Notes |
|---|---|---|
| Dice pool | `DicePool` (`resources/dice/mod.rs`) | Pure counters, cap 16 (D47/D48) |
| Hand | `HandState` (`families/action_card/zones/hand/mod.rs`) | Cap 10; opening hand 5 (D90) |
| Pile | `PileState` (`families/action_card/zones/pile/mod.rs`) | Position-keyed + segments + shared state list + pending random marks (D93/D105/D188) |
| Hand-card states | `HandCardStates` (`families/action_card/areas/hand_state.rs`) | Cost reduction / Empowerment / Untunable / Invalidation, etc. (Z13) |
| Current cost per hand card | `HandCosts` (`state/player.rs`) | Entity id -> `CurrentCost`; base cost from the upper layer, deltas stacked (D170/D172/D178) |
| Character zone | `CharacterZone` (`families/character/zones/field.rs`) | 3 slots + active pointer; slot order preserved after a character leaves |
| Combat status zone | `CombatStatusZone` (`families/character/zones/combat_status.rs`) | Attached to the active character; transfers on switch |
| Per-character equipment zone | `Vec<EquipmentZone>` (`families/character/zones/equipment.rs`) | One instance per character |
| Summon zone | `SummonZone` (`families/summon/zones/mod.rs`) | Cap 4, same-name merge, usages |
| Support zone | `SupportZone` (`families/action_card/areas/support.rs`) | Cap 4; support cards may carry `SupportDuration` (Z14) |
| Per-round skill usage counters | `SkillUses` (`families/character/skill_flow/usage.rs`) | `reset_round` at round start |
| Plunging-attack mark | `PlungingMark` (`families/character/skill_flow/attack_form.rs`) | One per side; set after a switch, cleared at combat-action end / round boundary (D261 / S8.10) |
| Defeat zone | `Vec<CharacterSlot>` (this file) | Internal record: identity + post-cleanup state for revive; not a rulebook-visible zone (留空 #75) |

Accessors: `dice()/hand()/pile()/costs()/cost_of()/characters()/combat_status()/equipment()/equipment_of()/summons()/supports()/hand_states()/skill_uses()/plunging()/defeated()/defeated_character()`; all derivations are `with_*`. The only mutable entry point, `pub(crate) parts_mut`, is reserved for `state::flows`.

### 2.3 Terminal check `Outcome`

File: `crates/definitions/src/state/terminal.rs`. Order: `round == 0` -> `Ongoing`; both character zones empty -> `BothLose`; exactly one side empty -> the other side wins; both sides have characters and `round >= 15` -> `RoundLimit` (both lose); otherwise `Ongoing`.

| Result | Meaning |
|---|---|
| `Ongoing` | The match continues (round 1 not started / both sides have characters below the limit) |
| `MeWins` / `OppWins` | One character zone is empty; the other side wins (S1.4) |
| `RoundLimit` | Start of round 15 with no winner: both lose (S7.7) |
| `BothLose` | Both character zones empty at a step boundary: both lose, not a draw (D267/D268) |

> [!IMPORTANT]
> Wipe-out is judged by "character zone is empty", not by "number of characters with HP > 0".
> HP 0 without defeat cleanup still yields `Ongoing` (S7.3 / S7.4).
> Judgement happens only at step boundaries; no early termination inside a step (D204 / D267).

## 3. Public entry points

### Index

Source blocks are copied verbatim from the current repository (`use` / `derive` / `impl` lines included; file paths in the subsection headings). Long functions list the doc comment, signature, and the first few body lines only; `// ...` marks omitted body.

| Entry / type | Notes |
|---|---|
| `DefState` | `state/def_state.rs`: full match state (`me` / `opp` / `round` / `first_player`) and all public methods |
| `PlayerDefs` | `state/player.rs`: all zones of one side; the source block is an `impl` excerpt of key derivations |
| `terminal` / `Outcome` / `ROUND_LIMIT` | `state/terminal.rs`: read-only terminal check and the round-15 limit |

### Source: `state/def_state.rs` (`use` + `DefState` + `impl DefState`)

```rust
use crate::base::Perspective;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::Nat;

/// **一局对局的完整状态**：我方全部区 + 敌方全部区（两个具名实例，D34）+ 回合数。
///
/// 容器没有身份（D42）：它就是「这一局」这个变量本身，不出现对局号、不登记。
#[derive(Clone, Debug, PartialEq)]
pub struct DefState {
    /// 我方的全部区。
    me: PlayerDefs,
    /// 敌方的全部区。
    opp: PlayerDefs,
    /// 回合数（0 = 还没开始第一回合；每回合开始 +1；上限见
    /// `crate::state::terminal::ROUND_LIMIT`）。
    ///
    /// 依据：规则书·其他规则 对局回合上限「第15回合开始时，如果仍未分出胜负，则双方都会输掉
    /// 这场对局」—— 判定需要知道「现在是第几回合」，所以回合数由根状态持有。
    round: Nat,
    /// **本回合的先手牌手**（行动阶段 / 结束阶段的处理起始方；[`None`] = 还没裁定）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】「先『宣布回合结束』的牌手会成为
    /// 『先手牌手』，并在下回合的行动阶段优先行动」；其他规则·结算顺序第 1 条
    /// 「先结算当前牌手的所有卡牌效果」。
    ///
    /// 写入方：开局 / 加载流程（第一回合由外部裁定；之后由上一回合的先宣布者接任）。
    /// 读取方：引擎求解接口 `state::terminal::DefRules::to_move`（当前轮到谁行动）。
    first_player: Option<Perspective>,
}

impl DefState {
    /// 空的一局（双方都是完全空的一方；回合数 0 = 第一回合还没开始）。
    pub const fn new() -> DefState {
        DefState {
            me: PlayerDefs::new(),
            opp: PlayerDefs::new(),
            round: Nat::ZERO,
            first_player: None,
        }
    }

    /// 用两方各自的全部区建一局（建局 / 测试注入初始状态用；回合数从 0 开始、先手未知）。
    pub const fn from_players(me: PlayerDefs, opp: PlayerDefs) -> DefState {
        DefState {
            me,
            opp,
            round: Nat::ZERO,
            first_player: None,
        }
    }

    /// 我方的全部区（**只读指针**，D183）。
    pub fn me(&self) -> &PlayerDefs {
        &self.me
    }

    /// 敌方的全部区（**只读指针**，D183）。
    pub fn opp(&self) -> &PlayerDefs {
        &self.opp
    }

    /// 当前回合数（0 = 还没开始第一回合；1 = 第 1 回合）。
    ///
    /// 推进点只有一个：回合开始流程 `crate::state::flows::round_begin::begin_round`。
    pub fn round(&self) -> Nat {
        self.round
    }

    /// 换掉回合数（纯函数：返回新的一局，不改入参；建局 / 测试注入用）。
    ///
    /// 正式流程不要直接调它 —— 回合推进走
    /// `crate::state::flows::round_begin::begin_round`（+1 并处理第 15 回合边界）。
    pub fn with_round(self, round: Nat) -> DefState {
        DefState { round, ..self }
    }

    /// **本回合的先手牌手**（[`None`] = 还没裁定；只读指针，D183）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】+ 其他规则·结算顺序第 1 条。
    pub fn first_player(&self) -> Option<Perspective> {
        self.first_player
    }

    /// 换掉先手牌手（纯函数：返回新的一局，不改入参）。
    ///
    /// - 写入方：**只有开局 / 加载流程**（第一回合由外部裁定；之后由上一回合的先宣布者接任，
    ///   见 flow 层的宣布结束流程）；
    /// - [`None`] = 还没裁定（建局后、第一回合先手确定之前）；
    /// - 用 [`Option`] 而不是「默认我方」：规则书没有规定第一回合先手归谁，不自行编一个
    ///   默认值（默认值只出现在引擎适配 `to_move` 的**占位**里，见 `state::terminal`）。
    pub fn with_first_player(self, first_player: Option<Perspective>) -> DefState {
        DefState {
            first_player,
            ..self
        }
    }

    /// 换掉我方的全部区（纯函数：返回新的一局，不改入参）。
    pub fn with_me(self, me: PlayerDefs) -> DefState {
        DefState { me, ..self }
    }

    /// 换掉敌方的全部区（纯函数：返回新的一局，不改入参）。
    pub fn with_opp(self, opp: PlayerDefs) -> DefState {
        DefState { opp, ..self }
    }

    /// 我方的可变入口（`pub(crate)`；只有 `state::flows` 用）。
    pub(crate) fn me_mut(&mut self) -> &mut PlayerDefs {
        &mut self.me
    }

    /// 敌方的可变入口（`pub(crate)`；只有 `state::flows` 用）。
    pub(crate) fn opp_mut(&mut self) -> &mut PlayerDefs {
        &mut self.opp
    }
}
```

### Source: `state/player.rs` (excerpt: `PlayerDefs` and key derivations; the `impl` line and selected methods are verbatim)

```rust
use crate::base::CardId;
use crate::families::action_card::areas::cost::CurrentCost;
use crate::families::action_card::areas::hand_state::HandCardStates;
use crate::families::action_card::areas::support::SupportZone;
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::pile::PileState;
use crate::families::character::skill_flow::{PlungingMark, SkillUses};
use crate::families::character::zones::combat_status::CombatStatusZone;
use crate::families::character::zones::equipment::EquipmentZone;
use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
use crate::families::summon::zones::SummonZone;
use crate::resources::dice::{CostComponent, DicePool};
use gi_genius_invokation_engine::{EntityId, Nat};

/// **一方的全部区**（我方 / 敌方各一个实例，D34）。
///
/// 容器没有身份（D42 / D182）：它就是 `DefState` 里的一个字段；「这是谁的全部区」由
/// `DefState.me` / `DefState.opp` 两个具名实例表达，本类型不出现敌我参数、不出现区号。
///
/// 相等语义：`PartialEq` 按字段比较，**统计不参与**（各容器的 `PartialEq` 已忽略 stats，D187）。
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerDefs {
    /// 骰池（纯计数，上限 16）。
    dice: DicePool,
    /// 手牌（上限 10）。
    hand: HandState,
    /// 牌库（位置为键 + 段 + 公共状态列表）。
    pile: PileState,
    /// 每张手牌的当前费用。
    costs: HandCosts,
    /// 角色区（3 槽位 + 出战指针）。
    characters: CharacterZone,
    /// 出战状态区（挂在出战角色上）。
    combat_status: CombatStatusZone,
    /// 每角色一个装备栏（用 `EquipmentZone::owner()` 对上是哪个角色）。
    equipment: Vec<EquipmentZone>,
    /// 召唤物区（上限 4）。
    summons: SummonZone,
    /// 支援区（上限 4）。
    supports: SupportZone,
    /// 手牌卡状态（减费 / 赋能…，按手牌实体号）。
    hand_states: HandCardStates,
    /// 每回合技能使用计数。
    skill_uses: SkillUses,
    /// **下落攻击**待消耗标记（S8.10：切换为出战角色后，本回合下一个战斗行动若为普通攻击
    /// 则视为下落攻击）。
    ///
    /// 写入点 / 清除点见 `state::flows::plunging`；回合边界由
    /// `state::flows::round_begin::begin_round` 自动清除。
    plunging: PlungingMark,
    /// **击倒区**：被击倒清理移出角色区的角色（身份 + 清理后的运行期状态）。
    ///
    /// - **规则书没有这个可见区域**：其他规则 击倒只说「角色就会被击倒」，
    ///   没说击倒后的角色牌放在哪（`defeat.rs` 的待复核同款问题）；
    /// - 这里只是**内部记录**：不参与结算、不派发时机，唯一用途是让复苏
    ///   （`state::flows::revive`）把角色放回角色区（需要角色身份 + 生命上限等数据）；
    /// - 顺序 = 被击倒的先后（可复现）；TODO：规则书 / 实测若给出正式「击倒区」规则，按它改。
    defeated: Vec<CharacterSlot>,
}

impl PlayerDefs {
    /// 跟当前手牌同步费用表（见 [`HandCosts::synced`]）。
    pub fn with_synced_hand_costs(
        &self,
        base_cost: &dyn Fn(CardId) -> Vec<CostComponent>,
    ) -> PlayerDefs {
        let mut next = self.clone();
        next.costs = self.costs.synced(&self.hand, base_cost);
        next
    }

    /// 回合开始：所有技能的「本回合已用次数」清零（上限保留；规则书·回合流程「开始新的回合」）。
    ///
    /// 复用 `skill_flow::usage::SkillUses::reset_round`（纯函数）。
    pub fn with_skill_uses_reset(&self) -> PlayerDefs {
        let mut next = self.clone();
        next.skill_uses = self.skill_uses.reset_round();
        next
    }

    /// **下落攻击待消耗标记**（S8.10；纯值拷贝，不是指针）。
    pub fn plunging(&self) -> PlungingMark {
        self.plunging
    }

    /// **击倒区**（只读指针；顺序 = 被击倒先后；内部记录，不是规则书可见区域）。
    pub fn defeated(&self) -> &[CharacterSlot] {
        &self.defeated
    }

    /// 击倒区里某个角色（**只读指针**；不在击倒区 -> None）。
    pub fn defeated_character(&self, character: EntityId) -> Option<&CharacterSlot> {
        self.defeated
            .iter()
            .find(|slot| slot.character == character)
    }

    /// 写入**下落攻击待消耗标记**（纯函数；切换成功后把 flow 返回的
    /// `SwitchCharacterOutcome::plunging` 写回这里）。
    pub fn with_plunging(&self, plunging: PlungingMark) -> PlayerDefs {
        PlayerDefs {
            plunging,
            ..self.clone()
        }
    }

    /// **一次战斗行动结束**：清除下落攻击标记（S8.10；纯函数）。
    ///
    /// 调用点见 `state::flows::plunging::battle_action_end_me/opp`。
    pub fn with_plunging_after_combat_action(&self) -> PlayerDefs {
        self.with_plunging(self.plunging.after_combat_action())
    }

    /// **回合边界**：清除下落攻击标记（S8.10「本回合内」；纯函数）。
    ///
    /// `state::flows::round_begin::begin_round` 已自动调用，不需要调用方再做。
    pub fn with_plunging_cleared_round(&self) -> PlayerDefs {
        self.with_plunging(self.plunging.cleared_round())
    }
}
```

### Source: `state/terminal.rs` (`use` + terminal check)

```rust
use crate::state::def_state::DefState;
use gi_genius_invokation_engine::{nadd, Nat};

/// **对局回合上限**：第 15 回合（规则书·其他规则 对局回合上限）。
///
/// 规则书原文：「第15回合开始时，如果仍未分出胜负，则双方都会输掉这场对局。」
pub const ROUND_LIMIT: Nat = Nat::new(15);

/// 回合数是否已经到达 / 超过上限（第 15 回合开始及其后）。
pub fn at_round_limit(round: Nat) -> bool {
    round >= ROUND_LIMIT
}

/// **推进一个回合**（回合开始用）：+1；到 [`ROUND_LIMIT`] 后**不再增长**。
///
/// 为什么封顶：第 15 回合开始时游戏就结束了（S7.7），不存在第 16 回合；封顶是对
/// 「调用了不该再调用的推进」的边界处理，不改变任何规则语义。
pub fn next_round(round: Nat) -> Nat {
    if round < ROUND_LIMIT {
        nadd!(round, Nat::new(1))
    } else {
        round
    }
}

/// **终局判定的结果**（纯判定值；引擎侧映射见 [`DefRules`]）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Outcome {
    /// 对局继续（还没开始第一回合、双方都有角色且未到回合上限）。
    Ongoing,
    /// 我方获胜：敌方所有角色牌都已被击倒（S1.4）。
    MeWins,
    /// 敌方获胜：我方所有角色牌都已被击倒（S1.4）。
    OppWins,
    /// 第 15 回合开始仍未分胜负：**双方都输**（S7.7）。引擎侧 = `GameResult::RoundLimit`。
    RoundLimit,
    /// **双方同时全灭 = 双方都输**（D267 / D268 用户明确：「一步内有人全阵亡不下一步，但是本步执行完」）。
    ///
    /// 不是特例：同一步内双方各自被打空，步执行完在步边界判 -> 自然就是双负。
    ///
    /// - 步边界上双方角色区都为空 -> 双负；**不是平局**，也不是任何一方胜；
    /// - 规则书 S1.4 / S7.7 没有这一条，这是**用户裁定**（不是本模块自编）；
    /// - 引擎侧 = `GameResult::BothLose`。
    BothLose,
}

impl Outcome {
    /// 是否已经终局（[`Outcome::Ongoing`] 之外都是）。
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Outcome::Ongoing)
    }
}

/// **终局判定**（纯函数：只读 `&DefState`，不改任何区）。
///
/// 判定顺序（语义即规则书条文顺序 + D267）：
/// 1. 回合数为 0（第一回合还没开始 / 空建局）-> [`Outcome::Ongoing`]；
/// 2. 双方角色区都为空 -> [`Outcome::BothLose`]（D267：双方都输）；
/// 3. 某一方角色区为空 -> 对方获胜（S1.4）；
/// 4. 双方都有角色且回合数到 15 -> [`Outcome::RoundLimit`]（S7.7「仍未分出胜负」）；
/// 5. 其余 -> [`Outcome::Ongoing`]。
///
/// **调用时点 = 步边界**（D267）：一步全部执行完才调；步内「只有一方空」的中间态不判终局。
pub fn terminal(state: &DefState) -> Outcome {
    // 还没开始第一回合：没有可判的终局（建局 / 开局准备阶段）。
    if state.round() == Nat::ZERO {
        return Outcome::Ongoing;
    }
```

## 4. Invariants and prohibitions

> [!IMPORTANT]
> All constructors / derivations take `&self` or `self` and return a new value; `DefState` has no public mutable entry point (D183).
> The two sides are two named instances; no player index / side id parameters (D29 / D34).
> The only place that advances `round` is `state::flows::round_begin::begin_round` (`next_round`, clamped at 15).

> [!WARNING]
> Do not judge "wiped out" before defeat cleanup: HP 0 is not a defeat.
> Do not treat `defeated` as the rulebook's defeat zone; it dispatches no timings and takes no part in settlement (留空 #75).
> Do not add a special-case branch for "both sides wiped simultaneously"; it follows from step-boundary judgement (D267/D268).

> [!CAUTION]
> Do not compute damage taken / rankings from the fixed Max HP in card definitions.
> The only runtime actual value is `CharacterState::max_hp` (D238); `DefeatHpReset.definition` is the definition value used for the defeat reset.

## 5. Basis (rulebook and D numbers)

| Rule / decision | Landing point |
|---|---|
| S1.4 victory condition | `MeWins` / `OppWins` of `terminal` |
| S7.3 defeat (only after settlement completes) | HP 0 does not immediately lose; only `state::flows::defeat` cleanup counts |
| S7.4 saved from defeat | character still in the character zone -> `Ongoing` |
| S7.7 round limit (start of round 15, both lose) | `ROUND_LIMIT` / `RoundLimit` / `next_round` clamp |
| Game zones I–VI (characters / hand / support / summons / pile / elemental dice) | `PlayerDefs` zone fields |
| D34 / D42 two named instances, no zone registry | `me/opp` fields |
| D182 / D183 zones own their data + read-only pointers | private fields + `&T` accessors |
| D204 a step is the atomic unit | terminal check only at step boundaries |
| D238 Max HP definition / actual value | `DefeatHpReset`, `CharacterState::max_hp` |
| D255 the terminal check is read-only in flow | thin `flow::game_end::settlement_outcome` |
| D260 first player stored in `DefState`, `Perspective` moved to base | `first_player` field |
| D261 a switch arms the plunging mark | `PlayerDefs::plunging` + `state::flows::switch` |
| D267 / D268 / D269 double loss = `BothLose` | `Outcome::BothLose` + engine `GameResult::BothLose` |
| D270 round-7 wrap-up | this page matches the 1130-test baseline |

## 6. Common misuse

> [!WARNING]
> Judging a wipe-out with `slots().iter().filter(hp > 0).count()`; use `characters().is_empty()` instead.
> Calling `terminal` in the middle of damage settlement (inside a step) and ending early; judge only at step boundaries (D204 / D267).
> Deciding the winner from whether `defeated()` is empty; the revive window only exists after settlement and the rulebook does not define a pre-terminal defeat-zone check (留空 #41/#65).

- Computing damage taken / lowest HP with `CharacterDef::max_hp`: the runtime `CharacterState::max_hp` is mandatory.
- Assuming `first_player` defaults to our side: when undecided, `ME` in `to_move` is only a placeholder.
- Calling `with_round(r)` directly to bypass `begin_round`, or letting `next_round` grow past 15.

## 7. Tests

- `state/terminal.rs`: `both_sides_wiped_at_the_step_boundary_means_both_lose`, `step_is_atomic_so_the_mid_step_single_side_state_is_not_judged`, `same_step_enemy_damage_plus_self_damage_ends_in_both_lose`, `fifteenth_round_start_is_the_round_limit`, `defeat_takes_precedence_over_the_round_limit`, `hp_zero_is_not_a_loss_until_the_defeat_settlement_completes`, `next_round_counts_up_and_stops_at_the_limit`, `engine_adapter_maps_outcomes_to_game_results`, `engine_to_move_follows_the_first_player`.
- `state/flows/round_begin.rs`: `begin_round_advances_the_round_counter`, `begin_round_stops_at_the_round_limit_and_terminal_judges_it`, `begin_round_clears_the_plunging_marks_on_both_sides`.
- `flow/game_end.rs`: `settlement_outcome_reads_the_state_without_changing_it`, `unstarted_or_ongoing_games_are_not_terminal`.
- `state/flows/defeat.rs` / `revive.rs`: see [[定义层-State-Flows-EN]].

## 8. Pending integration and to-be-verified

> [!NOTE]
> The rulebook does not define the first player (留空 #33 / #59) or the End Phase "first player" convention (留空 #34);
> currently `first_player` is supplied externally and the End Phase uses this round's first player.

- Whether the terminal check should inspect the defeat zone for revivable characters: the rulebook does not say and the code does not check (留空 #41/#65).
- Whether a revived character becomes active / which timings are dispatched (留空 #73); whether Max-HP growth is restored (留空 #74).
- The "defeat zone" is not a rulebook-visible zone, only an internal record (留空 #75).
- The defeat-cleanup injection point `DefeatCleanup` only covers `use_skill_*` / `play_card_*`; the End Phase branch is not injected yet (留空 §22 #122).

---

Last verified: 2026-10-19 (code: `crates/definitions/src/state/def_state.rs`:DefState; `crates/definitions/src/state/terminal.rs`:terminal)
