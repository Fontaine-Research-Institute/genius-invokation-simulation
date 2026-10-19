# 定义层-Summon-Enemy-Status

> 中文 · [[定义层-Summon-Enemy-Status-EN|English]]

*本页范围: `crates/definitions/src/families/{summon,enemy,status}/` —— 召唤物 (定义 + 召唤物区) , 敌人 (仅定义) , 状态 (定义 + 身份 + 秘传揭令容器; 承载在角色族) .*

---

## 1. 职责

三个"没有类型树"的牌族: **召唤物** (定义 + 召唤物区) , **敌人** (只有定义) , **状态** (定义 + 状态身份 + 秘传揭令容器) .状态的**承载不在本族**: 角色状态进 `CharacterState.states`, 出战状态进 `CombatStatusZone`.

## 2. 关键概念与类型

### 2.1 召唤物 (`summon/`) 

| 类型 | 文件 | 要点 |
|---|---|---|
| `SummonDef<Tags>` | `summon/def.rs` | `id` / `name` / `description` / `effect_kind: SummonEffectKind` / `effect_points: Nat` / `default_uses: Option<Nat>` / `tags`; `entry(summon) -> SummonEntry` 纯搬运 |
| `SummonEffectKind` | `summon/def.rs` | 最小集合: `Damage` / `Heal` / `Status` / `Other` (数据没有结构化字段, 反推; 待复核)  |
| `SummonEntry` | `summon/zones/mod.rs` | `summon` (实体) / `card` (卡 id) / `uses: Option<Nat>` (可用次数) / `points: Option<Nat>` (效果量) / `effect_kind: Option<SummonEffectKind>` |
| `SummonZone` | `summon/zones/mod.rs` | `entries` (顺序 = 召唤先后) + `stats`; `SUMMON_CAP = 4` |

召唤物区规则: 

- 上限 4, 满员**无法生成新召唤物**; 但**同名召唤物仍先尝试合并** (合并不新增占位, 不违反上限, 待复核) .
- **同名合并**: 次数与效果量取**最大**, 效果类型取**最新**, **保留原位置** (分工计划 §4.4.15) .
- **可用次数按触发次数递减** (D12) : `tick_with_triggers` 每次触发 -1, 到 0 **立刻弃置** (复用 `rules::ops::lifetime`) .
- 时机键: `SummonTiming::{Summoned, Merged, Triggered, Depleted, Discarded}` (D185) ; `execute_summon` 执行待执行.
- 待接入: 具体执行动作 (`SummonAction` / `SummonMeasure`) 留给召唤物执行块, 本块只给计量 + 正式效果类型.

### 2.2 敌人 (`enemy/`) 

- `id` / `name` / `description: Localized`: 数据没有顶层描述 → `Localized::EMPTY` (D56 显式缺失) .
- `max_hp: Nat`: 定义数据 (上限来源) .
- `skills: &'static [&'static SkillDef]`: 复用角色技能定义 (D121 / D162) .
- `tags`: 元素 / 势力 / 武器子类 / 其他武器 (D55 唯一定义) .
- **源码见 §3.3** (`crates/definitions/src/families/enemy/def.rs`) .

- 敌人牌**没有区, 没有操作**: 本轮只做定义载体; 运行期承载与具体敌人实现待接入 (代码注释标 `待接入`, 留空.md 尚无专条, 建议补登) .
- 数据有 `maxenergy` 字段但本块未收 (充能上限归充能块, 待接入) .
- 描述不从技能文案拼 (那会是第二份数据) .

### 2.3 状态 (`status/`) 

| 类型 | 文件 | 要点 |
|---|---|---|
| `StatusType` | `status/mod.rs` | `CharacterStatus` / `CombatStatus`; **两个类型共用状态牌族的同一个 id 区域** (D51) , 靠类型区分 |
| `StatusDef<Tags>` | `status/def.rs` | `id` / `status_type` / `text: StateText` / `default_layers: Option<Nat>` (None = 布尔) / `duration: Option<Nat>` / `tags`; `form()` 由 `default_layers` 推导 |
| 正式身份 (5 个)  | `status/known.rs` | 203 出战护盾 / 134 角色护盾 / 106 冻结 / 116 草原核 / 117 激化领域; 身份实例持有 `&'static StatusDef` (指针大小, **不是 ZST**: ZST 静态地址会重合, D200)  |
| `ArcaneEdict` + `ARCANE_EDICT` | `status/arcane_edict.rs` | 秘传揭令 (秘传揭令) : **没有状态卡行 → 不伪造 StatusDef**; 只给状态身份 (布尔) + 每位牌手一份的使用机会容器 |

状态类型与承载 (D51 / D105 / D201) : 

```text
角色状态 -> CharacterState.states（公共 StateList；位置 CHARACTER_STATE_POS = 0）
出战状态 -> CombatStatusZone（挂在出战角色；切换时转移；结束阶段 duration -1）
状态牌族本身没有区：StatusDef 只是定义数据，身份 = 指向定义数据的静态指针
```

- 状态**共用一个公共列表** `StateList` (D104 / D105) : `StateEntry { state, pos, layers }`; 布尔类 `layers = None` (在列表里即代表"有") .
- **不建第三类状态**: 数据里的"角色装备" (276 条) 与 D51 冲突 → 不建 (D201, 待复核) .
- **标签**: `tags/status/` 本轮为空, 状态定义只导入别处已有的唯一定义 (护盾 / 草元素产物) , 其余 `()`.

### 2.4 关键状态身份 (跨文件) 

| 状态 | 身份实例 | 形态 | 数据 / 依据 | 定义位置 |
|---|---|---|---|---|
| 冻结 | `FROZEN` | 布尔 (duration 1, 待复核)  | 状态 106; S6.6'角色无法使用技能' | `families/status/known.rs` |
| 眩晕 | `STUN` | 布尔 | 眩晕 + 数据 100001 | `families/character/skill_flow/forbidden.rs` (共享身份)  |
| 石化 | `PETRIFIED` | 布尔 | 数据 116033 / 176033 (同效果两份)  | 同上 |
| 饱腹 | `SATIATED` | 布尔 | 数据 303300; S8.1 | `families/character/satiated.rs` (共享身份)  |
| 护盾 (出战 / 角色)  | `SHIELD_COMBAT` / `SHIELD_CHARACTER` | 层数 | 状态 203 / 134 | `families/status/known.rs` |
| 草原核 / 激化领域 | `DENDRO_CORE` / `CATALYZING_FIELD` | 层数 (可用次数)  | 状态 116 / 117 | `families/status/known.rs` |
| 伤害抵消 (减伤)  | `DAMAGE_NEGATION` | 层数 | 规则书·特殊词条; 无独立卡牌实体 | `families/character/damage_chain/statuses.rs` (共享身份)  |
| 伤害免疫 | `DAMAGE_IMMUNITY` | 布尔 | 穿透伤害 + 卡面"免疫所有伤害"; 无独立卡牌实体 | 同上 (共享身份)  |

桥表 (`families/character/damage_chain/statuses.rs`) : 

- `STATUS_ENTITIES: [StatusEntity; 8]` = 106 / 203 / 116 / 117 / 134 + 100001 眩晕 / 116033 石化 / 303300 饱腹.
- `status_of_card(CardId) -> Option<&'static dyn StateIdentity>`; `same_identity` / `identity_in` 只比 `identity_address()` (D57 / D58 / D200) .
- 集合: `SHIELD_STATUSES` / `REDUCTION_STATUSES` / `IMMUNITY_STATUSES`; 手牌 6 状态有 `hand_status_identity` / `hand_status_of_identity` (见 [[定义层-Action-Card]]) .

### 2.5 状态持续时间 (D13 与规则书的冲突) 

- 规则书·特殊词条"持续回合": **每个结束阶段** -1, 耗尽立刻弃置 (英文 "Each End Phase") .
- 代码 `rules/ops/lifetime.rs` 按**规则书**实现 (`ticks_in_end_phase() = true`, `duration_after_end_phase`) , 并在注释里写明"与 D13 的旧写法 (回合开始递减) 冲突, 已按规则书为准".
- 出战状态区的 `tick_end_phase_with_triggers` 与支援区的同名操作都按结束阶段实现.

> [!IMPORTANT]
> 可用次数 ≠ 持续回合: 可用次数按**触发次数**递减 (D12) ; 持续回合按**结束阶段**递减 (规则书, D13 旧口径已被覆盖) .
> 不要把两者混成一个字段, 也不要按"行动阶段开始 -1"实现 (参考实现的老 bug, D12) .

## 3. 公开入口

> [!NOTE]
> 本节代码取自当前仓库 (含 use / derive / 文档注释等样板) .超长函数体截断为"签名 + 开头几行"并注明从略; 未截断的即为全文.

### 3.1 召唤物定义

**`SummonEffectKind`** — `crates/definitions/src/families/summon/def.rs`

```rust
/// 召唤物的**效果类型**（最小集合：伤害 / 治疗 / 状态 / 其它）。
///
/// 区域层（`zones/mod.rs`）不解释这个枚举，只把它当「效果类型标识」参与同名合并
/// （取最新，分工计划 §4.4.15）；具体效果由召唤物定义 / 执行动作解释。
///
/// `待复核`：数据里没有效果类型字段，本枚举是**反推**的最小集合；
/// 元素伤害的「元素」不在这里（那是伤害类型，归角色族 / 伤害块），本枚举只分「做什么」。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SummonEffectKind {
    /// 造成伤害（效果量 = 伤害点数）。
    Damage,
    /// 治疗（效果量 = 治疗点数）。
    Heal,
    /// 赋予 / 附属状态（效果量 = 状态层数 / 可用次数，含义由状态定义解释）。
    Status,
    /// 其它（加伤 / 减伤 / 特殊效果等没有可计量点数的效果；效果量通常为 `Nat::ZERO`）。
    Other,
}
```

**`SummonDef`** — `crates/definitions/src/families/summon/def.rs`

```rust
/// 一个**召唤物**的定义（纯数据）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
#[derive(Clone, Copy, Debug)]
pub struct SummonDef<Tags> {
    /// 召唤物卡的卡 id（召唤物牌族 id 区域内唯一）。
    pub id: CardId,
    /// 4 语名字（本块只填中文，D120）。
    pub name: Localized,
    /// 4 语描述（本块只填中文，D120）。
    pub description: Localized,
    /// 效果类型（最小集合，见 [`SummonEffectKind`]）。
    pub effect_kind: SummonEffectKind,
    /// 效果量：伤害点数 / 治疗点数 / 状态层数等；`Nat::ZERO` = 无可计量效果量。
    pub effect_points: Nat,
    /// 默认可用次数：`None` = 没有可用次数（触发不递减，D12 / 分工计划「三项改可选」）。
    pub default_uses: Option<Nat>,
    /// 标签集合（D54 / D55）。
    pub tags: Tags,
}
```

**`entry`** — `crates/definitions/src/families/summon/def.rs`

```rust
    /// 用这份定义造召唤物区条目（实体 + 卡 id + 可用次数 + 效果量 + 效果类型）。
    ///
    /// 纯搬运：不改定义、不查任何表；同名合并口径在区域层（D12 / 分工计划 §4.4.15）。
    pub fn entry(&self, summon: EntityId) -> SummonEntry {
        SummonEntry::new(summon, self.id)
            .with_uses(self.default_uses)
            .with_points(Some(self.effect_points))
            .with_effect_kind(Some(self.effect_kind))
    }
```

### 3.2 召唤物区

**`SUMMON_CAP`** — `crates/definitions/src/families/summon/zones/mod.rs`

```rust
/// 一方召唤物区上限：4（规则书·游戏区域 IV）。
pub const SUMMON_CAP: Nat = Nat::new(4);
```

**`SummonEntry`** — `crates/definitions/src/families/summon/zones/mod.rs`

```rust
/// 一个召唤物：召唤物实体 + 召唤物卡 id + 计量（可用次数 / 效果量 / 效果类型标识）。
///
/// - `uses`：可用次数（`None` = 没有可用次数，触发不递减 —— 分工计划「三项改可选」）；
/// - `points`：效果量（伤害点数 / 重复次数…，含义由执行动作解释，`None` = 无计量）；
/// - `effect_kind`：效果类型（`SummonEffectKind`，由召唤物定义给出；区域层不解释）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SummonEntry {
    /// 召唤物实体号。
    pub summon: EntityId,
    /// 召唤物卡定义 id。
    pub card: CardId,
    /// 可用次数（None = 无可用次数）。
    pub uses: Option<Nat>,
    /// 效果量（含义由执行动作解释）。
    pub points: Option<Nat>,
    /// 效果类型（正式枚举；区域层不解释）。
    pub effect_kind: Option<SummonEffectKind>,
}
```

**`SummonZone`** — `crates/definitions/src/families/summon/zones/mod.rs`

```rust
/// **召唤物区**：最多 4 个召唤物，按召唤先后排列。
///
/// 容器没有身份（D42 / D182）：它就是状态里的一个字段；「这是谁的区域」由持有它的
/// 我方 / 敌方两个具名实例表达（D29 / D34）。
#[derive(Clone, Debug, Default)]
pub struct SummonZone {
    /// 召唤物条目（顺序 = 召唤先后）。
    entries: Vec<SummonEntry>,
    /// 统计（D187；feature 关闭时零大小、空操作）。
    stats: SummonZoneStats,
}
```

**`summon_with_triggers`** — `crates/definitions/src/families/summon/zones/mod.rs`

```rust
    /// **召唤 / 生成**（规则书·游戏区域 IV）+ 派发「生成 / 合并」。
    ///
    /// - 同名（同卡 id）已存在：**合并**（使用次数取最大、效果量取最大、效果类型取最新），
    ///   不新增占位、不改变原位置；`outcome = Merged`；
    /// - 未满：占一个新位置；`outcome = Summoned`；
    /// - 已满且没有同名：**无法生成**，状态不变；`outcome = RejectedFull`（不派发）。
    pub fn summon_with_triggers(
        &self,
        entry: SummonEntry,
        triggers: &SummonTriggers,
    ) -> SummonResolution {
        let mut zone = self.clone();
        let mut merged: Option<(EntityId, Option<Nat>)> = None;
    // ...（其余实现从略；见 crates/definitions/src/families/summon/zones/mod.rs）
```

**`tick_with_triggers`** — `crates/definitions/src/families/summon/zones/mod.rs`

```rust
    /// **触发一次**（规则书·特殊词条 可用次数）：可用次数 -1；耗尽则立刻弃置。
    ///
    /// - 有可用次数且减到 0：移出 + 派发 `Depleted`；
    /// - 有可用次数且还有剩余：写回剩余次数 + 派发 `Triggered`（`amount` = 剩余次数）；
    /// - **没有可用次数**：不递减、不移除，只派发 `Triggered`（`amount` = 0）。
    ///
    /// 失败（返回 None、不改状态）：这个召唤物不在区里。
    pub fn tick_with_triggers(
        &self,
        summon: EntityId,
        triggers: &SummonTriggers,
    ) -> Option<SummonResolution> {
        let position = self
            .entries
    // ...（其余实现从略；见 crates/definitions/src/families/summon/zones/mod.rs）
```

### 3.3 敌人牌

**`EnemyDef`** — `crates/definitions/src/families/enemy/def.rs`

```rust
/// 一张**敌人牌**的定义（纯数据）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；与 `CharacterDef` 同一做法）。
#[derive(Clone, Copy, Debug)]
pub struct EnemyDef<Tags> {
    /// 敌人牌卡 id（敌人牌族 id 区域内唯一）。
    pub id: CardId,
    /// 敌人名（本块只填中文，D120）。
    pub name: Localized,
    /// 敌人牌描述（数据没有顶层描述 -> `Localized::EMPTY`，见模块文档）。
    pub description: Localized,
    /// 生命上限（定义数据；运行期当前值不在本层）。
    pub max_hp: Nat,
    /// 技能列表（复用 [`SkillDef`]；顺序 = 数据里的技能顺序）。
    pub skills: &'static [&'static SkillDef],
    /// 标签集合（D54 / D55）。
    pub tags: Tags,
}
```

**`new`** — `crates/definitions/src/families/enemy/def.rs`

```rust
    /// 建一张敌人牌定义（常量 / 测试用）。
    pub const fn new(
        id: CardId,
        name: Localized,
        description: Localized,
        max_hp: Nat,
        skills: &'static [&'static SkillDef],
        tags: Tags,
    ) -> EnemyDef<Tags> {
        EnemyDef {
            id,
            name,
            description,
            max_hp,
            skills,
            tags,
        }
    }
```

**`has_skills`** — `crates/definitions/src/families/enemy/def.rs`

```rust
    /// 是否有技能（数据里每个敌人都有技能，但定义层不假设至少一个）。
    pub fn has_skills(&self) -> bool {
        !self.skills.is_empty()
    }
```

### 3.4 状态定义

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

**`StatusDef`** — `crates/definitions/src/families/status/def.rs`

```rust
/// 一张**状态牌**的定义（纯数据；身份由 `known.rs` 的具体类型给）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
/// 本类型**不实现** `StateIdentity`：身份是「指向静态实例的指针」（D95 / D96），
/// 定义只是那份静态实例里的一份数据。
#[derive(Clone, Copy, Debug)]
pub struct StatusDef<Tags> {
    /// 状态牌的卡 id（状态牌族 id 区域内唯一）。
    ///
    /// 没有独立卡牌的共享身份（如「伤害抵消」）**不建** `StatusDef`，见
    /// `damage_chain::statuses`（那是「效果共享身份」，不是某张卡）。
    pub id: CardId,
    /// 状态类型（角色状态 / 出战状态，D51）。
    pub status_type: StatusType,
    /// 4 语名字 + 4 语描述（**唯一一份**；身份经它取，不另存）。
    pub text: StateText,
    /// 默认层数：`None` = 布尔形态；`Some(n)` = 层数形态默认 n 层。
    pub default_layers: Option<Nat>,
    /// 持续回合数：`None` = 不按回合清减；`Some(n)` = 回合开始清减 n（D13）。
    pub duration: Option<Nat>,
    /// 标签集合（D54：标签是零大小类型；集合形态本轮未定）。
    pub tags: Tags,
}
```

**`new`** — `crates/definitions/src/families/status/def.rs`

```rust
    /// 建一份状态定义（常量 / 测试用；6 个参数，不再拆构造器）。
    pub const fn new(
        id: CardId,
        status_type: StatusType,
        text: StateText,
        default_layers: Option<Nat>,
        duration: Option<Nat>,
        tags: Tags,
    ) -> StatusDef<Tags> {
        StatusDef {
            id,
            status_type,
            text,
            default_layers,
            duration,
            tags,
        }
    }
```

**`form`** — `crates/definitions/src/families/status/def.rs`

```rust
    /// 状态的形态（D104）：`default_layers` 为 `None` -> 布尔；`Some` -> 层数。
    ///
    /// 形态由「默认层数」这一份数据推导，不另存第二份（D64 精神）。
    pub const fn form(&self) -> StateForm {
        match self.default_layers {
            None => StateForm::Boolean,
            Some(_) => StateForm::Layered,
        }
    }
```

**`status_identity`** — `crates/definitions/src/families/status/known.rs`

```rust
/// 声明一个状态的「身份类型 + 身份静态实例」。
///
/// 身份实例 = 指向该状态静态定义的一份指针（`&'static StatusDef<Tags>`）；地址即身份
/// （D95 / D96），文本从定义里取（只有一份，D110）。
macro_rules! status_identity {
    (
        $(#[$meta:meta])*
    // ...（其余实现从略；见 crates/definitions/src/families/status/known.rs）
```

**`SHIELD_COMBAT_DEF`** — `crates/definitions/src/families/status/known.rs`

```rust
/// 出战状态「护盾」（id 203）的定义。
///
/// 数据：`shield.json`；名字「护盾」，`statustypetext` = 阵营出战状态，
/// 描述「为我方出战角色提供1点护盾。（可叠加，没有上限）」，标签 `GCG_TAG_SHEILD`。
pub static SHIELD_COMBAT_DEF: StatusDef<Shield> = StatusDef::new(
    CardId::new(203),
    StatusType::CombatStatus,
    StateText {
        name: Localized::from_parts(Some("护盾"), None, None, None),
        description: Localized::from_parts(
            Some("为我方出战角色提供1点护盾。（可叠加，没有上限）"),
            None,
            None,
            None,
        ),
    },
    // 卡面「提供1点护盾」；待复核：数据没有结构化层数字段。
    Some(Nat::new(1)),
    None,
    Shield,
);
```

**`FROZEN_DEF`** — `crates/definitions/src/families/status/known.rs`

```rust
/// 角色状态「冻结」（id 106）的定义。
///
/// 数据：`frozen.json`；名字「冻结」，`statustypetext` = 角色状态，
/// 描述「角色无法使用技能。（持续到回合结束）\n角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。」；
/// 标签 `GCG_TAG_FORBIDDEN_ATTACK`（标签类型还没有 -> `()`，待接入）。
pub static FROZEN_DEF: StatusDef<()> = StatusDef::new(
    CardId::new(106),
    StatusType::CharacterStatus,
    StateText {
        name: Localized::from_parts(Some("冻结"), None, None, None),
        description: Localized::from_parts(
            Some("角色无法使用技能。（持续到回合结束）\n角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。"),
            None,
            None,
            None,
        ),
    },
    // 布尔形态（D104：在列表里即代表「有」）。
    None,
    // 数据只写「持续到回合结束」，按 1 回合计（D13 回合开始清减）；待复核。
    Some(Nat::new(1)),
    (),
);
```

### 3.5 状态身份桥

**`StatusEntity`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 一条「卡 id -> 状态身份」登记。
#[derive(Clone, Copy, Debug)]
pub struct StatusEntity {
    /// 状态卡的卡 id（状态牌族 id 区域）。
    pub card: CardId,
    /// 状态身份（类型化静态实例指针，D95 / D96）。
    pub identity: &'static dyn StateIdentity,
}
```

**`STATUS_ENTITIES`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 反应会附属 / 生成的状态**静态登记表**（顺序 = 规则书条目顺序，不参与判定）。
///
/// 后三项不是反应产物，而是**关键字 / 卡牌会按卡 id 附属**的状态（R5 / L1）：
/// 眩晕 100001、石化 116033、饱腹 303300 —— 登记后 `keywords::attach_status`
/// 用同一张表就能查到身份（状态块落地前先由本族提供共享身份）。
pub const STATUS_ENTITIES: [StatusEntity; 8] = [
    StatusEntity::new(entities::FROZEN, &FROZEN),
    StatusEntity::new(entities::SHIELD, &SHIELD_COMBAT),
    StatusEntity::new(entities::DENDRO_CORE, &DENDRO_CORE),
    StatusEntity::new(entities::CATALYZING_FIELD, &CATALYZING_FIELD),
    StatusEntity::new(CardId::new(134), &SHIELD_CHARACTER),
    StatusEntity::new(STUN_CARD, &STUN),
    StatusEntity::new(PETRIFIED_CARD, &PETRIFIED),
    StatusEntity::new(SATIATED_CARD, &SATIATED),
];
```

**`status_of_card`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 按卡 id 查状态身份（没登记 -> `None`）。
pub fn status_of_card(card: CardId) -> Option<&'static dyn StateIdentity> {
    STATUS_ENTITIES
        .iter()
        .find(|entry| entry.card == card)
        .map(|entry| entry.identity)
}
```

**`same_identity`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 两个状态身份是否**同一份身份**（比较 `identity_address()` = 指向定义数据的地址；见模块文档）。
pub fn same_identity(left: &'static dyn StateIdentity, right: &'static dyn StateIdentity) -> bool {
    crate::families::action_card::status::same_state_identity(left, right)
}
```

**`identity_in`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 这条身份是否在给定的身份集合里（按地址比较）。
pub fn identity_in(
    identity: &'static dyn StateIdentity,
    set: &[&'static dyn StateIdentity],
) -> bool {
    set.iter()
        .copied()
        .any(|candidate| same_identity(candidate, identity))
}
```

**`SHIELD_STATUSES`** — `crates/definitions/src/families/character/damage_chain/statuses.rs`

```rust
/// 算作**护盾**的全部状态身份（出战状态 203 + 角色状态 134）。
///
/// 两种写法（「为我方出战角色提供 N 点护盾」/「每层抵消 1 点此角色受到的伤害」）在规则上
/// 都是护盾，所以伤害修正链把两处承载的点数**相加**后一起抵消。
pub const SHIELD_STATUSES: &[&'static dyn StateIdentity] = &[&SHIELD_COMBAT, &SHIELD_CHARACTER];
```

### 3.6 本页引用的规则 ops

> [!NOTE]
> 以下为本文引用的关键字层 / 规则 ops 的真实源码 (签名与文档注释取自仓库) .

**`USES_PER_TRIGGER`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 每次触发消耗的可用次数（规则书：1 次）。
pub const USES_PER_TRIGGER: Nat = Nat::new(1);
```

**`uses_after_trigger`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 触发一次后的可用次数（减到 0 为止；调用方随后弃置）。
pub fn uses_after_trigger(uses: Nat) -> Nat {
    nsub!(uses, USES_PER_TRIGGER)
}
```

**`is_depleted`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 是否耗尽（= 0 -> 立刻弃置）。
pub fn is_depleted(value: Nat) -> bool {
    value == Nat::ZERO
}
```

**`DURATION_PER_END_PHASE`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 每个结束阶段扣减的持续回合（规则书：1）。
pub const DURATION_PER_END_PHASE: Nat = Nat::new(1);
```

**`duration_after_end_phase`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 经过一个结束阶段后的持续回合（减到 0 为止）。
pub fn duration_after_end_phase(duration: Nat) -> Nat {
    nsub!(duration, DURATION_PER_END_PHASE)
}
```

**`ticks_in_end_phase`** — `crates/definitions/src/rules/ops/lifetime.rs`

```rust
/// 持续回合的递减发生在**结束阶段**（不是回合开始、也不是行动阶段开始）。
///
/// 规则书原文：「每个结束阶段，都会使『持续回合』-1」；英文 "Each End Phase"。
/// 注意：这与决策日志 D13 的旧写法（回合开始递减）冲突，已按规则书为准改口径。
pub fn ticks_in_end_phase() -> bool {
    true
}
```

## 4. 不变量与禁止

- **可用次数 ≠ 持续回合** (D12 / 规则书"持续回合") .
- **召唤物上限 4**: 满员不生成新占位; 同名合并不新增占位, **保留原位置**; 次数 / 效果量取最大, 效果类型取最新 (分工计划 §4.4.15) .
- **状态只有一个公共列表** (D104 / D105) : 不建第二套容器 / 编号表 / 分区列表.
- **身份 = 定义数据地址** (D57 / D58 / D200) : 比较用 `same_identity`, 不要比 `&dyn` 胖指针或 ZST 自身地址.
- **框架不解释状态语义** (D108) : `StateList` / `CombatStatusZone` 只存身份 + 层数 + duration.
- **状态只有两个类型** (D51 / D201) : 不建"角色装备"第三类.
- **共享身份不伪造卡 id / StatusDef** (秘传揭令, 伤害抵消, 伤害免疫) : 没有卡牌数据的身份只给静态文本.
- **敌人牌本轮只定义, 不建区**: 不要把角色区逻辑复制给敌人.
- 纯函数, 失败不改状态; 全 `Nat` / `Fraction`; 零 `f64`, 零 `HashMap`.

> [!WARNING]
> 给状态建编号 / 分区表 / 第二套列表, 或用 ZST 自身地址做身份, 都会让状态合并与反应命中失效 (D42 / D105 / D200) .

> [!NOTE]
> "饱腹 / 眩晕 / 石化"当前是角色牌族的**共享身份**; 正式 `StatusDef` 归状态块, 二选一 (留空 #79) , 不要两套并存.

## 5. 依据 (规则书与 D 编号) 

| 主题 | 依据 |
|---|---|
| 召唤物区上限 4 / 满员无法生成 / 按排列顺序结算 | 规则书"游戏区域 IV" |
| 可用次数按触发次数递减, 耗尽弃置 | 规则书·特殊词条"可用次数"; D12 |
| 持续回合每个结束阶段 -1 | 规则书·特殊词条"持续回合"; 代码按规则书, D13 旧写法已覆盖 |
| 同名召唤物合并口径 | 分工计划 §4.4.15 (Lead 已定)  |
| 状态两个类型共用 id 区域 | D51; 规则书"游戏区域 II" |
| 状态公共列表 / 布尔存在即真 / 身份指针 | D95 / D96 / D104 / D105 / D108 / D110 / D200 |
| 秘传揭令每人一份 | 规则书"特殊词条 秘传"+ 秘传揭令; D264; 留空 #68 / #98 / #99 |
| 饱腹 / 冻结 / 眩晕 / 石化 | S8.1 / S6.6 / 眩晕 / 特技 / S8.4; 留空 #31 / #79 / #80 / #82 |
| 敌人牌 | 数据 `tcgenemycards`; 运行期承载未设计 (待接入)  |

### 5.1 文档与代码不一致 (供复核) 

> [!NOTE]
> 本轮复核发现以下偏差; 以当前代码 + 规则书为准.

1. `决策日志.md` **D13** 写"duration 在回合开始递减", 规则书原文是"每个结束阶段 -1"; `rules/ops/lifetime.rs` 已按规则书实现并在注释声明覆盖 D13 —— 建议决策日志补一条修正记录.
2. `结构.md` 把状态写成 `families/status/` 自带区; 实际状态承载在角色族 (`CharacterState.states` / `CombatStatusZone`) , `families/status/` 只有定义与身份.
3. 眩晕 / 石化 / 饱腹的正式状态定义尚未进 `families/status/known.rs`, 当前由角色牌族给共享身份; 与"状态牌族管状态"的文档口径不完全一致, 属已知技术债 (留空 #79) .

## 6. 常见误用

> [!WARNING]
> 以下写法在类型层面大多不报错, 但会直接改变结算结果.

1. **把"可用次数 -1"写在行动阶段开始**: 参考实现的老 bug (D12 点名) , 正确是按触发次数; duration 才按结束阶段.
2. **同名召唤物新建占位**: 应合并 (次数 / 效果量取最大, 类型最新, 位置不变) .
3. **给状态建编号 / 分区表 / 第二套列表**: 违反 D42 / D105.
4. **用 ZST 自身地址身份**: 不同 ZST 静态实例地址可能相同 → 状态合并 / 反应命中失效 (D200) ; 一律用 `identity_address()`.
5. **把秘传揭令当状态卡**: 没有卡 id, 没有 `StatusDef`; 判定用 `ArcaneEdict::can_play`.
6. **给"饱腹 / 眩晕 / 石化"再建一套身份**: 当前是共享身份, 正式定义归状态块且不许两套并存 (留空 #79) .
7. **把状态类型当第三类"角色装备"**: 数据里有但决策不建 (D201) .
8. **敌人牌复制角色区**: 敌人运行期承载尚未设计.

## 7. 测试

| 文件 | 代表测试 |
|---|---|
| `summon/def.rs` | `summon_def_carries_id_text_kind_points_uses_and_tags`, `entry_carries_the_def_measurements_into_the_zone_entry` |
| `summon/zones/mod.rs` | `cap_is_four_and_fifth_is_rejected_without_state_change`, `same_card_merges_uses_points_and_keeps_kind_newest`, `merge_keeps_position_and_works_when_full`, `tick_decrements_uses_and_discards_at_zero` |
| `enemy/def.rs` | `enemy_def_carries_id_name_description_hp_tags_and_skills`, `enemy_def_reuses_skill_defs_in_data_order` |
| `status/def.rs` | `status_def_carries_id_type_text_layers_duration_and_tags`, `form_is_derived_from_default_layers_only` |
| `status/known.rs` | `known_status_defs_use_the_data_ids_and_types`, `identities_are_distinct_static_instances_with_one_text_copy` |
| `status/arcane_edict.rs` | `fresh_container_allows_arcane_and_records_play`, `consumed_edict_blocks_arcane_with_no_edict` |
| `families/character/damage_chain/statuses.rs` | `keyword_and_card_statuses_are_registered_by_their_data_ids` (100001 / 116033 / 303300 桥登记)  |
| `families/character/satiated.rs` | `satiated_identity_comes_from_the_status_data`, `food_effect_grants_satiated_once_and_blocks_food_targeting` |

## 8. 待接入与待实测

- **敌人牌运行期承载 / 充能上限**: 代码标"待接入"; 留空.md 尚无专条 (建议补登) .
- **饱腹 / 眩晕 / 石化的正式 `StatusDef`**: 当前为共享身份, #79 要求二选一 (不许两套并存) ; 描述与规则书措辞差异 #80; 石化两份数据共用身份 #82.
- **具体护罩状态身份** (冰/雷/水/火元素护罩, 涌流/轰霆/涌火, 弱点暴露条件免疫) : #31.
- **状态第三类"角色装备"**: 不建, 待复核 #23.
- **手牌状态身份与描述**: #10 / #106 / #115.
- **秘传揭令**: 容器持有方 / 判定与打牌条件先后 / 是否显示为状态: #68 / #98 / #99.
- **召唤物 / 手牌状态的叠加上限** (`stack_cap` / `stack_limit`) : #70 / #71 / #72.
- **元素幻变**: #108 / #118.
- **持续回合与 D13**: 代码按规则书 (结束阶段) , D13 旧口径已覆盖 —— 建议决策日志补修正记录.

---
最后核对: 2026-10-19 (代码: `crates/definitions/src/rules/ops/lifetime.rs::duration_after_end_phase`) 
