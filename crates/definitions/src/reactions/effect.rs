//! 反应效果的**表达类型**（纯数据 + 纯函数；D183 / D190）。
//!
//! 口径：
//! - 效果函数是**函数指针**（D186），签名见 [`ReactionEffectFn`]：吃反应事件 + 局面摘要，
//!   返回 [`ReactionEffectResult`]；**不执行、不采样、不持有状态**；
//! - [`ReactionEffectResult`] = 「本伤害加成」+ 若干「分支（底层操作清单 + 精确概率）」；
//! - 每个 [`EffectOp`] 都**一一对应一个已有底层操作**（伤害 / 附着 / 状态 / 召唤 / 手牌），
//!   由执行器（待接入）转调；本模块**不自己结算**；
//! - 需要随机的效果（月感电 / 月绽放）在**观测点**展开成「具体分支 + `Fraction` 精确概率」，
//!   **不采样**（D190）；展开是临时的（D191：结果只作为一次结算的待执行清单，不写进任何状态）；
//! - 反应对元素附着的消耗**不在这里**：由伤害入口按 D167 完成（本模块不扣附着）。
//!
//! 「我方 / 敌方」的写法（规则书从**造成伤害的一方**视角写）用 [`Side`] 表达为
//! 「触发方 / 承载方」的相对关系 —— 引擎不分敌我（D29 / D34），具体两侧由调用方绑定。
//!
//! **迁移桥接（D208）**：每个 [`EffectOp`] 等价的「关键字函数调用」在
//! `crate::keywords::migrate`（`EffectOp::to_keyword_op` / `step_from_reaction_ops`）；
//! 一条反应分支的多个操作属于**一步**。本模块行为保持不变（既有测试全绿）。

use crate::base::CardId;
use crate::families::character::types::DamageType;
use gi_genius_invokation_engine::nat::ExactNat128;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::Nat;

/// **相对侧**（不是敌我枚举，D29 / D34）：由调用方把「触发方 / 承载方」映射到自己的两个具名实例。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Side {
    /// 触发方（造成这次元素伤害、引发反应的一方；规则书里的「我方」）。
    Source,
    /// 承载方（身上有附着、这次反应作用在其角色身上的一方；规则书里的「敌方」）。
    Target,
}

/// 目标范围（相对 [`Side`] / 反应事件的实体）。
///
/// 每个变体都能由 `ReactionEvent` 的 `source` / `target` + 双方队伍信息唯一解析，
/// 所以效果函数不需要完整的战场状态（纯函数、可测试）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TargetScope {
    /// 承载反应的那个角色（`ReactionEvent::target`）。
    Reacted,
    /// 触发方的出战角色。
    SourceActive,
    /// 承载方的出战角色。
    TargetActive,
    /// 承载方**除承载角色外**的全部角色（规则书「目标以外的所有敌方角色」）。
    TargetOthers,
}

/// 状态的挂载位置（状态牌族只有「角色状态 / 出战状态」两种，见 `families::status`）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StatusTarget {
    /// 角色状态：挂在某个角色身上（冻结 / 护盾）。
    Character(TargetScope),
    /// 出战状态：挂在某一方场上（草原核 / 激化领域）。
    Side(Side),
}

/// 反应赋予**手牌**的状态（关键字，不是卡牌）。
///
/// 依据：规则书·元素反应·效果 月感电「赋予敌方随机1张手牌[电击]」、
/// 月绽放「赋予我方随机1张手牌[费用降低]」；关键字表 `tcgkeywords_4lang.json`
/// K3003 = 电击 / K3005 = 费用降低（手牌状态，具体状态块待接入）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum HandCardStatus {
    /// 电击（K3003）：月感电的雷暴云入场 / 可用次数增加时赋予敌方手牌。
    ElectricShock,
    /// 费用增加（K3004）。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_COST_INCREASE。
    CostIncrease,
    /// 费用降低（K3005）：月绽放赋予我方手牌。
    CostReduction,
    /// 赋能（K3006）。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_EMPOWERMENT。
    Empowerment,
    /// 不可调和（K3007）。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_UNTUNABLE。
    Untunable,
    /// 无效化（K3008）。
    ///
    /// 关键字数据只有名字（description 为 null），语义 / 形态待实测；身份见
    /// families::action_card::areas::hand_state::HAND_INVALIDATION。
    Invalidation,
}

impl HandCardStatus {
    /// 全部 6 个手牌状态（顺序 = K 码顺序，不表示规则语义）。
    pub const ALL: [HandCardStatus; 6] = [
        HandCardStatus::ElectricShock,
        HandCardStatus::CostIncrease,
        HandCardStatus::CostReduction,
        HandCardStatus::Empowerment,
        HandCardStatus::Untunable,
        HandCardStatus::Invalidation,
    ];

    /// 对应的关键字数据 K 码（K3003–K3008）。
    pub const fn k_code(self) -> Nat {
        match self {
            HandCardStatus::ElectricShock => Nat::new(3003),
            HandCardStatus::CostIncrease => Nat::new(3004),
            HandCardStatus::CostReduction => Nat::new(3005),
            HandCardStatus::Empowerment => Nat::new(3006),
            HandCardStatus::Untunable => Nat::new(3007),
            HandCardStatus::Invalidation => Nat::new(3008),
        }
    }

    /// 按 K 码查手牌状态（只认 K3003–K3008）。
    pub fn from_k_code(code: Nat) -> Option<HandCardStatus> {
        HandCardStatus::ALL
            .iter()
            .copied()
            .find(|status| status.k_code() == code)
    }
}

/// **一条底层操作请求**：效果函数只产出它，不执行。
///
/// 变体与已有底层操作一一对应（括号里是执行器将来要调的入口）：
/// - [`EffectOp::Damage`] -> `families::character::damage::damage`（扣血 / 附着 / 反应判定都在里面）；
/// - [`EffectOp::Status`] -> 状态区 `apply`（**待接入**：`families::status` 目前只有骨架）；
/// - [`EffectOp::Summon`] -> 召唤物区 `summon` / `merge_same_kind`（**待接入**：`families::summon` 同上）；
/// - [`EffectOp::ForceSwitch`] -> 角色区 `switch_active`（**待接入**：出战 / 后台区尚在别的块）；
/// - [`EffectOp::AddCardToHand`] -> 手牌区 `add`（**待接入**：手牌区已存在，但「生成一张牌」的入口在卡牌层）；
/// - [`EffectOp::HandCardStatus`] -> 行动牌状态 `apply`（**待接入**，D112/D113）。
///
/// 每个变体隐含牌族：`Status` 的 `card` 是**状态牌族**、`Summon` 的是**召唤物牌族**、
/// `AddCardToHand` 的是**行动牌牌族**（CardId 只在族内唯一，所以不能丢掉牌族）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EffectOp {
    /// 造成伤害（交给已有伤害入口；本模块不结算增减伤 / 护盾）。
    Damage {
        /// 伤害数值。
        amount: Nat,
        /// 伤害类型（元素 / 物理 / 穿透）。
        ty: DamageType,
        /// 目标范围。
        target: TargetScope,
    },
    /// 附属 / 叠加状态（交给状态区；**待接入**）。
    Status {
        /// 状态牌 id（状态牌族）。
        card: CardId,
        /// 挂载位置（角色状态 / 出战状态）。
        target: StatusTarget,
        /// 叠加层数（规则书「可用次数」不是层数，单独放 `uses`）。
        stacks: Nat,
        /// 可用次数（`None` = 该状态没有可用次数概念）。
        uses: Option<Nat>,
        /// 叠加上限（`None` = 不设上限；结晶的「最多2点」放这里）。
        stack_cap: Option<Nat>,
    },
    /// 生成 / 叠加召唤物（交给召唤物区；**待接入**）。
    Summon {
        /// 召唤物牌 id（召唤物牌族）。
        card: CardId,
        /// 生成在哪一方。
        side: Side,
        /// 初始可用次数。
        uses: Nat,
        /// 叠加上限（`None` = 无限叠加）。
        stack_limit: Option<Nat>,
    },
    /// 强制切换到下一个角色（交给角色区；**待接入**）。
    ForceSwitch {
        /// 被强制切换的角色范围（超载 = 承载反应的角色）。
        target: TargetScope,
    },
    /// 生成一张牌加入手牌（交给手牌区；**待接入**）。
    AddCardToHand {
        /// 行动牌 id（行动牌牌族）。
        card: CardId,
        /// 加入哪一方的手牌。
        side: Side,
    },
    /// 给**随机一张**手牌赋予状态（分支已按 D190 展开；交给行动牌状态区；**待接入**）。
    HandCardStatus {
        /// 要赋予的手牌状态。
        status: HandCardStatus,
        /// 哪一方的手牌。
        side: Side,
        /// 手牌下标（`0..手牌数`；下标 -> 具体牌的映射由手牌区只读指针给出，D183）。
        index: Nat,
    },
}

/// 一条**已展开**的分支：底层操作清单 + 该分支的精确概率（D190，不采样）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReactionBranch {
    /// 本分支要执行的底层操作（按规则书条文顺序；可能为空 = 纯加伤 / 无额外效果）。
    pub ops: Vec<EffectOp>,
    /// 该分支的精确概率（`ExactNat128` 分子分母，零浮点）。
    pub probability: Fraction,
}

/// 一次反应效果结算的**纯函数结果**。
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ReactionEffectResult {
    /// 「本伤害 +N」（确定性；所有分支相同）。
    pub damage_bonus: Nat,
    /// 分支列表。确定性效果 = 恰好一支、概率 1；随机效果 = 每个可能结果一支、概率和为 1。
    pub branches: Vec<ReactionBranch>,
}

impl ReactionEffectResult {
    /// 确定性结果：一个分支（概率 1），带给定操作清单。
    pub fn single(damage_bonus: Nat, ops: Vec<EffectOp>) -> ReactionEffectResult {
        ReactionEffectResult {
            damage_bonus,
            branches: vec![ReactionBranch {
                ops,
                probability: Fraction::ONE,
            }],
        }
    }

    /// **非伤害形式引发的反应**：忽略所有伤害类效果（规则书·元素反应·规则 第（2）条原文：
    /// 「以非伤害的形式施加特定类型的『元素附着』时也会引发『元素反应』。结算这类
    /// 『元素反应』效果时，会忽略所有伤害类效果。」）。
    ///
    /// 只做一件事：damage_bonus 归零、把每个分支里的 [EffectOp::Damage] 过滤器掉；
    /// 其它操作（状态 / 召唤物 / 强制切换 / 手牌…）原样保留。**不新增第二套效果语义**。
    pub fn without_damage(&self) -> ReactionEffectResult {
        let branches = self
            .branches
            .iter()
            .map(|branch| ReactionBranch {
                ops: branch
                    .ops
                    .iter()
                    .copied()
                    .filter(|op| !matches!(op, EffectOp::Damage { .. }))
                    .collect(),
                probability: branch.probability,
            })
            .collect();
        ReactionEffectResult {
            damage_bonus: Nat::ZERO,
            branches,
        }
    }

    /// 分支概率之和（精确分数；用于断言「概率守恒」，D150 / D158）。
    pub fn probability_total(&self) -> Fraction {
        self.branches
            .iter()
            .fold(Fraction::ZERO, |sum, branch| {
                sum.exact_add(branch.probability)
            })
    }
}

/// 效果函数指针（D186：登记的是**执行函数指针**，不是效果数据 / trait object / 闭包）。
///
/// 参数：
/// - `&ReactionEvent`：谁触发 / 作用在谁身上 / 命中的已覆盖元素 / 本次攻击元素；
/// - `&EffectContext`：本次效果需要的最小局面摘要（是否出战角色、双方手牌数）。
pub type ReactionEffectFn =
    fn(&crate::families::character::attachment::ReactionEvent, &EffectContext) -> ReactionEffectResult;

/// 效果函数需要的**最小局面摘要**（不是完整状态；纯数据、`Copy`）。
///
/// 只放「规则书条文里出现、且效果函数必须知道」的量：
/// - 超载：「如果目标是**出战角色**，就强制切换」；
/// - 月感电：「赋予敌方随机1张手牌[电击]」-> 需要承载方手牌数；
/// - 月绽放：「赋予我方随机1张手牌[费用降低]」-> 需要触发方手牌数。
///
/// 手牌内容不在这里：下标 -> 具体牌由手牌区的只读指针给（D183），效果函数只产出下标分支。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EffectContext {
    /// 承载反应的角色是不是「出战角色」（超载强制切换的前提）。
    pub target_is_active: bool,
    /// 触发方（攻击方）手牌数（月绽放的随机池大小）。
    pub source_hand_len: Nat,
    /// 承载方手牌数（月感电的随机池大小）。
    pub target_hand_len: Nat,
}

impl EffectContext {
    /// 完整构造（是否出战角色 + 双方手牌数）。
    pub const fn new(
        target_is_active: bool,
        source_hand_len: Nat,
        target_hand_len: Nat,
    ) -> EffectContext {
        EffectContext {
            target_is_active,
            source_hand_len,
            target_hand_len,
        }
    }

    /// 双方手牌数都当 0 的摘要（没有随机手牌效果的测试 / 场景用）。
    pub const fn no_hands(target_is_active: bool) -> EffectContext {
        EffectContext {
            target_is_active,
            source_hand_len: Nat::ZERO,
            target_hand_len: Nat::ZERO,
        }
    }
}

/// 把手牌数展开成均匀随机分支（D190：**返回分支 + 精确概率，不采样**）。
///
/// 每支概率 `1 / count`（规则书只说「随机1张」，未给权重 -> 均匀）；
/// `count == 0` -> 空列表（随机池为空，效果不发生；调用方按「无操作」处理）。
pub fn uniform_hand_branches(
    count: Nat,
    mut ops_at: impl FnMut(Nat) -> Vec<EffectOp>,
) -> Vec<ReactionBranch> {
    if count.get() == 0 {
        return Vec::new();
    }
    let probability = Fraction::new(
        ExactNat128::new(1),
        ExactNat128::new(count.get() as u128),
    );
    (0..count.get())
        .map(|index| ReactionBranch {
            ops: ops_at(Nat::new(index)),
            probability,
        })
        .collect()
}
