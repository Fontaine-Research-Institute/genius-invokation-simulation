//! **目标选择器**（关键字函数的参数）：把卡面写法（敌方出战角色、(下)(1)个角色、
//! 所有敌方后台角色、生命最低的敌方角色…）解析成**具体实体集合**。
//!
//! ## 口径（与工作区一致，逐条）
//!
//! - **不做选择策略**（D25 / D26）：本模块只把选择器**筛成候选集合**（TargetSet），
//!   不采样、不排序策略；随机一张由关键字层在观测点展开成「分支 + 精确概率」（D190）；
//! - **相对视角，不是敌我编号**（D29 / D34）：选择器里写的是「我方 / 敌方」——相对
//!   **视角方**（[Perspective]）而言；视角方由成对入口（execute_step_me / execute_step_opp）
//!   自动绑定，卡面写法不手写数字；
//! - **自己只写 Subject**（D205 / D206）：[TargetSelector::Subject] 解析时从执行上下文
//!   （[ResolveContext::subject]）取**效果主体**实体号，写效果时不出现任何实体号；
//! - **区自持、对外只读指针**（D182 / D183）：解析只读 &DefState 的只读指针，不写任何区；
//! - **候选集合**：同一选择器可能解析出多个实体（例如所有敌方后台角色）；顺序只是
//!   区的容器顺序（可复现），**不承诺它是策略顺序**。
//!
//! ## 依据
//!
//! - 规则书·游戏区域 I：角色区 / 出战角色 / 后台角色；
//! - 规则书·进行行动 使用技能：技能攻击对方的出战角色；
//! - 卡面文本：(下)(1)个角色、所有敌方后台角色、生命值最低的敌方角色等；
//! - D204 / D205 / D206（Lead 拍板）：一步原子；自己是选择器的一个值、由上下文函数解析。

use crate::families::character::character::CharacterState;
use crate::state::{DefState, PlayerDefs};
use gi_genius_invokation_engine::{nadd, nsub, EntityId, Nat};

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

impl HpTie {
    /// 默认口径（Lead 建议）：**出战优先 + 从左到右**。**待实测**。
    pub const DEFAULT: HpTie = HpTie {
        active_first: true,
        direction: SlotOrder::LeftToRight,
    };

    /// 建并列规则。
    pub const fn new(active_first: bool, direction: SlotOrder) -> HpTie {
        HpTie {
            active_first,
            direction,
        }
    }
}

impl Default for HpTie {
    fn default() -> HpTie {
        HpTie::DEFAULT
    }
}

// 视角方类型已上移到 `base`（state 层要先手信息，而 state 不能反向依赖 keywords）：
// 这里只重导出，旧路径 `crate::keywords::target::Perspective` 继续可用（不破坏调用方）。
pub use crate::base::perspective::Perspective;

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
    /// （权威关键字 K7 / L12）。
    ///
    /// 依据：权威关键字 K7「距离我方出战角色最近的角色」——「对方场上距离我方出战角色
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
        /// 参照方：用这一方的出战角色位置作参照（K7 = Own）。
        reference: SideScope,
        /// 候选方：从这一方的角色里选最近的（K7 = Opponent）。
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

impl TargetSelector {
    /// 敌方出战角色（卡面最常见的目标）。
    pub const fn opponent_active() -> TargetSelector {
        TargetSelector::Active(SideScope::Opponent)
    }

    /// 我方出战角色。
    pub const fn own_active() -> TargetSelector {
        TargetSelector::Active(SideScope::Own)
    }

    /// 自己（效果主体）。
    pub const fn self_subject() -> TargetSelector {
        TargetSelector::Subject
    }

    /// K7：距离我方出战角色最近的敌方角色。
    pub const fn nearest_opponent_to_own_active() -> TargetSelector {
        TargetSelector::NearestToActive {
            reference: SideScope::Own,
            candidates: SideScope::Opponent,
        }
    }

    /// 所有敌方后台角色。
    pub const fn all_opponent_standby() -> TargetSelector {
        TargetSelector::Standby(SideScope::Opponent)
    }

    /// 所有敌方角色（出战 + 后台）。
    pub const fn all_opponent_characters() -> TargetSelector {
        TargetSelector::Characters(SideScope::Opponent)
    }

    /// 生命最低的敌方角色（= 生命值升序第 1 个；默认并列口径）。
    pub const fn lowest_hp_opponent() -> TargetSelector {
        TargetSelector::HpRank {
            side: SideScope::Opponent,
            order: HpOrder::Asc,
            n: Nat::new(1),
            tie: HpTie::DEFAULT,
        }
    }

    /// 生命最高的敌方角色（= 生命值降序第 1 个；默认并列口径）。
    pub const fn highest_hp_opponent() -> TargetSelector {
        TargetSelector::HpRank {
            side: SideScope::Opponent,
            order: HpOrder::Desc,
            n: Nat::new(1),
            tie: HpTie::DEFAULT,
        }
    }

    /// 按生命值排名第 n 个敌方角色（n 从 1 开始；默认并列口径）。
    pub const fn opponent_hp_rank(order: HpOrder, n: Nat) -> TargetSelector {
        TargetSelector::HpRank {
            side: SideScope::Opponent,
            order,
            n,
            tie: HpTie::DEFAULT,
        }
    }

    /// 受伤最多的敌方角色（= 受伤量降序第 1 个）。
    pub const fn most_lost_hp_opponent() -> TargetSelector {
        TargetSelector::LostHpRank {
            side: SideScope::Opponent,
            order: LostHpOrder::Most,
            n: Nat::new(1),
            tie: HpTie::DEFAULT,
        }
    }

    /// 受伤最少的敌方角色（= 受伤量升序第 1 个）。
    pub const fn least_lost_hp_opponent() -> TargetSelector {
        TargetSelector::LostHpRank {
            side: SideScope::Opponent,
            order: LostHpOrder::Least,
            n: Nat::new(1),
            tie: HpTie::DEFAULT,
        }
    }

    /// 按受伤量排名第 n 个敌方角色（默认并列口径；D237）。
    pub const fn opponent_lost_hp_rank(order: LostHpOrder, n: Nat) -> TargetSelector {
        TargetSelector::LostHpRank {
            side: SideScope::Opponent,
            order,
            n,
            tie: HpTie::DEFAULT,
        }
    }

    /// 按受伤量排名第 n 个敌方角色（显式并列规则；D237）。
    pub const fn opponent_lost_hp_rank_with(
        order: LostHpOrder,
        n: Nat,
        tie: HpTie,
    ) -> TargetSelector {
        TargetSelector::LostHpRank {
            side: SideScope::Opponent,
            order,
            n,
            tie,
        }
    }

    /// 按生命值排名第 n 个敌方角色（显式给并列规则；D229）。
    pub const fn opponent_hp_rank_with(order: HpOrder, n: Nat, tie: HpTie) -> TargetSelector {
        TargetSelector::HpRank {
            side: SideScope::Opponent,
            order,
            n,
            tie,
        }
    }

    /// 牌组顶的卡牌（我方）。
    pub const fn pile_top(side: SideScope) -> TargetSelector {
        TargetSelector::PileTop(side)
    }

    /// 出战角色之后第 n 个敌方角色（卡面「(下)(n)个角色」）。
    pub const fn opponent_after_active(offset: Nat) -> TargetSelector {
        TargetSelector::AfterActive {
            side: SideScope::Opponent,
            offset,
        }
    }
}

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

// ======================================================================
// 解析
// ======================================================================

/// 某一侧的区（只读指针）。
pub fn player_of(state: &DefState, side: Perspective) -> &PlayerDefs {
    match side {
        Perspective::Me => state.me(),
        Perspective::Opp => state.opp(),
    }
}

/// 把选择器**解析成具体实体集合**（纯读；不采样、不排序策略）。
pub fn resolve(state: &DefState, selector: TargetSelector, context: &ResolveContext) -> TargetSet {
    let mut set = TargetSet::new();
    match selector {
        TargetSelector::Subject => {
            if let Some(side) = locate(state, context.subject) {
                set.push(side, context.subject);
            }
        }
        TargetSelector::Reacted => {
            if let Some(reacted) = context.reacted {
                if let Some(side) = locate(state, reacted) {
                    set.push(side, reacted);
                }
            }
        }
        TargetSelector::Active(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                if let Some(active) = player_of(state, *side).characters().active_entity() {
                    set.push(*side, active);
                }
            }
        }
        TargetSelector::Characters(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                for slot in player_of(state, *side).characters().slots() {
                    set.push(*side, slot.character);
                }
            }
        }
        TargetSelector::Standby(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                let zone = player_of(state, *side).characters();
                for slot in zone.slots() {
                    if zone.active_entity() != Some(slot.character) {
                        set.push(*side, slot.character);
                    }
                }
            }
        }
        TargetSelector::NearestToActive {
            reference,
            candidates,
        } => {
            let (reference_sides, reference_count) = sides_of(reference, context.viewer);
            let (candidate_sides, candidate_count) = sides_of(candidates, context.viewer);
            for reference_side in reference_sides.iter().take(reference_count) {
                for candidate_side in candidate_sides.iter().take(candidate_count) {
                    if let Some(entity) =
                        nearest_to_active(state, *reference_side, *candidate_side)
                    {
                        set.push(*candidate_side, entity);
                    }
                }
            }
        }
        TargetSelector::AfterActive { side, offset } => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                if let Some(entity) = offset_character(state, *side, offset, true) {
                    set.push(*side, entity);
                }
            }
        }
        TargetSelector::BeforeActive { side, offset } => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                if let Some(entity) = offset_character(state, *side, offset, false) {
                    set.push(*side, entity);
                }
            }
        }
        TargetSelector::HpRank {
            side,
            order,
            n,
            tie,
        } => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                if let Some(entity) =
                    rank_characters(state, *side, order, current_hp, n, tie)
                {
                    set.push(*side, entity);
                }
            }
        }
        TargetSelector::LostHpRank {
            side,
            order,
            n,
            tie,
        } => {
            let direction = match order {
                LostHpOrder::Most => HpOrder::Desc,
                LostHpOrder::Least => HpOrder::Asc,
            };
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                if let Some(entity) =
                    rank_characters(state, *side, direction, lost_hp, n, tie)
                {
                    set.push(*side, entity);
                }
            }
        }
        TargetSelector::AllExceptReacted(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                for slot in player_of(state, *side).characters().slots() {
                    if Some(slot.character) != context.reacted {
                        set.push(*side, slot.character);
                    }
                }
            }
        }
        TargetSelector::HandCards(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                for instance in player_of(state, *side).hand().entries() {
                    set.push(*side, instance.entity);
                }
            }
        }
        TargetSelector::HandCardIndex { side, index } => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                if let Some(instance) = player_of(state, *side)
                    .hand()
                    .entries()
                    .get(index.get() as usize)
                {
                    set.push(*side, instance.entity);
                }
            }
        }
        TargetSelector::Summons(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                for entry in player_of(state, *side).summons().entries() {
                    set.push(*side, entry.summon);
                }
            }
        }
        TargetSelector::Supports(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                for entry in player_of(state, *side).supports().entries() {
                    set.push(*side, entry.entity);
                }
            }
        }
        TargetSelector::Equipments(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                for zone in player_of(state, *side).equipment() {
                    for entry in zone.entries() {
                        set.push(*side, entry.equip);
                    }
                }
            }
        }
        TargetSelector::PileTop(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                if let Some(entry) = player_of(state, *side).pile().entry(Nat::ZERO) {
                    set.push(*side, entry.entity);
                }
            }
        }
        TargetSelector::PileCards(side) => {
            let (sides, count) = sides_of(side, context.viewer);
            for side in sides.iter().take(count) {
                for entry in player_of(state, *side).pile().entries() {
                    set.push(*side, entry.entity);
                }
            }
        }
    }
    set
}

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

/// 在双方区里找某个实体属于哪一侧（角色 / 手牌 / 召唤物 / 支援 / 装备）。
pub fn locate(state: &DefState, entity: EntityId) -> Option<Perspective> {
    for side in [Perspective::Me, Perspective::Opp] {
        let player = player_of(state, side);
        let in_equipment = player
            .equipment()
            .iter()
            .any(|zone| zone.entries().iter().any(|entry| entry.equip == entity));
        if player.characters().contains(entity)
            || player.hand().contains(entity)
            || player.summons().contains(entity)
            || player.supports().contains(entity)
            || in_equipment
        {
            return Some(side);
        }
    }
    None
}

/// **距离最近**（K7 / L12）：从 reference 方出战角色的槽位下标出发，选 candidates 方
/// 槽位下标**绝对差最小**的角色；并列取槽位更靠前（下标更小）的那个。
///
/// 距离口径与并列都是 K7 字面口径（规则书无此词条 → TODO）；
/// 被击倒的角色不参与（「对方场上」，当前按 hp == 0 判定，**待实测**）。
fn nearest_to_active(
    state: &DefState,
    reference: Perspective,
    candidates: Perspective,
) -> Option<EntityId> {
    let reference_zone = player_of(state, reference).characters();
    let active = reference_zone.active_entity()?;
    let active_index = reference_zone
        .slots()
        .iter()
        .position(|slot| slot.character == active)?;
    let reference_position = Nat::new(active_index as _);
    let candidate_zone = player_of(state, candidates).characters();
    let mut best: Option<(Nat, EntityId)> = None;
    for (index, slot) in candidate_zone.slots().iter().enumerate() {
        if is_defeated(&slot.state) {
            continue;
        }
        let position = Nat::new(index as _);
        let distance = if reference_position >= position {
            nsub!(reference_position, position)
        } else {
            nsub!(position, reference_position)
        };
        match best {
            None => best = Some((distance, slot.character)),
            // 只有「严格更近」才替换 -> 并列时保留先出现的（槽位靠前）的那个。
            Some((best_distance, _)) if distance < best_distance => {
                best = Some((distance, slot.character));
            }
            Some(_) => {}
        }
    }
    best.map(|(_, entity)| entity)
}

/// 出战角色之后 / 之前第 offset 个角色（在角色区槽位顺序上循环）。**待实测**。
fn offset_character(
    state: &DefState,
    side: Perspective,
    offset: Nat,
    forward: bool,
) -> Option<EntityId> {
    let zone = player_of(state, side).characters();
    let slots = zone.slots();
    if slots.is_empty() {
        return None;
    }
    let active = zone.active_entity()?;
    let start = slots.iter().position(|slot| slot.character == active)?;
    let len = Nat::new(slots.len() as _);
    let mut index = Nat::new(start as _);
    let mut steps = Nat::ZERO;
    while steps < offset {
        index = if forward {
            step_forward(index, len)
        } else {
            step_backward(index, len)
        };
        steps = nadd!(steps, Nat::new(1));
    }
    slots.get(index.get() as usize).map(|slot| slot.character)
}

/// 环形前进一格。
fn step_forward(index: Nat, len: Nat) -> Nat {
    let next = nadd!(index, Nat::new(1));
    if next >= len {
        Nat::ZERO
    } else {
        next
    }
}

/// 环形后退一格。
fn step_backward(index: Nat, len: Nat) -> Nat {
    if index == Nat::ZERO {
        nsub!(len, Nat::new(1))
    } else {
        nsub!(index, Nat::new(1))
    }
}

/// 某一方按当前生命值排名的第 n 个角色（n 从 1 开始；越界 -> None）。
///
/// **当前生命值**（HpRank 的排名指标）。
fn current_hp(state: &CharacterState) -> Nat {
    state.hp()
}

/// **受伤量** = **实际最大生命值 − 当前生命值**（LostHpRank 的排名指标）。
///
/// 只用运行期 CharacterState::max_hp（= 实际最大生命值；成长 / 装备经
/// growth::GrowthList::apply_to_character 写进它）。**禁止**用 CharacterDef::max_hp
/// （卡牌定义里的固定值，不随成长 / 装备变化）。D237。
fn lost_hp(state: &CharacterState) -> Nat {
    nsub!(state.max_hp(), state.hp())
}

/// 角色是否**已被击倒**（D237：两类排名都要跳过被击倒角色）。
///
/// **待实测 / 待接入**：CharacterState 没有独立的击倒标记，这里以「当前生命值 == 0」
/// 判定；免于被击倒期间（治疗前）的 0 血角色会被暂时排除，需要独立标记后修正。
fn is_defeated(state: &CharacterState) -> bool {
    state.hp() == Nat::ZERO
}

/// 按指标排名取第 n 个角色（n 从 1 开始；越界 / 0 / 全部被击倒 -> None）。
///
/// 比较顺序**固定三级**（D230：出战角色优先先于左右优先）：
/// 1. 指标值（order：降序 / 升序；HpRank 用当前生命值，LostHpRank 用受伤量）；
/// 2. 是否出战角色（tie.active_first：出战排在同指标值其它角色之前）；
/// 3. 槽位方向（tie.direction：从左到右 / 从右到左）。
///
/// 被击倒的角色**先被过滤掉**，不参与排名（D237）。
fn rank_characters(
    state: &DefState,
    side: Perspective,
    order: HpOrder,
    metric: fn(&CharacterState) -> Nat,
    n: Nat,
    tie: HpTie,
) -> Option<EntityId> {
    use std::cmp::Ordering;
    if n == Nat::ZERO {
        return None;
    }
    let zone = player_of(state, side).characters();
    let active = zone.active_entity();
    let slots = zone.slots();
    let len = Nat::new(slots.len() as _);
    let mut ranked: Vec<(EntityId, Nat, bool, Nat)> = slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| !is_defeated(&slot.state))
        .map(|(index, slot)| {
            let position = Nat::new(index as _);
            let directed = match tie.direction {
                SlotOrder::LeftToRight => position,
                SlotOrder::RightToLeft => nsub!(nsub!(len, Nat::new(1)), position),
            };
            (
                slot.character,
                metric(&slot.state),
                Some(slot.character) == active,
                directed,
            )
        })
        .collect();
    ranked.sort_by(|left, right| {
        let by_metric = match order {
            HpOrder::Desc => right.1.cmp(&left.1),
            HpOrder::Asc => left.1.cmp(&right.1),
        };
        if by_metric != Ordering::Equal {
            return by_metric;
        }
        if tie.active_first {
            let by_active = right.2.cmp(&left.2);
            if by_active != Ordering::Equal {
                return by_active;
            }
        }
        left.3.cmp(&right.3)
    });
    ranked
        .get(nsub!(n, Nat::new(1)).get() as usize)
        .map(|entry| entry.0)
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::character::character::CharacterState;
    use crate::families::character::growth::{Growth, GrowthAttribute, GrowthList};
    use crate::families::character::zones::field::{CharacterSlot, CharacterZone};
    use crate::state::testing::{characters, hand, n, summons};
    use crate::state::PlayerDefs;

    /// 双方各 3 个角色；我方手牌 2 张、敌方 1 张；敌方召唤物 1 个。
    fn sample_state() -> DefState {
        let me = PlayerDefs::new()
            .with_characters(characters(&[101, 102, 103]))
            .with_hand(hand(&[201, 202]));
        let opp = PlayerDefs::new()
            .with_characters(characters(&[301, 302, 303]))
            .with_hand(hand(&[401]))
            .with_summons(summons(&[(501, 2)]));
        DefState::from_players(me, opp)
    }

    /// 用指定生命值建角色区（第 1 个是出战角色）。
    fn with_hps(hps: &[u32]) -> CharacterZone {
        let slots: Vec<CharacterSlot> = hps
            .iter()
            .enumerate()
            .map(|(index, hp)| {
                CharacterSlot::new(
                    nadd!(n(index as u32), n(1)),
                    CharacterState::new(n(*hp), n(10), Nat::ZERO, n(2)),
                )
            })
            .collect();
        CharacterZone::try_from_slots(slots).expect("合法角色区")
    }

    /// 用 (当前生命, 生命上限) 建角色区（第 1 个是出战角色；上限 10 以外的用例用）。
    fn with_stats(stats: &[(u32, u32)]) -> CharacterZone {
        let slots: Vec<CharacterSlot> = stats
            .iter()
            .enumerate()
            .map(|(index, (hp, max_hp))| {
                CharacterSlot::new(
                    nadd!(n(index as u32), n(1)),
                    CharacterState::new(n(*hp), n(*max_hp), Nat::ZERO, n(2)),
                )
            })
            .collect();
        CharacterZone::try_from_slots(slots).expect("合法角色区")
    }

    fn context(subject: EntityId, viewer: Perspective) -> ResolveContext {
        ResolveContext::new(viewer, subject)
    }

    #[test]
    fn active_standby_and_all_characters() {
        let state = sample_state();
        let ctx = context(n(101), Perspective::Me);
        let active = resolve(&state, TargetSelector::opponent_active(), &ctx);
        assert_eq!(active.opp(), &[n(301)]);
        assert!(active.me().is_empty());
        let standby = resolve(&state, TargetSelector::all_opponent_standby(), &ctx);
        assert_eq!(standby.opp(), &[n(302), n(303)]);
        let all = resolve(&state, TargetSelector::all_opponent_characters(), &ctx);
        assert_eq!(all.opp(), &[n(301), n(302), n(303)]);
        // 我方视角 / 敌方视角：同样的选择器按视角解析到不同的一侧。
        let own = resolve(&state, TargetSelector::own_active(), &ctx);
        assert_eq!(own.me(), &[n(101)]);
        let opp_viewer = context(n(301), Perspective::Opp);
        let own_from_opp = resolve(&state, TargetSelector::own_active(), &opp_viewer);
        assert_eq!(own_from_opp.opp(), &[n(301)]);
    }

    #[test]
    fn both_side_scope_fills_two_named_sets() {
        let state = sample_state();
        let ctx = context(n(101), Perspective::Me);
        let both = resolve(&state, TargetSelector::Characters(SideScope::Both), &ctx);
        assert_eq!(both.me(), &[n(101), n(102), n(103)]);
        assert_eq!(both.opp(), &[n(301), n(302), n(303)]);
        assert_eq!(both.len(), n(6));
        assert!(both.contains(n(302)));
    }

    #[test]
    fn after_and_before_active_cycle_in_slot_order() {
        let state = sample_state();
        let ctx = context(n(101), Perspective::Me);
        let next = resolve(&state, TargetSelector::opponent_after_active(n(1)), &ctx);
        assert_eq!(next.opp(), &[n(302)]);
        let next_two = resolve(
            &state,
            TargetSelector::AfterActive {
                side: SideScope::Opponent,
                offset: n(2),
            },
            &ctx,
        );
        assert_eq!(next_two.opp(), &[n(303)]);
        // 循环：第 3 个回到出战角色。
        let wrapped = resolve(
            &state,
            TargetSelector::AfterActive {
                side: SideScope::Opponent,
                offset: n(3),
            },
            &ctx,
        );
        assert_eq!(wrapped.opp(), &[n(301)]);
        let before = resolve(
            &state,
            TargetSelector::BeforeActive {
                side: SideScope::Opponent,
                offset: n(1),
            },
            &ctx,
        );
        assert_eq!(before.opp(), &[n(303)]);
    }

    #[test]
    fn nearest_to_active_uses_slot_distance() {
        // 我方出战在中间（下标 1）；敌方三人在 0/1/2 -> 最近 = 下标 1 的 302。
        let me = PlayerDefs::new().with_characters(
            characters(&[101, 102, 103])
                .set_active(n(102))
                .expect("102 在角色区"),
        );
        let opp = PlayerDefs::new().with_characters(characters(&[301, 302, 303]));
        let state = DefState::from_players(me, opp);
        let ctx = context(n(102), Perspective::Me);
        assert_eq!(
            resolve(
                &state,
                TargetSelector::nearest_opponent_to_own_active(),
                &ctx,
            )
            .opp(),
            &[n(302)]
        );
        // 我方出战在最左（下标 0）-> 敌方最左（下标 0）最近。
        let me = PlayerDefs::new().with_characters(characters(&[101, 102, 103]));
        let state = DefState::from_players(
            me,
            PlayerDefs::new().with_characters(characters(&[301, 302, 303])),
        );
        let ctx = context(n(101), Perspective::Me);
        assert_eq!(
            resolve(
                &state,
                TargetSelector::nearest_opponent_to_own_active(),
                &ctx,
            )
            .opp(),
            &[n(301)]
        );
    }

    #[test]
    fn nearest_to_active_tie_breaks_to_foremost() {
        // 我方出战在下标 1；敌方中间角色被击倒（0 血）-> 剩下下标 0 / 2 距离都是 1，
        // 并列按 K7「靠前的角色视为最近」-> 取下标 0（实体 1）。
        let me = PlayerDefs::new().with_characters(
            characters(&[101, 102, 103])
                .set_active(n(102))
                .expect("102 在角色区"),
        );
        let opp = PlayerDefs::new().with_characters(with_stats(&[(10, 10), (0, 10), (10, 10)]));
        let state = DefState::from_players(me, opp);
        let ctx = context(n(102), Perspective::Me);
        assert_eq!(
            resolve(
                &state,
                TargetSelector::nearest_opponent_to_own_active(),
                &ctx,
            )
            .opp(),
            &[n(1)],
            "并列取靠前（下标小）的角色"
        );
    }

    #[test]
    fn nearest_to_active_is_empty_without_reference_or_candidates() {
        // 没有参照方出战角色 -> 空。
        let state = DefState::from_players(
            PlayerDefs::new(),
            PlayerDefs::new().with_characters(characters(&[301, 302])),
        );
        let ctx = context(n(1), Perspective::Me);
        assert!(resolve(
            &state,
            TargetSelector::nearest_opponent_to_own_active(),
            &ctx,
        )
        .is_empty());
        // 没有候选方角色 -> 空。
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101, 102])),
            PlayerDefs::new(),
        );
        let ctx = context(n(101), Perspective::Me);
        assert!(resolve(
            &state,
            TargetSelector::nearest_opponent_to_own_active(),
            &ctx,
        )
        .is_empty());
    }

    #[test]
    fn hp_rank_takes_the_nth_by_current_hp_with_stable_ties() {
        // 生命值 [10, 5, 5]；槽位顺序 = 出战 1, 后台 2, 后台 3。
        let me = PlayerDefs::new()
            .with_characters(characters(&[101]))
            .with_hand(hand(&[201]));
        let opp = PlayerDefs::new().with_characters(with_hps(&[10, 5, 5]));
        let state = DefState::from_players(me, opp);
        let ctx = context(n(101), Perspective::Me);
        let rank = |order: HpOrder, n_value: Nat| {
            resolve(
                &state,
                TargetSelector::HpRank {
                    side: SideScope::Opponent,
                    order,
                    n: n_value,
                    tie: HpTie::DEFAULT,
                },
                &ctx,
            )
        };
        // 降序第 1 / 2 / 3 高。（并列 5/5 按槽位顺序：2 在 3 前）
        assert_eq!(rank(HpOrder::Desc, n(1)).opp(), &[n(1)]);
        assert_eq!(rank(HpOrder::Desc, n(2)).opp(), &[n(2)]);
        assert_eq!(rank(HpOrder::Desc, n(3)).opp(), &[n(3)]);
        // 升序第 1 / 2 低。
        assert_eq!(rank(HpOrder::Asc, n(1)).opp(), &[n(2)]);
        assert_eq!(rank(HpOrder::Asc, n(2)).opp(), &[n(3)]);
        assert_eq!(rank(HpOrder::Asc, n(3)).opp(), &[n(1)]);
        // 越界 / n = 0 -> 空集合（不 panic）。
        assert!(rank(HpOrder::Desc, n(4)).is_empty());
        assert!(rank(HpOrder::Asc, Nat::ZERO).is_empty());
        // 极值 = 第 1 个（旧写法的等价表达）。
        assert_eq!(
            resolve(&state, TargetSelector::lowest_hp_opponent(), &ctx).opp(),
            &[n(2)]
        );
        assert_eq!(
            resolve(&state, TargetSelector::highest_hp_opponent(), &ctx).opp(),
            &[n(1)]
        );
    }

    #[test]
    fn hp_rank_tie_breaks_by_active_then_direction() {
        // 槽位 [1,2,3]，出战 = 2；三人同血 7。
        let opp_zone = with_hps(&[7, 7, 7]).set_active(n(2)).expect("2 在角色区");
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(opp_zone),
        );
        let ctx = context(n(101), Perspective::Me);
        let rank = |tie: HpTie, n_value: Nat| {
            resolve(
                &state,
                TargetSelector::HpRank {
                    side: SideScope::Opponent,
                    order: HpOrder::Desc,
                    n: n_value,
                    tie,
                },
                &ctx,
            )
        };
        // ① 同血量、不同出战态：出战优先压过左右优先（D230 第 2 级先于第 3 级）。
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(1)).opp(),
            &[n(2)]
        );
        assert_eq!(
            rank(HpTie::new(false, SlotOrder::LeftToRight), n(1)).opp(),
            &[n(1)]
        );
        // ② 同血量、同出战态（出战已排第一之后）、不同方向。
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(2)).opp(),
            &[n(1)]
        );
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::RightToLeft), n(2)).opp(),
            &[n(3)]
        );
        // ③ 综合：出战 -> 方向（三级顺序的完整投影）。
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(1)).opp(),
            &[n(2)]
        );
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(2)).opp(),
            &[n(1)]
        );
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(3)).opp(),
            &[n(3)]
        );
        assert!(rank(HpTie::DEFAULT, n(4)).is_empty());
        // 第 1 级（生命值）压过出战优先：出战 2 只有 7 血、1 有 10 血 -> 出战也不是第一。
        let opp_zone = with_hps(&[10, 7, 7]).set_active(n(2)).expect("2 在角色区");
        let higher_hp = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(opp_zone),
        );
        let ranked = resolve(
            &higher_hp,
            TargetSelector::HpRank {
                side: SideScope::Opponent,
                order: HpOrder::Desc,
                n: n(1),
                tie: HpTie::new(true, SlotOrder::LeftToRight),
            },
            &ctx,
        );
        assert_eq!(ranked.opp(), &[n(1)]);
    }

    #[test]
    fn hp_rank_handles_single_and_empty_zones() {
        let single = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(with_hps(&[7])),
        );
        let ctx = context(n(101), Perspective::Me);
        let rank = |state: &DefState, order: HpOrder, n_value: Nat| {
            resolve(
                state,
                TargetSelector::HpRank {
                    side: SideScope::Opponent,
                    order,
                    n: n_value,
                    tie: HpTie::DEFAULT,
                },
                &ctx,
            )
        };
        assert_eq!(rank(&single, HpOrder::Desc, n(1)).opp(), &[n(1)]);
        assert!(rank(&single, HpOrder::Asc, n(2)).is_empty());
        // 空区。
        let empty = DefState::new();
        assert!(rank(&empty, HpOrder::Desc, n(1)).is_empty());
        assert!(rank(&empty, HpOrder::Asc, n(1)).is_empty());
    }

    #[test]
    fn subject_is_resolved_from_context_not_written_in_code() {
        let state = sample_state();
        // 上下文换主体 -> 结果跟着变（D206）。
        let first = resolve(
            &state,
            TargetSelector::Subject,
            &context(n(101), Perspective::Me),
        );
        assert_eq!(first.me(), &[n(101)]);
        let second = resolve(
            &state,
            TargetSelector::Subject,
            &context(n(202), Perspective::Me),
        );
        assert_eq!(second.me(), &[n(202)]);
        // 主体在敌方召唤物区也能定位。
        let summon = resolve(
            &state,
            TargetSelector::Subject,
            &context(n(501), Perspective::Me),
        );
        assert_eq!(summon.opp(), &[n(501)]);
    }

    #[test]
    fn hand_and_pile_selectors() {
        let state = sample_state();
        let ctx = context(n(101), Perspective::Me);
        let hand_cards = resolve(&state, TargetSelector::HandCards(SideScope::Own), &ctx);
        assert_eq!(hand_cards.me(), &[n(201), n(202)]);
        let second = resolve(
            &state,
            TargetSelector::HandCardIndex {
                side: SideScope::Own,
                index: n(1),
            },
            &ctx,
        );
        assert_eq!(second.me(), &[n(202)]);
        let out_of_range = resolve(
            &state,
            TargetSelector::HandCardIndex {
                side: SideScope::Own,
                index: n(10),
            },
            &ctx,
        );
        assert!(out_of_range.is_empty());
        let top = resolve(&state, TargetSelector::PileTop(SideScope::Own), &ctx);
        assert!(top.is_empty(), "空牌库 -> 空集");
    }

    #[test]
    fn reaction_and_equipment_selectors() {
        let state = sample_state();
        let ctx = context(n(101), Perspective::Me).with_reacted(Some(n(302)));
        let reacted = resolve(&state, TargetSelector::Reacted, &ctx);
        assert_eq!(reacted.opp(), &[n(302)]);
        let others = resolve(
            &state,
            TargetSelector::AllExceptReacted(SideScope::Opponent),
            &ctx,
        );
        assert_eq!(others.opp(), &[n(301), n(303)]);
        let summons_set = resolve(&state, TargetSelector::Summons(SideScope::Opponent), &ctx);
        assert_eq!(summons_set.opp(), &[n(501)]);
        let equipment = resolve(&state, TargetSelector::Equipments(SideScope::Both), &ctx);
        assert!(equipment.is_empty());
    }

    #[test]
    fn lost_hp_rank_uses_actual_max_hp_minus_current_hp() {
        // 上限都是 10：生命 [10, 5, 5] -> 受伤量 [0, 5, 5]。
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(with_hps(&[10, 5, 5])),
        );
        let ctx = context(n(101), Perspective::Me);
        let rank = |order: LostHpOrder, n_value: Nat| {
            resolve(
                &state,
                TargetSelector::LostHpRank {
                    side: SideScope::Opponent,
                    order,
                    n: n_value,
                    tie: HpTie::DEFAULT,
                },
                &ctx,
            )
        };
        // 受伤最多：第 1 个 = 5（并列按槽位：2 在 3 前）；第 2 个 = 3；第 3 个 = 1。
        assert_eq!(rank(LostHpOrder::Most, n(1)).opp(), &[n(2)]);
        assert_eq!(rank(LostHpOrder::Most, n(2)).opp(), &[n(3)]);
        assert_eq!(rank(LostHpOrder::Most, n(3)).opp(), &[n(1)]);
        // 受伤最少：第 1 个 = 1；第 2 个 = 2。
        assert_eq!(rank(LostHpOrder::Least, n(1)).opp(), &[n(1)]);
        assert_eq!(rank(LostHpOrder::Least, n(2)).opp(), &[n(2)]);
        // 越界 / 0 -> 空集合。
        assert!(rank(LostHpOrder::Most, n(4)).is_empty());
        assert!(rank(LostHpOrder::Least, Nat::ZERO).is_empty());
        // 便捷构造：受伤最多 / 最少。
        assert_eq!(
            resolve(&state, TargetSelector::most_lost_hp_opponent(), &ctx).opp(),
            &[n(2)]
        );
        assert_eq!(
            resolve(&state, TargetSelector::least_lost_hp_opponent(), &ctx).opp(),
            &[n(1)]
        );
    }

    #[test]
    fn lost_hp_rank_ties_follow_the_same_three_levels() {
        // 三个角色受伤量都是 0：1 = 10/10（出战）、2 = 7/7、3 = 7/7。
        // 三级顺序：受伤量相同 -> 出战 1 优先 -> 其余按方向（2/3）。
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(with_stats(&[(10, 10), (7, 7), (7, 7)])),
        );
        let ctx = context(n(101), Perspective::Me);
        let rank = |tie: HpTie, n_value: Nat| {
            resolve(
                &state,
                TargetSelector::LostHpRank {
                    side: SideScope::Opponent,
                    order: LostHpOrder::Least,
                    n: n_value,
                    tie,
                },
                &ctx,
            )
        };
        // 受伤量相同：出战 1 优先 -> 其余按方向（LTR: 2,3；RTL: 3,2）。
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(1)).opp(),
            &[n(1)]
        );
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(2)).opp(),
            &[n(2)]
        );
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::LeftToRight), n(3)).opp(),
            &[n(3)]
        );
        assert_eq!(
            rank(HpTie::new(true, SlotOrder::RightToLeft), n(2)).opp(),
            &[n(3)]
        );
        // 出战不优先 + 从右到左：最右的 3 先。
        assert_eq!(
            rank(HpTie::new(false, SlotOrder::RightToLeft), n(1)).opp(),
            &[n(3)]
        );
    }

    #[test]
    fn lost_hp_rank_uses_runtime_max_hp_not_definition() {
        // 角色 1 用成长把实际生命上限从 10 提到 15（当前 10 -> 受伤 5）；
        // 角色 2 没成长（当前 10 / 上限 10 -> 受伤 0）。
        let base = CharacterState::new(n(10), n(10), Nat::ZERO, n(2));
        let grown = GrowthList::new()
            .with(Growth::increase(GrowthAttribute::MaxHp, n(5)))
            .apply_to_character(&base);
        let zone = CharacterZone::try_from_slots(vec![
            CharacterSlot::new(n(1), grown),
            CharacterSlot::new(n(2), base),
        ])
        .expect("合法角色区");
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(zone),
        );
        let ctx = context(n(101), Perspective::Me);
        // 用实际上限：受伤最多 = 1（受伤 5）。
        assert_eq!(
            resolve(&state, TargetSelector::most_lost_hp_opponent(), &ctx).opp(),
            &[n(1)]
        );
        // 受伤最少 = 2（受伤 0）。若误用卡牌定义上限 10，两人的受伤都会算成 0，
        // 并列时出战优先（1 是出战）会把 1 选中 -> 这一条会失败，钉死「用实际上限」。
        assert_eq!(
            resolve(&state, TargetSelector::least_lost_hp_opponent(), &ctx).opp(),
            &[n(2)]
        );
    }

    #[test]
    fn defeated_characters_are_excluded_from_hp_and_lost_hp_rankings() {
        // 1 被击倒（0/10，受伤 10）；2 满血（10/10，受伤 0）；3 半血（5/10，受伤 5）。
        let state = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(with_hps(&[0, 10, 5])),
        );
        let ctx = context(n(101), Perspective::Me);
        let hp_rank = |order: HpOrder, n_value: Nat| {
            resolve(
                &state,
                TargetSelector::HpRank {
                    side: SideScope::Opponent,
                    order,
                    n: n_value,
                    tie: HpTie::DEFAULT,
                },
                &ctx,
            )
        };
        // 被击倒的 1 不参与：最高 = 2，最低 = 3；只剩两个可排名。
        assert_eq!(hp_rank(HpOrder::Desc, n(1)).opp(), &[n(2)]);
        assert_eq!(hp_rank(HpOrder::Asc, n(1)).opp(), &[n(3)]);
        assert!(hp_rank(HpOrder::Desc, n(3)).is_empty());
        let lost_rank = |order: LostHpOrder, n_value: Nat| {
            resolve(
                &state,
                TargetSelector::LostHpRank {
                    side: SideScope::Opponent,
                    order,
                    n: n_value,
                    tie: HpTie::DEFAULT,
                },
                &ctx,
            )
        };
        // 受伤最多本来是 1（0 血 = 受伤 10），但它被击倒 -> 3（受伤 5）。
        assert_eq!(lost_rank(LostHpOrder::Most, n(1)).opp(), &[n(3)]);
        assert_eq!(lost_rank(LostHpOrder::Least, n(1)).opp(), &[n(2)]);
        assert!(lost_rank(LostHpOrder::Most, n(3)).is_empty());
        // 全被击倒 -> 空集合（不 panic）。
        let all_down = DefState::from_players(
            PlayerDefs::new().with_characters(characters(&[101])),
            PlayerDefs::new().with_characters(with_hps(&[0, 0])),
        );
        assert!(resolve(&all_down, TargetSelector::highest_hp_opponent(), &ctx).is_empty());
        assert!(resolve(&all_down, TargetSelector::most_lost_hp_opponent(), &ctx).is_empty());
    }

    #[test]
    fn empty_zones_resolve_to_empty_sets() {
        let state = DefState::new();
        let ctx = context(n(1), Perspective::Me);
        for selector in [
            TargetSelector::opponent_active(),
            TargetSelector::all_opponent_characters(),
            TargetSelector::all_opponent_standby(),
            TargetSelector::lowest_hp_opponent(),
            TargetSelector::highest_hp_opponent(),
            TargetSelector::most_lost_hp_opponent(),
            TargetSelector::least_lost_hp_opponent(),
            TargetSelector::nearest_opponent_to_own_active(),
            TargetSelector::HandCards(SideScope::Both),
            TargetSelector::PileTop(SideScope::Both),
            TargetSelector::Summons(SideScope::Both),
            TargetSelector::Supports(SideScope::Both),
            TargetSelector::Equipments(SideScope::Both),
            TargetSelector::Subject,
            TargetSelector::Reacted,
        ] {
            assert!(resolve(&state, selector, &ctx).is_empty(), "{selector:?}");
            let both = resolve(&state, selector, &ResolveContext::new(Perspective::Opp, n(1)));
            assert!(both.is_empty(), "{selector:?}");
        }
    }
}
