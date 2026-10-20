# Definitions Layer: State

> [[定义层-State|中文]] · English

*Scope: the pure-data match state, per-side zone tables, and the read-only terminal check.*

---

## 1. Responsibility

The `state` layer does two things. It describes a whole match as pure data, and it exposes a read-only terminal check.

Every flow takes `&DefState` and returns a new `DefState`; the input is never mutated. There is no mutable entry point and no hidden state.

## 2. Key concepts and types

`DefState` holds four pieces of data: two `PlayerDefs` (`me` and `opp`), the round counter, and the first player of the round.

All zones of one side live in `PlayerDefs`. Following the rulebook game zones:

- Dice pool: `DicePool`, pure counters, cap 16;
- Hand: `HandState`, cap 10, opening hand 5;
- Pile: `PileState`, position-keyed, with segments and pending random marks;
- Hand-card states: `HandCardStates`, cost and status table;
- Current hand cost: `HandCosts`, entity id to `CurrentCost`;
- Character zone: `CharacterZone`, three slots plus an active pointer;
- Combat status zone: `CombatStatusZone`, attached to the active character and transferred on switch;
- Equipment zones: one `EquipmentZone` per character;
- Summon zone: `SummonZone`, cap 4;
- Support zone: `SupportZone`, cap 4, whole-game or limited duration;
- Per-round skill counters: `SkillUses`;
- Plunging mark: one `PlungingMark` per side;
- Defeat zone: `Vec<CharacterSlot>`, an internal record used by revive.

Terminal checks live in `state/terminal.rs` and follow a fixed order: round 0 means ongoing; both zones empty means double loss; one zone empty means the other side wins; round 15 with both alive means the round limit; otherwise ongoing.

## 3. Public entry points

### 3.1 Root state

Start with the root state. All four fields are private:


文件: crates/definitions/src/state/def_state.rs

```rust
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
```

An empty match starts from `new`; `from_players` builds a state from two sides.


文件: crates/definitions/src/state/def_state.rs

```rust
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
```

Reading is limited to four accessors: both sides, the round, and the first player.


文件: crates/definitions/src/state/def_state.rs

```rust
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

    /// **本回合的先手牌手**（[`None`] = 还没裁定；只读指针，D183）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】+ 其他规则·结算顺序第 1 条。
    pub fn first_player(&self) -> Option<Perspective> {
        self.first_player
    }
```

Derivations are pure. Do not bypass the round-start flow when changing the round; the first player and both sides also have `with_*`.


文件: crates/definitions/src/state/def_state.rs

```rust
    /// 换掉回合数（纯函数：返回新的一局，不改入参；建局 / 测试注入用）。
    ///
    /// 正式流程不要直接调它 —— 回合推进走
    /// `crate::state::flows::round_begin::begin_round`（+1 并处理第 15 回合边界）。
    pub fn with_round(self, round: Nat) -> DefState {
        DefState { round, ..self }
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
```

### 3.2 All zones of one side

`PlayerDefs` packs every zone of one side. Fields are private and exposed through read-only pointers:


文件: crates/definitions/src/state/player.rs

```rust
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
```

After cards enter or leave the hand, sync the cost table. Only new cards are rebuilt; existing cost deltas are kept.


文件: crates/definitions/src/state/player.rs

```rust
    /// 跟当前手牌同步费用表（见 [`HandCosts::synced`]）。
    pub fn with_synced_hand_costs(
        &self,
        base_cost: &dyn Fn(CardId) -> Vec<CostComponent>,
    ) -> PlayerDefs {
        let mut next = self.clone();
        next.costs = self.costs.synced(&self.hand, base_cost);
        next
    }
```

The plunging mark and the defeat zone have their own accessors and derivations.


文件: crates/definitions/src/state/player.rs

```rust
    /// **下落攻击待消耗标记**（S8.10；纯值拷贝，不是指针）。
    pub fn plunging(&self) -> PlungingMark {
        self.plunging
    }

    /// **击倒区**（只读指针；顺序 = 被击倒先后；内部记录，不是规则书可见区域）。
    pub fn defeated(&self) -> &[CharacterSlot] {
        &self.defeated
    }

    /// 写入**下落攻击待消耗标记**（纯函数；切换成功后把 flow 返回的
    /// `SwitchCharacterOutcome::plunging` 写回这里）。
    pub fn with_plunging(&self, plunging: PlungingMark) -> PlayerDefs {
        PlayerDefs {
            plunging,
            ..self.clone()
        }
    }
```

### 3.3 Terminal check

The round limit is a constant, and advancing the round clamps at 15.


文件: crates/definitions/src/state/terminal.rs

```rust
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
```

The result is a five-value enum; double loss is its own result, not a draw.


文件: crates/definitions/src/state/terminal.rs

```rust
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
```

The check itself only reads `&DefState`, and must run after settlement and defeat cleanup.


文件: crates/definitions/src/state/terminal.rs

```rust
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
    // ...
```

## 4. Invariants and prohibitions

> [!IMPORTANT]

> All constructors and derivations return new values; `DefState` has no public mutable entry point.

> The terminal check only looks at empty character zones, never at the number of characters with HP above zero.

> [!WARNING]

> Do not judge a wipe-out before defeat cleanup has run. HP zero is not a defeat.

> Do not treat the internal defeat zone as a rulebook-visible zone; it dispatches no timings.

## 5. Basis

- S1.4 victory: `MeWins` and `OppWins` of `terminal`;
- S7.3 defeat: a character is defeated after settlement, so HP zero alone does not lose;
- S7.4 saved from defeat: the character is still in the character zone;
- S7.7 round limit: at the start of round 15 both sides lose;
- Game zones I to VI: the zone fields of `PlayerDefs`;
- D34 and D42: two named instances, no side ids;
- D182 and D183: zones own their data, exposed as read-only pointers;
- D204: a step is atomic, so terminal checks only happen at step boundaries;
- D238: definition value and actual Max HP;
- D255: the read-only thin terminal entry;
- D260: first player stored in `DefState`, `Perspective` moved to base;
- D267 and D268: double loss is `BothLose`;
- D270: round-7 wrap-up, matching the 1130-test baseline.

## 6. Common misuse

> [!WARNING]

> Judging a wipe-out by counting characters with HP above zero, or by checking whether `defeated()` is empty.

> Calling `terminal` in the middle of damage settlement and ending early.

- Computing damage taken from the fixed Max HP in card definitions; read the runtime `CharacterState::max_hp`;
- Assuming `first_player` defaults to our side; when undecided it is a placeholder;
- Calling `with_round` to bypass the round-start flow.

## 7. Tests

- `state/terminal.rs`: double loss, no mid-step judgement, round limit, saved-from-defeat window;
- `state/flows/round_begin.rs`: round advance and plunging-mark clearing;
- `flow/game_end.rs`: read-only entry and unstarted matches.

## 8. Pending integration

> [!NOTE]

> The rulebook does not define who decides the first player of round 1; the setup flow supplies it.

> The old note that defeat cleanup belongs to the caller is superseded by the `DefeatCleanup` injection point.

- Whether the terminal check looks for revivable characters in the defeat zone: not defined by the rulebook;
- Whether a revived character becomes active and restores Max-HP growth: to be verified;
- The End Phase branch is not injected with defeat cleanup yet.

## Summary

The state layer stores data only; every change returns a new state. The terminal check is a pure read with two conditions: an empty character zone, judged at a step boundary.

---

Last verified: 2026-10-20 (code: `crates/definitions/src/state/def_state.rs`:DefState; `state/terminal.rs`:terminal)
