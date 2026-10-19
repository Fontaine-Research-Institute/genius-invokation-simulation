# Card Research: Furina (Talent → Effect → Normal Attack → Double Lose)

> [[卡牌定义-芙宁娜|中文]] · English

*Scope: Furina (Arkhe: Ousia form / Arkhe: Pneuma form), the talent "Hear Me — Let Us Raise the Chalice of Love!" and the status Center of Attention — **card text + implementation code + playtest record**. Furina as a whole is **not implemented**; the "planned implementation" in §2 is a design draft.*

---

## 1. Card text

### 1.1 "Hear Me — Let Us Raise the Chalice of Love!"

Talent
3 Hydro Dice

> Combat Action: When your active character is Furina, equip this card.
> After Furina equips this card, immediately use Salon Solitaire once.
> When Furina, who has this card equipped, uses Salon Solitaire, she will apply Center of Attention to herself. (Normal Attack will trigger different effects depending on state)
> (You must have Furina in your deck to add this card to your deck.)


Implementation: register a function for "after the equipped character uses Salon Solitaire"; the function body is one step (attach status).

### 1.2 "Center of Attention"

Character Status

> When the character uses a Normal Attack: Physical DMG dealt by the character is converted to Hydro DMG.If the character is in the Ousia state, heal all of your characters on standby for 1 HP. If the character is in the Pneuma state, +2 DMG for this instance, but deal 1 Piercing DMG to your character who has taken the least DMG.
> Usage(s): 1

### 1.3 Furina's two form cards

#### Arkhe: Ousia form

| Item | Content |
|---|---|
| HP cap | 12 |
| Energy cap | 2 |
| Tags | Hydro / Sword / Fontaine / Arkhe: Ousia |
| Transformation | can transform into the Arkhe: Pneuma form |

**Soloist's Solicitation**
Normal Attack
1 Hydro Die + 2 Unaligned Dice

> Deals 2 Physical DMG.
> Once per Round: If you do not have Seats Sacred and Secular in your Hand, create 1 Seats Sacred and Secular card in your Hand.


**Salon Solitaire**
Elemental Skill
3 Hydro Dice

> Furina is currently in the Arkhe: Ousia state. Summons Salon Members.
> (When Furina is in the Arkhe: Pneuma state, she will summon Singer of Many Waters instead)


**Let the People Rejoice**
Elemental Burst
4 Hydro Dice
2 Energy

> Deals 2 Hydro DMG, creates 1 Universal Revelry.


**Arkhe: Seats Sacred and Secular**
Passive Skill
> (Passive) When battle begins, create 1 Seats Sacred and Secular card in your Hand.

#### Arkhe: Pneuma form

| Item | Content |
|---|---|
| HP cap | 12 |
| Energy cap | 2 |
| Tags | Hydro / Sword / Fontaine / Arkhe: Pneuma |
| Transformation | can transform into the Arkhe: Ousia form |

**Soloist's Solicitation**
Normal Attack
1 Hydro Die + 2 Unaligned Dice

> Deals 2 Physical DMG.
> Once per Round: Create 1 Seats Sacred and Secular card in your Hand.


**Salon Solitaire**
Elemental Skill
3 Hydro Dice

> Furina is currently in the Arkhe: Pneuma state. Summons Singer of Many Waters.
> (When Furina is in the Arkhe: Ousia state, she will summon Salon Members instead)


**Let the People Rejoice**
Elemental Burst
4 Hydro Dice
2 Energy

> Deals 2 Hydro DMG, creates 1 Universal Revelry.


**Arkhe: Seats Sacred and Secular**
Passive Skill
> (Passive) When battle begins, create 1 Seats Sacred and Secular card in your Hand.

### 1.4 Related event / summons / combat status

| Object | Text |
|---|---|
| Event "Seats Sacred and Secular" | When in Arkhe: Ousia or Arkhe: Pneuma states, switch Furina's state. If you have Salon Members or Singer of Many Waters on the field, change their state as well. |
| Summon "Salon Members" | End Phase: Deal 1 Hydro DMG. If your side has characters with at least 6 HP, deal 1 Piercing DMG to your character who has taken the least DMG, and deal another 1 Hydro DMG. Usage(s): 2 (Can stack. Max 4 stacks) |
| Summon "Singer of Many Waters" | End Phase: Heal all your characters for 1 HP. If your side has characters with no more than 5 HP, heal the most injured character for an additional 1 HP. Usage(s): 2 (Can stack. Max 4 stacks) |
| Combat status "Universal Revelry" | After your active character takes DMG or is healed: Add 1 stack of Revelry. Duration (Rounds): 2 |

### 1.5 Keyword notes

| Keyword | Description |
|---|---|
| Combat Action | After you perform a combat action, the opponent acts next. Playing a card with this rule is a combat action, not a fast action. |
| Usage(s) | After this card's effect triggers, consume 1 usage; when usage is depleted, discard this card immediately. |
| Duration (Rounds) | Each End Phase, duration -1; when depleted, discard this card immediately. |
| Piercing DMG | Piercing DMG ignores shields / damage reduction / immunity. |
| Physical DMG | Physical DMG does not attach elements and does not trigger elemental reactions. |
| Hydro DMG | Attaches Hydro and may trigger reactions (Vaporize / Electro-Charged / Frozen / Bloom, etc.). |
| Revelry | Your DMG +1 (including Swirl DMG caused by characters). Usage 1 (stackable, no limit). |
| Center of Attention | When the character uses a Normal Attack: convert Physical DMG to Hydro DMG … Usage(s): 1. (Full text in §1.2) |
| Salon Members | End Phase: deal 1 Hydro DMG … Usage(s): 2 (can stack, max 4). |
| Singer of Many Waters | End Phase: heal all your characters for 1 HP … Usage(s): 2 (can stack, max 4). |

### 1.6 Related rulebook entries

| Entry | Excerpt |
|---|---|
| S1.4 Victory | Defeat all of the opponent's character cards to win. |
| S7.3 Defeat | When a character's HP is reduced to 0, the character is defeated after the skill or effect finishes settling. |
| S7.4 Avoid Defeat | At that point, the character is not considered defeated. |
| Special entry Piercing DMG | Piercing DMG ignores shields / damage reduction / immunity. |
| Special entry Usage(s) | Consume 1 per trigger; discard immediately when depleted. |
| Special entry Duration (Rounds) | -1 each End Phase; discard when depleted. |
| Elemental Reactions · Rules | Elemental DMG attaches first, then the reaction is checked, then the reaction effect settles. |

## 2. Implementation code

> [!WARNING]
> **Furina as a whole is not implemented**: `crates/cards/` currently contains only the Aino sample (`cards/character/aino`), whose effect bodies are still `todo!`. §2.1 is **real source from the current repository** (the primitives this chain needs); §2.2 is the **planned complete implementation (design draft, does not compile, not implemented)**; §2.3 lists what is missing.

### 2.1 Available primitives (real source)

> Source blocks are copied from the repository; comment lines that cited keyword numbers had the numbers removed (user rule: no K codes).

#### 2.1.1 Damage type conversion (Physical → Hydro)

**`DamageChainEnv`** — `crates/definitions/src/families/character/damage_chain/chain.rs`

*Keyword codes in comments were replaced with names; code lines unchanged*
```rust
/// 修正链从「区」读数据的**只读依赖**（D183：只借不拥有、不拷贝、不重造承载）。
///
/// 生命周期都是 `&'a`：区由状态层的两个具名实例（我方 / 敌方）持有。
#[derive(Debug)]
pub struct DamageChainEnv<'a> {
    /// 出战状态区（只读指针）：护盾 / 减伤的**出战状态**承载。
    pub combat_status: &'a CombatStatusZone,
    /// 目标角色的状态列表（只读指针）：护盾 / 减伤的**角色状态**承载。
    pub character_states: &'a StateList,
    /// 算作**护盾**的状态身份集合（默认 [`statuses::SHIELD_STATUSES`]）。
    pub shields: &'a [&'static dyn StateIdentity],
    /// 算作**减伤**的状态身份集合（默认 [`statuses::REDUCTION_STATUSES`]）。
    pub reductions: &'a [&'static dyn StateIdentity],
    /// 算作**伤害免疫**的状态身份集合（默认 [`statuses::IMMUNITY_STATUSES`]）。
    ///
    /// 命中 = 本次伤害归 0（不结算）；**穿透伤害不吃免疫**（穿透伤害）。
    pub immunities: &'a [&'static dyn StateIdentity],
    /// 乘算 / 除算 / 区外加减（分工计划 §4.4.5；默认 [`DamageModifiers::NONE`]）。
    pub modifiers: DamageModifiers<'a>,
    /// **伤害类型覆盖**（卡面「把造成的物理伤害变为水元素伤害」类效果；默认 None = 不覆盖）。
    ///
    /// 作用点 = **进入计算前**：覆盖后的类型用于穿透 / 免疫 / 乘算等全部数值判定；
    /// 是否覆盖后**再判反应**由调用方决定（[crate::families::character::damage::damage_with_chain]
    /// 会用同一个 effective type 做反应预测与判定，所以覆盖后元素伤害正常引发反应）。
    pub type_override: Option<DamageType>,
}
```

**`with_type_override` / `effective_type`** — same file

```rust
    /// **覆盖本次伤害的类型**（默认 None = 不覆盖；None 时与改前逐字节等价）。
    ///
    /// 依据：天赋 / 状态卡面「使角色造成的物理伤害变为水元素伤害」类效果；规则书没有
    /// 「伤害类型转换」词条 —— 口径 = 覆盖后完全按新类型结算（含反应判定、穿透 / 免疫）；
    /// 覆盖发生在**进入链之前**，所以记录里能看到原始类型与新类型两条。
    pub fn with_type_override(mut self, ty: Option<DamageType>) -> Self {
        self.type_override = ty;
        self
    }
```

```rust
    /// 本次伤害**实际生效**的类型（有覆盖用覆盖值，否则用原始值）。
    pub fn effective_type(&self, original: DamageType) -> DamageType {
        self.type_override.unwrap_or(original)
    }
```

#### 2.1.2 Damage bonus (+2)

**`MulMode` / `Rounding` / `AmountOp` / `DamageModifiers`** — `crates/definitions/src/families/character/damage_chain/bonus.rs`

```rust
/// 乘法怎么算（分工计划 §4.4.5）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MulMode {
    /// 独立乘法：单独乘一次。
    Standalone,
    /// 加入统一倍数区：区内乘数**相加**后统一乘一次。
    Zone,
}
```

```rust
/// 取整方向（分工计划 §4.4.5：具体值**暂不决定**，由调用方给；默认 `Down`）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    /// 向上取整（ceil）。
    Up,
    /// 向下取整（floor；整数除法的自然结果）。
    Down,
}
```

```rust
/// 一条**数值修正**（分工计划 §4.4.5 的 `AmountOp`）：只允许「运算 + 数值」，
/// 不允许把代码（闭包 / trait 对象 / 回调）注入结算（分工计划 §4.4.5 的硬规矩）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AmountOp {
    /// 加：可声明是否计入乘算区（`true` = 乘除之前，会被乘数放大）。
    Add {
        /// 加多少。
        value: Nat,
        /// 是否计入乘算区（`true` = 第 1 段；`false` = 第 4 段）。
        in_mul_zone: bool,
    },
    /// 乘：必须声明是独立乘法还是进统一倍数区。
    Mul {
        /// 乘数（`2` = 翻倍）。
        value: Nat,
        /// 独立乘法 / 倍数区。
        mode: MulMode,
    },
    /// 除：所有除数相乘成一个分母后一起除。
    Div {
        /// 除数（**不能为 0**，否则结算时 panic，D44）。
        value: Nat,
    },
    /// 减（伤害减免 / 伤害降低）：同样可声明是否计入乘算区；用 `Nat` 饱和减法，不会为负。
    Reduce {
        /// 减多少。
        value: Nat,
        /// 是否计入乘算区（`true` = 第 1 段；`false` = 第 4 段）。
        in_mul_zone: bool,
    },
}
```

```rust
/// 一次伤害的**乘算 / 除算输入**（第 2 / 3 / 4 段的修正列表 + 取整方向）。
///
/// 与 [`DamageBonus`] 分开：`DamageBonus` 是「加伤」的既有入口（签名不改），本类型只加
/// 「乘 / 除 / 区外加减」；调用方按需用 [`DamageChainEnv::with_modifiers`] 挂进伤修链。
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DamageModifiers<'a> {
    /// 数值修正列表（顺序**不影响**结果：同类运算可交换；只用于观测与将来的保序扩展）。
    pub ops: &'a [AmountOp],
    /// 取整方向（`None` -> `Down`）。
    pub rounding: Option<Rounding>,
}
```

#### 2.1.3 The least-injured target

**`TargetSelector`** — `crates/definitions/src/keywords/target.rs` (whole enum, including `LostHpRank`; enum-level docs omitted, full source in file)

*Keyword codes in comments were replaced with names; code lines unchanged*
```rust
pub enum TargetSelector {
    /// **自己**（效果主体自身；D205 / D206）。实体号从执行上下文取，不手写。
    Subject,
    /// **反应承载角色**（反应效果关键字用；上下文没给 -> 空集）。
    Reacted,
    /// 出战角色（每方最多一个；Both = 双方各一个）。
    Active(SideScope),
    /// **所有角色**（出战 + 后台，按角色区顺序）。
    Characters(SideScope),
    /// **所有后台角色**（出战角色以外的角色）。
    Standby(SideScope),
    /// **距离最近**：从 reference 方的出战角色出发，选 candidates 方**位置最接近**的角色
    /// （权威关键字 距离最近 / L12）。
    ///
    /// 依据：权威关键字 距离最近「距离我方出战角色最近的角色」——「对方场上距离我方出战角色
    /// 『最近』的角色，指的是位置与我方出战角色最接近的敌方角色。如果存在多个位置最接近的
    /// 角色，则其中靠前的角色被视为『最近』。」（tcgkeywords 4lang；英文 foremost /
    /// 日文 一番左側）。规则书正文**没有**这个词条（L12 = C 档），所以：
    /// - 距离口径 = 出战角色在**本队槽位里的下标**与候选角色槽位下标之**绝对差**；
    /// - 并列 = 槽位更靠前（下标更小 / 左侧）的候选优先；
    /// - **被击倒的角色不参与**（「对方场上」；当前按 hp == 0 判定，与 HpRank 同口径）。
    ///
    /// **待实测 / TODO**：规则书没写这个词条；距离是否按镜像位置（我方下标 i <-> 对方
    /// len-1-i）计算、被击倒角色是否仍占位导致并列，均无权威依据 —— 见 留空 记录。
    NearestToActive {
        /// 参照方：用这一方的出战角色位置作参照（距离最近 = Own）。
        reference: SideScope,
        /// 候选方：从这一方的角色里选最近的（距离最近 = Opponent）。
        candidates: SideScope,
    },
    /// 出战角色**之后第 n 个**角色（卡面「(下)(n)个角色」）。
    ///
    /// 在角色区槽位顺序上**循环**偏移；offset = 0 等价于出战角色本身。**待实测**
    /// （被击倒的角色是否跳过、是否循环，规则书未写清）。
    AfterActive {
        /// 哪一方的角色。
        side: SideScope,
        /// 偏移量（1 = 出战角色之后第 1 个）。
        offset: Nat,
    },
    /// 出战角色**之前第 n 个**角色（卡面「(上)(n)个角色」）。**待实测**（同 [Self::AfterActive]）。
    BeforeActive {
        /// 哪一方的角色。
        side: SideScope,
        /// 偏移量。
        offset: Nat,
    },
    /// **按受伤量排名**取第 n 个角色（D237）。
    ///
    /// 受伤量 = **实际最大生命值 − 当前生命值**：
    /// - **只能用实际最大生命值**（运行期 CharacterState::max_hp；成长 / 装备通过
    ///   growth::GrowthList::apply_to_character 写进它，运行期它就是"实际值"。
    ///   DefState 不持有每角色的 GrowthList，所以选择器层读到的唯一实际值就是
    ///   CharacterState::max_hp；growth.rs 保证它 == GrowthList::actual_max_hp(定义值)）；
    /// - **禁止**用卡牌定义里的固定最大生命值（CharacterDef::max_hp）——那是定义数据，
    ///   不随成长 / 装备变化，拿来算会得到错误结果；
    /// - **被击倒的角色不参与排名**（当前按 hp == 0 判定，见 is_defeated 的待实测说明）。
    ///
    /// 比较顺序同 [TargetSelector::HpRank] 的**固定三级**（D230）：
    /// 1. 受伤量（[LostHpOrder]）；
    /// 2. 是否出战（[HpTie::active_first]）；
    /// 3. 槽位方向（[HpTie::direction]）。
    ///
    /// n 从 1 开始；越界 / 0 -> 空集合。
    LostHpRank {
        /// 哪一方。
        side: SideScope,
        /// 受伤量排序方向（最多 / 最少开始）。
        order: LostHpOrder,
        /// 第几个（1 起）。
        n: Nat,
        /// 并列时的定序规则。
        tie: HpTie,
    },
    /// **按当前生命值排名**取第 n 个角色（D228：不是只取极值；极值 = n 1）。
    ///
    /// 排序口径（D228 / D229 / D230）——**固定三级**：
    /// 1. 当前生命值（order：降序 / 升序）；
    /// 2. 是否出战角色（tie.active_first 为真时出战优先）；
    /// 3. 槽位方向（tie.direction：从左到右 / 从右到左）。
    ///
    /// n 从 1 开始；越界 -> 空集合。**待实测**（规则书没写并列怎么取）。
    HpRank {
        /// 哪一方。
        side: SideScope,
        /// 排序方向（最高开始 / 最低开始）。
        order: HpOrder,
        /// 第几个（1 起）。
        n: Nat,
        /// 并列时的定序规则。
        tie: HpTie,
    },
    /// 某方的所有角色，**去掉反应承载角色**（卡面「目标以外的所有敌方角色」）。
    AllExceptReacted(SideScope),
    /// 手牌（候选集合 = 那一方当前手牌；随机一张牌由上层展开分支）。
    HandCards(SideScope),
    /// 手牌中**第 index 张**（顺序 = 手牌容器顺序；0 起）。
    ///
    /// **待复核**：手牌容器声明「无序」，这里只按容器顺序取下标，用于把
    /// reactions::effect::EffectOp::HandCardStatus 的 index 迁到关键字层。
    HandCardIndex {
        /// 哪一方的手牌。
        side: SideScope,
        /// 下标（0 起）。
        index: Nat,
    },
    /// 召唤物（候选集合 = 那一方当前召唤物）。
    Summons(SideScope),
    /// 支援牌（候选集合 = 那一方当前支援牌；弃置支援区自己 = Subject 落在支援区）。
    Supports(SideScope),
    /// 装备（候选集合 = 那一方所有角色装备栏里的装备）。
    Equipments(SideScope),
    /// **牌组顶的卡牌**（牌库第 0 位；空牌库 -> 空集）。
    PileTop(SideScope),
    /// 牌组里的全部卡牌（候选集合 = 当前牌库内容，按牌库位置顺序）。
    PileCards(SideScope),
}
```

**`LostHpOrder`** — same file

```rust
/// **受伤量排序方向**（[TargetSelector::LostHpRank] 用；D237）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LostHpOrder {
    /// 受伤**最多**开始（受伤量降序；第 1 个 = 受伤最多）。
    Most,
    /// 受伤**最少**开始（受伤量升序；第 1 个 = 受伤最少）。
    Least,
}
```

#### 2.1.4 Status identity and layers (usage)

**`StateIdentity`** — `crates/definitions/src/families/action_card/status/state.rs`

```rust
/// 状态身份（D95/D96/D110）：具体状态模块定义自己的零大小类型 + 一份静态实例；
/// 运行期身份 = 指向那份实例的指针；静态定义数据 = 它给出的 StateText（名字 + 描述）。
///
/// 谁定义状态谁给类型；使用处 import 那个模块、拿到静态实例后交给 StateList 的操作。
/// 本层**只解释身份 + 形态**，不解释减费 / 赋能等具体语义（D108）。
pub trait StateIdentity: 'static + std::fmt::Debug {
    /// 状态的静态定义数据（4 语名字 + 描述；只有一份，不拷贝）。
    fn text(&self) -> &'static StateText;

    /// 这个状态是布尔值还是必须带层数。
    fn form(&self) -> StateForm;

    /// **身份地址** = 指向该身份**定义数据**的指针（D57 / D58 口径）。
    ///
    /// 为什么不用 `&dyn StateIdentity` 的地址：零大小（ZST）静态实例**没有独立地址**
    /// （本仓库实测 rustc 1.98.1 下不同 ZST 静态实例地址相同），直接比 `&dyn` 会把
    /// 不同身份判成同一身份。所以身份一律取「它指向的那份数据」的地址。
    ///
    /// 默认实现 = [`StateIdentity::text`] 返回的那份 `StateText` 的地址：本仓库每个身份
    /// 都有唯一一份文本数据；将来有独立定义数据（如 `&'static StatusDef`）的实现可以覆盖，
    /// 返回定义本身的地址（同样唯一）。
    fn identity_address(&self) -> *const  {
        self.text as *const StateText as *const 
    }

    /// 状态的可读名字（调试 / 日志用；**不参与身份判断**）。
    ///
    /// 默认从 StateText 取中文原文；状态模块想换语言自己覆盖。
    fn name(&self) -> &'static str {
        self.text
            .name
            .get(crate::generated::Lang::ZhCn)
            .text
            .unwrap_or("")
    }
}
```

**`StateText` / `StateForm` / `StateList` / `StateEntry` / `layers`** — same file

```rust
/// 状态的静态定义数据（D110）：**与 TagText 同构** —— 4 语名字 + 4 语描述。
///
/// 状态「相当于 tag」：定义模块给一份静态 StateText，状态身份 = 指向它所属静态实例的指针；
/// 列表 / 手牌里只存身份指针 + pos + layers，**不拷贝名字描述**（只有一份，D64 精神）。
#[derive(Clone, Copy, Debug)]
pub struct StateText {
    /// 4 语名字。
    pub name: Localized,
    /// 4 语描述。
    pub description: Localized,
}
```

```rust
/// 状态的形态（D104）：必须带层数 / 布尔值。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StateForm {
    /// 布尔值：只有「有 / 没有」，不记录层数。
    Boolean,
    /// 必须带层数：每条状态有层数，同状态同位置合并时相加。
    Layered,
}
```

```rust
/// **公共状态列表**（D105）：一个列表装所有状态，entries 可跨位置。
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct StateList {
    entries: Vec<StateEntry>,
}
```

```rust
/// 公共状态列表里的一条：**列表里有它 = 这个位置有这个状态**。
///
/// - layers = None：布尔类（不额外存 bool）；
/// - layers = Some(n)：层数类。
#[derive(Clone, Copy)]
pub struct StateEntry {
    /// 状态身份（类型化静态实例指针，D95/D96）。
    pub state: &'static dyn StateIdentity,
    /// 位置（上层解释：牌库里 = 从顶往下数的序号）。
    pub pos: Nat,
    /// 层数（布尔类为 None；层数类为 Some）。
    pub layers: Option<Nat>,
}
```

```rust
    /// 某个位置这个状态的层数：不存在 -> None；存在 -> Some(layers)（布尔类为 Some(None)）。
    pub fn layers(&self, pos: Nat, state: &'static dyn StateIdentity) -> Option<Option<Nat>> {
        self.entries
            .iter
            .find(|entry| entry.pos == pos && same_state_identity(entry.state, state))
            .map(|entry| entry.layers)
    }
```

#### 2.1.5 Step composition

**`Step` / `execute_step_me`** — `crates/definitions/src/keywords/step.rs`

```rust
/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}
```

```rust
/// **执行一步**（我方视角）。
///
/// 只调用关键字函数：**不做任何 insert / execute_pending**（D204）；返回的待执行由调用方
/// 在步边界执行。
pub fn execute_step_me(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc).with_viewer(Perspective::Me);
    run_step(state, step, &scoped)
}
```

#### 2.1.6 Damage arguments

**`DamageArgs` / `with_modifiers`** — `crates/definitions/src/keywords/op.rs`

```rust
/// **造成伤害**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DamageArgs {
    /// 目标选择器（敌方出战角色 / 所有敌方后台角色 / 自己…）。
    pub targets: TargetSelector,
    /// 基础伤害数值。
    pub amount: Nat,
    /// 伤害类型（七元素 / 物理 / 穿透；枚举本身就是「元素 + 物理 + 穿透」）。
    pub ty: DamageType,
    /// 伤害来源类别（普通攻击 / 元素战技 / 元素爆发 / 状态 / 召唤物 / 反应…）。
    pub source_kind: DamageSourceKind,
    /// 目标选取策略。
    pub pick: TargetPick,
    /// **乘算 / 除算修饰**（分工计划 §4.4.5 的 `AmountOp` 列表 + 取整方向）。
    ///
    /// - 默认 [`DamageModifiers::NONE`]（不乘不算）；
    /// - 卡面「此次伤害翻倍 / +N / 减半」这类**本段伤害自己的**数值修正写在这里
    ///   （状态按层数给的倍率由效果方换算成 `AmountOp` 再放进来）；
    /// - 穿透伤害**整段跳过**（规则书·特殊词条 穿透伤害）。
    pub modifiers: DamageModifiers<'static>,
}
```

```rust
    /// 补上乘算 / 除算修饰（链式）。
    pub const fn with_modifiers(mut self, modifiers: DamageModifiers<'static>) -> DamageArgs {
        self.modifiers = modifiers;
        self
    }
```

#### 2.1.7 Terminal verdict (double lose)

**`Outcome`** — `crates/definitions/src/state/terminal.rs`

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

#### 2.1.8 Summon merging (the carrier of form sync)

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
        let mut zone = self.clone;
        let mut merged: Option<(EntityId, Option<Nat>)> = None;
    // ...（其余实现从略；见 crates/definitions/src/families/summon/zones/mod.rs）
```

### 2.2 Planned implementation (complete code, not implemented)

> [!WARNING]
> All 11 blocks below are **not implemented and do not compile**; type and function names follow real definitions where possible, and section numbers inside comments refer to the old draft. They are the complete design draft of "how we would write it now".

> Id constants in the code (`FURINA_OUSIA_CARD_ID` etc.) are filled from the card data table; this page writes no concrete numbers (user rule).

#### 2.2.1 Character and tag definitions

```rust
// 设计稿：crates/cards/src/cards/character/furina/mod.rs（未创建）
use gi_genius_invokation_definitions::base::{CardId, Element, SkillId};
use gi_genius_invokation_definitions::families::character::definition::CharacterDef;
use gi_genius_invokation_definitions::families::character::skill::{SkillCost, SkillDef};
use gi_genius_invokation_definitions::families::character::types::SkillKind;
use gi_genius_invokation_definitions::generated::Localized;
use gi_genius_invokation_definitions::resources::dice::CostComponent;
use gi_genius_invokation_definitions::tags::action_card::equipment::sword::Sword;
use gi_genius_invokation_definitions::tags::character::arkhe::ousia::Ousia;
use gi_genius_invokation_definitions::tags::character::arkhe::pneuma::Pneuma;
use gi_genius_invokation_definitions::tags::character::element::hydro::Hydro;
use gi_genius_invokation_definitions::tags::character::nation::fontaine::Fontaine;
use gi_genius_invokation_engine::Nat;

pub const ID_OUSIA: CardId = CardId::new(FURINA_OUSIA_CARD_ID);   // 荒性
pub const ID_PNEUMA: CardId = CardId::new(FURINA_PNEUMA_CARD_ID);  // 芒性（istransformation）
pub const SKILL_ID_NORMAL: SkillId = SkillId::new(FURINA_NORMAL_ATTACK_SKILL_ID);
pub const SKILL_ID_SKILL: SkillId = SkillId::new(FURINA_ELEMENTAL_SKILL_ID);
pub const SKILL_ID_BURST: SkillId = SkillId::new(FURINA_ELEMENTAL_BURST_SKILL_ID);
pub const SKILL_ID_PASSIVE: SkillId = SkillId::new(FURINA_PASSIVE_SKILL_ID);

const fn zh(text: &'static str) -> Localized {
    Localized::from_parts(Some(text), None, None, None)
}

pub type TagsOusia = (Hydro, Sword, Fontaine, Ousia);
pub const TAGS_OUSIA: TagsOusia = (Hydro, Sword, Fontaine, Ousia);
pub type TagsPneuma = (Hydro, Sword, Fontaine, Pneuma);
pub const TAGS_PNEUMA: TagsPneuma = (Hydro, Sword, Fontaine, Pneuma);

pub static FURINA_OUSIA: CharacterDef<TagsOusia> = CharacterDef {
    id: ID_OUSIA,
    name: zh("芙宁娜"),
    tags: TAGS_OUSIA,
    max_hp: Nat::new(12),
    max_energy: Nat::new(2),
    skills: &[&SKILL_NORMAL_ATTACK, &SKILL_ELEMENTAL_SKILL, &SKILL_ELEMENTAL_BURST, &SKILL_PASSIVE],
};

pub static FURINA_PNEUMA: CharacterDef<TagsPneuma> = CharacterDef {
    id: ID_PNEUMA,
    name: zh("芙宁娜"),
    tags: TAGS_PNEUMA,
    max_hp: Nat::new(12),
    max_energy: Nat::new(2),
    skills: &[&SKILL_PNEUMA_NORMAL_ATTACK, &SKILL_PNEUMA_ELEMENTAL_SKILL, &SKILL_ELEMENTAL_BURST, &SKILL_PASSIVE],
};
```

#### 2.2.2 The four skills (effect bodies are placeholders)

```rust
// 设计稿：技能定义（SkillEffect 目前是 fn，D162；效果体只能 todo!）
fn furina_normal_attack_effect {
    // 造成 2 点物理伤害；每回合 1 次：生成手牌「圣俗杂座」。
    // 缺：SkillEffect 上下文（见 §4）；缺：每回合 1 次计数（SkillUses 只按技能 id）。
    todo!("缺：SkillEffect 带上下文 + 每回合 1 次计数")
}

fn furina_elemental_skill_effect {
    // 按当前形态召唤「沙龙成员」/「众水的歌者」。
    // 缺：读取当前形态（DefState 没有 FormState 字段）。
    todo!("缺：形态字段 + 召唤走 Step")
}

fn furina_elemental_burst_effect {
    // 造成 2 点水元素伤害，生成出战状态「普世欢腾」（持续 2）。
    todo!("缺：SkillEffect 带上下文")
}

fn furina_passive_effect {
    // 战斗开始时生成手牌「圣俗杂座」。
    todo!("缺：战斗开始时机 / 被动注册")
}

pub static SKILL_NORMAL_ATTACK: SkillDef = SkillDef {
    id: SKILL_ID_NORMAL,
    name: zh("独舞之邀"),
    description: zh("造成2点物理伤害。每回合1次：如果手牌中没有圣俗杂座，则生成手牌圣俗杂座。"),
    kind: SkillKind::normal_attack,
    cost: SkillCost::from_dice(&[
        CostComponent::SpecificElement(Element::Hydro, Nat::new(1)),
        CostComponent::AnyColor(Nat::new(2)),
    ]),
    effect: furina_normal_attack_effect,
};

pub static SKILL_ELEMENTAL_SKILL: SkillDef = SkillDef {
    id: SKILL_ID_SKILL,
    name: zh("孤心沙龙"),
    description: zh("芙宁娜当前处于「始基力：荒性」形态：召唤沙龙成员。"),
    kind: SkillKind::ElementalSkill,
    cost: SkillCost::from_dice(&[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))]),
    effect: furina_elemental_skill_effect,
};

pub static SKILL_ELEMENTAL_BURST: SkillDef = SkillDef {
    id: SKILL_ID_BURST,
    name: zh("万众狂欢"),
    description: zh("造成2点水元素伤害，生成普世欢腾。"),
    kind: SkillKind::ElementalBurst,
    cost: SkillCost::from_dice_and_energy(
        &[CostComponent::SpecificElement(Element::Hydro, Nat::new(4))],
        Nat::new(2),
    ),
    effect: furina_elemental_burst_effect,
};

pub static SKILL_PASSIVE: SkillDef = SkillDef {
    id: SKILL_ID_PASSIVE,
    name: zh("始基力：圣俗杂座"),
    description: zh("【被动】战斗开始时，生成手牌圣俗杂座。"),
    kind: SkillKind::Passive,
    cost: SkillCost::FREE,
    effect: furina_passive_effect,
};
```

#### 2.2.3 Form definitions

```rust
// 设计稿：form.rs 的 FormDef 需要 &'static [&'static TagText] 指针切片（D67）。
use gi_genius_invokation_definitions::families::character::form::{Form, FormDef, FormState};

pub static FORM_OUSIA_DEF: FormDef = FormDef {
    card_id: ID_OUSIA,
    name: zh("芙宁娜（荒性）"),
    element: Element::Hydro,
    // 缺：&'static [&'static TagText] 的静态切片 —— 标签身份是 &TAGS.<字段>，
    //     目前没有「把 (Hydro,Sword,Fontaine,Ousia) 变成指针切片」的现成接口。
    tags: &[],
    skills: &[&SKILL_NORMAL_ATTACK, &SKILL_ELEMENTAL_SKILL, &SKILL_ELEMENTAL_BURST, &SKILL_PASSIVE],
    is_transformation: false,
};

pub static FORM_PNEUMA_DEF: FormDef = FormDef {
    card_id: ID_PNEUMA,
    name: zh("芙宁娜（芒性）"),
    element: Element::Hydro,
    tags: &[],
    skills: &[&SKILL_PNEUMA_NORMAL_ATTACK, &SKILL_PNEUMA_ELEMENTAL_SKILL, &SKILL_ELEMENTAL_BURST, &SKILL_PASSIVE],
    is_transformation: true,
};

pub static FORM_OUSIA: Form = Form::new(&FORM_OUSIA_DEF);
pub static FORM_PNEUMA: Form = Form::new(&FORM_PNEUMA_DEF);

/// 设计稿：切形态本身是纯函数；难点是**这个 FormState 存在哪**（见 §4）。
pub fn toggle_form(current: FormState) -> FormState {
    if current.current.card_id.0.get == ID_OUSIA.0.get {
        current.transform_to(FORM_PNEUMA)
    } else {
        current.transform_to(FORM_OUSIA)
    }
}
```

#### 2.2.4 Event "Seats Sacred and Secular": switch form + summon form

```rust
// 设计稿：事件牌效果体。Step/KeywordOp 是真实类型；但「切形态」「替换召唤物卡 id」
// 目前都没有 KeywordOp 分支 -> 需要先加原语（见 §4）。
use gi_genius_invokation_definitions::base::CardId;
use gi_genius_invokation_definitions::families::summon::def::SummonEffectKind;
use gi_genius_invokation_definitions::keywords::op::{KeywordOp, SummonArgs};
use gi_genius_invokation_definitions::keywords::step::Step;
use gi_genius_invokation_definitions::keywords::target::SideScope;
use gi_genius_invokation_engine::Nat;

/// 圣俗杂座的「逻辑」：(1) 切换芙宁娜的形态；(2) 若场上有沙龙成员 / 众水的歌者，
/// 把它的卡 id 换成另一种形态的对应召唤物（「沙龙成员」<->「众水的歌者」），可用次数保持不变。
pub fn seats_sacred_and_secular_step -> Step {
    // 缺：KeywordOp::TransformForm { .. }（形态原语）；
    // 缺：KeywordOp::ReplaceSummonCard { from: CardId, to: CardId }（保留 uses / points）。
    let _ = Step::new(vec![
        // 占位：目前只能用「召唤 / 弃置」两步模拟，但会丢掉可用次数与位置。
        KeywordOp::Summon(SummonArgs {
            side: SideScope::Own,
            card: CardId::new(SALON_MEMBERS_CARD_ID),
            uses: Some(Nat::new(2)),
            points: Some(Nat::new(1)),
            effect_kind: Some(SummonEffectKind::Damage),
            stack_limit: Some(Nat::new(4)),
        }),
    ]);
    todo!("缺：切形态原语 + 替换召唤物卡 id 原语")
}
```

#### 2.2.5 Talent card: play condition + immediate Salon Solitaire

```rust
// 设计稿：天赋（装备 / 战斗行动 / 天赋）。
use gi_genius_invokation_definitions::families::action_card::def::{
    ActionCardDef, PlayCondition, ViewSide,
};
use gi_genius_invokation_definitions::families::action_card::ActionCardType;
use gi_genius_invokation_definitions::state::DefState;
use gi_genius_invokation_definitions::tags::action_card::common::combat_action::CombatAction;
use gi_genius_invokation_definitions::tags::action_card::equipment::talent::Talent;

pub const TALENT_ID: CardId = CardId::new(TALENT_CARD_ID);

/// 打出条件（D217：在「比较是否付得起」之前调用）。
fn talent_play_condition(state: &DefState, viewer: ViewSide) -> PlayCondition {
    let characters = match viewer {
        gi_genius_invokation_definitions::base::Perspective::Me => state.me.characters,
        gi_genius_invokation_definitions::base::Perspective::Opp => state.opp.characters,
    };
    let Some(active) = characters.active_entity else {
        return PlayCondition::Rejected("没有出战角色");
    };
    // 缺：DefState / CharacterSlot 上没有「角色实体 -> 角色卡 id」的映射，
    //     所以无法判断 active 是不是芙宁娜（见 §4）。
    let _ = active;
    PlayCondition::Rejected("缺：角色实体 -> 角色卡 id 映射")
}

pub static TALENT: ActionCardDef<(Talent, CombatAction)> = ActionCardDef::new(
    TALENT_ID,
    ActionCardType::Equipment,
    zh("「诸君听我颂，共举爱之杯！」"),
    zh("战斗行动：我方出战角色为芙宁娜时，装备此牌。装备后立刻使用一次孤心沙龙。装备有此牌时使用孤心沙龙会对自身附属万众瞩目。"),
    (Talent, CombatAction),
    &[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))],
    talent_play_condition,
);
```

#### 2.2.5.1 Register the "after Salon Solitaire" hook (one step (attach the status))

```rust
// 设计稿（未实现）：装备注册「装备此牌的角色使用孤心沙龙后」的回调。
// 回调效果 = 一步(对角色自身赋予「万众瞩目」状态)。
fn talent_after_salon_solitaire() -> Step {
    Step::new(vec![KeywordOp::Status(StatusArgs {
        targets: TargetSelector::Subject,
        place: StatusPlace::Character,
        card: CENTER_OF_ATTENTION_CARD_ID,
        layers: Some(Nat::new(1)),
        duration: None,
        stack_cap: Some(Nat::new(1)),
    })])
}

// 装备注册：时机 = 使用孤心沙龙后；效果 = 上面的函数（返回一步）。
pub static TALENT_HOOK: EquipmentHook = EquipmentHook {
    timing: EquipmentTiming::AfterSkillUsed(SKILL_SALON_SOLITAIRE_ID),
    effect: talent_after_salon_solitaire,
};
```

#### 2.2.6 Center of Attention status identity

```rust
// 设计稿：状态身份（与 satiated.rs / damage_chain/statuses.rs 同款写法）。
use gi_genius_invokation_definitions::base::CardId;
use gi_genius_invokation_definitions::families::action_card::status::{
    StateForm, StateIdentity, StateText,
};

pub const CENTER_OF_ATTENTION_CARD: CardId = CardId::new(CENTER_OF_ATTENTION_CARD_ID);

static CENTER_OF_ATTENTION_TEXT: StateText = StateText {
    name: zh("万众瞩目"),
    description: zh("角色进行普通攻击时：使角色造成的物理伤害变为水元素伤害。如果角色处于「荒」形态，则治疗我方所有后台角色1点；如果角色处于「芒」形态，则此伤害+2，但是对一位受伤最少的我方角色造成1点穿透伤害。可用次数：1"),
};

#[derive(Debug)]
pub struct CenterOfAttention(&'static StateText);

impl StateIdentity for CenterOfAttention {
    fn text(&self) -> &'static StateText { self.0 }
    fn form(&self) -> StateForm {
        // 「可用次数：1」按层数承载（层数 = 可用次数），1 层用完即移除。
        StateForm::Layered
    }
}

pub static CENTER_OF_ATTENTION: CenterOfAttention = CenterOfAttention(&CENTER_OF_ATTENTION_TEXT);
```

#### 2.2.7 Normal Attack triggers Center of Attention (+2 / heal standby / piercing to the least injured)

```rust
// 设计稿：普通攻击触发（「角色进行普通攻击时」）。
// 现在 SkillEffect 是 fn、没有上下文，所以先在关键字层设计一个显式参数的纯函数。
use gi_genius_invokation_definitions::families::character::damage_chain::bonus::{
    AmountOp, DamageModifiers,
};
use gi_genius_invokation_definitions::families::character::types::DamageType;
use gi_genius_invokation_definitions::keywords::op::{
    DamageArgs, DamageSourceKind, HealArgs, KeywordOp,
};
use gi_genius_invokation_definitions::keywords::target::{
    HpTie, LostHpOrder, SideScope, TargetSelector,
};
use gi_genius_invokation_engine::Nat;

/// 本次普攻是否处于始基力：芒性形态。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArkheForm { Ousia, Pneuma }

/// 始基力：芒性形态的「本伤害 +2」：进乘算区（会被倍率放大；顺序见留空 #28）。
static ADD_TWO: [AmountOp; 1] = [AmountOp::add_before_mul(Nat::new(2))];

/// 「普通攻击时」的修正：返回按顺序执行的 KeywordOp 列表。
///
/// 参数都是显式的：form（缺源：DefState 没有形态字段）、has_center_of_attention（缺源：
/// 现在只能读 CharacterState.states，但没有「移除 1 层」的原语）。
pub fn normal_attack_with_center_of_attention(
    form: ArkheForm,
    has_center_of_attention: bool,
    base_damage: Nat,
) -> Vec<KeywordOp> {
    if !has_center_of_attention {
        return vec![KeywordOp::Damage(DamageArgs::new(
            TargetSelector::Active(SideScope::Opponent),
            base_damage,
            DamageType::Physical,
            DamageSourceKind::NormalAttack,
        ))];
    }
    // 1) 类型转换：物理 -> 水（走 DamageChainEnv::type_override；见 §4 的接线缺口）。
    // 2) 始基力：荒性形态：治疗我方所有后台角色 1 点。
    // 3) 始基力：芒性形态：本伤害 +2，并对一位受伤最少的我方角色造成 1 点穿透伤害。
    let mut ops = vec![KeywordOp::Damage(
        DamageArgs::new(
            TargetSelector::Active(SideScope::Opponent),
            base_damage,
            DamageType::elemental(Element::Hydro),
            DamageSourceKind::NormalAttack,
        )
        .with_modifiers(DamageModifiers { ops: &ADD_TWO, rounding: None }),
    )];
    match form {
        ArkheForm::Ousia => ops.push(KeywordOp::Heal(HealArgs::new(
            TargetSelector::Standby(SideScope::Own),
            Nat::new(1),
        ))),
        ArkheForm::Pneuma => ops.push(KeywordOp::Damage(DamageArgs::new(
            TargetSelector::LostHpRank {
                side: SideScope::Own,
                order: LostHpOrder::Least,
                n: Nat::new(1),
                tie: HpTie::DEFAULT,
            },
            Nat::new(1),
            DamageType::Piercing,
            DamageSourceKind::Other,
        ))),
    }
    // 缺：消耗万众瞩目 1 层可用次数（没有 KeywordOp::RemoveStatus / 层数递减原语）。
    ops
}
```

#### 2.2.8 Salon Solitaire summons by form

```rust
// 设计稿：战技效果（召唤物卡 id 按形态分派）。
use gi_genius_invokation_definitions::families::summon::def::SummonEffectKind;
use gi_genius_invokation_definitions::keywords::op::{KeywordOp, SummonArgs};
use gi_genius_invokation_definitions::keywords::target::SideScope;
use gi_genius_invokation_engine::Nat;

pub const SUMMON_SALON: CardId = CardId::new(SALON_MEMBERS_CARD_ID);  // 沙龙成员
pub const SUMMON_SINGER: CardId = CardId::new(SINGER_OF_MANY_WATERS_CARD_ID); // 众水的歌者

pub fn salon_step(form: ArkheForm) -> Vec<KeywordOp> {
    let (card, kind) = match form {
        ArkheForm::Ousia => (SUMMON_SALON, SummonEffectKind::Damage),
        ArkheForm::Pneuma => (SUMMON_SINGER, SummonEffectKind::Heal),
    };
    vec![KeywordOp::Summon(SummonArgs {
        side: SideScope::Own,
        card,
        uses: Some(Nat::new(2)),
        points: Some(Nat::new(1)),
        effect_kind: Some(kind),
        stack_limit: Some(Nat::new(4)),
    })]
}
```

#### 2.2.9 Summon definitions

```rust
// 设计稿：召唤物定义（SummonDef；结束阶段执行动作仍待接入）。
use gi_genius_invokation_definitions::families::summon::def::{SummonDef, SummonEffectKind};
use gi_genius_invokation_engine::Nat;

pub static SALON_MEMBERS: SummonDef<> = SummonDef::new(
    SUMMON_SALON,
    zh("沙龙成员"),
    zh("结束阶段：造成1点水元素伤害。如果我方存在生命值至少为6的角色，则对一位受伤最少的我方角色造成1点穿透伤害，然后再造成1点水元素伤害。可用次数：2（可叠加，最多叠加到4次）"),
    SummonEffectKind::Damage,
    Nat::new(1),
    Some(Nat::new(2)),
    ,
);

pub static SINGER_OF_MANY_WATERS: SummonDef<> = SummonDef::new(
    SUMMON_SINGER,
    zh("众水的歌者"),
    zh("结束阶段：治疗所有我方角色1点。如果我方存在生命值不多于5的角色，则再治疗一位受伤最多的角色1点。可用次数：2（可叠加，最多叠加到4次）"),
    SummonEffectKind::Heal,
    Nat::new(1),
    Some(Nat::new(2)),
    ,
);
```

#### 2.2.10 Future SkillContext (the signature proposed by #125)

```rust
// 设计稿：这是 §4 建议的 SkillEffect 新签名（当前 SkillDef::effect 是 fn，D162）。
use gi_genius_invokation_definitions::families::character::form::FormState;
use gi_genius_invokation_definitions::keywords::step::Step;
use gi_genius_invokation_definitions::state::DefState;

pub struct SkillContext<'a> {
    pub state: &'a DefState,
    pub who: gi_genius_invokation_engine::EntityId,
    pub form: FormState,
    pub step: Step,
}

pub fn use_skill_with_context(context: &SkillContext<'_>) {
    // 未来：skill_flow::use_skill 执行 SkillDef::effect 时把上下文传进来，
    // 卡牌在这里 push 自己的 KeywordOp / Step。
    let _ = context;
}
```

#### 2.2.11 Test examples

```rust
// 设计稿：卡牌测试将放在 crates/cards/src/cards/character/furina/mod.rs 的 tests 里，
// 复用 definitions 的公开入口（这里展示会用到的真实 API）。
use gi_genius_invokation_definitions::base::Element;
use gi_genius_invokation_definitions::families::action_card::status::StateList;
use gi_genius_invokation_definitions::families::character::damage_chain::chain::{
    DamageChainEnv, DamageInput, settle,
};
use gi_genius_invokation_definitions::families::character::damage_chain::bonus::DamageBonus;
use gi_genius_invokation_definitions::families::character::types::DamageType;
use gi_genius_invokation_definitions::families::character::zones::combat_status::CombatStatusZone;
use gi_genius_invokation_engine::Nat;

#[test]
fn center_of_attention_turns_physical_into_hydro {
    let combat_status = CombatStatusZone::new;
    let states = StateList::new;
    // 类型覆盖：物理 -> 水（真实 API：with_type_override）。
    let env = DamageChainEnv::new(&combat_status, &states)
        .with_type_override(Some(DamageType::elemental(Element::Hydro)));
    let outcome = settle(
        DamageInput::new(Nat::new(2), DamageType::Physical),
        &env,
        &DamageBonus::NONE,
    );
    assert_eq!(outcome.input.ty, DamageType::Physical); // 原始类型仍记录
    assert_eq!(outcome.to_hp, Nat::new(2));             // 数值不变，只换类型
}

#[test]
fn pneuma_branch_deals_two_bonus_and_one_piercing {
    // 始基力：芒性形态：DamageArgs::with_modifiers(add_before_mul(2)) + LostHpRank(Least) 1 点穿透。
    // 具体断言按 §2.6 的 ops 列表逐条比对。
}
```

### 2.3 What is still missing before landing (capability list)

| Gap | Impact | 留空 id |
|---|---|---|
| No skill-effect context (`SkillEffect = fn()`, D162) | The four skill effect bodies cannot be written | #125 |
| Entity → card-id mapping missing (`CharacterSlot` has no card id) | The talent condition "active character is Furina" cannot be written | #126 |
| `FormState` is not bound to the character entity | Form branches cannot read "current form" | #127 |
| No "consume 1 layer" primitive for statuses | Center of Attention cannot decrement / be removed at 0 | #128 |
| Summon same-name merge takes the maximum + no "replace summon card id" | 2+2=4 stacking and form sync cannot be done | #129 |
| Equipment triggers cannot reach `DefState` | "Immediately use a skill after equipping" cannot be wired | #130 |
| Old draft wrongly wrote "a second Salon Solitaire" | Corrected: Salon Solitaire has exactly one skill; the talent only "applies a status when the skill is used"; no skill-id issue | #131 (old draft, corrected) |
| Tag key vs tagstext inverted | Ousia / Pneuma must be judged by tagstext | #132 |

## 3. Playtest

### 3.1 Reproducible position (double lose)

Position: your Furina is in the **Arkhe: Pneuma form**, has the talent "Hear Me — Let Us Raise the Chalice of Love!" equipped and Center of Attention attached (usage 1) from Salon Solitaire; your side has only this one character left and its **current HP is 1**; the opponent has only one character left. It is Furina's turn to Normal Attack.

Reproduction steps:

1. Play the talent "Hear Me — Let Us Raise the Chalice of Love!" on Furina (combat action, 3 Hydro Dice) → triggers "immediately use Salon Solitaire once".
2. Salon Solitaire resolves: summon Singer of Many Waters (Pneuma), and apply Center of Attention to Furina (1 usage).
3. Remove all your other characters so only the 1-HP Furina remains; the opponent also keeps only one character.
4. Furina performs a Normal Attack (2 Physical → converted to Hydro by Center of Attention, Pneuma +2 = 4 Hydro DMG).
5. Within the same settlement, Center of Attention appends "1 Piercing DMG to your character who has taken the least DMG" → hits the 1-HP Furina.
6. Observe: the opponent's last character is defeated by 4 Hydro DMG; your Furina is defeated by 1 Piercing DMG — both character zones become empty within the same step.

Expected (by rules + code): after the step completes, the step boundary yields `Outcome::BothLose` (both players lose, D267 / D268); not a draw, and not "whoever settles first wins".

### 3.2 Playtest record

| Item | Position / operation | In-game result | Our implementation | Conclusion |
|---|---|---|---|---|
| Double lose (core of this page) | Arkhe: Pneuma form + talent "Hear Me — Let Us Raise the Chalice of Love!" + Normal Attack; one character left each side, ours at 1 HP | **pending test** | Passed: `state/terminal.rs :: tests::same_step_enemy_damage_plus_self_damage_ends_in_both_lose` | Both players lose (`BothLose`); pending in-game confirmation |
| Physical → Hydro | Normal Attack with Center of Attention (2 Physical) | **pending test** | Passed: `damage.rs :: tests::type_override_turns_physical_into_hydro_and_vaporizes` | Settles by the override type, value unchanged |
| Pneuma +2 | Normal Attack in Arkhe: Pneuma form | **pending test** | Not implemented (card layer missing) | Test after implementation |
| Pneuma 1 Piercing DMG | The appended instance in Arkhe: Pneuma form | **pending test** | Not implemented (card layer missing) | Test after implementation |
| Piercing ignores shields / immunity | 1 Piercing DMG against a shield / immunity | **pending test** | Available (chain skips shields / reductions / immunity for piercing) | Piercing always lands |
| Usage consumption timing | Whether Center of Attention disappears after the trigger | **pending test** | Not implemented (#128, no "consume 1 layer" primitive) | Consumed on trigger, not at round end |
| Form and summon form sync | Play event "Seats Sacred and Secular" | **pending test** | Not implemented (#129) | Character and on-field summons switch together |

## 4. Notes

### 4.1 Pitfalls

> [!WARNING]
> These four directly change the double-lose conclusion or the form branches.

1. **Switch the form and the summon form together**: event Seats Sacred and Secular requires "if you have Salon Members or Singer of Many Waters on the field, change their state as well"; switching only the character form makes the End Phase effect use wrong numbers.
2. **Usage consumption timing**: Center of Attention consumes 1 usage when a Normal Attack triggers it, **not** at round end / action-phase start (the reference implementation's old bug named by D12).
3. **Piercing ignores shields / reductions / immunity**: if the Arkhe: Pneuma form branch's 1 Piercing DMG were absorbed by a shield, "your last character is defeated" would not hold and the double-lose conclusion collapses (rulebook special entry: Piercing DMG).
4. **Tag key vs tagstext inverted**: `furina.json` (Chinese "荒性") carries `GCG_TAG_ARKHE_PNEUMA` while the English tagstext is `Arkhe: Ousia`; map by tagstext (#132).

### 4.2 Related D-numbers

| D no. | Content | Relation to this page |
|---|---|---|
| D204 | A step is the atomic settlement unit; no early termination inside a step | The double lose is judged only at the step boundary |
| D267 / D268 | Both sides wiped within one step = both lose | The core conclusion of this page |
| D12 | Usage decrements per trigger | Center of Attention consumption timing |
| D164 / D167 | Damage attaches elements and the reaction check is unified at the damage entry | Reaction check after Physical → Hydro |
| D185 / D186 | Timing keys are defined by the family; effects register function pointers | Trigger / dispatch model |
| D193 / D194 | Avoid-defeat is separate from defeat | The step-boundary criterion uses zones |

### 4.3 留空.md §二十四 (framework gaps for the Furina implementation)

| No. | Gap |
|---|---|
| #125 | `SkillEffect` has no context (`fn()`, D162) |
| #126 | Entity → card-id mapping missing |
| #127 | `FormState` is not bound to the character entity |
| #128 | No "consume 1 layer" primitive for statuses |
| #129 | Summon same-name merge does not match card text + no replace-card-id primitive |
| #130 | Equipment triggers cannot reach `DefState` |
| #131 | Old draft wrongly wrote "a second Salon Solitaire": corrected (one skill; the talent triggers a status) |
| #132 | Tag key vs tagstext inverted |

### 4.4 Related pages

| Page | Relation |
|---|---|
| [[定义层-Character-EN]] / [[定义层-Character|中文]] | Character family, damage chain, real source of skill_flow |
| [[定义层-Summon-Enemy-Status-EN]] / [[定义层-Summon-Enemy-Status|中文]] | Status identities / summon zone |
| [[定义层-Action-Card-EN]] / [[定义层-Action-Card|中文]] | Playing and placing the talent as equipment |

---
Last verified: 2026-10-19 (code: `crates/definitions/src/state/terminal.rs`, `crates/definitions/src/families/character/damage_chain/chain.rs`, `crates/definitions/src/keywords/target.rs`)
