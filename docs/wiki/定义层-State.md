# 定义层-State

> 中文 · [[定义层-State-EN|English]]

*本页范围：`state` 层的纯数据状态（`DefState` / `PlayerDefs`）、每方区表与只读终局判定。*

---

## 1. 职责

`state` 层定义**一局对局的完整纯数据状态**（`DefState` = 我方 `PlayerDefs` + 敌方 `PlayerDefs` + 回合数 + 先手），并提供**只读**的终局判定。所有流程（`flow/*`）都吃 `&DefState`、返回**新的** `DefState`，不改入参。

## 2. 关键概念与类型

### 2.1 根状态 `DefState`

文件：`crates/definitions/src/state/def_state.rs`。字段全部私有，对外只读。

| 字段 | 类型 | 含义 |
|---|---|---|
| `me` / `opp` | `PlayerDefs` | 两个具名玩家实例（D34：不用敌我编号 / 下标） |
| `round` | `Nat` | 回合数；`0` = 第一回合还没开始，每回合开始 +1，上限见 `ROUND_LIMIT` |
| `first_player` | `Option<Perspective>` | 本回合先手牌手（D260）；`None` = 尚未裁定，不是默认我方 |

`Perspective` 在 `crates/definitions/src/base/perspective.rs`（D260：state 不能依赖 keywords），旧路径 `keywords::target::Perspective` 仍重导出。

### 2.2 每方全部区 `PlayerDefs`

文件：`crates/definitions/src/state/player.rs`。字段私有，只读指针（D183）；对应规则书游戏区域。

| 区 / 表 | 类型（文件） | 说明 |
|---|---|---|
| 骰池 | `DicePool`（`resources/dice/mod.rs`） | 纯计数，上限 16（D47/D48） |
| 手牌 | `HandState`（`families/action_card/zones/hand/mod.rs`） | 上限 10；开局 5 张（D90） |
| 牌库 | `PileState`（`families/action_card/zones/pile/mod.rs`） | 位置为键 + 段视图 + 公共状态列表 + 待展开随机状态（D93/D105/D188） |
| 手牌卡状态 | `HandCardStates`（`families/action_card/areas/hand_state.rs`） | 减费 / 赋能 / 不可调和 / 无效化等（Z13） |
| 每张手牌当前费用 | `HandCosts`（`state/player.rs`） | 实体号 -> `CurrentCost`；基础费用由上层给，加减费叠 delta（D170/D172/D178） |
| 角色区 | `CharacterZone`（`families/character/zones/field.rs`） | 3 槽位 + 出战指针；角色离场后槽位保留顺序 |
| 出战状态区 | `CombatStatusZone`（`families/character/zones/combat_status.rs`） | 挂在出战角色上，切换时转移 |
| 每角色装备栏 | `Vec<EquipmentZone>`（`families/character/zones/equipment.rs`） | 每角色一个实例 |
| 召唤物区 | `SummonZone`（`families/summon/zones/mod.rs`） | 上限 4、同名合并、可用次数 |
| 支援区 | `SupportZone`（`families/action_card/areas/support.rs`） | 上限 4；支援牌可带 `SupportDuration`（Z14） |
| 每回合技能使用计数 | `SkillUses`（`families/character/skill_flow/usage.rs`） | 回合开始 `reset_round` |
| 下落攻击标记 | `PlungingMark`（`families/character/skill_flow/attack_form.rs`） | 每方一个；切换后置位、战斗行动结束 / 回合边界清除（D261 / S8.10） |
| 击倒区 | `Vec<CharacterSlot>`（本文件） | 内部记录：身份 + 清理后状态，供复苏放回角色区；不是规则书可见区域（留空 #75） |

访问器：`dice()/hand()/pile()/costs()/cost_of()/characters()/combat_status()/equipment()/equipment_of()/summons()/supports()/hand_states()/skill_uses()/plunging()/defeated()/defeated_character()`；派生一律 `with_*`。唯一可变入口 `pub(crate) parts_mut` 只给 `state::flows` 用。

### 2.3 终局判定 `Outcome`

文件：`crates/definitions/src/state/terminal.rs`。判定顺序：`round == 0` -> `Ongoing`；双方角色区都空 -> `BothLose`；只有一方空 -> 对方胜；双方都有角色且 `round >= 15` -> `RoundLimit`（双方都输）；否则 `Ongoing`。

| 结果 | 含义 |
|---|---|
| `Ongoing` | 对局继续（未开始第一回合 / 双方都有角色且未到上限） |
| `MeWins` / `OppWins` | 一方角色区为空，另一方获胜（S1.4） |
| `RoundLimit` | 第 15 回合开始仍未分胜负：双方都输（S7.7） |
| `BothLose` | 步边界双方角色区都为空：双方都输，不是平局（D267/D268） |

> [!IMPORTANT]
> 全灭判据是「角色区为空」，不是「当前 HP > 0 的角色数」。
> HP 归 0 但尚未击倒清理时仍判 `Ongoing`（S7.3 / S7.4）。
> 判定时机只能是步边界，步内不提前终止（D204 / D267）。

## 3. 公开入口

### 索引

源码块逐字摘自当前仓库（含 `use` / `derive` / `impl` 行，路径见小节标题）；超长函数只列文档注释 + 签名 + 函数体开头几行，`// ...` 表示其余函数体省略。

| 入口 / 类型 | 说明 |
|---|---|
| `DefState` | `state/def_state.rs`：一局完整状态（`me` / `opp` / `round` / `first_player`）与全部公开方法 |
| `PlayerDefs` | `state/player.rs`：一方的全部区；源码块为 `impl` 内关键派生节选 |
| `terminal` / `Outcome` / `ROUND_LIMIT` | `state/terminal.rs`：只读终局判定与第 15 回合上限 |

### 源码：`state/def_state.rs`（`use` + `DefState` + `impl DefState`）

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

### 源码：`state/player.rs`（节选：`PlayerDefs` 与关键派生；`impl` 行与所选方法均逐字摘自源码）

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

### 源码：`state/terminal.rs`（`use` + 终局判定）

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

## 4. 不变量与禁止

> [!IMPORTANT]
> 所有构造 / 派生吃 `&self` 或 `self`，返回新值；`DefState` 没有公开可变入口（D183）。
> 两方是两个具名实例，不出现玩家下标 / 敌我编号参数（D29 / D34）。
> `round` 的唯一推进点是 `state::flows::round_begin::begin_round`（`next_round`，15 封顶）。

> [!WARNING]
> 不要在未做击倒清理时判「已全灭」：HP = 0 不等于击倒。
> 不要把 `defeated` 当规则书「击倒区」建模，它不派发时机、不参与结算（留空 #75）。
> 不要给「双方同时全灭」加特例分支，它是步边界判定的自然结果（D267/D268）。

> [!CAUTION]
> 不要用卡牌定义里的固定最大生命值计算受伤量 / 排名。
> 运行期实际值只有 `CharacterState::max_hp`（D238）；`DefeatHpReset.definition` 是击倒重置用的定义值。

## 5. 依据（规则书与 D 编号）

| 规则 / 决策 | 落点 |
|---|---|
| S1.4 胜利条件 | `terminal` 的 `MeWins` / `OppWins` |
| S7.3 击倒（结算完成后才击倒） | HP=0 不立即判负；`state::flows::defeat` 清理后才算 |
| S7.4 免于被击倒 | 角色仍在角色区 -> `Ongoing` |
| S7.7 对局回合上限（第 15 回合开始，双方都输） | `ROUND_LIMIT` / `RoundLimit` / `next_round` 封顶 |
| 游戏区域 I–VI（角色 / 手牌 / 支援 / 召唤物 / 牌堆 / 元素骰） | `PlayerDefs` 各区字段 |
| D34 / D42 两具名实例、无区号 | `me/opp` 字段 |
| D182 / D183 区自持 + 只读指针 | 字段私有 + `&T` 访问器 |
| D204 步是原子单位 | 终局判定只在步边界 |
| D238 生命上限定义值 / 实际值 | `DefeatHpReset`、`CharacterState::max_hp` |
| D255 终局判定 flow 只读 | `flow::game_end::settlement_outcome` 薄入口 |
| D260 先手进 `DefState`、`Perspective` 上移 base | `first_player` 字段 |
| D261 切换置位下落标记 | `PlayerDefs::plunging` + `state::flows::switch` |
| D267 / D268 / D269 双负 = `BothLose` | `Outcome::BothLose` + 引擎 `GameResult::BothLose` |
| D270 第七轮收口 | 本页口径对应 1130 tests 基线 |

## 6. 常见误用

> [!WARNING]
> 用 `slots().iter().filter(hp > 0).count()` 判全灭；应使用 `characters().is_empty()`。
> 在伤害结算中途（步内）调 `terminal` 并提前结束；只能在步边界判（D204 / D267）。
> 拿 `defeated()` 是否为空决定胜负；复活窗口只在结算后、规则书没写终局前检查击倒区（留空 #41/#65）。

- 用 `CharacterDef::max_hp` 算受伤量 / 最低生命：必须用运行期 `CharacterState::max_hp`。
- 假设 `first_player` 默认我方：未裁定时 `to_move` 的 `ME` 只是占位。
- 直接 `with_round(r)` 绕过 `begin_round`，或让 `next_round` 超过 15 后继续增长。

## 7. 测试

- `state/terminal.rs`：`both_sides_wiped_at_the_step_boundary_means_both_lose`、`step_is_atomic_so_the_mid_step_single_side_state_is_not_judged`、`same_step_enemy_damage_plus_self_damage_ends_in_both_lose`、`fifteenth_round_start_is_the_round_limit`、`defeat_takes_precedence_over_the_round_limit`、`hp_zero_is_not_a_loss_until_the_defeat_settlement_completes`、`next_round_counts_up_and_stops_at_the_limit`、`engine_adapter_maps_outcomes_to_game_results`、`engine_to_move_follows_the_first_player`。
- `state/flows/round_begin.rs`：`begin_round_advances_the_round_counter`、`begin_round_stops_at_the_round_limit_and_terminal_judges_it`、`begin_round_clears_the_plunging_marks_on_both_sides`。
- `flow/game_end.rs`：`settlement_outcome_reads_the_state_without_changing_it`、`unstarted_or_ongoing_games_are_not_terminal`。
- `state/flows/defeat.rs` / `revive.rs`：见 [[定义层-State-Flows]]。

## 8. 待接入与待实测

> [!NOTE]
> 规则书没写先手裁定（留空 #33 / #59）与结束阶段「先手」口径（留空 #34）；
> 现在 `first_player` 由开局 / 加载流程外部给，结束阶段用本回合先手。

- 终局判定前是否要检查击倒区里还有可复苏角色：规则书未写，现不检查（留空 #41/#65）。
- 复苏后是否成为出战 / 派发哪些时机（留空 #73）；是否恢复生命上限成长（留空 #74）。
- 「击倒区」不是规则书可见区域，仅是内部记录（留空 #75）。
- 击倒清理注入点 `DefeatCleanup` 只覆盖 `use_skill_*` / `play_card_*`；结束阶段分支尚未注入（留空 §二十二 #122）。

---

最后核对：2026-10-19（代码：`crates/definitions/src/state/def_state.rs`:DefState；`crates/definitions/src/state/terminal.rs`:terminal）
