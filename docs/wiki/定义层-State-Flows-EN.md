# Definitions Layer: State Flows

> [[定义层-State-Flows|中文]] · English

*Scope: the zone-flow pure functions under `state/flows/*`.*

---

## 1. Responsibility

These functions perform zone-level work: game setup, switch, defeat, revive, the End Phase, round start, and clearing the plunging mark.

Each function takes `&DefState`, returns a new state plus dispatched pending items. It never executes those effects itself.

## 2. Key concepts and types

Seven files split the work; each file owns one zone-level flow.

- `opening.rs`: draw the opening hand, mulligan, choose starting characters;
- `switch.rs`: switch the active character and transfer combat status;
- `defeat.rs`: remove the character, discard equipment, clear states and Energy, reset Max HP;
- `revive.rs`: put a defeated character back into the character zone;
- `end_phase.rs`: tick combat-status and summon duration and usages, clear Satiated;
- `round_begin.rs`: advance the round and reset per-round counters;
- `plunging.rs`: clear the plunging mark when one combat action ends.

All entries are paired as `*_me` and `*_opp`, so no side parameter appears in the signatures.

## 3. Public entry points

### 3.1 Game setup

Setup receives two hooks: one says whether a card is Arcane, one gives its base cost.


文件: crates/definitions/src/state/flows/opening.rs

```rust
/// 开局时上层注入的两个查询函数（D186 风格：函数指针 / `Fn`，不是效果数据）。
pub struct PregameHooks<'a> {
    /// 这张牌是不是「秘传」（起手第 1 步按它抓；标签语义在行动牌块，本层不认识）。
    pub arcane: &'a dyn Fn(CardId) -> bool,
    /// 这张牌的基础元素骰费用（建「每张手牌的当前费用」用；费用属性在卡牌定义里）。
    pub base_cost: &'a dyn Fn(CardId) -> Vec<CostComponent>,
}
```

Player decisions are passed as a small struct, built with `new` and two `with_*` setters.


文件: crates/definitions/src/state/flows/opening.rs

```rust
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
```

Drawing the opening hand and mulliganing both expand into branches with exact probabilities.


文件: crates/definitions/src/state/flows/opening.rs

```rust
/// 双方各抓 5 张起手牌；返回**分支 + 精确概率**（两侧独立，概率相乘）。
///
/// 抓牌原语是 [`HandState::draw_opening`]（秘传优先 + 补足到 5）；本函数只做
/// 「两侧组合 + 费用表同步」。牌库不够 5 张时按实际张数（不 panic）。
pub fn draw_opening_hands(state: &DefState, hooks: &PregameHooks) -> Vec<(DefState, Fraction)> {
    let me_branches = draw_side(state.me(), hooks);
    let opp_branches = draw_side(state.opp(), hooks);
    let mut branches: Vec<(DefState, Fraction)> = Vec::new();
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
    // ...
```

Choosing the starting character and running the whole setup are the last two entries; an illegal choice makes the whole setup empty.


文件: crates/definitions/src/state/flows/opening.rs

```rust
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
    // ...
```

### 3.2 Switch and replacement

A switch returns the new state plus character-zone and combat-status pending items.


文件: crates/definitions/src/state/flows/switch.rs

```rust
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
```

`switch_opp` mirrors it, while replacement after a defeat only dispatches in and changed.


文件: crates/definitions/src/state/flows/switch.rs

```rust
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

### 3.3 Defeat cleanup

Cleanup needs the definition Max HP and the character's growth list, packed in `DefeatHpReset`.


文件: crates/definitions/src/state/flows/defeat.rs

```rust
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
```

The result carries new state, character and equipment pending, cleared Energy, the cleared state, and reset growth.


文件: crates/definitions/src/state/flows/defeat.rs

```rust
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
```

Both sides share the same implementation through paired entries.


文件: crates/definitions/src/state/flows/defeat.rs

```rust
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

### 3.4 Revive

Revive returns the new state plus character-zone pending items.


文件: crates/definitions/src/state/flows/revive.rs

```rust
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

### 3.5 End Phase

The End Phase only ticks zones; it returns pending items instead of executing them.


文件: crates/definitions/src/state/flows/end_phase.rs

```rust
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

### 3.6 Round start and plunging

Round start advances the round and resets both sides; combat-action end clears only the acting side's mark.


文件: crates/definitions/src/state/flows/round_begin.rs

```rust
/// **回合开始**：回合数 +1（到第 15 回合封顶）+ 双方每回合计数清零（技能使用次数；上限保留）。
///
/// 依据：规则书·其他规则 对局回合上限（回合数推进点）+ 规则书·回合流程「开始新的回合！」
/// （每回合计数清零）。
pub fn begin_round(state: &DefState) -> DefState {
    state
        .clone()
        .with_me(
    // ...
```


文件: crates/definitions/src/state/flows/plunging.rs

```rust
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

### 3.7 Referenced rule op

Revive callers check the per-round count through this rule helper first.


文件: crates/definitions/src/rules/ops/food.rs

```rust
/// 每回合最多通过料理复苏 1 个角色（规则书第 4 条 + 卡面 333009）。
pub const FOOD_REVIVE_PER_ROUND: Nat = Nat::new(1);

/// 本回合已通过料理复苏 revived_this_round 个角色时，还能不能复苏（规则书第 4 条）。
pub fn can_revive(revived_this_round: Nat) -> bool {
    revived_this_round < FOOD_REVIVE_PER_ROUND
}
```

## 4. Invariants and prohibitions

> [!IMPORTANT]

> The input `&DefState` is never mutated; every failure is `None` or `Err`.

> Zone flows dispatch pending items but do not execute them.

> [!WARNING]

> The switch flow does not pay dice, the defeat flow does not choose a replacement, and revive does not check the count or apply Satiated.

> The defeat zone is an internal record, not a rulebook-visible zone.

## 5. Basis

- Rulebook game setup: opening hand of five, mulligan, choose starting characters;
- S8.1 and S2.4: End Phase duration and discard when depleted;
- S7.3: defeat discards equipment and states, clears Energy;
- Game zone I(5): replacement after the active character is defeated;
- Special keyword Food: Satiated is cleared at round end or on defeat;
- S8.10: plaguing mark after a switch, cleared at combat-action end;
- D34 and D35: paired entries;
- D121 and D133: payment belongs to the thing being spent;
- D186: dispatch only orders, does not execute;
- D190: branches with exact probabilities in setup;
- D193: defeat cleanup belongs to the character block;
- D238: definition and actual Max HP;
- D255: cleanup order is settle, clean up, judge;
- D261: a switch arms the plunging mark;
- D262: the End Phase executes effects before drawing.

## 6. Common misuse

> [!WARNING]

> Deducting dice inside `switch_me`, or expecting `defeat_*` to choose the replacement.

> Looking for support-card duration ticking in `end_phase.rs`; it lives in `flow::turn`.

- Pre-checking the character-zone capacity before revive and reusing the result; the zone layer already returns `None`;
- Expecting revive to restore equipment, states, Energy, or Max-HP growth;
- Writing `first_player` in the setup flow; the first round's first player must be decided externally.

## 7. Tests

- `state/flows/opening.rs`: setup, Arcane preference, mulligan, illegal choice;
- `state/flows/switch.rs`: switch and replacement pending items;
- `state/flows/defeat.rs`: cleanup, growth reset, Satiated clearing;
- `state/flows/revive.rs`: revive and rejection cases;
- `state/flows/end_phase.rs`: ticks and Satiated clearing;
- `state/flows/round_begin.rs` and `plunging.rs`: round advance and mark clearing.

## 8. Pending integration

> [!NOTE]

> Old notes that revive and plunging were not wired are outdated; see the Flow page.

- First-player decision for round 1: not defined by the rulebook;
- Support-card duration decrement timing: currently done in the End Phase, to be verified;
- Whether replacement after a defeat counts as a switch action: pending review;
- Whether combat statuses are discarded when a character is defeated: currently transferred on replacement, to be verified.

## Summary

Zone flows only edit zones and dispatch; the caller decides when effects run. Keep the input immutable and let each zone's own primitives do the work.

---

Last verified: 2026-10-20 (code: `crates/definitions/src/state/flows/`; `rules/ops/food.rs`)
