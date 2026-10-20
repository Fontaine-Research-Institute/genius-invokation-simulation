# 定义层-State-Flows

> 中文 · [[定义层-State-Flows-EN|English]]

*本页范围: `state/flows/*` 的区流程纯函数.*

---

## 1. 职责

这一层做区域级工作: 开局, 切换, 击倒, 复苏, 结束阶段, 回合开始, 以及清除下落标记.

每个函数吃 `&DefState`, 返回新局面加派发出来的待执行. 它自己不执行这些效果.

## 2. 关键概念与类型

七份文件分工, 每份负责一条区域级流程.

- `opening.rs`: 起手抓牌, 换牌, 选首发;
- `switch.rs`: 切换出战角色并转移出战状态;
- `defeat.rs`: 移出角色, 弃装备, 清状态与充能, 重置生命上限;
- `revive.rs`: 把被击倒的角色放回角色区;
- `end_phase.rs`: 出战状态与召唤物的持续回合和可用次数 tick, 解除饱腹;
- `round_begin.rs`: 推进回合并清零每回合计数;
- `plunging.rs`: 战斗行动结束时清除下落标记.

所有入口都成对给出 `*_me` 与 `*_opp`, 签名里不出现侧参数.

## 3. 公开入口

### 3.1 游戏准备

开局准备接收两个回调: 一个判断这张牌是不是秘传, 一个给出基础费用.


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

玩家决策收在一个小结构里, 由 `new` 与两个 `with_*` 构造.


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

起手抓牌与换牌都会展开成带精确概率的分支.


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

选首发与串起整个开局是最后两个入口; 选择非法会让整个开局返回空.


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

### 3.2 切换与补位

切换返回新局面, 以及角色区与出战状态区的待执行.


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

`switch_opp` 与之对称; 击倒后的补位只派发进入与变更.


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

### 3.3 击倒清理

清理需要定义最大生命值与这个角色的成长列表, 打包在 `DefeatHpReset` 里.


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

结果带新局面, 角色与装备区的待执行, 清掉的充能, 清理后的状态, 以及重置后的成长.


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

双方通过成对入口共用同一份实现.


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

### 3.4 复苏

复苏返回新局面与角色区待执行.


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

### 3.5 结束阶段

结束阶段只做 tick, 返回待执行而不执行.


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

### 3.6 回合开始与下落标记

回合开始推进回合并重置双方; 战斗行动结束只清行动方的标记.


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

### 3.7 被引用的规则操作

复苏的调用方会先用这个规则函数判每回合次数.


文件: crates/definitions/src/rules/ops/food.rs

```rust
/// 每回合最多通过料理复苏 1 个角色（规则书第 4 条 + 卡面 333009）。
pub const FOOD_REVIVE_PER_ROUND: Nat = Nat::new(1);

/// 本回合已通过料理复苏 revived_this_round 个角色时，还能不能复苏（规则书第 4 条）。
pub fn can_revive(revived_this_round: Nat) -> bool {
    revived_this_round < FOOD_REVIVE_PER_ROUND
}
```

## 4. 不变量与禁止

> [!IMPORTANT]

> 入参 `&DefState` 不会变; 所有失败都是 `None` 或 `Err`.

> 区流程只派发待执行, 不执行.

> [!WARNING]

> 切换不付骰, 击倒不选补位, 复苏不判次数也不加饱腹.

> 击倒区是内部记录, 不是规则书可见区域.

## 5. 依据

- 规则书游戏准备: 起手五张, 换牌, 选首发;
- S8.1 与 S2.4: 结束阶段持续回合与耗尽弃置;
- S7.3: 击倒弃装备与状态, 清空充能;
- 游戏区域 I(5): 出战角色被击倒后补位;
- 特殊词条料理: 饱腹在回合结束或击倒时解除;
- S8.10: 切换后置位下落标记, 战斗行动结束清除;
- D34 与 D35: 成对入口;
- D121 与 D133: 支付归被花的东西;
- D186: 派发只排序, 不执行;
- D190: 开局的分支带精确概率;
- D193: 击倒清理归角色块;
- D238: 生命上限的定义值与实际值;
- D255: 顺序是结算, 清理, 判定;
- D261: 切换会置位下落标记;
- D262: 结束阶段先效果再抓牌.

## 6. 常见误用

> [!WARNING]

> 在 `switch_me` 里扣骰, 或指望 `defeat_*` 选补位.

> 在 `end_phase.rs` 里找支援牌持续回合 tick; 它在 `flow::turn` 里.

- 复苏前自己判角色区是否满并复用结果; 区层已经返回 `None`;
- 指望复苏恢复装备, 状态, 充能或生命上限成长;
- 在开局流程里写 `first_player`; 第一回合先手必须由外部裁定.

## 7. 测试

- `state/flows/opening.rs`: 开局, 秘传优先, 换牌, 非法选择;
- `state/flows/switch.rs`: 切换与补位的待执行;
- `state/flows/defeat.rs`: 清理, 成长重置, 饱腹解除;
- `state/flows/revive.rs`: 复苏与拒绝场景;
- `state/flows/end_phase.rs`: tick 与饱腹解除;
- `state/flows/round_begin.rs` 与 `plunging.rs`: 回合推进与标记清除.

## 8. 待接入

> [!NOTE]

> 旧条目说复苏与下落标记未接线, 已过时; 见 Flow 页.

- 第一回合先手裁定: 规则书未写;
- 支援牌持续回合递减时机: 现按结束阶段, 待实测;
- 击倒补位算不算一次切换行动: 待复核;
- 角色被击倒时出战状态是否弃置: 现按补位时转移, 待实测.

## 小结

区流程只改区并派发, 何时执行效果由调用方决定. 保持入参不可变, 把具体操作交给各区自己的原语.

---

最后核对: 2026-10-20 (代码: `crates/definitions/src/state/flows/`; `rules/ops/food.rs`)
