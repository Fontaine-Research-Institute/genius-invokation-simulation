# 定义层-State

> 中文 · [[定义层-State-EN|English]]

*本页范围: `state` 层的纯数据状态, 每方区表与只读终局判定.*

---

## 1. 职责

`state` 层只做两件事. 一是把一局对局完整地描述成纯数据, 二是提供只读的终局判定.

所有流程都吃一个 `&DefState`, 返回新的 `DefState`, 不改入参. 这里没有可变入口, 也没有隐藏状态.

## 2. 关键概念与类型

根状态是 `DefState`, 只持四项数据: `me` 与 `opp` 两个 `PlayerDefs`, 回合数 `round`, 本回合先手 `first_player`.

每方的全部区都收在 `PlayerDefs` 里. 按规则书游戏区域, 可以逐项列出:

- 骰池: `DicePool`, 纯计数, 上限 16;
- 手牌: `HandState`, 上限 10, 开局 5 张;
- 牌库: `PileState`, 位置为键, 带段视图与待展开随机状态;
- 手牌卡状态: `HandCardStates`, 减费与赋能等状态表;
- 每张手牌当前费用: `HandCosts`, 实体号到 `CurrentCost`;
- 角色区: `CharacterZone`, 三个槽位加一个出战指针;
- 出战状态区: `CombatStatusZone`, 挂在出战角色上, 切换时转移;
- 装备栏: 每个角色一个 `EquipmentZone`;
- 召唤物区: `SummonZone`, 上限 4;
- 支援区: `SupportZone`, 上限 4, 可带整局或有限持续回合;
- 每回合技能使用计数: `SkillUses`;
- 下落攻击标记: 每方一个 `PlungingMark`;
- 击倒区: `Vec<CharacterSlot>`, 内部记录, 供复苏使用.

终局判定集中在 `state/terminal.rs`, 顺序固定: 未开始第一回合继续; 双方角色区都空是双负; 只有一方空则对方胜; 双方都有角色且到第 15 回合是回合上限; 其余继续.

## 3. 公开入口

### 3.1 根状态

先看根状态本身. 四个字段都是私有的, 外部只能通过访问器读取:


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

空局从 `new` 开始, 建局或测试注入初始状态用 `from_players`:


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

读取只有四个访问器: 双方区, 回合数与先手.


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

派生一律是纯函数. 换回合数时不要绕开回合开始流程; 先手与双方区也都有对应的 `with_*`:


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

### 3.2 每方全部区

`PlayerDefs` 把一方的所有区装在一起. 字段私有, 对外只给只读指针:


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

手牌进出后要同步费用表. 这里只重建新牌的费用, 已有牌的加减费 delta 保留:


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

下落攻击标记与击倒区各有专用访问器与派生:


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

### 3.3 终局判定

回合上限是一个常量, 推进回合时到 15 就封顶:


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

判定结果是一个五值枚举, 双负是独立结果, 不并进平局:


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

真正的判定函数只读 `&DefState`, 调用时点必须在结算与击倒清理之后:


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

## 4. 不变量与禁止

> [!IMPORTANT]

> 所有构造与派生都返回新值, `DefState` 没有公开可变入口.

> 终局只按角色区是否为空判定, 不按 HP 大于零的角色数.

> [!WARNING]

> 不要在未做击倒清理时判已全灭. HP 为零不等于击倒.

> 不要把内部击倒区当成规则书可见区域, 它不派发时机.

## 5. 依据

- S1.4 胜利条件: `terminal` 的 `MeWins` 与 `OppWins`;
- S7.3 击倒: 结算完成后才击倒, HP 为零不立即判负;
- S7.4 免于被击倒: 角色仍在角色区就继续;
- S7.7 对局回合上限: 第 15 回合开始双方都输;
- 游戏区域 I 到 VI: `PlayerDefs` 的区字段;
- D34 与 D42: 两个具名实例, 没有敌我编号;
- D182 与 D183: 区自持, 对外只读指针;
- D204: 步是原子单位, 终局只在步边界判;
- D238: 生命上限的定义值与实际值;
- D255: 终局判定的只读薄入口;
- D260: 先手存进 `DefState`, `Perspective` 上移到 base;
- D267 与 D268: 双负是 `BothLose`;
- D270: 第七轮收口, 对应 1130 tests 基线.

## 6. 常见误用

> [!WARNING]

> 用 HP 大于零的角色数判全灭, 或用 `defeated()` 是否为空判胜负.

> 在伤害结算中途调 `terminal` 并提前结束.

- 用卡牌定义里的固定最大生命值算受伤量, 应读运行期的 `CharacterState::max_hp`;
- 假设 `first_player` 默认我方, 未裁定时它只是占位;
- 直接 `with_round` 绕过回合开始流程.

## 7. 测试

- `state/terminal.rs`: 双负, 步内不判, 回合上限, 免于击倒窗口等用例;
- `state/flows/round_begin.rs`: 回合推进与下落标记清理;
- `flow/game_end.rs`: 终局入口只读, 未开局不终局.

## 8. 待接入

> [!NOTE]

> 规则书没写第一回合先手由谁裁定, 现由开局流程外部给.

> 旧条目「击倒清理仍归上层」已由 `DefeatCleanup` 注入点覆盖.

- 终局前是否检查击倒区还有可复苏角色: 规则书未写;
- 复苏后是否成为出战, 是否恢复生命上限成长: 待实测;
- 结束阶段分支尚未注入击倒清理.

## 小结

状态层只保存数据, 所有变化都由上层返回新状态. 终局判定是纯读取, 记住两个条件: 角色区为空, 且在步边界.

---

最后核对: 2026-10-20 (代码: `crates/definitions/src/state/def_state.rs`:DefState; `state/terminal.rs`:terminal)
