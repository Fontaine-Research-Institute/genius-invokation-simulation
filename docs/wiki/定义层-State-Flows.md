# 定义层-State-Flows

> 中文 · [[定义层-State-Flows-EN|English]]

*本页范围: `state/flows/*` 区流程纯函数 (开局 / 切换 / 击倒 / 复苏 / 结束阶段 / 回合开始 / 下落标记清除) .*

---

## 1. 职责

`state/flows/*` 是**区流程纯函数层**: 把'开局 / 切换 / 击倒 / 复苏 / 结束阶段 / 回合开始 / 下落标记清除'这些**区域级操作**写成'`&DefState` -> 新 `DefState` + 派发出来的待执行'.本层只改区, 只派发 (D186) , **不执行**触发效果, 不判终局.

## 2. 关键概念与类型

| 文件 | 职责 | 关键类型 |
|---|---|---|
| `crates/definitions/src/state/flows/opening.rs` | 游戏准备: 起手抓 5, 换牌, 选首发 | `PregameHooks`, `PregameChoices` |
| `state/flows/switch.rs` | 切换角色 / 击倒后补位 + 出战状态转移 | `SwitchOutcome` |
| `state/flows/defeat.rs` | 击倒清理 (移出角色区, 弃装备, 清状态 / 充能, 重置生命上限, 记入击倒区)  | `DefeatHpReset`, `DefeatOutcome` |
| `state/flows/revive.rs` | 复苏: 击倒区 -> 角色区, 生命复位 | `ReviveOutcome` |
| `state/flows/end_phase.rs` | 结束阶段 tick: 出战状态 / 召唤物持续回合与可用次数, 解除饱腹 | `EndPhasePending`, `EndPhaseOutcome` |
| `state/flows/round_begin.rs` | 回合开始: 回合 +1, 每回合计数清零, 下落标记清除 | — |
| `state/flows/plunging.rs` | 战斗行动结束: 清除该方下落攻击标记 | — |

共同口径: 纯函数 (入参不变, 内部克隆后改私有区) ; 只派发不执行 (D186) ; 成对入口 `*_me` / `*_opp`, 不传 Side 参数 (D34) .

## 3. 公开入口

### 索引

本小节的源码块为当前仓库的实现源码 (含 `use` / `derive` / `impl` 行, 路径见小节标题) ; 超长函数只列文档注释 + 签名 + 函数体开头几行, `// ...` 表示其余函数体省略.

| 流程 | 说明 |
|---|---|
| 游戏准备 | `state/flows/opening.rs`: `PregameHooks` / `PregameChoices` / `draw_opening_hands` / `mulligan` / `choose_first_characters` / `pregame` |
| 切换 / 补位 | `state/flows/switch.rs`: `SwitchOutcome` / `switch_me` / `switch_opp` / `choose_active_after_defeat_me/opp` |
| 击倒清理 | `state/flows/defeat.rs`: `DefeatHpReset` / `DefeatOutcome` / `defeat_me` / `defeat_opp` |
| 复苏 | `state/flows/revive.rs`: `ReviveOutcome` / `revive_me` / `revive_opp` |
| 结束阶段 | `state/flows/end_phase.rs`: `EndPhasePending` / `EndPhaseOutcome` / `end_phase_me_first/opp_first` |
| 回合开始 / 下落标记 | `state/flows/round_begin.rs`, `state/flows/plunging.rs`: `begin_round` / `battle_action_end_me/opp` |

### 源码: `state/flows/opening.rs`

```rust
use crate::base::CardId;
use crate::families::action_card::zones::hand::HandState;
use crate::families::action_card::zones::pile::PileState;
use crate::families::action_card::zones::CardInstance;
use crate::families::character::zones::field::CharacterZone;
use crate::resources::dice::CostComponent;
use crate::rules::ops::opening::{pregame_steps, PregameStep};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 开局时上层注入的两个查询函数（D186 风格：函数指针 / `Fn`，不是效果数据）。
pub struct PregameHooks<'a> {
    /// 这张牌是不是「秘传」（起手第 1 步按它抓；标签语义在行动牌块，本层不认识）。
    pub arcane: &'a dyn Fn(CardId) -> bool,
    /// 这张牌的基础元素骰费用（建「每张手牌的当前费用」用；费用属性在卡牌定义里）。
    pub base_cost: &'a dyn Fn(CardId) -> Vec<CostComponent>,
}

/// 玩家在游戏准备阶段做的**决策**（本层不替你选，只按给定决策推进世界）。
#[derive(Clone, Debug)]
pub struct PregameChoices {
    /// 我方要换掉的手牌实体号（不认识的实体号会被忽略；重复只算一次）。
    pub me_mulligan: Vec<EntityId>,
    /// 敌方要换掉的手牌实体号。
    pub opp_mulligan: Vec<EntityId>,
    /// 我方首发角色实体号。
    pub me_first: EntityId,
    /// 敌方首发角色实体号。
    pub opp_first: EntityId,
}

impl PregameChoices {
    /// 只给首发角色（两边都不换牌）。
    pub fn new(me_first: EntityId, opp_first: EntityId) -> PregameChoices {
        PregameChoices {
            me_mulligan: Vec::new(),
            opp_mulligan: Vec::new(),
            me_first,
            opp_first,
        }
    }

    /// 设我方的换牌名单（纯值，返回新决策）。
    pub fn with_me_mulligan(mut self, entities: Vec<EntityId>) -> PregameChoices {
        self.me_mulligan = entities;
        self
    }

    /// 设敌方的换牌名单（纯值，返回新决策）。
    pub fn with_opp_mulligan(mut self, entities: Vec<EntityId>) -> PregameChoices {
        self.opp_mulligan = entities;
        self
    }
}

/// 双方各抓 5 张起手牌；返回**分支 + 精确概率**（两侧独立，概率相乘）。
///
/// 抓牌原语是 [`HandState::draw_opening`]（秘传优先 + 补足到 5）；本函数只做
/// 「两侧组合 + 费用表同步」。牌库不够 5 张时按实际张数（不 panic）。
pub fn draw_opening_hands(state: &DefState, hooks: &PregameHooks) -> Vec<(DefState, Fraction)> {
    let me_branches = draw_side(state.me(), hooks);
    let opp_branches = draw_side(state.opp(), hooks);
    let mut branches: Vec<(DefState, Fraction)> = Vec::new();
    for (me, me_probability) in &me_branches {
    // ...

/// 换牌；返回**分支 + 精确概率**（换牌后抓的牌来自牌库，可能多分支）。
///
/// 每侧的顺序：舍弃选中的手牌 -> 抓同样张数 -> 把换掉的牌均匀洗回牌库（见文件头「待复核」）。
/// 选中的实体号不在手牌 -> 忽略；重复 -> 只算一次。
pub fn mulligan(
    state: &DefState,
    me: &[EntityId],
    opp: &[EntityId],
    hooks: &PregameHooks,
) -> Vec<(DefState, Fraction)> {
    let me_branches = mulligan_side(state.me(), me, hooks);
    let opp_branches = mulligan_side(state.opp(), opp, hooks);
    let mut branches: Vec<(DefState, Fraction)> = Vec::new();
    for (me, me_probability) in &me_branches {
    // ...

/// 双方各自选一张自己的角色牌作为出战角色（规则书·游戏准备 (2)）。
///
/// 失败（返回 None、不改状态）：某一方选的角色**不在**那一方的角色区里。
/// 「已经是出战角色」视为**成功的无操作**（开局默认出战角色可能已经是它）。
pub fn choose_first_characters(
    state: &DefState,
    me: EntityId,
    opp: EntityId,
) -> Option<DefState> {
    let me_zone = choose_first(state.me().characters(), me)?;
    let opp_zone = choose_first(state.opp().characters(), opp)?;
    let mut next = state.clone();
    {
        let parts = next.me_mut().parts_mut();
        *parts.characters = me_zone;
    }
    {
        let parts = next.opp_mut().parts_mut();
        *parts.characters = opp_zone;
    }
    Some(next)
}

/// **完整游戏准备流程**：严格按 [`pregame_steps`] 的顺序逐步执行。
///
/// 返回分支 + 精确概率；任一步骤没有任何分支（例如选的角色不在角色区）-> 整体空 Vec
/// （**不改状态**：入参一个字节都不动）。
pub fn pregame(
    state: &DefState,
    choices: &PregameChoices,
    hooks: &PregameHooks,
) -> Vec<(DefState, Fraction)> {
    let mut branches: Vec<(DefState, Fraction)> = vec![(state.clone(), Fraction::ONE)];
    for step in pregame_steps() {
        branches = match step {
            PregameStep::DrawOpeningHand => {
    // ...
```

### 源码: `state/flows/switch.rs`

```rust
use crate::families::character::zones::combat_status::{
use crate::families::character::zones::field::{CharacterZonePending, CharacterZoneTriggers};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::EntityId;

/// 一次切换 / 补位的结果：新的一局 + 派发出来的待执行（本层不执行，D186）。
pub struct SwitchOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 `execute_character_zone`）。
    pub characters: Vec<CharacterZonePending>,
    /// 出战状态区派发的待执行（执行时用 `execute_combat_status`）。
    pub combat_status: Vec<CombatStatusPending>,
}

/// **我方切换角色**（把后台角色 `to` 切为出战角色）。
///
/// 失败（返回 None、不改状态）：没有出战角色 / `to` 不在我方角色区 / `to` 已经是出战角色。
/// **不付骰子**（规则书要求支付 1 个任意元素骰，由调用方在调本流程前从骰池扣）。
pub fn switch_me(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        switch_side(next.me_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}

/// **敌方切换角色**（同 [`switch_me`]，作用在 `opp` 上）。
pub fn switch_opp(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        switch_side(next.opp_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}

/// **我方被击倒后的强制补位**（规则书·游戏区域 I(5)）：选一个我方后台角色成为新出战角色。
///
/// 失败（返回 None、不改状态）：`to` 不在我方角色区 / `to` 已经是出战角色。
pub fn choose_active_after_defeat_me(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        choose_after_defeat_side(next.me_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}

/// **敌方被击倒后的强制补位**（同 [`choose_active_after_defeat_me`]）。
pub fn choose_active_after_defeat_opp(
    state: &DefState,
    to: EntityId,
    characters: &CharacterZoneTriggers,
    combat_status: &CombatStatusTriggers,
) -> Option<SwitchOutcome> {
    let mut next = state.clone();
    let (character_pending, combat_pending) =
        choose_after_defeat_side(next.opp_mut(), to, characters, combat_status)?;
    Some(SwitchOutcome {
        state: next,
        characters: character_pending,
        combat_status: combat_pending,
    })
}
```

### 源码: `state/flows/defeat.rs`

```rust
use crate::families::action_card::status::StateList;
use crate::families::character::attachment::Attachment;
use crate::families::character::character::CharacterState;
use crate::families::character::growth::GrowthList;
use crate::families::character::satiated::clear_on_defeat;
use crate::families::character::zones::equipment::{EquipmentPending, EquipmentTriggers};
use crate::families::character::zones::field::{
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 击倒时把「实际最大生命值上限」重置回定义值所需的资料（D238）。
///
/// 生命上限有两个值（见 `families/character/growth.rs`）：
/// - **定义值**：`CharacterDef::max_hp`，不可变 —— [`DefeatHpReset::definition`]；
/// - **实际值**：定义值 + 成长增益，可变 —— 就是 `CharacterState::max_hp`。
///
/// 重置 = 清掉该角色的「生命上限」成长条目（[`GrowthList::reset_max_hp`]），
/// 再按 `actual_max_hp(定义值)` 重算实际值。**定义值本身不写进成长列表**（growth.rs 口径）。
///
/// 待接入：逐角色的成长目前还没有聚合进状态层（成长条目由装备 / 天赋 / 场地各自持有），
/// 所以这里由调用方把该角色当前的 `GrowthList` 传进来。
#[derive(Clone, Copy, Debug)]
pub struct DefeatHpReset<'a> {
    /// 卡牌定义的生命上限（`CharacterDef::max_hp`；D238「定义值」）。
    pub definition: Nat,
    /// 这个角色的成长条目（D238「实际值」= 定义值 + 生命上限成长）。
    pub growth: &'a GrowthList,
}

impl<'a> DefeatHpReset<'a> {
    /// 用「定义值 + 该角色成长」建重置资料。
    pub const fn new(definition: Nat, growth: &'a GrowthList) -> DefeatHpReset<'a> {
        DefeatHpReset { definition, growth }
    }
}

/// 一次击倒清理的结果：新的一局 + 派发 + 清理后的角色状态。
pub struct DefeatOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 `execute_character_zone`）。
    pub characters: Vec<CharacterZonePending>,
    /// 装备栏派发的待执行（执行时用 `execute_equipment`）。
    pub equipment: Vec<EquipmentPending>,
    /// 清理前的充能（= 被清空的量）。
    pub cleared_energy: Nat,
    /// 清理后的角色运行期状态（已离场：状态列表空、元素附着空、充能 0、
    /// **实际最大生命值上限已回到定义值**，D238）。
    pub cleared: CharacterState,
    /// 重置后的成长条目（**生命上限条目已清空**、其它属性成长保留，D238）：
    /// 由持有成长的那一层写回（本层不持有成长，见 [`DefeatHpReset`]）。
    pub reset_growth: GrowthList,
}

/// **我方角色被击倒后的清理**（规则书·其他规则 击倒）。
///
/// `hp`：这个角色的**定义值 + 成长**（D238：击倒后实际上限回到定义值）。
///
/// 失败（返回 None、不改状态）：这个角色不在我方角色区。
pub fn defeat_me(
    state: &DefState,
    character: EntityId,
    hp: &DefeatHpReset,
    characters: &CharacterZoneTriggers,
    equipment: &EquipmentTriggers,
) -> Option<DefeatOutcome> {
    let mut next = state.clone();

/// 一次击倒清理的结果：新的一局 + 派发 + 清理后的角色状态。
pub struct DefeatOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 `execute_character_zone`）。
    pub characters: Vec<CharacterZonePending>,
    /// 装备栏派发的待执行（执行时用 `execute_equipment`）。
    pub equipment: Vec<EquipmentPending>,
    /// 清理前的充能（= 被清空的量）。
    pub cleared_energy: Nat,
    /// 清理后的角色运行期状态（已离场：状态列表空、元素附着空、充能 0、
    /// **实际最大生命值上限已回到定义值**，D238）。
    pub cleared: CharacterState,
    /// 重置后的成长条目（**生命上限条目已清空**、其它属性成长保留，D238）：
    /// 由持有成长的那一层写回（本层不持有成长，见 [`DefeatHpReset`]）。
    pub reset_growth: GrowthList,
}

/// **我方角色被击倒后的清理**（规则书·其他规则 击倒）。
///
/// `hp`：这个角色的**定义值 + 成长**（D238：击倒后实际上限回到定义值）。
///
/// 失败（返回 None、不改状态）：这个角色不在我方角色区。
pub fn defeat_me(
    state: &DefState,
    character: EntityId,
    hp: &DefeatHpReset,
    characters: &CharacterZoneTriggers,
    equipment: &EquipmentTriggers,
) -> Option<DefeatOutcome> {
    let mut next = state.clone();
    let cleanup = defeat_side(next.me_mut(), character, hp, characters, equipment)?;
    Some(DefeatOutcome {
        state: next,
        characters: cleanup.characters,
        equipment: cleanup.equipment,
        cleared_energy: cleanup.cleared_energy,
        cleared: cleanup.cleared,
        reset_growth: cleanup.reset_growth,
    })
}

/// **敌方角色被击倒后的清理**（同 [`defeat_me`]，作用在 `opp` 上）。
pub fn defeat_opp(
    state: &DefState,
    character: EntityId,
    hp: &DefeatHpReset,
    characters: &CharacterZoneTriggers,
    equipment: &EquipmentTriggers,
) -> Option<DefeatOutcome> {
    let mut next = state.clone();
    let cleanup = defeat_side(next.opp_mut(), character, hp, characters, equipment)?;
    Some(DefeatOutcome {
        state: next,
        characters: cleanup.characters,
        equipment: cleanup.equipment,
        cleared_energy: cleanup.cleared_energy,
        cleared: cleanup.cleared,
        reset_growth: cleanup.reset_growth,
    })
}
```

### 源码: `state/flows/revive.rs`

```rust
use crate::families::character::zones::field::{CharacterZonePending, CharacterZoneTriggers};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::{EntityId, Nat};

/// 一次复苏的结果：新的一局 + 角色区派发的待执行。
#[derive(Clone, Debug)]
pub struct ReviveOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 角色区派发的待执行（执行时用 `execute_character_zone`）。
    pub characters: Vec<CharacterZonePending>,
}

/// **我方角色复苏**：把击倒区里的角色放回我方角色区（规则书·特殊词条 料理「复苏」）。
///
/// - `hp`：复苏后设置的生命值（卡面给；规则书没写具体值）；
/// - 失败（返回 `None`、不改状态）：该角色不在我方击倒区 / 角色区已满。
pub fn revive_me(
    state: &DefState,
    character: EntityId,
    hp: Nat,
    characters: &CharacterZoneTriggers,
) -> Option<ReviveOutcome> {
    let mut next = state.clone();
    let pending = revive_side(next.me_mut(), character, hp, characters)?;
    Some(ReviveOutcome {
        state: next,
        characters: pending,
    })
}

/// **敌方角色复苏**（同 [`revive_me`]，作用在 `opp` 上；D35 共用实现）。
pub fn revive_opp(
    state: &DefState,
    character: EntityId,
    hp: Nat,
    characters: &CharacterZoneTriggers,
) -> Option<ReviveOutcome> {
    let mut next = state.clone();
    let pending = revive_side(next.opp_mut(), character, hp, characters)?;
    Some(ReviveOutcome {
        state: next,
        characters: pending,
    })
}
```

### 源码: `state/flows/end_phase.rs`

```rust
use crate::families::character::character::CharacterState;
use crate::families::character::satiated::clear_at_round_end;
use crate::families::character::zones::combat_status::{CombatStatusPending, CombatStatusTriggers};
use crate::families::summon::zones::{SummonPending, SummonTriggers};
use crate::state::def_state::DefState;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::EntityId;

/// 一次结束阶段派发出来的全部待执行（本层不执行，D186）。
///
/// 执行顺序（= 本层调用顺序）：先手方出战状态 -> 先手方召唤物 -> 另一方出战状态 -> 另一方召唤物。
#[derive(Clone, Debug, Default)]
pub struct EndPhasePending {
    /// 我方出战状态区派发的待执行。
    pub me_combat_status: Vec<CombatStatusPending>,
    /// 我方召唤物区派发的待执行。
    pub me_summons: Vec<SummonPending>,
    /// 敌方出战状态区派发的待执行。
    pub opp_combat_status: Vec<CombatStatusPending>,
    /// 敌方召唤物区派发的待执行。
    pub opp_summons: Vec<SummonPending>,
}

/// 一次结束阶段的结果：新的一局 + 待执行。
pub struct EndPhaseOutcome {
    /// 操作后的一局（入参不变）。
    pub state: DefState,
    /// 派发出来的待执行。
    pub pending: EndPhasePending,
}

/// **结束阶段**：我方先手（我方出战状态 -> 我方召唤物 -> 敌方出战状态 -> 敌方召唤物）。
pub fn end_phase_me_first(
    state: &DefState,
    combat_status: &CombatStatusTriggers,
    summons: &SummonTriggers,
) -> EndPhaseOutcome {
    let mut next = state.clone();
    let (me_combat_status, me_summons) = tick_side(next.me_mut(), combat_status, summons);
    let (opp_combat_status, opp_summons) = tick_side(next.opp_mut(), combat_status, summons);
    EndPhaseOutcome {
        state: next,
        pending: EndPhasePending {
            me_combat_status,
            me_summons,
            opp_combat_status,
            opp_summons,
        },
    }
}

/// **结束阶段**：敌方先手（敌方出战状态 -> 敌方召唤物 -> 我方出战状态 -> 我方召唤物）。
pub fn end_phase_opp_first(
    state: &DefState,
    combat_status: &CombatStatusTriggers,
    summons: &SummonTriggers,
) -> EndPhaseOutcome {
    let mut next = state.clone();
    let (opp_combat_status, opp_summons) = tick_side(next.opp_mut(), combat_status, summons);
    let (me_combat_status, me_summons) = tick_side(next.me_mut(), combat_status, summons);
    EndPhaseOutcome {
        state: next,
        pending: EndPhasePending {
            me_combat_status,
            me_summons,
            opp_combat_status,
            opp_summons,
        },
    }
}
```

### 源码: `state/flows/round_begin.rs`

```rust
use crate::state::def_state::DefState;
use crate::state::terminal::next_round;

/// **回合开始**：回合数 +1（到第 15 回合封顶）+ 双方每回合计数清零（技能使用次数；上限保留）。
///
/// 依据：规则书·其他规则 对局回合上限（回合数推进点）+ 规则书·回合流程「开始新的回合！」
/// （每回合计数清零）。
pub fn begin_round(state: &DefState) -> DefState {
    state
        .clone()
        .with_me(
            state
    // ...
```

### 源码: `state/flows/plunging.rs`

```rust
use crate::state::def_state::DefState;

/// **我方的一次战斗行动结束**：清除我方的下落攻击标记（S8.10；纯函数）。
pub fn battle_action_end_me(state: &DefState) -> DefState {
    state
        .clone()
        .with_me(state.me().with_plunging_after_combat_action())
}

/// **敌方的一次战斗行动结束**：清除敌方的下落攻击标记（S8.10；纯函数；D35 共用实现）。
pub fn battle_action_end_opp(state: &DefState) -> DefState {
    state
        .clone()
        .with_opp(state.opp().with_plunging_after_combat_action())
}
```

### 源码: `rules/ops/food.rs` (`flow::revive` 引用的复苏次数判定) 

```rust
use gi_genius_invokation_engine::Nat;

/// 每回合最多通过料理复苏 1 个角色（规则书第 4 条 + 卡面 333009）。
pub const FOOD_REVIVE_PER_ROUND: Nat = Nat::new(1);

/// 本回合已通过料理复苏 revived_this_round 个角色时，还能不能复苏（规则书第 4 条）。
pub fn can_revive(revived_this_round: Nat) -> bool {
    revived_this_round < FOOD_REVIVE_PER_ROUND
}
```

## 4. 不变量与禁止

> [!IMPORTANT]
> 入参 `&DefState` 一个字节不改; 所有失败都是 `None` / `Err` 且状态不变.
> 区流程不执行 `*Pending` (D186) ; 调用方必须在步边界执行 / 插入, 顺序见 `flow::pending`.
> 击倒清理必须发生在结算完成之后 (S7.3) ; HP 归 0 未清理时 `terminal` 仍判 `Ongoing`.

> [!WARNING]
> 切换流程不付骰子, 击倒流程不选补位, 复苏流程不判次数 / 不加饱腹.
> 这些分别由 `flow::switch_character` / `flow::turn::choose_*` / `flow::revive` / `flow::play_card_*_food` 负责.
> 击倒 / 复苏不改击倒区规则语义: `defeated` 是内部记录, 不派发时机 (留空 #75) .

## 5. 依据 (规则书与 D 编号) 

| 规则 / 决策 | 落点 |
|---|---|
| 规则书·游戏准备 (起手 5, 换牌, 选首发)  | `opening.rs` |
| S8.1 / S2.4 结束阶段 (持续回合 -1, 耗尽弃置)  | `end_phase.rs` + `flow::turn` |
| S7.3 击倒 (装备 / 状态弃置, 充能清空)  | `defeat.rs` |
| 游戏区域 I(5) 出战角色被击倒后补位 | `switch::choose_active_after_defeat_*` |
| 特殊词条'料理'饱腹: 回合结束或击倒解除 | `end_phase.rs` + `defeat.rs` |
| S8.10 切换后下落攻击标记 | `switch.rs` + `plunging.rs` |
| D34 / D35 成对入口 | 全部 `*_me/*_opp` |
| D121 / D133 支付归被花的东西 | 切换不付骰 |
| D186 派发只排序 / 不执行 | 全部 `*Pending` |
| D190 分支 + 精确概率 | `opening` 的抓牌 / 换牌 |
| D193 击倒清理归角色块 | `defeat.rs` |
| D238 生命上限定义值 / 实际值 | `DefeatHpReset` / `DefeatOutcome::reset_growth` |
| D255 清理顺序 (结算 -> 清理 -> 判终局)  | `revive/defeat` 供 flow 注入使用 |
| D261 切换置位下落标记 | `flow::switch_character` 返回 `plunging` |
| D262 结束阶段先效果再抓牌 | `flow::turn::turn_end` |

## 6. 常见误用

> [!WARNING]
> 在 `switch_me` 里顺便扣 1 个骰; 骰子支付在 `flow::switch_character`.
> 击倒后忘记补位: `defeat_*` 只把角色移走并置空出战指针, 补位要调 `choose_active_after_defeat_*`.
> 在 `end_phase.rs` 里找支援牌持续回合递减; 它在 flow::turn 的 support tick 里 (Z6) .

- 用 `revive_*` 前自己判'角色区是否满'还复用结果: 区层已返回 `None`; 次数判定用 `rules::ops::food::can_revive`.
- 期望复苏恢复装备 / 状态 / 充能 / 生命上限成长: 现不恢复 (留空 #73/#74) .
- 在 `opening` 里写 `first_player`: 第一回合先手必须由外部裁定 (留空 #33/#59) .

## 7. 测试

- `state/flows/opening.rs`: `pregame_draws_five_and_chooses_first_character`, `opening_hand_prefers_arcane_card_on_top`, `mulligan_swaps_cards_and_returns_them_to_the_pile`, `mulligan_with_nothing_to_swap_changes_nothing`, `choose_first_accepts_current_active_but_rejects_unknown_character`, `pregame_fails_as_a_whole_when_first_character_is_illegal`, `opening_keeps_both_sides_independent`.
- `state/flows/switch.rs`: `switch_changes_active_transfers_status_and_keeps_slot_order`, `switch_without_combat_status_dispatches_nothing`, `switch_rejects_unknown_or_same_or_no_active`, `opp_switch_does_not_touch_me`, `choose_active_after_defeat_dispatches_in_and_changed_only`.
- `state/flows/defeat.rs`: `defeat_removes_character_discards_equipment_and_clears_energy`, `defeat_rejects_character_not_in_zone`, `defeat_opp_does_not_touch_me`, `defeat_resets_actual_max_hp_to_definition_and_keeps_other_growth`, `defeat_clamps_current_hp_to_the_reset_max_hp`, `defeat_clears_satiated_of_the_defeated_character_and_keeps_others`.
- `state/flows/revive.rs`: `revive_returns_the_character_and_terminal_stops_judging_loss`, `revive_rejects_a_character_that_is_not_in_the_defeat_zone`, `revive_does_not_restore_discarded_equipment_or_cleared_state`, `revive_opp_does_not_touch_me`.
- `state/flows/end_phase.rs`: `end_phase_ticks_durations_and_usages_and_discards_when_depleted`, `end_phase_without_statuses_or_summons_dispatches_nothing`, `first_player_order_changes_only_the_dispatch_order_not_the_state`, `end_phase_clears_satiated_on_both_sides_and_keeps_other_character_states`.
- `state/flows/round_begin.rs`: `begin_round_clears_usage_and_keeps_limits_and_other_zones`, `begin_round_advances_the_round_counter`, `begin_round_stops_at_the_round_limit_and_terminal_judges_it`, `begin_round_clears_the_plunging_marks_on_both_sides`, `begin_round_resets_both_sides_independently`.
- `state/flows/plunging.rs`: `battle_action_end_clears_only_the_side_that_acted`, `battle_action_end_is_a_noop_without_a_mark`.

## 8. 待接入与待实测

> [!NOTE]
> 旧条目 留空 #76 (复苏接线未做) 与 #77 (下落标记 flow 端到端未做) 已完成, 口径见 [[定义层-Flow]].

- 第一回合先手裁定: 留空 #33 / #59; `opening.rs` 不写 `first_player`.
- 结束阶段'先手'是本回合先手还是刚宣布先手: 留空 #34 (现分开存) .
- 支援牌持续回合的递减时机: 留空 #67 / #93 (现按结束阶段; 已接线) .
- 击倒补位算不算一次'切换角色'行动: `switch.rs` 待复核.
- 出战状态在角色被击倒时是否弃置: `defeat.rs` 按'不弃置, 补位时转移'实现, 待实测.
- 复苏后是否成为出战 / 是否派发 Entered / 是否恢复生命上限成长: 留空 #73 / #74.
- 击倒区不是规则书可见区域: 留空 #75.
- 料理饱腹的正式 `StatusDef`, 石化 / 眩晕去重: 留空 #79.

---

最后核对: 2026-10-19 (代码: `crates/definitions/src/state/flows/` 各文件; `crates/definitions/src/state/player.rs`:PlayerDefs) 
