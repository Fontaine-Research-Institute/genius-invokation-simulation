# 定义层-Families

> 中文 · [[定义层-Families-EN|English]]

*本页范围：`crates/definitions/src/families/` 的五个牌族总览——各自拥有哪个区、扩展点在哪。*

---

## 1. 职责

按**五个牌族（L1 `CardFamily`）**组织「类型定义 + 区域状态 + 区域操作」。**具体卡牌不在本 crate**：按 D240 它们位于 `gi-genius-invokation-cards`（`new/crates/cards/`）。

## 2. 关键概念与类型

### 2.1 三级模型（D51 / D53–D55）

| 层级 | 概念 | 定义位置 | 说明 |
|---|---|---|---|
| L1 | 牌族 | `src/base/family.rs` 的 `CardFamily` | `ActionCard` / `Summon` / `Character` / `Enemy` / `Status`；**牌族决定 id 区域**，跨牌族 id 不保证唯一 |
| L2 | 类型 | **只有行动牌与状态有**：`src/families/action_card/mod.rs` 的 `ActionCardType`、`src/families/status/mod.rs` 的 `StatusType` | 角色 / 召唤物 / 敌人没有类型；`src/families/mod.rs` 用穷尽 match 钉死 |
| L3 | 标签 | 顶层 `src/tags/`（与 families 同级，D53） | 每个标签 = 一个**零大小类型 + 一份静态 `TagText`**（D54 / D57 / D58 / D64）；没有「标签总枚举」、没有编号 |

### 2.2 五个牌族一览

| 牌族 | 定义载体 | 本族拥有的区 / 容器 | 扩展点 | 文件 |
|---|---|---|---|---|
| ActionCard | `ActionCardDef`（类型 / 文案 / 基础费用 / 打出条件 D217） | 手牌 `HandState`、牌库 `PileState`、支援区 `SupportZone`、手牌卡状态 `HandCardStates`、当前费用 `CurrentCost` | 新行动牌：cards crate；新类型级行为：`rules/` 规则行 + `flow/` | `src/families/action_card/{def.rs, zones/, areas/}` |
| Character | `CharacterDef` + `SkillDef` + `SkillKind` | 角色区 `CharacterZone`（3 槽 + 出战指针）、每角色装备栏 `EquipmentZone`、出战状态区 `CombatStatusZone`、`CharacterState`（生命 / 充能 / 状态 / 附着） | 新技能：`SkillDef`；新时机：`CharacterTiming`；新反应：`ReactionRegistry`；伤害修饰：`DamageModifiers` | `src/families/character/` |
| Summon | `SummonDef` + `SummonEffectKind` | 召唤物区 `SummonZone`（上限 4 / 同名合并 / 可用次数递减 D12） | 新召唤物：cards crate；执行动作待接 | `src/families/summon/{def.rs, zones/mod.rs}` |
| Enemy | `EnemyDef`（生命 / 技能列表，复用 `SkillDef`） | **暂无区 / 无操作** | 只给定义数据；运行期承载尚未设计 | `src/families/enemy/def.rs` |
| Status | `StatusDef` + `known.rs` 的 5 个正式身份 + `ArcaneEdict` | **状态没有自己的区**：角色状态在 `CharacterState.states`（公共 `StateList`，D105），出战状态在 `CombatStatusZone` | 新状态：`StatusDef` + 身份实例；具体状态语义归状态块 | `src/families/status/{def.rs, known.rs, arcane_edict.rs}` |

### 2.3 区的口径（D40 / D42 / D182 / D183）

- **区 = 状态结构里的一个字段（变量）**：没有 `AreaId`、没有区登记表、没有「实体 → 所属区」映射。
- 敌我 = **同一套容器类型建两个具名实例**（`DefState.me` / `DefState.opp`），不传 side 参数、不用 0/1 编码（D29 / D34）。
- 区对外只给**只读指针**（`entries()` / `len()` / `stats()` 等）；更新一律是**纯函数**（吃 `&self` 返回新值）。
- 时机：每个区自己定义时机键（如 `SupportTiming` / `SummonTiming` / `CharacterZoneTiming` / `CombatStatusTiming`）→ `timeline::dispatch` **只排序、不执行**（D185 / D186）。

> [!IMPORTANT]
> 区就是结构体字段（D42）：不要给区编号，也不要建「实体 → 区」表；实体在哪，由取它的那份代码自己知道。
> 敌我靠两个具名实例表达（D29 / D34），不靠参数或数字编码。

## 3. 公开入口

> [!NOTE]
> 本节代码逐字取自当前仓库（含 use / derive / 文档注释等样板）。超长函数体截断为「签名 + 开头几行」并注明从略；未截断的即为全文。

### 3.1 牌族与类型枚举

**`CardFamily`** — `crates/definitions/src/base/family.rs`

```rust
/// L1 牌族（id 区域）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum CardFamily {
    /// 行动牌（有类型：支援 / 装备 / 事件）。
    ActionCard,
    /// 召唤物（无类型）。
    Summon,
    /// 角色牌（无类型）。
    Character,
    /// 敌人牌（无类型）。
    Enemy,
    /// 状态（无类型；角色状态 / 出战状态在牌族自己的 StatusType 里区分）。
    Status,
}
```

**`ActionCardType`** — `crates/definitions/src/families/action_card/mod.rs`

```rust
/// 行动牌的类型（L2）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ActionCardType {
    /// 支援牌：打到支援区，留在场上持续生效（场地 / 伙伴 / 道具 / 护盾 / 元素幻变 …）。
    Support,
    /// 装备牌：挂到角色身上（武器 / 圣遗物 / 天赋 / 特技 …）。
    Equipment,
    /// 事件牌：结算完就离开（料理 / 秘传 / 元素共鸣 / 战斗行动 …）。
    Event,
}
```

**`StatusType`** — `crates/definitions/src/families/status/mod.rs`

```rust
/// 状态的类型（L2）：角色状态 / 出战状态。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StatusType {
    /// 角色状态：附在**某个角色**身上（跨回合，跟随角色）。
    CharacterStatus,
    /// 出战状态：附在**某方场上**（不跟随具体角色）。
    CombatStatus,
}
```

## 4. 不变量与禁止

- **不给区编号**（D42）；**容器不解释语义**（D108 / D112）；**数据只有一份**：文案在 `generated`（D61 / D64）。
- **纯函数优先**：操作吃 `&self` 返回新值；失败 = 不改状态 + `None` / `false` / 空分支，不 panic（除零除外，D44）。
- **不建集中注册表**：框架只给注册接口，具体卡自己注册（D20 / D22）。
- **具体卡不进 definitions**（D240 / D241）；`cards` 默认不接入 simulation（D243）。
- 数值一律 `Nat`（饱和）或 `ExactNat128`（概率）；禁止裸 `+ - * /` 与裸整型（§8.3）、禁止 `f64`（D159）、禁止 `HashMap`。

> [!WARNING]
> 在 `families/` 里建注册表或统计中枢，违反 D20 / D240；框架只给接口，具体功能谁要谁实现。
> 把 tag 当编号、按字符串比较 tag，违反 D54 / D57；比较标签请用指针身份（D199 / D200）。

## 5. 依据（规则书与 D 编号）

| 主题 | 依据 |
|---|---|
| 三个区域（角色 / 支援 / 召唤物）+ 手牌 / 牌库 / 装备附属 | 规则书「游戏区域 I–VI」；`families/*/zones/` |
| 牌族 = id 区域；类型 / 标签分层 | D51 / D53 / D54 / D55 |
| 敌我不进引擎、两个具名实例 | D29 / D30 / D34 |
| 区 = 字段、无区号 / 区表 | D40 / D42 |
| 统计下沉 `stats/`、只读指针、可裁剪 | D181 / D182 / D183 / D187 |
| 时机由牌族自己定义并主动派发 | D184 / D185 / D186 |
| 记录 API / 内容类型单独文件 | D221 / D223 / D224 / D225 |
| 框架 / 具体卡分 crate | D240 / D241 / D242 / D243 |

### 5.1 文档与代码不一致（供复核）

> [!NOTE]
> 本轮复核发现 `结构.md` 与代码有偏差；以 D53 / D55 与当前代码为准，结构图待更新。

1. `结构.md` §4.3 把标签画在 `families/<族>/types/<类型>/tags/` 下；实际在顶层 `src/tags/`。
2. `结构.md` 计划了 `families/action_card/types/`、`families/character/skills/`、`zones/status.rs`、`zones/skill.rs`；实际不存在（行动牌类型级行为在 `rules/` + `flow/`；技能流程在 `skill_flow/`；角色状态走公共 `StateList`）。
3. `结构.md` 把出战状态写成 `zones/status.rs`；实际文件是 `zones/combat_status.rs`。

## 6. 常见误用

> [!WARNING]
> 下面几类写法会破坏「区 = 字段」「数据只有一份」「不建注册表」三条全局约定。

1. **把 tag 当编号**：标签是零大小类型 + 静态元数据指针（D54 / D57），不能 `as u32`、不能按字符串比较。
2. **给区加编号 / 建「实体 → 区」表**：违反 D42。
3. **在 `families/` 里建注册表或统计中枢**：违反 D20 / D240。
4. **同一 id 不带牌族就引用**：不同牌族 id 可重复（D51），必须 `(CardFamily, CardId)`。
5. **把 stats 算进局面等价**：各区 `PartialEq` 明确排除统计（D187）。
6. **在定义里抄文案**：文案从 `generated` 取（D61 / D64 / D120）。
7. **把「持续回合」当「可用次数」**：可用次数按触发次数递减（D12），持续回合按结束阶段递减（规则书；D13 旧口径已被覆盖）。

## 7. 测试

| 位置 | 测试 |
|---|---|
| `src/families/mod.rs` | `tests::only_action_card_and_status_have_types`、`tests::type_enums_belong_to_their_own_family` |
| `src/families/action_card/mod.rs` | `tests::action_card_has_exactly_three_types` |
| `src/families/status/mod.rs` | `tests::status_types_share_one_id_region` |
| 各牌族专页 | 见 [[定义层-Character]] / [[定义层-Action-Card]] / [[定义层-Summon-Enemy-Status]] 的「测试」节 |

## 8. 待接入与待实测

- **敌人牌没有区 / 运行期承载**：代码标「待接入」；`留空.md` 目前没有专条（建议补登）。
- **具体卡牌只有 1 张样板**（`cards/src/cards/character`）：属卡牌实现阶段；`留空 #12` 记角色牌实现结构待问用户，多版本加载见 `留空 #24`（D248 / D249 / D251）。
- **卡牌版本区间加载**未实现：`留空 #24`。
- **行动牌 / 状态没有 `types/` 子目录**：见 §5.1。
- 各区自己的待实测（持续回合时机、支援区顺序、召唤物合并口径等）见对应牌族专页，并引用 `留空 #67` / `#93` / `#94`。

---
最后核对：2026-10-19（代码：`crates/definitions/src/families/mod.rs::tests::only_action_card_and_status_have_types`）
