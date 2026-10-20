# 定义层-Keywords

> 中文 · [[定义层-Keywords-EN|English]]

*本页范围: 关键字层: 目标选择器, 关键字操作, 一步原子, 反应子结算, 迁移与轨迹.*

---

## 1. 职责

关键字层把卡面文本拆成数据: 目标选择器筛候选, 关键字操作说明发生什么, 关键字函数在状态上执行它.

它不执行自己派发的触发, 也不在步内插入.

## 2. 关键概念与类型

目标选择器只筛候选. 选哪个要么是全部, 要么是均匀随机展开成分支.

一步是原子单位: 操作按书写顺序执行, 步末才允许执行待执行效果.

反应在步内判定, 但在步边界通过专用帧结算.

## 3. 公开入口

### 3.1 目标选择器

相对侧用作用域表达, 不用侧编号.


文件: crates/definitions/src/keywords/target.rs

```rust
/// **相对侧**（相对视角方；不是敌我编号 / 不是下标，D29 / D34）。
///
/// - [SideScope::Own]：视角方自己这一边（卡面「我方」）；
/// - [SideScope::Opponent]：对方那一边（卡面「敌方」）；
/// - [SideScope::Both]：双方（卡面「双方」）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SideScope {
    /// 视角方（「我方」）。
    Own,
    /// 对方（「敌方」）。
    Opponent,
    /// 双方。
    Both,
}

/// **生命值排序方向**（[TargetSelector::HpRank] 用；D228）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HpOrder {
    /// 从**当前生命值最高**开始（第 1 个 = 生命最高）。
    Desc,
    /// 从**当前生命值最低**开始（第 1 个 = 生命最低）。
    Asc,
}

/// **受伤量排序方向**（[TargetSelector::LostHpRank] 用；D237）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum LostHpOrder {
    /// 受伤**最多**开始（受伤量降序；第 1 个 = 受伤最多）。
    Most,
    /// 受伤**最少**开始（受伤量升序；第 1 个 = 受伤最少）。
    Least,
}

/// **并列时的槽位方向**（[HpTie] 用）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SlotOrder {
    /// 从左到右（槽位顺序：先出战的在前）。
    LeftToRight,
    /// 从右到左（槽位顺序反过来）。
    RightToLeft,
}

/// **生命值并列时的定序规则**（D229；规则书没写并列怎么取，全部 **待实测**）。
///
/// 比较顺序**固定三级**（D230：出战角色优先先于左右优先）：
/// 1. **当前生命值**（[TargetSelector::HpRank] 的 order：高 / 低）；
/// 2. **是否出战角色**（本结构的 active_first 为真时，出战排在同血量其它角色之前）；
/// 3. **槽位方向**（本结构的 direction：从左到右 / 从右到左）。
///
/// 也就是：出战优先**压过**左右优先，左右优先只在「同血量且同出战态」时才起作用。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct HpTie {
    /// 并列时**出战角色优先**。
    pub active_first: bool,
    /// 其余角色按槽位方向排（出战优先之后再看）。
    pub direction: SlotOrder,
}
```

解析需要视角, 效果主体, 以及可选的反应承载角色.


文件: crates/definitions/src/keywords/target.rs

```rust
/// **解析上下文**：效果主体（自己）+ 视角方 + 反应承载角色（可选）。
///
/// 由 [crate::keywords::effect::EffectEnv] 提供；关键字函数自己不拼实体号（D206）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ResolveContext {
    /// 视角方（成对入口填）。
    pub viewer: Perspective,
    /// 效果主体实体号（谁在使用这张牌 / 这个技能；D205）。
    pub subject: EntityId,
    /// 反应承载角色（反应效果关键字用；没有 -> None）。
    pub reacted: Option<EntityId>,
}

impl ResolveContext {
    /// 建解析上下文。
    pub const fn new(viewer: Perspective, subject: EntityId) -> ResolveContext {
        ResolveContext {
            viewer,
            subject,
            reacted: None,
        }
    }

    /// 带上「反应承载角色」。
    pub const fn with_reacted(mut self, reacted: Option<EntityId>) -> ResolveContext {
        self.reacted = reacted;
        self
    }
}
```

这是完整的选择器枚举. 每个变体只描述候选.


文件: crates/definitions/src/keywords/target.rs

```rust
/// **目标选择器**（关键字函数的参数）。
///
/// 每个变体都只描述「候选集合怎么筛」，不描述「选哪一个」：随机 / 选择策略由上层在
/// 观测点展开成分支（D190）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
    /// 角色，则其中靠前的角色被视为『最近』。「（tcgkeywords 4lang；英文 foremost /
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

解析结果是分侧列表, 不排序也不选.


文件: crates/definitions/src/keywords/target.rs

```rust
/// **解析出来的具体实体集合**（按两个具名实例分开；D34：不编号）。
///
/// 顺序 = 角色区 / 手牌 / 召唤物 / 支援区自己的容器顺序（可复现）；不是策略顺序。
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct TargetSet {
    /// 我方实体。
    me: Vec<EntityId>,
    /// 敌方实体。
    opp: Vec<EntityId>,
}

impl TargetSet {
    /// 空集合。
    pub const fn new() -> TargetSet {
        TargetSet {
            me: Vec::new(),
            opp: Vec::new(),
        }
    }

    /// 把实体放进某一侧。
    pub fn push(&mut self, side: Perspective, entity: EntityId) {
        match side {
            Perspective::Me => self.me.push(entity),
            Perspective::Opp => self.opp.push(entity),
        }
    }

    /// 某一侧的实体（只读切片）。
    pub fn of(&self, side: Perspective) -> &[EntityId] {
        match side {
            Perspective::Me => &self.me,
            Perspective::Opp => &self.opp,
        }
    }

    /// 我方实体（只读切片）。
    pub fn me(&self) -> &[EntityId] {
        &self.me
    }

    /// 敌方实体（只读切片）。
    pub fn opp(&self) -> &[EntityId] {
        &self.opp
    }

    /// 总条数。
    pub fn len(&self) -> Nat {
        nadd!(Nat::new(self.me.len() as _), Nat::new(self.opp.len() as _))
    }

    /// 是否一个实体都没有。
    pub fn is_empty(&self) -> bool {
        self.me.is_empty() && self.opp.is_empty()
    }

    /// 是否包含某个实体。
    pub fn contains(&self, entity: EntityId) -> bool {
        self.me.contains(&entity) || self.opp.contains(&entity)
    }

    /// 按「我方在前、敌方在后」遍历 (侧, 实体)。
    pub fn iter(&self) -> impl Iterator<Item = (Perspective, EntityId)> + '_ {
        self.me
            .iter()
            .copied()
            .map(|entity| (Perspective::Me, entity))
            .chain(
                self.opp
                    .iter()
                    .copied()
                    .map(|entity| (Perspective::Opp, entity)),
            )
    }
}
```

解析与辅助函数都是纯读.


文件: crates/definitions/src/keywords/target.rs

```rust
/// 把选择器**解析成具体实体集合**（纯读；不采样、不排序策略）。
pub fn resolve(state: &DefState, selector: TargetSelector, context: &ResolveContext) -> TargetSet {
    let mut set = TargetSet::new();
    match selector {
    // ...

/// 把相对侧展开成「具体两侧」：(候选数组, 前 count 个有效)。
///
/// 用固定长度数组避免 Vec 分配；Own / Opponent 只看第一个。
pub fn sides_of(scope: SideScope, viewer: Perspective) -> ([Perspective; 2], usize) {
    match scope {
        SideScope::Own => ([viewer, viewer], 1),
        SideScope::Opponent => ([viewer.other(), viewer.other()], 1),
        SideScope::Both => ([Perspective::Me, Perspective::Opp], 2),
    }
}

/// 某一侧的区（只读指针）。
pub fn player_of(state: &DefState, side: Perspective) -> &PlayerDefs {
    match side {
        Perspective::Me => state.me(),
        Perspective::Opp => state.opp(),
    }
}

/// 在双方区里找某个实体属于哪一侧（角色 / 手牌 / 召唤物 / 支援 / 装备）。
pub fn locate(state: &DefState, entity: EntityId) -> Option<Perspective> {
    for side in [Perspective::Me, Perspective::Opp] {
        let player = player_of(state, side);
    // ...
```

### 3.2 操作与参数

伤害参数携带选择器, 数值, 类型, 来源类别, 选取策略与乘算修饰.


文件: crates/definitions/src/keywords/op.rs

```rust
/// **伤害来源类别**（卡面「造成的伤害」属于哪一类：普通攻击 / 元素战技 / 元素爆发 /
/// 状态 / 召唤物 / 反应…）。
///
/// 它只作**元数据**：伤害数值 / 附着 / 反应的语义不在本层重写（走
/// families::character::damage::damage_with_chain），本类别供上层的加伤 / 触发
/// 过滤使用（**待接入**：当前没有消费方，只随参数带回）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum DamageSourceKind {
    /// 普通攻击。
    NormalAttack,
    /// 元素战技。
    ElementalSkill,
    /// 元素爆发。
    ElementalBurst,
    /// 被动技能。
    PassiveSkill,
    /// 角色装备 / 状态造成的伤害。
    Status,
    /// 召唤物造成的伤害。
    Summon,
    /// 支援牌造成的伤害。
    Support,
    /// 元素反应造成的伤害。
    Reaction,
    /// 其它（卡面没写清 / 待复核）。
    Other,
}

/// **目标选取策略**（在候选集合上怎么取）。
///
/// 候选集合由 [TargetSelector] 解析；本枚举只决定「取全部」还是「均匀随机取一个」。
/// 随机取一个 = 把多个可能结果展开成多支（每支概率 1 / 候选数），**不采样**（D190）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetPick {
    /// 对候选集合里**每个**实体各执行一次（例如「对所有敌方后台角色造成 2 点伤害」）。
    All,
    /// 从候选集合里**均匀随机取一个**（不采样：展开成每支概率 1 / 候选数的分支）。
    OneUniform,
}

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

impl DamageArgs {
    /// 一个目标、取全部（无乘算修饰）。
    pub const fn new(
        targets: TargetSelector,
        amount: Nat,
        ty: DamageType,
        source_kind: DamageSourceKind,
    ) -> DamageArgs {
        DamageArgs {
            targets,
            amount,
            ty,
            source_kind,
            pick: TargetPick::All,
            modifiers: DamageModifiers::NONE,
        }
    }

    /// 补上乘算 / 除算修饰（链式）。
    pub const fn with_modifiers(mut self, modifiers: DamageModifiers<'static>) -> DamageArgs {
        self.modifiers = modifiers;
        self
    }
}
```

其它操作各自有小的参数结构.


文件: crates/definitions/src/keywords/op.rs

```rust
/// **治疗**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HealArgs {
    /// 目标选择器（角色）。
    pub targets: TargetSelector,
    /// 治疗量。
    pub amount: Nat,
}

/// **附着元素**的参数（非伤害形式的元素附着）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AttachElementArgs {
    /// 目标选择器（角色）。
    pub targets: TargetSelector,
    /// 附着元素。
    pub element: Element,
}

/// 状态挂载位置（对应三个区）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StatusPlace {
    /// 角色状态（CharacterState::states）。
    Character,
    /// 出战状态（CombatStatusZone）。
    CombatStatus,
    /// 手牌状态（HandCardStates）。
    HandCard,
}

/// **施加状态**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatusArgs {
    /// 目标选择器（角色 / 手牌）。
    pub targets: TargetSelector,
    /// 挂载位置。
    pub place: StatusPlace,
    /// 状态牌 id（状态牌族；函数内部用 statuses::status_of_card 换状态身份）。
    pub card: CardId,
    /// 层数（None = 布尔类状态）。
    pub layers: Option<Nat>,
    /// 持续回合（只有出战状态用；None = 无持续回合概念）。
    pub duration: Option<Nat>,
    /// **叠加上限**（Z15 / 权威关键字 K？：「最多叠加到 N」）：None = 没有上限。
    ///
    /// 依据（卡面 / 规则书例子）：
    /// - 护盾：「护盾值可叠加，最多 2 点」（规则书·元素反应·效果 结晶）；
    /// - 燃烧烈焰：「可用次数 1，最多叠加到 2」；
    /// - 激化领域：「生成 2 层……最多叠加 2 层」；
    /// - 雷暴云：「可叠加，没有上限」。
    ///
    /// 语义（本实现，规则书未写清超限处理 → **TODO / 待实测**）：同身份同承载的层数
    /// 相加后**截断到上限**，超出部分丢弃（卡面「最多」的常见解释）。布尔类状态不参与。
    pub stack_cap: Option<Nat>,
}

/// **召唤 / 生成召唤物**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SummonArgs {
    /// 生成在哪一方（相对视角方）。
    pub side: SideScope,
    /// 召唤物牌 id（召唤物牌族）。
    pub card: CardId,
    /// 初始可用次数。
    pub uses: Option<Nat>,
    /// 初始效果量（如伤害值）。
    pub points: Option<Nat>,
    /// 效果类别（没有就沿用已有的）。
    pub effect_kind: Option<SummonEffectKind>,
    /// **同名/同源叠加上限**（Z15）：None = 没有上限（例如雷暴云「没有上限」）。
    ///
    /// 对召唤物按**可用次数**解释（燃烧烈焰「可用次数 1，最多叠加到 2」）：
    /// 新召唤物 / 合并后的可用次数**截断到上限**。规则书没写清超限处理 → **TODO / 待实测**。
    pub stack_limit: Option<Nat>,
}
```


文件: crates/definitions/src/keywords/op.rs

```rust
/// **抽几张**的表达（有动态量：沙中遗事抓「原手牌数 + 1」）。
///
/// 动态度在**一步开始时**取（D204：一步是原子结算单位），所以「先把手牌放回牌组、
/// 再抓原手牌数 + 1「这种写法不会因为手牌已经离手而算错。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawCount {
    /// 固定张数。
    Fixed(Nat),
    /// **本步开始时**那一方手牌数 + 偏移（沙中遗事 = +1）。
    HandAtStepStartPlus(Nat),
}

/// **抽牌**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DrawCardsArgs {
    /// 哪一方抽（相对视角方）。
    pub side: SideScope,
    /// 抽几张（不够则少给）。
    pub count: DrawCount,
}

/// **弃置**的参数。
///
/// 目标选择器解析出的实体按「支援区 -> 手牌 -> 召唤物 -> 装备栏」顺序尝试弃置；
/// 找到就弃置并停止（同一实体只可能在一个区里）。
///
/// 支援区弃置自己 = DiscardArgs { targets: TargetSelector::Subject }（D205）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DiscardArgs {
    /// 目标选择器（自己 / 敌方支援牌 / 手牌…）。
    pub targets: TargetSelector,
}

/// **获得骰子**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GainDiceArgs {
    /// 哪一方获得（相对视角方）。
    pub side: SideScope,
    /// 骰面。
    pub face: DieFace,
    /// 数量。
    pub count: Nat,
}

/// **转化骰**的参数（把来源骰转成目标骰面；来源 None = 任意骰）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConvertDiceArgs {
    /// 哪一方转化。
    pub side: SideScope,
    /// 来源骰面（None = 任意）。
    pub source: Option<DieFace>,
    /// 数量。
    pub count: Nat,
    /// 目标骰面。
    pub target: DieFace,
}

/// **生成一张卡入手**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CreateCardArgs {
    /// 加入哪一方的手牌（相对视角方）。
    pub side: SideScope,
    /// 行动牌 id（行动牌牌族）。
    pub card: CardId,
}
```


文件: crates/definitions/src/keywords/op.rs

```rust
/// **切换角色**（普通切换，把后台角色切为出战角色；费用由骰池另算）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SwitchCharacterArgs {
    /// 切到哪个角色（目标选择器；必须解析到 1 个我方角色）。
    pub to: TargetSelector,
}

/// **强制切换**（例如超载：把出战角色强制换成下一个角色）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ForceSwitchArgs {
    /// 被强制切换的角色范围（解析到出战角色才生效）。
    pub targets: TargetSelector,
}

/// **充能量**（消耗充能关键字用）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EnergyAmount {
    /// 全部（元素爆发：消耗所有充能）。
    All,
    /// 固定点数。
    Fixed(Nat),
}

/// **消耗充能**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConsumeEnergyArgs {
    /// 目标选择器（角色）。
    pub targets: TargetSelector,
    /// 消耗多少。
    pub amount: EnergyAmount,
}

/// **赋予卡牌状态**的参数（手牌 / 牌组里的卡牌都走它）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CardStatusArgs {
    /// 目标选择器（手牌实体或牌组里的实体；月兆·满辉 = 随机手牌 + 牌组顶）。
    pub targets: TargetSelector,
    /// 要赋予的卡牌状态（电击 / 费用降低…）。
    pub status: HandCardStatus,
    /// 目标选取策略（随机一张手牌 -> OneUniform，按 D190 展开成下标分支）。
    pub pick: TargetPick,
    /// **叠加上限**（Z15 / 手牌卡状态）：None = 没有上限。
    ///
    /// 与 StatusArgs.stack_cap 同口径：同身份同承载的层数相加后截断到上限。
    /// 该状态是布尔还是层数由 statuses::hand_status_identity 的身份决定；
    /// 规则书未写清手牌状态的超限处理 -> **TODO / 待实测**。
    pub stack_cap: Option<Nat>,
}
```

重复用函数指针, 调用方可以按序号生成操作, 不用闭包.


文件: crates/definitions/src/keywords/op.rs

```rust
/// **按序号生成操作用的关键字函数指针**（D186 / D209：登记函数指针，不登记闭包）。
///
/// 输入 = 这一次是第几次（0 起），输出 = 这一次要执行的操作清单。
pub type RepeatOpFn = fn(Nat) -> Vec<KeywordOp>;

/// **顺序函数**：输入 (处理序下标, 总数)，输出 = 手牌 / 牌库容器里的下标。
///
/// 用于「按顺序放 / 取」的操作（例如沙中遗事逐张置底的顺序）；顺序不写死在关键字里，
/// 由调用方注入（D209）。
pub type OrderFn = fn(Nat, Nat) -> Nat;

/// **重复**包装：把一组关键字操作**在同一操作位置**按顺序执行 n 次。
///
/// 依据（D208 / D209）：召唤物的「执行 3 次」、月兆的两次赋予、逐张洗牌这类效果
/// **同属一步**；重复内部不插入、不派发，全部做完才回到一步的步末统一派发（D204）。
///
/// plan 是**函数指针**（D186：不登记效果数据 / 闭包）：输入「第几次」（0 起），
/// 输出这一次要执行的操作清单；所以「第 i 次做不同的事」不用各写一套循环（D209）。
#[derive(Clone, Copy, Debug)]
pub struct RepeatArgs {
    /// 执行次数。
    pub count: Nat,
    /// 第 i 次要执行的操作（序号 0 起依次传入；D209）。
    pub plan: RepeatOpFn,
}

/// **把某一方全部手牌置于牌组底**（沙中遗事；D208）。
///
/// 顺序口径（用户口径）：**后处理的牌在牌组底更上面**。处理顺序**不写死**：由调用方
/// 注入 [OrderFn]（D209），默认 [default_put_bottom_order]（按手牌容器顺序从后往前）。
/// **待实测**（手牌无序，顺序只按容器顺序）。
#[derive(Clone, Copy, Debug)]
pub struct PutHandToPileBottomArgs {
    /// 哪一方（相对视角方）。
    pub side: SideScope,
    /// 处理序 -> 手牌容器下标（注入顺序，D209）。
    pub order: OrderFn,
}

/// 默认置底顺序：**从后往前**处理手牌（后处理的牌最终在牌组底更上面）。
///
/// 输入 (处理序号 i, 手牌数 n)，输出手牌下标 = n - 1 - i。
pub fn default_put_bottom_order(index: Nat, total: Nat) -> Nat {
    if total == Nat::ZERO {
        return Nat::ZERO;
    }
    gi_genius_invokation_engine::nsub!(
        gi_genius_invokation_engine::nsub!(total, Nat::new(1)),
        index
    )
}
```

操作枚举是一步里一个动作的数据形式.


文件: crates/definitions/src/keywords/op.rs

```rust
/// **一条关键字操作**（一步里的一个操作；数据，不含实体号）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum KeywordOp {
    /// 造成伤害。
    Damage(DamageArgs),
    /// 治疗。
    Heal(HealArgs),
    /// 附着元素。
    AttachElement(AttachElementArgs),
    /// 施加状态（角色 / 出战 / 手牌）。
    Status(StatusArgs),
    /// 召唤 / 生成召唤物。
    Summon(SummonArgs),
    /// 抽牌。
    DrawCards(DrawCardsArgs),
    /// 弃置。
    Discard(DiscardArgs),
    /// 获得骰子。
    GainDice(GainDiceArgs),
    /// 转化骰。
    ConvertDice(ConvertDiceArgs),
    /// 生成一张卡入手。
    CreateCard(CreateCardArgs),
    /// 切换角色。
    SwitchCharacter(SwitchCharacterArgs),
    /// 强制切换角色。
    ForceSwitch(ForceSwitchArgs),
    /// 消耗充能。
    ConsumeEnergy(ConsumeEnergyArgs),
    /// 赋予卡牌状态（手牌 / 牌组）。
    CardStatus(CardStatusArgs),
    /// 把某一方全部手牌置于牌组底。
    PutHandToPileBottom(PutHandToPileBottomArgs),
    /// 重复执行一组操作（D208：同属一步）。
    Repeat(RepeatArgs),
}
```

每个结果携带带精确概率的分支, 待执行, 以及反应请求.


文件: crates/definitions/src/keywords/op.rs

```rust
/// **一支关键字结算分支**：局面 + 精确概率 + 待执行 + 待插入的反应子结算（D190）。
#[derive(Clone, Debug)]
pub struct KeywordBranch {
    /// 这一支结算后的完整局面。
    pub state: DefState,
    /// 这一支的精确概率。
    pub probability: Fraction,
    /// 这一支之后还要执行的待执行（本层不执行；步末统一执行，D204）。
    pub pending: PendingSet,
    /// 这一支产生的**反应子结算**（R4 / R2）：由步执行器在**步边界**插入执行
    /// （D204：步内不插；执行完回到原步骤）。
    pub reactions: Vec<super::reaction::ReactionRequest>,
}

impl KeywordBranch {
    /// 建一支分支（无反应子结算）。
    pub fn new(state: DefState, probability: Fraction, pending: PendingSet) -> KeywordBranch {
        KeywordBranch {
            state,
            probability,
            pending,
            reactions: Vec::new(),
        }
    }

    /// 确定性分支（概率 1、无待执行、无反应）。
    pub fn determined(state: DefState) -> KeywordBranch {
        KeywordBranch {
            state,
            probability: Fraction::ONE,
            pending: PendingSet::new(),
            reactions: Vec::new(),
        }
    }

    /// 带上反应子结算。
    pub fn with_reactions(
        mut self,
        reactions: Vec<super::reaction::ReactionRequest>,
    ) -> KeywordBranch {
        self.reactions = reactions;
        self
    }
}

/// **关键字函数的统一返回值**：分支列表 + 推进后的发号器。
#[derive(Clone, Debug)]
pub struct KeywordResult {
    /// 结算分支（确定性 = 恰好一支、概率 1）。
    pub branches: Vec<KeywordBranch>,
    /// 推进后的实体发号器（生成过召唤物 / 卡牌时前移）。
    pub alloc: EntityAlloc,
}

impl KeywordResult {
    /// 只有一支、概率 1、无待执行（局面没变）。
    pub fn unchanged(state: &DefState, alloc: EntityAlloc) -> KeywordResult {
        KeywordResult {
            branches: vec![KeywordBranch::determined(state.clone())],
            alloc,
        }
    }

    /// 只有一支、概率 1、带待执行。
    pub fn single(state: DefState, pending: PendingSet, alloc: EntityAlloc) -> KeywordResult {
        KeywordResult {
            branches: vec![KeywordBranch::determined(state).with_pending(pending)],
            alloc,
        }
    }

    /// 各分支概率之和（精确分数；用于断言概率守恒）。
    pub fn probability_total(&self) -> Fraction {
        self.branches
            .iter()
            .fold(Fraction::ZERO, |sum, branch| sum.exact_add(branch.probability))
    }

    /// 分支局面 + 概率（Vec<(DefState, Fraction)> 形态；方便观测）。
    pub fn states(&self) -> Vec<(DefState, Fraction)> {
        self.branches
            .iter()
            .map(|branch| (branch.state.clone(), branch.probability))
            .collect()
    }
}
```

### 3.3 一步

一步只是一个操作列表, 自身没有状态.


文件: crates/definitions/src/keywords/step.rs

```rust
/// **一步**：一组关键字操作（数据；顺序 = 显示 / 执行顺序）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Step {
    ops: Vec<KeywordOp>,
}

impl Step {
    /// 用一组操作建一步。
    pub fn new(ops: Vec<KeywordOp>) -> Step {
        Step { ops }
    }

    /// 没有操作的一步。
    pub fn empty() -> Step {
        Step { ops: Vec::new() }
    }

    /// 操作清单（只读）。
    pub fn ops(&self) -> &[KeywordOp] {
        &self.ops
    }

    /// 操作条数。
    pub fn len(&self) -> Nat {
        Nat::new(self.ops.len() as _)
    }

    /// 是否没有操作。
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}
```

执行一步返回分支与推进后的发号器.


文件: crates/definitions/src/keywords/step.rs

```rust
/// **一步的执行结果**：结算分支 + 推进后的发号器。
///
/// 分支里的待执行**还没有执行**（D204：步内不插入；调用方在步边界统一执行）。
#[derive(Clone, Debug)]
pub struct StepOutcome {
    /// 结算分支（每支 = 局面 + 精确概率 + 待执行）。
    pub branches: Vec<KeywordBranch>,
    /// 推进后的实体发号器。
    pub alloc: EntityAlloc,
}

impl StepOutcome {
    /// 确定性（只有一支）时取出那一支；多支时 None。
    pub fn single(&self) -> Option<&KeywordBranch> {
        if self.branches.len() == 1 {
            self.branches.first()
        } else {
            None
        }
    }

    /// 各分支概率之和（精确分数；用于断言概率守恒）。
    pub fn probability_total(&self) -> Fraction {
        self.branches
            .iter()
            .fold(Fraction::ZERO, |sum, branch| sum.exact_add(branch.probability))
    }
}

/// **执行一步**（我方视角）。
///
/// 只调用关键字函数：**不做任何 insert / execute_pending**（D204）；返回的待执行由调用方
/// 在步边界执行。
pub fn execute_step_me(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_step(state, step, &scoped)
}

/// **执行一步**（敌方视角；与 execute_step_me 共用一份实现）。
pub fn execute_step_opp(state: &DefState, step: &Step, env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Opp);
    run_step(state, step, &scoped)
}

/// **顺序执行多步**（我方视角）：每步之间执行上一步派发出来的待执行（插入只在步边界，D204）。
pub fn execute_steps_me(state: &DefState, steps: &[Step], env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Me);
    run_steps(state, steps, &scoped)
}

/// **顺序执行多步**（敌方视角）。
pub fn execute_steps_opp(state: &DefState, steps: &[Step], env: &EffectEnv<'_>) -> StepOutcome {
    let scoped = env.fork(env.alloc()).with_viewer(Perspective::Opp);
    run_steps(state, steps, &scoped)
}
```

### 3.4 关键字函数

每个关键字函数形状相同: 状态, 环境, 参数, 结果.


文件: crates/definitions/src/keywords/keyword.rs

```rust
/// **造成伤害**：对选择器解析出的每个角色走伤修链（加伤 -> 护盾 -> 减伤 -> 扣血 / 附着 / 反应）。
///
/// 每次伤害都派发「生命值变动」待执行（不执行，步末统一做，D204）。
pub fn cause_damage(state: &DefState, env: &EffectEnv<'_>, args: &DamageArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    // ...

/// **治疗**：对每个目标角色 hp_add 并派发「生命值变动 -> 治疗后」。
pub fn heal(state: &DefState, env: &EffectEnv<'_>, args: &HealArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    // ...

/// **附着元素**（非伤害形式）：改元素附着集合，**并且也判反应**（R2 / 规则书 S5.2 第 2 条）。
///
/// 规则书·元素反应·规则 第（2）条：「以非伤害的形式施加特定类型的『元素附着』时也会引发
/// 『元素反应』。结算这类『元素反应』效果时，会忽略所有伤害类效果。「
///
/// 顺序（结算顺序.md §三）：附着 -> 判定反应 -> 结算反应效果 -> 继续原步骤。
/// 本关键字只做到「判定 + 记反应子结算」：反应效果由步边界插入执行（D204 步内不插），
/// 执行时由 [super::reaction::ReactionRequest::non_damage] 标记「忽略所有伤害类效果」。
///
/// 反应后的附着消耗规则书未写清；这里按 D167 同一口径（移除命中的已覆盖元素 + 本次施加
/// 元素，本次元素不再附着）—— **待实测**。
pub fn attach_element(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &AttachElementArgs,
) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let candidates = character_targets(state, &targets);
    // ...

/// **施加状态**：角色状态 / 出战状态 / 手牌状态三种承载。
///
/// - 角色状态：直接写 CharacterState::states（位置 = CHARACTER_STATE_POS；与反应执行器同口径）；
/// - 出战状态：CombatStatusZone::attach_with_triggers（派发待执行，不执行）；
/// - 手牌状态：HandCardStates::add_state_with_triggers（派发待执行，不执行）。
///
/// 状态身份由 statuses::status_of_card(card) 给；未知卡 -> 不做任何事（不 panic）。
pub fn apply_status(state: &DefState, env: &EffectEnv<'_>, args: &StatusArgs) -> KeywordResult {
    let Some(identity) = statuses::status_of_card(args.card) else {
        return KeywordResult::unchanged(state, env.alloc());
    // ...

/// **召唤 / 生成召唤物**（同名合并、满 4 拒绝由召唤物区自持；本层只写回）。
pub fn summon(state: &DefState, env: &EffectEnv<'_>, args: &SummonArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    // ...

/// **抽牌**：每张抽取的折叠分支原样展开（PileState::draw_many 的精确概率）。
pub fn draw_cards(state: &DefState, env: &EffectEnv<'_>, args: &DrawCardsArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut branches: Vec<KeywordBranch> = vec![KeywordBranch::determined(state.clone())];
    // ...

/// **弃置**：按「支援区 -> 手牌 -> 召唤物 -> 装备栏」找到实体所在的区并弃置。
///
/// 支援区弃置自己 = 目标选择器 Subject（D205）。
pub fn discard(state: &DefState, env: &EffectEnv<'_>, args: &DiscardArgs) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let mut current = state.clone();
    // ...

/// **获得骰子**。
pub fn gain_dice(state: &DefState, env: &EffectEnv<'_>, args: &GainDiceArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    for side in sides.iter().take(count) {
        let player = player_of(&current, *side);
        let dice = player.dice().add(args.face, args.count);
        current = set_player(&current, *side, player.clone().with_dice(dice));
    }
    KeywordResult::single(current, PendingSet::new(), env.alloc())
}

/// **转化骰**（来源不够 -> 不做任何事）。
pub fn convert_dice(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &ConvertDiceArgs,
) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    for side in sides.iter().take(count) {
        let player = player_of(&current, *side);
        if let Some(dice) = player.dice().convert(args.source, args.count, args.target) {
            current = set_player(&current, *side, player.clone().with_dice(dice));
        }
    }
    KeywordResult::single(current, PendingSet::new(), env.alloc())
}

/// **生成一张卡入手**（手牌满 / 实体重复 -> 不加入；费用表同步**待接入**）。
pub fn create_card(state: &DefState, env: &EffectEnv<'_>, args: &CreateCardArgs) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    // ...

/// **切换角色**（把选择器解析到的我方后台角色切为出战角色；不付骰子费用）。
///
/// 费用（规则书：切换角色支付 1 个任意元素骰）归骰池 / 调用方，本关键字只做区与时机。
pub fn switch_character(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &SwitchCharacterArgs,
) -> KeywordResult {
    let targets = resolve(state, args.to, &env.resolve_context());
    let mut current = state.clone();
    // ...

/// **强制切换**：把解析到的出战角色强制换成角色区里的下一个后台角色（超载类效果）。
pub fn force_switch(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &ForceSwitchArgs,
) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let mut current = state.clone();
    // ...

/// **消耗充能**（元素爆发：全部；其它：固定点数）。
pub fn consume_energy(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &ConsumeEnergyArgs,
) -> KeywordResult {
    let targets = resolve(state, args.targets, &env.resolve_context());
    let mut current = state.clone();
    // ...

/// **赋予卡牌状态**：目标在手牌 -> 手牌卡状态表；目标在牌组 -> 牌库位置状态（mark）。
///
/// 月兆·满辉（D208）：随机 1 张手牌 + 牌组顶各一次 -> 调用两次本关键字（同一步）。
pub fn apply_card_status(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &CardStatusArgs,
) -> KeywordResult {
    let identity = statuses::hand_status_identity(args.status);
    let targets = resolve(state, args.targets, &env.resolve_context());
    // ...

/// **把某一方全部手牌置于牌组底**（沙中遗事；D208）。
///
/// 顺序口径：**后处理的牌在牌组底更上面**。实现上按手牌容器顺序**从后往前**追加到
/// 牌库末尾（末尾 = 底），于是容器里靠后的手牌最终落在底区更上面。
///
/// **待接入 / 待实测**：
/// - 牌库区没有「把不在牌库的牌插到底部」的公开原语（families 本块只读），这里用
///   PileState::from_fixed 重建整副牌库 —— 会丢掉随机段 / 公共状态列表；
///   需要牌库区补一个 insert_bottom_with(instance) 原语；
/// - 手牌容器声明无序，这里只按容器顺序（可复现，不承诺是玩家选择顺序）。
pub fn put_hand_to_pile_bottom(
    state: &DefState,
    env: &EffectEnv<'_>,
    args: &PutHandToPileBottomArgs,
) -> KeywordResult {
    let (sides, count) = super::target::sides_of(args.side, env.viewer());
    let mut current = state.clone();
    // ...
```

### 3.5 反应子结算

反应请求是数据: 哪个反应, 谁触发, 谁承受, 以及是否来自非伤害附着.


文件: crates/definitions/src/keywords/reaction.rs

```rust
/// **一次待执行的插入式子结算：一条已判定的反应**。
///
/// 关键字函数只产出它（纯数据、Copy），一步在步边界统一结算（D204：步内不插）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReactionRequest {
    /// 已判定的反应（damage_chain::bonus::ReactionHit；四要素与 ReactionEvent 一致）。
    pub hit: ReactionHit,
    /// 触发方（相对侧；= 反应事件 Source 所在的那一方）。
    pub source: Perspective,
    /// 承载方（反应作用在其角色身上的一方）。
    pub target: Perspective,
    /// 是不是「非伤害形式的附着」引发的反应（真 = 结算时忽略所有伤害类效果）。
    pub non_damage: bool,
}

impl ReactionRequest {
    /// 建一条反应子结算请求。
    pub const fn new(
        hit: ReactionHit,
        source: Perspective,
        target: Perspective,
        non_damage: bool,
    ) -> ReactionRequest {
        ReactionRequest {
            hit,
            source,
            target,
            non_damage,
        }
    }

    /// 反应事件（交给执行器 / 注册表用）。
    pub fn event(self) -> ReactionEvent {
        self.hit.event()
    }
}

/// **插入式子结算帧**（与 timeline::Resolution 同语义：insert 插入子结算，内层做完回到插入点；
/// 不是 LIFO 触发栈、不重排已有帧）。
///
/// 帧的内容 = 一组 [ReactionRequest]（帧内按生成顺序结算）。用专用帧而不是 Resolution 的原因
/// 见模块文档（Queued<E> 装不下反应效果执行所需的整份区快照）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReactionFrames {
    frames: Vec<Vec<ReactionRequest>>,
}

impl ReactionFrames {
    /// 空帧栈。
    pub fn new() -> ReactionFrames {
        ReactionFrames { frames: Vec::new() }
    }

    /// **插入子结算**：压入一帧（空帧 = 不做任何事）。
    pub fn insert(&mut self, requests: Vec<ReactionRequest>) {
        if requests.is_empty() {
            return;
        }
        self.frames.push(requests);
    }

    /// 取下一帧（内层先做；做完回到外层）。没有帧 -> None。
    pub fn next_frame(&mut self) -> Option<Vec<ReactionRequest>> {
        self.frames.pop()
    }

    /// 是否已经全部做完。
    pub fn is_finished(&self) -> bool {
        self.frames.is_empty()
    }

    /// 嵌套层数。
    pub fn depth(&self) -> Nat {
        Nat::new(self.frames.len() as _)
    }
}
```

结算单条反应, 一组反应, 或步边界插入, 都走这三个入口.


文件: crates/definitions/src/keywords/reaction.rs

```rust
/// **执行一条反应子结算**：返回每支（局面 + 精确概率 + 待执行 + 发号器）。
///
/// 没登记效果的反应 -> 恰好一支、局面不变（不 panic）。
pub fn settle_reaction(
    state: &DefState,
    request: &ReactionRequest,
    env: &EffectEnv<'_>,
) -> Vec<(KeywordBranch, EntityAlloc)> {
    let event = request.event();
    let Some(effect) = table::resolve(&event, env.effect_context) else {
    // ...

/// **按顺序结算一组反应请求**（每条可能展开成多支；概率精确相乘；不折叠）。
pub fn settle_reactions(
    state: &DefState,
    requests: &[ReactionRequest],
    env: &EffectEnv<'_>,
) -> KeywordResult {
    let mut work = vec![Work {
        state: state.clone(),
    // ...

/// **插入式结算**（R4 的「回到原步骤」）：把一批反应请求作为子结算插入当前位置执行。
///
/// 步边界调用：本步的全部动作已经做完（D204 步内不插），这里把反应效果插入执行，
/// 做完回到调用方的下一步。
pub fn settle_inserted(
    state: &DefState,
    requests: &[ReactionRequest],
    env: &EffectEnv<'_>,
) -> KeywordResult {
    let mut frames = ReactionFrames::new();
    frames.insert(requests.to_vec());
    // ...
```

### 3.6 执行上下文与迁移

执行上下文借入双方的触发注册表, 不拥有它们.


文件: crates/definitions/src/keywords/effect.rs

```rust
/// **双方的触发注册表集合**（只借；D34：两个具名侧各一组）。
#[derive(Clone, Copy, Debug)]
pub struct TriggerBook<'a> {
    /// 我方的角色区 / 出战状态区 / 召唤物区 / 手牌状态触发。
    pub me: &'a SideTriggers,
    /// 敌方的角色区 / 出战状态区 / 召唤物区 / 手牌状态触发。
    pub opp: &'a SideTriggers,
    /// 我方的角色族触发（生命值变动 / 治疗后）。
    pub me_characters: &'a CharacterTriggers,
    /// 敌方的角色族触发（生命值变动 / 治疗后）。
    pub opp_characters: &'a CharacterTriggers,
    /// 我方的支援区触发。
    pub me_supports: &'a SupportTriggers,
    /// 敌方的支援区触发。
    pub opp_supports: &'a SupportTriggers,
    /// 我方的装备栏触发。
    pub me_equipment: &'a EquipmentTriggers,
    /// 敌方的装备栏触发。
    pub opp_equipment: &'a EquipmentTriggers,
}

/// **关键字函数的执行上下文**。
pub struct EffectEnv<'a> {
    subject: EntityId,
    viewer: Perspective,
    reacted: Option<EntityId>,
    alloc: EntityAlloc,
    /// 伤害入口依赖（允许附着集合 + 反应规则表 + 反应注册表）——直接复用
    /// families::character::damage::DamageEnv，不另造一套。
    pub damage: DamageEnv<'a>,
    /// 双方的触发注册表。
    pub triggers: TriggerBook<'a>,
    /// 触发方元素精通（反应加伤 / 结晶护盾强化）。
    pub mastery: Nat,
    /// 通用增伤（卡面「造成的伤害+N」；顺序由调用方排好）。
    pub increases: &'a [Nat],
    /// 反应效果函数的局面摘要（伤害链求反应加伤用）。
    pub effect_context: &'a EffectContext,
    /// **反应效果造成的伤害**的乘算 / 除算来源（默认 NONE；从卡牌 / 关键字传入）。
    ///
    /// 它与关键字伤害自己的 [crate::keywords::op::DamageArgs::modifiers] 分开：
    /// 触发伤害用 DamageArgs.modifiers；反应效果里的 EffectOp::Damage 用本字段。
    reaction_modifiers: DamageModifiers<'a>,
    /// **本步开始时**我方手牌数（动态张数用；D204 一步是原子结算单位）。
    step_start_me_hand: Nat,
    /// **本步开始时**敌方手牌数。
    step_start_opp_hand: Nat,
}
```

迁移把旧计划与反应操作转成一步, 并标出何时需要调用方提供反应承载角色.


文件: crates/definitions/src/keywords/migrate.rs

```rust
/// 迁移桥接的结果（一步 + 需要调用方给执行上下文的东西）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MigratedStep {
    /// 迁移后的一步。
    pub step: Step,
    /// 是否需要执行上下文里的「反应承载角色」（EffectOp 的 Reacted / TargetOthers 用）。
    pub needs_reacted: bool,
}

/// **旧技能效果体** -> 一步（迁移桥接；旧流程行为不变）。
pub fn step_from_skill_effect_plan(plan: SkillEffectPlan) -> Step {
    match plan {
        SkillEffectPlan::None => Step::empty(),
    // ...

/// **反应效果的一条底层操作** -> 一个关键字调用（迁移桥接）。
pub fn keyword_op_from_effect_op(op: EffectOp) -> KeywordOp {
    match op {
        EffectOp::Damage { amount, ty, target } => KeywordOp::Damage(DamageArgs::new(
    // ...

/// **一条反应分支**（操作清单）-> **一步**（D208：分支里的全部动作同属一步）。
pub fn step_from_effect_ops(ops: &[EffectOp]) -> MigratedStep {
    let needs_reacted = ops.iter().any(op_uses_reacted);
    MigratedStep::of(
        Step::new(ops.iter().copied().map(keyword_op_from_effect_op).collect()),
        needs_reacted,
    )
}

/// **一条反应分支**（操作清单）-> 一步 + 是否需要 reacted 上下文。
pub fn step_from_reaction_ops(ops: &[EffectOp]) -> MigratedStep {
    step_from_effect_ops(ops)
}

/// 反应效果的一步（Discard 迁移示例：效果自己弃置自己 -> Discard(Subject)）。
pub fn discard_self_step() -> Step {
    Step::new(vec![KeywordOp::Discard(DiscardArgs {
        targets: TargetSelector::Subject,
    })])
}
```

## 4. 不变量与禁止

> [!IMPORTANT]

> 关键字函数不硬写实体号; 自己用 `Subject`, 双方用相对作用域.

> 一步内部不插入, 也不执行待执行.

> [!WARNING]

> 不要采样随机; 展开成带精确分数的分支.

> 不要在操作中途读当前手牌; 用步开始时的快照.

## 5. 依据

- 游戏区域 I 与使用技能: 攻击对方出战角色;
- 关键字条目 distance nearest: 距离对方出战角色最近的槽位;
- S5.2: 非伤害附着也会引发反应, 结算时忽略伤害类效果;
- 结算顺序: 插入式结算, 不是栈;
- D25 与 D26: 选择器不做选择策略;
- D29 与 D34: 相对视角与成对入口;
- D105: 公共状态列表;
- D186: 派发只排序;
- D190 与 D191: 分支带精确概率;
- D204, D208, D209: 一步原子, 重复, 注入顺序;
- D205 与 D206: 自己是选择器;
- D228, D229, D230: 生命排名三级并列;
- D237 与 D238: 受伤量用运行期最大生命;
- D258 与 D259: 反应专用帧, 步边界结算.

## 6. 常见误用

> [!WARNING]

> 给 `resolve` 传错视角; `Own` 是视角方自己, 不是全局我方.

> 排名时用定义最大生命而不是运行期值.

- 指望选择器随机选目标; 用选取策略;
- 该用步边界插入时却用了 `settle_reactions`;
- 把 `Equipments` 当成某个角色的装备栏.

## 7. 测试

- `keywords/target.rs`: 作用域, 最近, 生命排名, 击倒排除;
- `keywords/keyword.rs`: 伤害, 治疗, 附着, 状态, 骰子, 卡牌状态, 叠加上限;
- `keywords/step.rs`: 原子顺序, 边界插入, 重复, 动态张数;
- `keywords/reaction.rs`: 帧, 非伤害反应, 概率与顺序.

## 8. 待接入

> [!NOTE]

> 规则书没写的口径一律标待实测, 不自行规定.

- 最近: 是否镜像, 击倒角色是否占位, 并列口径;
- 叠加上限与超限处理规则书未写;
- 反应伤害是否再引发第二层反应待复核;
- 迁移会丢弃暂时没有参数的状态可用次数与叠加上限.

## 小结

选择器筛候选, 操作描述动作, 一步原子执行, 反应等到步边界. 实体号留在上下文, 随机性留在精确分支.

---

最后核对: 2026-10-20 (代码: `crates/definitions/src/keywords/`)
