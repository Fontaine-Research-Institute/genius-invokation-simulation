//! **关键字操作**的数据表达 + 关键字函数的统一返回类型。
//!
//! 一个关键字一个函数（见 [super::keyword]），参数是这里的 Args 结构体；
//! 多个关键字操作组成一步（见 [super::step::Step]）。
//!
//! ## 口径
//!
//! - 关键字函数是**纯函数**：吃 &DefState + &EffectEnv + &Args，返回 [KeywordResult]；
//! - 返回的 [KeywordBranch] = 局面 + 精确概率 + 待执行（D190：不采样）；
//! - 待执行只是**派发结果**，本层不执行（D186）；执行由步末统一做（D204）；
//! - 实体号（自己 / 目标）**不在 Args 里手写**（D206）：目标一律是 [TargetSelector]。

use crate::base::{CardId, Element};
use crate::families::character::damage_chain::bonus::DamageModifiers;
use crate::families::character::damage_chain::executor::EntityAlloc;
use crate::families::character::types::DamageType;
use crate::families::summon::def::SummonEffectKind;
use crate::flow::pending::PendingSet;
use crate::reactions::effect::HandCardStatus;
use crate::resources::dice::DieFace;
use crate::state::DefState;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::Nat;

use super::target::{SideScope, TargetSelector};

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

/// **治疗**的参数。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HealArgs {
    /// 目标选择器（角色）。
    pub targets: TargetSelector,
    /// 治疗量。
    pub amount: Nat,
}

impl HealArgs {
    /// 建治疗参数。
    pub const fn new(targets: TargetSelector, amount: Nat) -> HealArgs {
        HealArgs { targets, amount }
    }
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

/// **抽几张**的表达（有动态量：沙中遗事抓「原手牌数 + 1」）。
///
/// 动态度在**一步开始时**取（D204：一步是原子结算单位），所以「先把手牌放回牌组、
/// 再抓原手牌数 + 1」这种写法不会因为手牌已经离手而算错。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawCount {
    /// 固定张数。
    Fixed(Nat),
    /// **本步开始时**那一方手牌数 + 偏移（沙中遗事 = +1）。
    HandAtStepStartPlus(Nat),
}

impl DrawCount {
    /// 固定一张。
    pub const fn one() -> DrawCount {
        DrawCount::Fixed(Nat::new(1))
    }
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

impl PartialEq for RepeatArgs {
    /// 函数指针按地址比较（derive 会对函数指针直接 == 并触发
    /// unpredictable_function_pointer_comparisons 警告，所以手写）。
    fn eq(&self, other: &RepeatArgs) -> bool {
        self.count == other.count && self.plan as usize == other.plan as usize
    }
}

impl Eq for RepeatArgs {}

impl RepeatArgs {
    /// 按序号生成操作的重复（D209：「序号 -> 关键字」）。
    pub const fn indexed(count: Nat, plan: RepeatOpFn) -> RepeatArgs {
        RepeatArgs { count, plan }
    }
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

impl PartialEq for PutHandToPileBottomArgs {
    fn eq(&self, other: &PutHandToPileBottomArgs) -> bool {
        self.side == other.side && self.order as usize == other.order as usize
    }
}

impl Eq for PutHandToPileBottomArgs {}

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

impl PutHandToPileBottomArgs {
    /// 用默认顺序建参数。
    pub const fn new(side: SideScope) -> PutHandToPileBottomArgs {
        PutHandToPileBottomArgs {
            side,
            order: default_put_bottom_order,
        }
    }
}

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

impl KeywordBranch {
    /// 带上待执行（链式构造）。
    pub fn with_pending(mut self, pending: PendingSet) -> KeywordBranch {
        self.pending = pending;
        self
    }
}
