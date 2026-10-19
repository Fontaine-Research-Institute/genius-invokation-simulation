//! **一次「打出行动牌」端到端**：打出条件（D217）-> 「打出前」派发执行 -> 付费 -> 离手 ->
//! 按 ActionCardType 落位 -> 派发「打出后」。
//!
//! ## 依据（唯一权威）
//!
//! - 规则书·进行行动 打出手牌：「用元素骰支付所需的费用，打出一张手牌」；
//! - 规则书·游戏区域 III 支援区：「『支援牌』打出后，会放置在支援区。支援区最多放置 4 张…」；
//!   满 4 时「需要先选择一张支援区内的支援牌弃置」（本层直接复用支援区的 add_with_triggers 分支）；
//! - 规则书·游戏区域 II(1)：「『装备牌』打出给角色后，会附属在角色牌上」（槽位判定复用装备栏）；
//! - 规则书·游戏区域 VI 手牌：打出的牌离开手牌（事件牌结算完离场，不进任何区）；
//! - D217：行动牌**自带打出条件**（`ActionCardDef::play_condition`，见
//!   `families/action_card/def.rs`），尝试打出时在**比较是否付得起之前**先调用它；
//!   不通过 -> 拒绝 + **状态不变**；
//! - D186：派发只排序、不执行；本层是「调用方」，执行仍走各区的 execute_*；
//! - 规则书·特殊词条 **秘传** + 权威关键字 #70 **秘传揭令**（Z12）：「在一场对局中，每位牌手
//!   只能使用一张「秘传」卡牌」「只有在「秘传揭令」仍存在时，才能打出「秘传」卡牌。打出
//!   「秘传」卡牌后，会消耗掉当前对局的「秘传揭令」使用机会。」-> 秘传牌走
//!   [play_card_me_arcane] / [play_card_opp_arcane]：**前置**判定揭令 + 本局已打出张数，
//!   **成功后** record_play（张数 +1 并消耗揭令）；普通牌入口行为不变；
//! - 规则书·其他规则 **结算顺序**（S7.5）：「1、先结算当前牌手的所有卡牌效果，再结算对方
//!   牌手的卡牌效果。2、同属于同一位牌手的卡牌效果…角色牌、角色装备和状态、阵营出战状态、
//!   召唤物、支援牌。3、同属于同一位牌手的同类型卡牌效果…进场的先后顺序」。
//!   「打出前 / 后」对**双方**注册表派发：当前牌手（打出方）的全部先，再对方的。
//!
//! ## 口径
//!
//! - **落位由 [CardPlacement] 决定**（三个变体一一对应 ActionCardType 的三个值，
//!   [CardPlacement::card_type] 给出类型；本层按类型分支落位，不写具体卡牌效果）；
//! - **支援牌持续回合**（Z6 / Z14；S8.6）：[CardPlacement::Support] 带
//!   `duration: SupportDuration`（由卡牌定义给），落位时写进 `SupportEntry`；
//!   结束阶段 tick 由 `flow::turn` 接（递减 / 耗尽弃置）；
//! - **打牌时机**（S3.1 行动 / S8.8 战斗行动（行动牌））：[play_card_me] / [play_card_opp]
//!   按「无战斗行动规则 = 快速行动」处理；牌带「战斗行动（行动牌）」规则时用
//!   [play_card_me_with_timing] / [play_card_opp_with_timing]（传入 `has_combat_action_tag`，
//!   由卡牌定义 / 标签给）。判定复用 [crate::rules::ops::stage::timing_of_card_play]，
//!   结果在 [PlayCardOutcome::timing]；行动权推进（是否换手）归调用方；
//! - **纯函数**：吃 &DefState、返回分支（支援区满 4 时是 4 支，每支带精确概率）+ 待执行；
//!   失败返回 Err，**入参一个字节都不改**（内部在克隆上做）；
//! - **分支带终局判定**（Task 2 方案 A）：[PlayBranch::outcome] 是只读 DefState 的
//!   [crate::flow::game_end::settlement_outcome]；调用方必须在「打出结算完成 +
//!   `state::flows::defeat` 清理」之后自行用它 / 再判一次（HP 归 0 但未清理 -> Ongoing，
//!   见 `state/terminal.rs` 的调用时点）；
//! - **打出前 / 打出后**：时机键由本层定义（timeline 通用层不定义具体时机，D185）；
//!   「打出前」的待执行**由本层执行**（后续落位可能依赖它，例如前效果改了骰池 / 手牌），
//!   「打出后」的待执行**原样返回**（D186：派发不执行，由调用方决定何时执行；
//!   行动牌效果体由卡牌代码在「打出后」之前自己执行，本层不写）；
//! - **手牌离开**：打出时同时从手牌、当前费用表、手牌卡状态表里移除（三者是同一张牌的三个视角）；
//! - **打出条件先于付费**（D217）：条件函数只吃 `&DefState` + 视角方（签名决定它改不了
//!   任何状态），不通过返回 [PlayCardError::ConditionNotMet]、**入参一个字节不变**
//!   （此时还没克隆）；条件为 `None` 的调用方行为与以前完全一致；
//! - **敌我不编号**（D34）：play_card_me / play_card_opp 两个入口，共用一份实现。
//!
//! ## L1 料理 / 饱腹（S8.1）
//!
//! 规则书·特殊词条 料理：「当『料理』事件牌对角色生效后，角色会进入『饱腹』状态。处于『饱腹』
//! 状态下的角色，无法成为其他『料理』事件牌的目标，也无法受到其他『料理』事件牌的效果影响。」
//!
//! - 料理牌走 [play_card_me_food] / [play_card_opp_food]（含 `_with_timing`）：flow 不认识
//!   「哪张是料理 / 目标是谁」，由调用方按卡牌定义给目标实体；
//! - **结算前**：`can_choose_as_food_target` / `can_apply_food_effect`（= 未饱腹）校验，
//!   拒绝 -> [PlayCardError::FoodTargetNotAllowed]、入参不变；
//! - **结算后**：每支分支写回目标角色「饱腹」（`CharacterState::with_satiated`）；
//! - 「回合结束 / 击倒解除」不在本层：由 state 层调度（defs-terminal 已接）。
//!
//! TODO / 待实测（规则书没写清）：
//! 1. 规则书没限制**料理目标阵营**（我方 / 敌方角色都可？）—— 本层不挡，待实测；
//! 2. 「目标非法」与「打出条件不通过」的先后规则书没规定 —— 本层目标校验在打出条件之前；
//! 3. **复苏料理**（L2）：目标在击破区时本入口自动走复苏路径——次数用
//!    `rules::ops::food::can_revive` 判（每回合最多 1 个，计数由调用方经 [FoodReviveEnv] 持有）、
//!    复活用 `flow::revive`（state::flows::revive），复活后再加「饱腹」；
//!    「复苏后生命值」由卡面给（[FoodReviveEnv::hp]）；复活与「料理生效」的先后规则书没写清，
//!    本层按「打牌结算完成 -> 复活 -> 饱腹」暂定（待实测）；
//! 4. 「料理生效后」的判定点：本层按「事件牌一步结算完成（步末 flush 之后）」算生效；
//!    若卡面要求「效果命中才算生效」，需卡牌层给命中信息（待复核）。
//!
//! ## 待接入 / 待复核
//!
//! - **费用修正链**：「打出前」里少花费 / 加费用的效果要先于支付生效（52 处卡面）；
//!   本层「打出前」执行在支付之前（落位在支付之后），但费用修正链本身尚未接（D184 第 3 步）；
//! - **事件牌效果体**：本层只负责「结算完离场」（打出手牌时已移除），效果体由卡牌代码执行；
//! - **两侧注册表**：「打出前 / 后」按 S7.5 第 1 条做「当前牌手（打出方）先、对方后」，
//!   同一侧内按第 2 / 3 条由 timeline::resolve 预留的顺序表保证（顺序表在 rules::ops::order）；
//! - **支援牌的进场时机**：由支援区 add_with_triggers 派发（本层只把结果摊平成统一待执行）；
//! - **装备槽位冲突**：复用装备栏的强制替换（Lead 裁定 2 / 3），待实测；
//! - **事件牌离开手牌时的手牌卡状态 / 费用**：与支援 / 装备一样统一移除，待复核是否需要保留到效果结算完；
//! - **费用判定走 `pay` 一步**（付不起 -> NotEnoughDice），没有先显式 `can_afford`：D217 的
//!   「先 `can_afford` 再支付」在骰子的折叠组上才有区别（`can_afford` 是「可能就行」），
//!   是否补一次显式判定，待复核；
//! - **条件相对「打出前」触发的位置**：本层在「打出前」派发**之前**求值条件（条件看到的是
//!   触发前的局面）；D217 只规定「在比较是否付得起之前」，待复核；
//! - **待接入**：`ActionCardDef` 写进 gi-genius-invokation-cards 的 `src/cards/**`（D240 起具体卡牌不在本 crate）；`PlayCardRequest::cost` 改为从定义的基础费用派生。

use crate::base::CardId;
use crate::families::action_card::areas::support::{
    SupportDuration, SupportEntry, SupportTriggers, SupportValue,
};
use crate::families::action_card::def::{PlayCondition, PlayConditionFn, ViewSide};
use crate::families::action_card::ActionCardType;
use crate::families::character::satiated::{can_apply_food_effect, can_choose_as_food_target};
use crate::families::character::zones::equipment::{
    EquipmentEntry, EquipmentOutcome, EquipmentTriggers, EquipmentZone,
};
use crate::families::character::zones::field::CharacterZoneTriggers;
use crate::families::status::arcane_edict::ArcaneEdict;
use crate::flow::game_end::{settlement_outcome, Outcome};
use crate::flow::hooks::{apply_defeat_cleanup, DefeatCleanup};
use crate::flow::pending::{execute_pending, FlowPending, PendingSet};
use crate::flow::record::FlowRecord;
use crate::flow::revive::{revive_me, revive_opp, ReviveEnv, ReviveError};
use crate::keywords::effect::{EffectEnv, TriggerBook};
use crate::keywords::step::{execute_step_me, execute_step_opp, Step};
use crate::resources::dice::CostComponent;
use crate::rules::ops::arcane::ArcanePlay;
use crate::rules::ops::equip::{CharacterEquip, EquipReject, SlotKind, WeaponType};
use crate::rules::ops::food::can_revive;
use crate::rules::ops::stage::{timing_of_card_play, Timing};
use crate::state::{DefState, PlayerDefs};
use crate::timeline::{resolve, Event, Pending, Queued, Trigger, TriggerRegistry};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, EntityId, Nat};

/// 行动牌打出的**时机键**（D185：由需要广播的块自己定义；timeline 不认识它）。
///
/// 依据：规则书·进行行动 打出手牌 + 卡面「打出时 / 打出后」（如「打出此牌时…」）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PlayTiming {
    /// 打出**前**（支付 / 落位之前）。
    Before,
    /// 打出**后**（支付 + 落位之后；效果体在两者之间由卡牌代码执行）。
    After,
}

/// 行动牌打出的效果函数指针：fn(&mut 对局, &Event, owner)。
///
/// 需要整个对局上下文（可能要改任意区 / 双方），所以签名收 &mut DefState。
pub type PlayEffect = fn(&mut DefState, &Event, EntityId);

/// 行动牌打出的触发登记项。
pub type PlayTrigger = Trigger<PlayTiming, PlayEffect>;

/// 行动牌打出的触发注册表（每个牌手一个具名实例，D34）。
pub type PlayTriggers = TriggerRegistry<PlayTiming, PlayEffect>;

/// 一条待执行（owned；事件按值带走）。
pub type PlayPending = Queued<PlayEffect>;

/// **两位牌手的行动牌打出注册表**（S7.5 第 1 条：当前牌手优先需要两张表）。
///
/// 两张表是两个具名实例（D34）；谁是当前牌手由 play_card_me / play_card_opp 绑定。
#[derive(Clone, Copy, Debug)]
pub struct PlayTriggerBook<'a> {
    /// 我方的「打出前 / 后」注册表。
    pub me: &'a PlayTriggers,
    /// 敌方的「打出前 / 后」注册表。
    pub opp: &'a PlayTriggers,
}

impl<'a> PlayTriggerBook<'a> {
    /// 两位牌手的注册表。
    pub const fn new(me: &'a PlayTriggers, opp: &'a PlayTriggers) -> PlayTriggerBook<'a> {
        PlayTriggerBook { me, opp }
    }

    /// 当前牌手（打出方）的注册表：playing_me = true 表示我方是当前牌手。
    fn current(self, playing_me: bool) -> &'a PlayTriggers {
        if playing_me {
            self.me
        } else {
            self.opp
        }
    }

    /// 对方牌手（非当前牌手）的注册表。
    fn other(self, playing_me: bool) -> &'a PlayTriggers {
        if playing_me {
            self.opp
        } else {
            self.me
        }
    }
}

/// 关键字层（一步）的**外部依赖**（只借不拥有；D227：事件牌效果体走一步时要它）。
///
/// 字段与 keywords::effect::EffectEnv 的构造一致（视角方由 play_card_me / play_card_opp 绑定）。
pub struct KeywordEnv<'a> {
    /// 双方触发集合（区触发 / 角色族触发 / 支援 / 装备）。
    pub triggers: TriggerBook<'a>,
    /// 伤害入口依赖（允许附着集合 + 反应规则表 + 反应注册表）。
    pub damage: crate::families::character::damage::DamageEnv<'a>,
    /// 元素精通。
    pub mastery: Nat,
    /// 通用增伤。
    pub increases: &'a [Nat],
    /// 反应效果函数的局面摘要。
    pub effect_context: &'a crate::reactions::effect::EffectContext,
    /// 实体发号器（生成召唤物 / 卡牌用）。
    pub alloc: crate::families::character::damage_chain::executor::EntityAlloc,
}

impl<'a> KeywordEnv<'a> {
    /// 建一步用的执行上下文（视角方由 execute_step_me / execute_step_opp 覆盖）。
    fn effect_env(&self, subject: EntityId) -> EffectEnv<'a> {
        EffectEnv::new(
            subject,
            crate::keywords::target::Perspective::Me,
            crate::families::character::damage::DamageEnv {
                allow: self.damage.allow,
                rules: self.damage.rules,
                registry: self.damage.registry,
            },
        )
        .with_alloc(self.alloc)
        .with_triggers(self.triggers)
        .with_mastery(self.mastery)
        .with_increases(self.increases)
        .with_effect_context(self.effect_context)
    }
}

/// 打出行动牌的**外部依赖**（只借不拥有）。
pub struct PlayCardEnv<'a> {
    /// 两位牌手的「打出前 / 后」触发注册表（S7.5：当前牌手优先）。
    pub triggers: PlayTriggerBook<'a>,
    /// 支援区触发注册表（进入 / 弃置）。
    pub support_triggers: &'a SupportTriggers,
    /// 装备栏触发注册表（装备 / 弃置）。
    pub equipment_triggers: &'a EquipmentTriggers,
    /// 关键字层依赖（事件牌效果体走一步时必需；没有效果体 / 旧路径传 None）。
    pub keywords: Option<KeywordEnv<'a>>,
    /// **击倒清理回调**（D255：结算 -> 清理 -> 判终局）。
    ///
    /// 上层（cards）注入真实实现（state::flows::defeat 需要按角色给 DefeatHpReset）；
    /// 不注入时传 crate::flow::hooks::NO_DEFEAT_CLEANUP（行为与旧版完全一致）。
    pub defeat_cleanup: DefeatCleanup,
}

/// 行动牌的**落位**（三个变体 = ActionCardType 的三个值；落位数据由调用方按卡牌定义给）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardPlacement<'a> {
    /// 支援牌：进支援区（值三选一，D182 / 分工计划 §4.4.15）。
    Support {
        /// 支援牌的值。
        value: SupportValue,
        /// **持续回合**（Z6 / Z14；S8.6 持续回合）：由卡牌定义给；
        /// 整局用 [SupportDuration::WholeGame]，有限回合用 [SupportDuration::Round]。
        duration: SupportDuration,
    },
    /// 装备牌：挂到指定角色身上（槽位 / 武器门由卡牌标签给）。
    Equipment {
        /// 挂到哪个角色。
        character: EntityId,
        /// 槽位种类。
        slot: SlotKind,
        /// 需要的角色武器类型（武器牌才有）。
        required_weapon: Option<WeaponType>,
        /// 该角色的装备条件（武器类型）。
        character_equip: CharacterEquip,
    },
    /// 事件牌：结算完离场（不进任何区）。
    Event {
        /// 效果体（D227：关键字层一步；None = 无效果体，由调用方在「打出后」之前自己执行）。
        effect: Option<&'a Step>,
    },
}

impl<'a> CardPlacement<'a> {
    /// 这次落位对应哪个 ActionCardType（本层按它分支）。
    pub fn card_type(self) -> ActionCardType {
        match self {
            CardPlacement::Support { .. } => ActionCardType::Support,
            CardPlacement::Equipment { .. } => ActionCardType::Equipment,
            CardPlacement::Event { .. } => ActionCardType::Event,
        }
    }

    /// 装备牌要挂到的角色（其它类型 -> None）。
    fn equip_character(self) -> Option<EntityId> {
        match self {
            CardPlacement::Equipment { character, .. } => Some(character),
            CardPlacement::Support { .. } | CardPlacement::Event { .. } => None,
        }
    }
}

/// 一次打牌的**请求**（本层不替你选牌 / 目标）。
pub struct PlayCardRequest<'a> {
    /// 手牌实体号（必须真在手牌里）。
    pub entity: EntityId,
    /// 当前费用（由调用方按当前费用表给：基础费用 + 加减费 delta）。
    pub cost: Vec<CostComponent>,
    /// 同色费用支付时选的颜色（SameColor 费用需要）。
    pub same_color: Option<crate::base::Element>,
    /// 落位。
    pub placement: CardPlacement<'a>,
    /// 打出的**条件**（D217；由调用方从 `ActionCardDef::play_condition()` 给，`None` = 无条件）。
    ///
    /// 本层在「手牌里有这张牌」之后、**比较是否付得起**与「打出前」触发**之前**调用它：
    /// 拒绝 -> [PlayCardError::ConditionNotMet]、入参不变。
    pub play_condition: Option<PlayConditionFn>,
}

/// **一支打牌分支**（支援区满 4 时是多支，D190）。
#[derive(Clone, Debug)]
pub struct PlayBranch {
    /// 这一支结算后的完整局面。
    pub state: DefState,
    /// 这一支的精确概率。
    pub probability: Fraction,
    /// 这一支之后还要执行的待执行（支援 / 装备区派发出来的；本层不执行）。
    pub pending: PendingSet,
    /// 这一支的终局判定（只读；调用时点见 [settlement_outcome]：每步结算 + defeat 清理之后）。
    pub outcome: Outcome,
}

/// 一次打牌的结果。
#[derive(Clone, Debug)]
pub struct PlayCardOutcome {
    /// 结算分支（支援区未满 -> 恰好一支）。
    pub branches: Vec<PlayBranch>,
    /// 「打出前」的待执行（**已由本层执行**；保留供观测）。
    pub before: Vec<PlayPending>,
    /// 「打出后」的待执行（**未执行**；由调用方执行）。
    pub after: Vec<PlayPending>,
    /// 本次打牌的**行动时机**（S3.1 行动 + S8.8 战斗行动（行动牌））。
    ///
    /// 由 [crate::rules::ops::stage::timing_of_card_play] 按「这张牌是否带『战斗行动』规则」判定：
    /// 默认快速行动；带规则时改为战斗行动。行动权推进（是否换手）归调用方，本层不切行动方。
    pub timing: Timing,
    /// 结算后的实体发号器（事件牌一步生成过召唤物 / 卡牌时前移；没有一步时 None）。
    pub alloc: Option<crate::families::character::damage_chain::executor::EntityAlloc>,
    /// 流程级记录（D225 / D227：打出牌 / 一步 / 步末 flush / 步边界插入）。
    ///
    /// 本层只产出内容（不持有记录器）：由 flow/recorded.rs 的入口写进调用方给的 Recorder。
    pub events: Vec<FlowRecord>,
}

impl PlayCardOutcome {
    /// 单支时唯一的那一支；多支（支援区满 4 的弃置选择）时返回 None。
    pub fn single(&self) -> Option<&PlayBranch> {
        if self.branches.len() == 1 {
            self.branches.first()
        } else {
            None
        }
    }
}

/// 打牌失败的原因（所有失败都保证入参不变）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayCardError {
    /// 这张牌不在打出方手牌里。
    NotInHand,
    /// 骰子不够付。
    NotEnoughDice,
    /// 装备被拒绝（槽位 / 武器类型门），状态不变。
    CannotEquip(EquipReject),
    /// 落位数据与 ActionCardType 不一致（构造错误；正常不会发生）。
    PlacementMismatch,
    /// 事件牌带了一步效果体，但环境里没有关键字层依赖（[PlayCardEnv::keywords] 为 None）。
    MissingKeywordEnv,
    /// 这张牌的**打出条件**不通过（D217）；带的是调试文案（中文短句，不是给玩家看的本地化文案）。
    ///
    /// 只读检查、**入参不变**；它发生在「比较是否付得起」**之前**，所以同一张牌
    /// 「条件不通过 + 骰子也不够」时，本层报的是这个错误，不是 [PlayCardError::NotEnoughDice]。
    ConditionNotMet(&'static str),
    /// 秘传（Z12 / K311）：揭令不存在，或本局已经打出过秘传（每人 1 张）。
    ArcaneNotPlayable(ArcanePlay),
    /// 料理目标不合法（S8.1 第 2 条）：目标角色不在场，或已处于「饱腹」
    /// （不能成为其他料理的目标 / 不受其效果）。只发生在 [play_card_me_food] 等料理入口。
    FoodTargetNotAllowed,
    /// 复苏料理不可用（L2 / S8.1 第 4 条）：次数已用完 / 目标不在击破区 / 角色区已满。
    FoodReviveNotAllowed(ReviveError),
}

// ======================================================================
// 公开入口（我方 / 敌方成对，D34）
// ======================================================================

/// **打出行动牌**（我方打出；无「战斗行动」规则 = 快速行动）。
///
/// 带「战斗行动（行动牌）」规则的牌用 [play_card_me_with_timing]（S8.8），否则时机会被
/// 判成快速行动。
pub fn play_card_me(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
) -> Result<PlayCardOutcome, PlayCardError> {
    play_card_me_with_timing(state, env, request, false)
}

/// **打出行动牌（我方；按牌是否带「战斗行动」规则判时机）**（S3.1 / S8.8）。
///
/// `has_combat_action_tag`：这张牌是否带「战斗行动（行动牌）」规则 —— 由**卡牌定义 / 标签**
/// 给（flow 不认识具体标签）；判定复用 `rules::ops::stage::timing_of_card_play`，
/// 结果放进 [PlayCardOutcome::timing]（默认快速行动；带规则 -> 战斗行动）。
pub fn play_card_me_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    has_combat_action_tag: bool,
) -> Result<PlayCardOutcome, PlayCardError> {
    play(state, env, request, true, has_combat_action_tag, None)
}

/// **打出行动牌**（敌方打出；与 play_card_me 共用一份实现，D35；无「战斗行动」规则）。
pub fn play_card_opp(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
) -> Result<PlayCardOutcome, PlayCardError> {
    play_card_opp_with_timing(state, env, request, false)
}

/// **打出行动牌（敌方；按牌是否带「战斗行动」规则判时机）**（与 [play_card_me_with_timing]
/// 共用一份实现，D35）。
pub fn play_card_opp_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    has_combat_action_tag: bool,
) -> Result<PlayCardOutcome, PlayCardError> {
    play(state, env, request, false, has_combat_action_tag, None)
}

// ======================================================================
// L1 料理 / 饱腹（S8.1）
// ======================================================================

/// 料理结算的**复苏参数**（L2 + S8.1 第 4 条；目标在击破区时才需要）。
#[derive(Clone, Copy)]
pub struct FoodReviveEnv<'a> {
    /// 角色区触发注册表（复苏会派发「进入角色区」时机）。
    pub characters: &'a CharacterZoneTriggers,
    /// 本回合已经通过料理复苏的角色数（调用方持有；每回合上限见 rules::ops::food）。
    pub revived_this_round: Nat,
    /// 复苏后生命值（卡面给；规则书没写具体值）。
    pub hp: Nat,
    /// 具有「复苏」效果的料理事件牌 id（记录 / 观测用；本层不解释卡牌语义）。
    pub card: CardId,
}

/// 一次料理打牌的**结果**：打牌结果 + 复苏后的本回合计数（非复苏料理 -> None）。
#[derive(Clone, Debug)]
pub struct FoodPlayOutcome {
    /// 普通打牌流程的完整结果（分支 / 前后派发 / 时机 / 记录）。
    pub play: PlayCardOutcome,
    /// 复苏料理：**更新后**的本回合复苏计数（= 传入值 + 1；调用方写回自己的计数）。
    /// 普通料理 -> None。
    pub revived_this_round: Option<Nat>,
}

/// 料理目标落在哪个区（决定走普通料理还是复苏料理）。
enum FoodTargetRoute {
    /// 在角色区（普通料理）。
    Field,
    /// 在击破区（复苏料理；on_me = 目标属于我方）。
    Revive { on_me: bool },
}

/// **打出「料理」行动牌**（我方；S8.1 料理）：结果与 [play_card_me] 相同，另外：
///
/// 1. **结算前**校验目标（S8.1 第 2 条）：
///    - 目标在**角色区** -> 普通料理：`can_choose_as_food_target` / `can_apply_food_effect`
///      （= 未饱腹）校验；
///    - 目标在**击破区** -> 复苏料理（L2）：次数用 `rules::ops::food::can_revive` 判
///      （每回合最多 1 个），复苏参数由 [FoodReviveEnv] 给；失败 ->
///      [PlayCardError::FoodReviveNotAllowed]；
/// 2. **结算后**（每支分支）：普通料理给目标加「饱腹」；复苏料理先复活目标、再给它加「饱腹」
///    （S8.1 第 1 / 4 条）。
///
/// 料理牌的**目标实体**由调用方给（flow 不认识卡面 / 选择策略）；目标可以是双方角色
/// （规则书 S8.1 未限制阵营，待实测）。
pub fn play_card_me_food(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_me_food_with_timing(state, env, request, target, revive, false)
}

/// **打出「料理」行动牌（我方；按牌是否带「战斗行动」规则判时机）**（S8.1 / S8.8）。
pub fn play_card_me_food_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
    has_combat_action_tag: bool,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_food(
        state,
        env,
        request,
        target,
        revive,
        true,
        has_combat_action_tag,
    )
}

/// **打出「料理」行动牌**（敌方；与 [play_card_me_food] 共用一份实现，D35）。
pub fn play_card_opp_food(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_opp_food_with_timing(state, env, request, target, revive, false)
}

/// **打出「料理」行动牌（敌方；按牌是否带「战斗行动」规则判时机）**。
pub fn play_card_opp_food_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
    has_combat_action_tag: bool,
) -> Result<FoodPlayOutcome, PlayCardError> {
    play_card_food(
        state,
        env,
        request,
        target,
        revive,
        false,
        has_combat_action_tag,
    )
}

/// 料理打牌的共用实现。
///
/// 顺序：手牌检查（与普通入口一致）-> 目标落区（角色区 / 击破区）-> 目标 / 次数校验 ->
/// 普通打牌流程（条件 / 付费 / 落位 / 效果）-> 每支分支：饱腹 / 复苏 + 饱腹。
/// **待复核**：规则书没规定「目标非法」与「打出条件不通过」的先后，本层把目标校验放在打出条件
/// 之前（条件在普通流程内求值）。
#[allow(clippy::too_many_arguments)]
fn play_card_food(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
    playing_me: bool,
    has_combat_action_tag: bool,
) -> Result<FoodPlayOutcome, PlayCardError> {
    let user = if playing_me { state.me() } else { state.opp() };
    if !user.hand().contains(request.entity) {
        return Err(PlayCardError::NotInHand);
    }
    let Some(route) = food_target_route(state, target) else {
        return Err(PlayCardError::FoodTargetNotAllowed);
    };
    match route {
        FoodTargetRoute::Field => {
            check_food_target(state, target)?;
            let play = play(
                state,
                env,
                request,
                playing_me,
                has_combat_action_tag,
                Some(target),
            )?;
            Ok(FoodPlayOutcome {
                play,
                revived_this_round: None,
            })
        }
        FoodTargetRoute::Revive { on_me } => play_card_food_revive(
            state,
            env,
            request,
            target,
            revive,
            on_me,
            playing_me,
            has_combat_action_tag,
        ),
    }
}

/// 复苏料理：前置校验 -> 普通打牌流程 -> 每支分支「复活 -> 加饱腹」。
#[allow(clippy::too_many_arguments)]
fn play_card_food_revive(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    target: EntityId,
    revive: Option<FoodReviveEnv<'_>>,
    on_me: bool,
    playing_me: bool,
    has_combat_action_tag: bool,
) -> Result<FoodPlayOutcome, PlayCardError> {
    let Some(revive) = revive else {
        return Err(PlayCardError::FoodTargetNotAllowed);
    };
    // 前置：次数（S8.1 第 4 条）-> 目标真的在击破区 -> 角色区未满（与 flow::revive 同口径）。
    if !can_revive(revive.revived_this_round) {
        return Err(PlayCardError::FoodReviveNotAllowed(
            ReviveError::NoReviveLeft,
        ));
    }
    let player = if on_me { state.me() } else { state.opp() };
    if player.defeated_character(target).is_none() {
        return Err(PlayCardError::FoodReviveNotAllowed(
            ReviveError::NotRevivable,
        ));
    }
    if player.characters().is_full() {
        return Err(PlayCardError::FoodReviveNotAllowed(
            ReviveError::CharacterZoneFull,
        ));
    }
    // 普通打牌流程（目标在击破区，不传 food_target；饱腹在复活后加）。
    let PlayCardOutcome {
        branches: play_branches,
        before,
        after,
        alloc,
        timing,
        events,
    } = play(state, env, request, playing_me, has_combat_action_tag, None)?;
    let mut branches: Vec<PlayBranch> = Vec::new();
    let mut revived_this_round = revive.revived_this_round;
    for branch in play_branches {
        let revive_env = ReviveEnv::new(revive.characters, revived_this_round);
        let revived = if on_me {
            revive_me(&branch.state, &revive_env, revive.card, target, revive.hp)
        } else {
            revive_opp(&branch.state, &revive_env, revive.card, target, revive.hp)
        }
        .map_err(PlayCardError::FoodReviveNotAllowed)?;
        revived_this_round = revived.revived_this_round;
        // 复苏后目标已在角色区 -> 加「饱腹」（S8.1 第 1 条），并重算终局判定。
        let state_after = apply_food_satiation(&revived.state, Some(target));
        let outcome = settlement_outcome(&state_after);
        let mut pending = branch.pending;
        let character_pending: Vec<FlowPending> = revived
            .characters
            .into_iter()
            .map(FlowPending::CharacterZone)
            .collect();
        if on_me {
            pending.me.extend(character_pending);
        } else {
            pending.opp.extend(character_pending);
        }
        branches.push(PlayBranch {
            state: state_after,
            probability: branch.probability,
            pending,
            outcome,
        });
    }
    Ok(FoodPlayOutcome {
        play: PlayCardOutcome {
            branches,
            before,
            after,
            alloc,
            timing,
            events,
        },
        revived_this_round: Some(revived_this_round),
    })
}

/// 料理目标在哪个区（角色区 -> 普通料理；击破区 -> 复苏料理）。
fn food_target_route(state: &DefState, target: EntityId) -> Option<FoodTargetRoute> {
    if state.me().defeated_character(target).is_some() {
        return Some(FoodTargetRoute::Revive { on_me: true });
    }
    if state.opp().defeated_character(target).is_some() {
        return Some(FoodTargetRoute::Revive { on_me: false });
    }
    if state.me().characters().contains(target) || state.opp().characters().contains(target) {
        return Some(FoodTargetRoute::Field);
    }
    None
}

/// 料理目标校验（S8.1 第 2 条）：目标角色在场 + 未饱腹。
fn check_food_target(state: &DefState, target: EntityId) -> Result<(), PlayCardError> {
    let character = state
        .me()
        .characters()
        .slot_of(target)
        .map(|slot| &slot.state)
        .or_else(|| {
            state
                .opp()
                .characters()
                .slot_of(target)
                .map(|slot| &slot.state)
        });
    let Some(character) = character else {
        return Err(PlayCardError::FoodTargetNotAllowed);
    };
    if can_choose_as_food_target(character) && can_apply_food_effect(character) {
        Ok(())
    } else {
        Err(PlayCardError::FoodTargetNotAllowed)
    }
}

/// 料理生效后给目标角色加「饱腹」（S8.1 第 1 条；纯函数；目标不在场 -> 原样）。
fn apply_food_satiation(state: &DefState, target: Option<EntityId>) -> DefState {
    let Some(target) = target else {
        return state.clone();
    };
    if let Some(slot) = state.me().characters().slot_of(target) {
        if let Some(characters) = state
            .me()
            .characters()
            .with_state(target, slot.state.with_satiated())
        {
            return state
                .clone()
                .with_me(state.me().clone().with_characters(characters));
        }
    }
    if let Some(slot) = state.opp().characters().slot_of(target) {
        if let Some(characters) = state
            .opp()
            .characters()
            .with_state(target, slot.state.with_satiated())
        {
            return state
                .clone()
                .with_opp(state.opp().clone().with_characters(characters));
        }
    }
    state.clone()
}

/// **秘传打出的结果**（Z12 / K311）：打牌结果 + **打出后**的秘传揭令容器。
#[derive(Clone, Debug)]
pub struct ArcanePlayOutcome {
    /// 普通打牌流程的完整结果（分支 / 前后派发 / 时机 / 记录）。
    pub play: PlayCardOutcome,
    /// 打出后的秘传揭令容器（已 record_play：本局张数 +1、按 K311 消耗揭令）。
    pub edict: ArcaneEdict,
}

/// **打出「秘传」行动牌**（我方；Z12 / K311）。
///
/// - 打牌**前置**：edict.can_play() 必须是 Allowed（揭令仍存在 + 本局未打出过）；
///   否则 Err(ArcaneNotPlayable(原因))、入参不变；
/// - 打牌**后置**：成功结算后 record_play（张数 +1、消耗揭令），新容器放进
///   ArcanePlayOutcome.edict（调用方写回自己持有的揭令状态）。
///
/// 与 play_card_me 的区别只在「秘传判定 + 揭令记账」：普通牌不要走这个入口。
pub fn play_card_me_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, true, false)
}

/// **打出「秘传」行动牌**（我方；按牌是否带「战斗行动」规则判时机；S8.8）。
pub fn play_card_me_arcane_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, true, has_combat_action_tag)
}

/// **打出「秘传」行动牌**（敌方；与 play_card_me_arcane 共用一份实现，D35）。
pub fn play_card_opp_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, false, false)
}

/// **打出「秘传」行动牌**（敌方；按牌是否带「战斗行动」规则判时机）。
pub fn play_card_opp_arcane_with_timing(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    play_card_arcane(state, env, request, edict, false, has_combat_action_tag)
}

/// 秘传打牌的共用实现：前置判定（K311）-> 普通打牌流程 -> 后置 record_play。
fn play_card_arcane(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    edict: ArcaneEdict,
    playing_me: bool,
    has_combat_action_tag: bool,
) -> Result<ArcanePlayOutcome, PlayCardError> {
    // 0) 手牌检查优先（与普通入口一致）：不在手牌 -> NotInHand，先于秘传 / 条件判定。
    let user = if playing_me { state.me() } else { state.opp() };
    if !user.hand().contains(request.entity) {
        return Err(PlayCardError::NotInHand);
    }
    // 1) 前置：秘传揭令仍存在 + 本局该牌手还没打出过秘传（规则书·特殊词条 秘传 + K311）。
    let verdict = edict.can_play();
    if verdict != ArcanePlay::Allowed {
        return Err(PlayCardError::ArcaneNotPlayable(verdict));
    }
    // 2) 普通打牌流程（纯函数；失败 -> 入参不变、不消耗揭令）。
    let play = play(state, env, request, playing_me, has_combat_action_tag, None)?;
    // 3) 后置：成功才记账（张数 +1 并消耗揭令）。前置已确认 Allowed，这里不会失败。
    let next = edict
        .record_play()
        .ok_or(PlayCardError::ArcaneNotPlayable(edict.can_play()))?;
    Ok(ArcanePlayOutcome { play, edict: next })
}

/// 共用实现：playing_me = true 表示我方打出。
fn play(
    state: &DefState,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
    playing_me: bool,
    has_combat_action_tag: bool,
    food_target: Option<EntityId>,
) -> Result<PlayCardOutcome, PlayCardError> {
    // 0) 打牌时机（S3.1 / S8.8）：默认快速行动，带「战斗行动（行动牌）」规则时改为战斗行动。
    let timing = timing_of_card_play(has_combat_action_tag);
    // 1) 基础检查（读）。
    let user = if playing_me { state.me() } else { state.opp() };
    if !user.hand().contains(request.entity) {
        return Err(PlayCardError::NotInHand);
    }

    // 2) 打出条件（D217）：在「比较是否付得起」与「打出前」触发**之前**求值。
    //    条件函数只吃 &DefState（签名决定它改不了任何状态），拒绝 -> Err、入参不变。
    check_play_condition(state, request, playing_me)?;

    // 3) 「打出前」：派发 + 执行（执行在克隆上；后面任何失败都不会写回入参）。
    let event = Event::new(
        Some(request.entity),
        request.placement.equip_character(),
        Nat::ZERO,
        None,
        None,
    );
    let current_triggers = env.triggers.current(playing_me);
    let other_triggers = env.triggers.other(playing_me);
    let before: Vec<PlayPending> =
        resolve_play_pending(current_triggers, other_triggers, PlayTiming::Before, &event);
    let mut next = state.clone();
    execute_play_pending(&mut next, &before);

    // 4) 重新读打出方（前效果可能改了手牌 / 骰池），走 支付 -> 离开手牌 -> 落位。
    let user = if playing_me { next.me() } else { next.opp() };
    let placed = place_card(user, env, request)?;

    // 5) 落位后的效果体（D227：事件牌走关键字一步）+ 步末 flush + 「打出后」派发。
    //
    //    一步内禁止插入（D204）：事件牌效果体 = 一个原子结算单位；它派发出来的待执行在
    //    **步末**由本层 flush（本层是 execute_step 的调用方）；「打出后」在步边界派发。
    let mut events: Vec<FlowRecord> = Vec::new();
    let mut alloc = env.keywords.as_ref().map(|keywords| keywords.alloc);
    let mut branches: Vec<PlayBranch> = Vec::new();
    for (player, probability, flow_pending) in placed {
        let mut set = PendingSet::new();
        if playing_me {
            set.me = flow_pending;
        } else {
            set.opp = flow_pending;
        }
        let base_state = if playing_me {
            next.clone().with_me(player)
        } else {
            next.clone().with_opp(player)
        };
        match request.placement {
            CardPlacement::Event { effect: Some(step) } => {
                let Some(keywords) = env.keywords.as_ref() else {
                    return Err(PlayCardError::MissingKeywordEnv);
                };
                let effect_env = keywords.effect_env(request.entity);
                let stepped = if playing_me {
                    execute_step_me(&base_state, step, &effect_env)
                } else {
                    execute_step_opp(&base_state, step, &effect_env)
                };
                alloc = Some(stepped.alloc);
                events.push(FlowRecord::Step {
                    index: Nat::ZERO,
                    ops: step.len(),
                    branches: Nat::new(stepped.branches.len() as _),
                });
                let mut pending_total = Nat::ZERO;
                for step_branch in stepped.branches {
                    pending_total = nadd!(pending_total, step_branch.pending.len());
                    // 步末 flush（D204：插入只能发生在步与步之间）。
                    let flushed = execute_pending(&step_branch.state, &step_branch.pending);
                    // L1（S8.1 第 1 条）：料理事件牌**生效后**，目标角色进入「饱腹」。
                    let flushed = apply_food_satiation(&flushed, food_target);
                    // D255：结算 -> 击倒清理 -> 判终局。清理待执行并进分支 pending。
                    let mut branch_pending = set.clone();
                    let flushed =
                        apply_defeat_cleanup(&flushed, env.defeat_cleanup, &mut branch_pending);
                    let outcome = settlement_outcome(&flushed);
                    branches.push(PlayBranch {
                        state: flushed,
                        probability: probability.exact_mul(step_branch.probability),
                        pending: branch_pending,
                        outcome,
                    });
                }
                events.push(FlowRecord::Flush {
                    after_step: Nat::ZERO,
                    pending: pending_total,
                });
            }
            _ => {
                let base_state = apply_food_satiation(&base_state, food_target);
                // D255：结算 -> 击倒清理 -> 判终局。
                let mut branch_pending = set;
                let base_state =
                    apply_defeat_cleanup(&base_state, env.defeat_cleanup, &mut branch_pending);
                let outcome = settlement_outcome(&base_state);
                branches.push(PlayBranch {
                    state: base_state,
                    probability,
                    pending: branch_pending,
                    outcome,
                });
            }
        }
    }

    // 「打出后」：只派发，不执行（D186；插入 / 执行由调用方在步边界做）。
    let after: Vec<PlayPending> =
        resolve_play_pending(current_triggers, other_triggers, PlayTiming::After, &event);
    if !after.is_empty() {
        events.push(FlowRecord::Insert {
            after_step: Nat::ZERO,
            pending: Nat::new(after.len() as _),
            at: "打出后",
        });
    }

    // 汇总记录（分支数在结算后才能知道，所以放最后）。
    events.push(FlowRecord::PlayCard {
        entity: request.entity,
        card_type: card_type_name(request.placement.card_type()),
        effect: match request.placement {
            CardPlacement::Event { effect: Some(_) } => "关键字一步",
            _ => "无效果体",
        },
        branches: Nat::new(branches.len() as _),
    });

    Ok(PlayCardOutcome {
        branches: fold_play_branches(branches),
        before,
        after,
        alloc,
        timing,
        events,
    })
}

/// 类型的中文短名（记录 / 观测用）。
fn card_type_name(card_type: ActionCardType) -> &'static str {
    match card_type {
        ActionCardType::Support => "支援",
        ActionCardType::Equipment => "装备",
        ActionCardType::Event => "事件",
    }
}

/// **打出条件**（D217）：`None` = 无条件；拒绝 -> [PlayCardError::ConditionNotMet]。
///
/// 只**读** `state`（此时还没克隆），所以拒绝时入参一个字节都没变。
/// 视角方由成对入口绑定（D29 / D34）：这里不传「敌我编号」，只把入口绑的那一侧交给条件函数。
fn check_play_condition(
    state: &DefState,
    request: &PlayCardRequest<'_>,
    playing_me: bool,
) -> Result<(), PlayCardError> {
    let Some(condition) = request.play_condition else {
        return Ok(());
    };
    let viewer = if playing_me {
        ViewSide::Me
    } else {
        ViewSide::Opp
    };
    match condition(state, viewer) {
        PlayCondition::Ok => Ok(()),
        PlayCondition::Rejected(reason) => Err(PlayCardError::ConditionNotMet(reason)),
    }
}

/// 支付 -> 从手牌 / 费用表 / 手牌卡状态表移除 -> 按 ActionCardType 落位。
///
/// 返回每支（局面 + 精确概率 + 待执行）；失败不改任何状态（只在克隆上算）。
fn place_card(
    user: &PlayerDefs,
    env: &PlayCardEnv<'_>,
    request: &PlayCardRequest<'_>,
) -> Result<Vec<(PlayerDefs, Fraction, Vec<FlowPending>)>, PlayCardError> {
    let Some(card) = user.hand().card_of(request.entity) else {
        return Err(PlayCardError::NotInHand);
    };
    // 支付（纯函数；付不起 -> Err、状态不变）。
    let Some(paid_dice) = user.dice().pay(&request.cost, request.same_color) else {
        return Err(PlayCardError::NotEnoughDice);
    };
    // 离开手牌：手牌区 + 当前费用表 + 手牌卡状态表（同一张牌的三个视角）。
    let hand = user
        .hand()
        .discard(request.entity)
        .unwrap_or_else(|| user.hand().clone());
    let costs = user.costs().without(request.entity);
    let hand_states = user
        .hand_states()
        .remove_card(request.entity)
        .unwrap_or_else(|| user.hand_states().clone());
    let base = user
        .clone()
        .with_dice(paid_dice)
        .with_hand(hand)
        .with_costs(costs)
        .with_hand_states(hand_states);

    // 按 ActionCardType 落位（三个变体一一对应；落位数据由调用方按卡牌定义给）。
    match (request.placement.card_type(), request.placement) {
        (ActionCardType::Support, CardPlacement::Support { value, duration }) => {
            let mut branches = Vec::new();
            for (result, probability) in base.supports().add_with_triggers(
                // Z6 生产写入路径：落位时把持续回合写进支援区条目（结束阶段 tick 用它递减 / 弃置）。
                SupportEntry::new(card, request.entity, value).with_support_duration(duration),
                env.support_triggers,
            ) {
                let player = base.clone().with_supports(result.zone);
                let pending = result
                    .pending
                    .into_iter()
                    .map(FlowPending::Support)
                    .collect();
                branches.push((player, probability, pending));
            }
            Ok(branches)
        }
        (
            ActionCardType::Equipment,
            CardPlacement::Equipment {
                character,
                slot,
                required_weapon,
                character_equip,
            },
        ) => {
            let zone = base
                .equipment_of(character)
                .cloned()
                .unwrap_or_else(|| EquipmentZone::new(character));
            let resolution = zone.equip_with_triggers(
                EquipmentEntry {
                    equip: request.entity,
                    card,
                    slot,
                },
                required_weapon,
                &character_equip,
                env.equipment_triggers,
            );
            if let EquipmentOutcome::Rejected(reject) = resolution.outcome {
                return Err(PlayCardError::CannotEquip(reject));
            }
            let player = base.with_equipment_zone(resolution.zone);
            let pending = resolution
                .pending
                .into_iter()
                .map(|item| FlowPending::Equipment {
                    owner: character,
                    pending: item,
                })
                .collect();
            Ok(vec![(player, Fraction::ONE, pending)])
        }
        (ActionCardType::Event, CardPlacement::Event { .. }) => {
            // 事件牌：结算完离场（已从手牌移除），不落任何区。
            Ok(vec![(base, Fraction::ONE, Vec::new())])
        }
        (_, _) => Err(PlayCardError::PlacementMismatch),
    }
}

/// **两侧派发**一个打出时机（规则书·其他规则 结算顺序 S7.5 第 1 条）。
///
/// timeline::resolve 的参数顺序就是结算先后：current 表（当前牌手）在前、other 表在后；
/// 两侧表内部各自按第 2 / 3 条排序（来源类别 -> 进场先后，顺序表在 rules::ops::order）。
/// 只派发、不执行（D186）。
fn resolve_play_pending(
    current: &PlayTriggers,
    other: &PlayTriggers,
    timing: PlayTiming,
    event: &Event,
) -> Vec<PlayPending> {
    resolve(current, other, timing, event)
        .into_iter()
        .map(Pending::into_queued)
        .collect()
}

/// 执行一批「打出前」待执行（调用方显式调用；各区 / 卡牌自己的效果函数指针）。
fn execute_play_pending(state: &mut DefState, pending: &[PlayPending]) {
    for item in pending {
        (item.effect)(state, &item.event, item.owner);
    }
}

/// 折叠回（D191）：只在「待执行为空」的分支上做（Queued 没有 PartialEq）。
fn fold_play_branches(branches: Vec<PlayBranch>) -> Vec<PlayBranch> {
    let mut folded: Vec<PlayBranch> = Vec::new();
    for branch in branches {
        if branch.pending.is_empty() {
            if let Some(existing) = folded
                .iter_mut()
                .find(|existing| existing.pending.is_empty() && existing.state == branch.state)
            {
                existing.probability = existing.probability.exact_add(branch.probability);
                continue;
            }
        }
        folded.push(branch);
    }
    folded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::families::action_card::areas::support::SupportZone;
    use crate::families::character::attachment::{AttachmentAllowSet, ReactionRegistry};
    use crate::families::character::character::CharacterState;
    use crate::families::character::damage::DamageEnv;
    use crate::families::character::damage_chain::executor::EntityAlloc;
    use crate::families::character::types::DamageType;
    use crate::families::character::zones::field::{
        CharacterSlot, CharacterZone, CharacterZoneTiming,
    };
    use crate::families::status::arcane_edict::ArcaneEdict;
    use crate::flow::game_end::Outcome;
    use crate::flow::pending::PendingSet;
    use crate::keywords::effect::TriggerBook;
    use crate::keywords::op::{DamageArgs, DamageSourceKind, KeywordOp};
    use crate::keywords::step::Step;
    use crate::keywords::target::TargetSelector;
    use crate::reactions::effect::EffectContext;
    use crate::resources::dice::DieFace;
    use crate::rules::ops::arcane::ArcanePlay;
    use crate::rules::ops::order::EffectSource;
    use crate::state::testing::{character, characters, hand, n};
    use gi_genius_invokation_engine::nat::ExactNat128;
    use gi_genius_invokation_engine::{nadd, nmul};

    fn noop_play(_state: &mut DefState, _event: &Event, _owner: EntityId) {}

    /// 骰池简写：n 个万能骰。
    fn omni(count: u32) -> crate::resources::dice::DicePool {
        crate::resources::dice::DicePool::empty().add(DieFace::Omni, Nat::new(count))
    }

    /// 我方：一个角色 + 指定手牌 + 指定骰池。
    fn me(hand_entities: &[u32], dice: crate::resources::dice::DicePool) -> PlayerDefs {
        PlayerDefs::new()
            .with_characters(characters(&[201]))
            .with_hand(hand(hand_entities))
            .with_dice(dice)
    }

    /// 任意骰费用。
    fn any_cost(count: u32) -> Vec<CostComponent> {
        vec![CostComponent::AnyColor(Nat::new(count))]
    }

    fn empty_env<'a>(
        triggers_me: &'a PlayTriggers,
        triggers_opp: &'a PlayTriggers,
        support_triggers: &'a SupportTriggers,
        equipment_triggers: &'a EquipmentTriggers,
    ) -> PlayCardEnv<'a> {
        PlayCardEnv {
            triggers: PlayTriggerBook {
                me: triggers_me,
                opp: triggers_opp,
            },
            support_triggers,
            equipment_triggers,
            keywords: None,
            defeat_cleanup: crate::flow::hooks::NO_DEFEAT_CLEANUP,
        }
    }

    fn play_effect_env() -> (
        PlayTriggers,
        PlayTriggers,
        SupportTriggers,
        EquipmentTriggers,
    ) {
        (
            PlayTriggers::new(),
            PlayTriggers::new(),
            SupportTriggers::new(),
            EquipmentTriggers::new(),
        )
    }

    #[test]
    fn support_card_goes_to_the_support_zone_and_pays_dice() {
        let before = DefState::from_players(me(&[211], omni(3)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: any_cost(2),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        assert_eq!(outcome.branches.len(), 1);
        let branch = outcome.single().expect("一支");
        let after = &branch.state;
        assert_eq!(after.me().hand().len(), Nat::ZERO);
        assert_eq!(after.me().supports().len(), Nat::new(1));
        assert!(after.me().supports().contains(n(211)));
        assert_eq!(after.me().dice().total(), Nat::new(1));
        // 纯函数
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert_eq!(before.me().dice().total(), Nat::new(3));
    }

    #[test]
    fn support_card_writes_its_duration_into_the_zone() {
        // Z6 生产写入路径：CardPlacement::Support 的 duration 落到 SupportEntry（结束阶段 tick 用）。
        let before = DefState::from_players(me(&[211], omni(1)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: any_cost(1),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::Round(n(2)),
            },
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        let branch = outcome.single().expect("一支");
        assert_eq!(
            branch.state.me().supports().support_duration_of(n(211)),
            Some(SupportDuration::Round(n(2)))
        );
    }

    #[test]
    fn full_support_zone_returns_one_branch_per_discard_choice() {
        let full_zone = {
            let triggers = SupportTriggers::new();
            let mut zone = SupportZone::new();
            for entity in [11, 12, 13, 14] {
                let branch = zone
                    .add_with_triggers(
                        SupportEntry::new(
                            CardId::new(300001),
                            Nat::new(entity),
                            SupportValue::None,
                        ),
                        &triggers,
                    )
                    .into_iter()
                    .next()
                    .expect("未满时只有一支");
                zone = branch.0.zone;
            }
            zone
        };
        let before = DefState::from_players(
            me(&[211], omni(0)).with_supports(full_zone),
            PlayerDefs::new(),
        );
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        assert_eq!(outcome.branches.len(), 4);
        let quarter = Fraction::new(ExactNat128::new(1), ExactNat128::new(4));
        let mut total = Fraction::ZERO;
        for branch in &outcome.branches {
            assert_eq!(branch.probability, quarter);
            total = total.exact_add(branch.probability);
            // 弃掉一张旧的 + 加一张新的 -> 仍 4 张
            assert_eq!(branch.state.me().supports().len(), Nat::new(4));
            assert!(branch.state.me().supports().contains(n(211)));
        }
        assert!(total.is_one());
    }

    #[test]
    fn equipment_card_goes_to_the_equipment_zone_of_the_target_character() {
        let before = DefState::from_players(me(&[212], omni(0)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(212),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Equipment {
                character: n(201),
                slot: SlotKind::Artifact,
                required_weapon: None,
                character_equip: CharacterEquip::OTHER_WEAPON,
            },
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        let after = &outcome.single().expect("一支").state;
        assert_eq!(after.me().hand().len(), Nat::ZERO);
        let zone = after.me().equipment_of(n(201)).expect("有装备栏");
        assert_eq!(zone.len(), Nat::new(1));
        assert_eq!(zone.entries()[0].equip, n(212));
        assert_eq!(zone.entries()[0].slot, SlotKind::Artifact);
    }

    #[test]
    fn equipment_rejection_returns_error_and_changes_nothing() {
        // 武器牌给「其他武器」角色 -> 装备栏拒绝；状态不变。
        let before = DefState::from_players(me(&[212], omni(0)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(212),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Equipment {
                character: n(201),
                slot: SlotKind::Weapon,
                required_weapon: Some(WeaponType::Bow),
                character_equip: CharacterEquip::OTHER_WEAPON,
            },
        };
        let error = play_card_me(&before, &env, &request).expect_err("装不上");
        assert_eq!(
            error,
            PlayCardError::CannotEquip(EquipReject::OtherWeaponCannotEquip)
        );
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert!(before.me().equipment().is_empty());
    }

    #[test]
    fn event_card_leaves_the_hand_and_lands_nowhere() {
        let before = DefState::from_players(me(&[213], omni(1)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(213),
            cost: any_cost(1),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        let branch = outcome.single().expect("一支");
        let after = &branch.state;
        assert_eq!(after.me().hand().len(), Nat::ZERO);
        assert!(after.me().supports().is_empty());
        assert!(after.me().equipment().is_empty());
        assert_eq!(after.me().dice().total(), Nat::ZERO);
        // 终局判定：回合数 0（第一回合还没开始）-> Ongoing。
        assert_eq!(branch.outcome, Outcome::Ongoing);
    }

    #[test]
    fn not_enough_dice_or_card_not_in_hand_changes_nothing() {
        let before = DefState::from_players(me(&[211], omni(1)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        // 不够付
        let too_expensive = PlayCardRequest {
            entity: n(211),
            cost: any_cost(3),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        };
        assert_eq!(
            play_card_me(&before, &env, &too_expensive).expect_err("不够付"),
            PlayCardError::NotEnoughDice
        );
        // 不在手牌
        let unknown = PlayCardRequest {
            entity: n(999),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        };
        assert_eq!(
            play_card_me(&before, &env, &unknown).expect_err("不在手牌"),
            PlayCardError::NotInHand
        );
        // 两次失败都不改入参
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert_eq!(before.me().dice().total(), Nat::new(1));
    }

    #[test]
    fn before_play_pending_is_executed_and_after_pending_is_returned() {
        // 「打出前」效果：给打出方 +1 火骰；「打出后」效果：只登记（本层不执行）。
        fn gain_pyro(state: &mut DefState, _event: &Event, _owner: EntityId) {
            let dice = state.me().dice().add(DieFace::Pyro, Nat::new(1));
            let player = state.me().clone().with_dice(dice);
            let next = state.clone().with_me(player);
            *state = next;
        }
        let before = DefState::from_players(me(&[211], omni(0)), PlayerDefs::new());
        let mut triggers = PlayTriggers::new();
        triggers.register(Trigger::new(
            PlayTiming::Before,
            n(211),
            EffectSource::Support,
            Nat::ZERO,
            gain_pyro,
        ));
        triggers.register(Trigger::new(
            PlayTiming::After,
            n(211),
            EffectSource::Support,
            Nat::ZERO,
            noop_play,
        ));
        let triggers_opp = PlayTriggers::new();
        let support_triggers = SupportTriggers::new();
        let equipment_triggers = EquipmentTriggers::new();
        let env = empty_env(
            &triggers,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        assert_eq!(outcome.before.len(), 1);
        assert_eq!(outcome.after.len(), 1);
        // 「打出前」已执行：+1 火骰（没有费用，所以留下来了）
        assert_eq!(
            outcome.single().expect("一支").state.me().dice().total(),
            Nat::new(1)
        );
        // 「打出后」未执行：本层只返回列表（这里用一个可观察的效果验证没跑）
        assert_eq!(outcome.after.len(), 1);
    }

    #[test]
    fn placement_card_type_maps_to_the_three_action_card_types() {
        assert_eq!(
            CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            }
            .card_type(),
            ActionCardType::Support
        );
        assert_eq!(
            CardPlacement::Equipment {
                character: n(201),
                slot: SlotKind::Artifact,
                required_weapon: None,
                character_equip: CharacterEquip::OTHER_WEAPON,
            }
            .card_type(),
            ActionCardType::Equipment
        );
        assert_eq!(
            CardPlacement::Event { effect: None }.card_type(),
            ActionCardType::Event
        );
    }

    #[test]
    fn opp_entry_binds_the_other_side() {
        let before = DefState::from_players(PlayerDefs::new(), me(&[211], omni(2)));
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: any_cost(2),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        let outcome = play_card_opp(&before, &env, &request).expect("打得出");
        let after = &outcome.single().expect("一支").state;
        assert_eq!(after.opp().hand().len(), Nat::ZERO);
        assert_eq!(after.opp().supports().len(), Nat::new(1));
        assert_eq!(after.opp().dice().total(), Nat::ZERO);
        assert_eq!(after.me(), before.me());
    }

    // ==================================================================
    // 打出条件（D217）：先于「比较是否付得起」
    // ==================================================================

    /// 打出条件：总是通过。
    fn condition_ok(_state: &DefState, _viewer: ViewSide) -> PlayCondition {
        PlayCondition::Ok
    }

    /// 打出条件：总是拒绝（中文调试文案）。
    fn condition_rejected(_state: &DefState, _viewer: ViewSide) -> PlayCondition {
        PlayCondition::Rejected("测试：这次不允许打出")
    }

    /// 打出条件：**视角方**场上至少 2 个角色才通过（看视角方绑定）。
    fn condition_needs_two_characters(state: &DefState, viewer: ViewSide) -> PlayCondition {
        let count = match viewer {
            ViewSide::Me => state.me().characters().len(),
            ViewSide::Opp => state.opp().characters().len(),
        };
        if count < Nat::new(2) {
            PlayCondition::Rejected("测试：视角方角色不足 2 个")
        } else {
            PlayCondition::Ok
        }
    }

    /// 打出条件：**视角方**骰子必须是 0（用来证明「打出前」触发还没跑）。
    fn condition_sees_zero_dice(state: &DefState, viewer: ViewSide) -> PlayCondition {
        let total = match viewer {
            ViewSide::Me => state.me().dice().total(),
            ViewSide::Opp => state.opp().dice().total(),
        };
        if total == Nat::ZERO {
            PlayCondition::Ok
        } else {
            PlayCondition::Rejected("测试：条件求值时骰子已经变了（打出前触发先跑了）")
        }
    }

    #[test]
    fn condition_passing_allows_the_play() {
        let before = DefState::from_players(me(&[213], omni(1)), PlayerDefs::new());
        let snapshot = before.clone();
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(213),
            cost: any_cost(1),
            same_color: None,
            play_condition: Some(condition_ok),
            placement: CardPlacement::Event { effect: None },
        };
        let outcome = play_card_me(&before, &env, &request).expect("条件通过 -> 打得出");
        let after = &outcome.single().expect("一支").state;
        assert_eq!(after.me().hand().len(), Nat::ZERO);
        assert_eq!(after.me().dice().total(), Nat::ZERO);
        // 纯函数：入参不变
        assert_eq!(before, snapshot);
    }

    #[test]
    fn condition_rejection_returns_error_and_changes_nothing() {
        let before = DefState::from_players(me(&[211], omni(2)), PlayerDefs::new());
        let snapshot = before.clone();
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: any_cost(2),
            same_color: None,
            play_condition: Some(condition_rejected),
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        let error = play_card_me(&before, &env, &request).expect_err("条件不通过 -> 打不出");
        assert_eq!(
            error,
            PlayCardError::ConditionNotMet("测试：这次不允许打出")
        );
        // 状态一个字节都不变：整局 / 手牌 / 骰池 / 支援区
        assert_eq!(before, snapshot);
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert_eq!(before.me().dice().total(), Nat::new(2));
        assert!(before.me().supports().is_empty());
    }

    #[test]
    fn condition_is_checked_before_the_cost_check() {
        // 骰子只有 1 个、费用要 3 个（付不起）；同时条件也拒绝。
        // D217：条件在「比较是否付得起」**之前** -> 报的必须是条件错误，不是「骰子不够」。
        let before = DefState::from_players(me(&[211], omni(1)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let rejected = PlayCardRequest {
            entity: n(211),
            cost: any_cost(3),
            same_color: None,
            play_condition: Some(condition_rejected),
            placement: CardPlacement::Event { effect: None },
        };
        assert_eq!(
            play_card_me(&before, &env, &rejected).expect_err("条件先拒绝"),
            PlayCardError::ConditionNotMet("测试：这次不允许打出")
        );
        // 同一份请求、同样的骰子，只把条件去掉 -> 这才轮到「付不起」
        let unconditional = PlayCardRequest {
            entity: n(211),
            cost: any_cost(3),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        };
        assert_eq!(
            play_card_me(&before, &env, &unconditional).expect_err("付不起"),
            PlayCardError::NotEnoughDice
        );
        // 两次失败都只读
        assert_eq!(before.me().dice().total(), Nat::new(1));
        assert_eq!(before.me().hand().len(), Nat::new(1));
    }

    #[test]
    fn condition_viewer_is_bound_by_the_paired_entry() {
        // 我方 2 角色、敌方 1 角色；同一个条件（视角方至少 2 角色）：
        // play_card_me -> 视角我方 -> 通过；play_card_opp -> 视角敌方 -> 拒绝。
        let before = DefState::from_players(
            me(&[211], omni(2)).with_characters(characters(&[201, 202])),
            me(&[212], omni(2)),
        );
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let mine = PlayCardRequest {
            entity: n(211),
            cost: any_cost(2),
            same_color: None,
            play_condition: Some(condition_needs_two_characters),
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        assert!(play_card_me(&before, &env, &mine).is_ok());
        let theirs = PlayCardRequest {
            entity: n(212),
            cost: any_cost(2),
            same_color: None,
            play_condition: Some(condition_needs_two_characters),
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        assert_eq!(
            play_card_opp(&before, &env, &theirs).expect_err("敌方只有 1 个角色"),
            PlayCardError::ConditionNotMet("测试：视角方角色不足 2 个")
        );
    }

    #[test]
    fn condition_is_evaluated_before_the_before_play_dispatch() {
        // 「打出前」触发：给打出方 +1 火骰。
        fn gain_pyro(state: &mut DefState, _event: &Event, _owner: EntityId) {
            let dice = state.me().dice().add(DieFace::Pyro, Nat::new(1));
            let player = state.me().clone().with_dice(dice);
            let next = state.clone().with_me(player);
            *state = next;
        }
        let before = DefState::from_players(me(&[211], omni(0)), PlayerDefs::new());
        let mut triggers = PlayTriggers::new();
        triggers.register(Trigger::new(
            PlayTiming::Before,
            n(211),
            EffectSource::Support,
            Nat::ZERO,
            gain_pyro,
        ));
        let triggers_opp = PlayTriggers::new();
        let support_triggers = SupportTriggers::new();
        let equipment_triggers = EquipmentTriggers::new();
        let env = empty_env(
            &triggers,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: Vec::new(),
            same_color: None,
            play_condition: Some(condition_sees_zero_dice),
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        // 待复核：本层把条件放在「打出前」派发**之前**，所以条件看到的是骰子 0 的原局面。
        // 若将来改成派发之后求值，条件会看到 1 个火骰并拒绝 —— 这个用例会失败（有意的钉子）。
        let outcome = play_card_me(&before, &env, &request).expect("条件看到的是触发前的局面");
        assert_eq!(
            outcome.single().expect("一支").state.me().dice().total(),
            Nat::new(1)
        );
    }

    // ------------------------------------------------------------------
    // D227：事件牌效果体走关键字一步
    // ------------------------------------------------------------------

    /// 建一个带关键字层依赖的 env（测试用空反应表 / 空触发）。
    fn env_with_keywords<'a>(
        triggers_me: &'a PlayTriggers,
        triggers_opp: &'a PlayTriggers,
        support_triggers: &'a SupportTriggers,
        equipment_triggers: &'a EquipmentTriggers,
        registry: &'a ReactionRegistry,
        context: &'a EffectContext,
    ) -> PlayCardEnv<'a> {
        PlayCardEnv {
            triggers: PlayTriggerBook {
                me: triggers_me,
                opp: triggers_opp,
            },
            support_triggers,
            equipment_triggers,
            keywords: Some(KeywordEnv {
                triggers: TriggerBook::empty(),
                damage: DamageEnv {
                    allow: AttachmentAllowSet::STANDARD,
                    rules: &[],
                    registry,
                },
                mastery: Nat::ZERO,
                increases: &[],
                effect_context: context,
                alloc: EntityAlloc::starting_at(n(1000)),
            }),
            defeat_cleanup: crate::flow::hooks::NO_DEFEAT_CLEANUP,
        }
    }

    fn event_step(amount: u32) -> Step {
        Step::new(vec![KeywordOp::Damage(DamageArgs::new(
            TargetSelector::opponent_active(),
            Nat::new(amount),
            DamageType::Physical,
            DamageSourceKind::Other,
        ))])
    }

    #[test]
    fn event_card_step_effect_runs_at_the_step_boundary() {
        let before = DefState::from_players(
            me(&[213], omni(1)),
            PlayerDefs::new().with_characters(characters(&[401])),
        );
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let env = env_with_keywords(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
            &registry,
            &context,
        );
        let step = event_step(3);
        let request = PlayCardRequest {
            entity: n(213),
            cost: any_cost(1),
            same_color: None,
            placement: CardPlacement::Event {
                effect: Some(&step),
            },
            play_condition: None,
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        let branch = outcome.single().expect("一支");
        // 牌离手、付费完成、一步伤害落地（对方出战 10 -> 7）。
        assert_eq!(branch.state.me().hand().len(), Nat::ZERO);
        assert_eq!(branch.state.me().dice().total(), Nat::ZERO);
        assert_eq!(
            branch
                .state
                .opp()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(7)
        );
        // 记录：一步 -> flush -> 打出牌。
        let kinds: Vec<&'static str> = outcome
            .events
            .iter()
            .map(|event| match event {
                FlowRecord::Step { .. } => "一步",
                FlowRecord::Flush { .. } => "flush",
                FlowRecord::PlayCard { .. } => "打出牌",
                _ => "其它",
            })
            .collect();
        assert_eq!(kinds, vec!["一步", "flush", "打出牌"]);
        // 「打出后」仍然只派发、不执行（这里没有登记，列表为空）。
        assert!(outcome.after.is_empty());
    }

    #[test]
    fn event_card_step_without_keyword_env_is_rejected_and_changes_nothing() {
        let before = DefState::from_players(
            me(&[213], omni(1)),
            PlayerDefs::new().with_characters(characters(&[401])),
        );
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        // 没有 keywords -> 事件牌一步效果体无法执行。
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let step = event_step(3);
        let request = PlayCardRequest {
            entity: n(213),
            cost: any_cost(1),
            same_color: None,
            placement: CardPlacement::Event {
                effect: Some(&step),
            },
            play_condition: None,
        };
        let error = play_card_me(&before, &env, &request).expect_err("没有关键字依赖");
        assert_eq!(error, PlayCardError::MissingKeywordEnv);
        // 入参一个字节都不改。
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert_eq!(before.me().dice().total(), Nat::new(1));
    }

    #[test]
    fn event_card_step_keeps_before_and_after_at_the_step_boundary() {
        // 打出前：+1 火骰（执行）；打出后：只派发（返回、不执行）。
        fn gain_pyro(state: &mut DefState, _event: &Event, _owner: EntityId) {
            let dice = state.me().dice().add(DieFace::Pyro, Nat::new(1));
            let player = state.me().clone().with_dice(dice);
            let next = state.clone().with_me(player);
            *state = next;
        }
        let before = DefState::from_players(
            me(&[213], omni(0)),
            PlayerDefs::new().with_characters(characters(&[401])),
        );
        let mut triggers = PlayTriggers::new();
        triggers.register(Trigger::new(
            PlayTiming::Before,
            n(213),
            EffectSource::Support,
            Nat::ZERO,
            gain_pyro,
        ));
        triggers.register(Trigger::new(
            PlayTiming::After,
            n(213),
            EffectSource::Support,
            Nat::ZERO,
            noop_play,
        ));
        let triggers_me = triggers;
        let triggers_opp = PlayTriggers::new();
        let support_triggers = SupportTriggers::new();
        let equipment_triggers = EquipmentTriggers::new();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let env = env_with_keywords(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
            &registry,
            &context,
        );
        let step = event_step(2);
        let request = PlayCardRequest {
            entity: n(213),
            cost: Vec::new(),
            same_color: None,
            placement: CardPlacement::Event {
                effect: Some(&step),
            },
            play_condition: None,
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        assert_eq!(outcome.before.len(), 1);
        assert_eq!(outcome.after.len(), 1);
        // 「打出前」已经执行（+1 火骰留着），一步伤害也落地。
        assert_eq!(
            outcome.single().expect("一支").state.me().dice().total(),
            Nat::new(1)
        );
        assert_eq!(
            outcome
                .single()
                .expect("一支")
                .state
                .opp()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(8)
        );
    }

    // ------------------------------------------------------------------
    // S7.5 第 1 条：当前牌手优先（两侧注册表）
    // ------------------------------------------------------------------

    /// 我方「顺序记号」角色：生命 0 / 上限 MAX（记号 hp * 10 + digit 不受钳制）。
    fn marking_me(hand_entities: &[u32], dice: crate::resources::dice::DicePool) -> PlayerDefs {
        let zone = CharacterZone::try_from_slots(vec![CharacterSlot::new(
            n(201),
            CharacterState::new(Nat::ZERO, Nat::MAX, Nat::ZERO, Nat::new(2)),
        )])
        .expect("合法槽位");
        PlayerDefs::new()
            .with_characters(zone)
            .with_hand(hand(hand_entities))
            .with_dice(dice)
    }

    /// 给我方出战角色追加一位数字（顺序记号）。
    fn mark_me_digit(state: &mut DefState, digit: Nat) {
        let player = state.me();
        let Some(slot) = player.characters().active() else {
            return;
        };
        let scaled = nmul!(slot.state.hp(), Nat::new(10));
        let marked = slot.state.hp_set(nadd!(scaled, digit));
        if let Some(zone) = player.characters().with_state(slot.character, marked) {
            let next = state.clone().with_me(player.clone().with_characters(zone));
            *state = next;
        }
    }

    fn mark_me_one(state: &mut DefState, _event: &Event, _owner: EntityId) {
        mark_me_digit(state, Nat::new(1));
    }

    fn mark_me_two(state: &mut DefState, _event: &Event, _owner: EntityId) {
        mark_me_digit(state, Nat::new(2));
    }

    #[test]
    fn play_card_triggers_follow_current_player_first_with_two_registries() {
        // 我方打出：当前牌手 = 我方 -> 「打出前」先 mark_one（我方）再 mark_two（对方）。
        let before = DefState::from_players(marking_me(&[211], omni(0)), PlayerDefs::new());
        let mut triggers_me = PlayTriggers::new();
        triggers_me.register(Trigger::new(
            PlayTiming::Before,
            n(211),
            EffectSource::Support,
            Nat::ZERO,
            mark_me_one,
        ));
        triggers_me.register(Trigger::new(
            PlayTiming::After,
            n(211),
            EffectSource::Support,
            Nat::ZERO,
            mark_me_one,
        ));
        let mut triggers_opp = PlayTriggers::new();
        triggers_opp.register(Trigger::new(
            PlayTiming::Before,
            n(999),
            EffectSource::Support,
            Nat::ZERO,
            mark_me_two,
        ));
        triggers_opp.register(Trigger::new(
            PlayTiming::After,
            n(999),
            EffectSource::Support,
            Nat::ZERO,
            mark_me_two,
        ));
        let support_triggers = SupportTriggers::new();
        let equipment_triggers = EquipmentTriggers::new();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(211),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        let outcome = play_card_me(&before, &env, &request).expect("打得出");
        let branch = outcome.single().expect("一支");
        // 「打出前」已执行：12 = 先我方 1、再对方 2；反过来会是 21。
        assert_eq!(
            branch
                .state
                .me()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(12)
        );
        // 「打出后」未执行：按返回顺序由调用方执行 -> 12 -> 121 -> 1212。
        let mut state = branch.state.clone();
        for item in &outcome.after {
            (item.effect)(&mut state, &item.event, item.owner);
        }
        assert_eq!(
            state.me().characters().active().expect("有").state.hp(),
            Nat::new(1212)
        );
    }

    #[test]
    fn play_card_triggers_follow_current_player_first_when_opp_plays() {
        // 敌方打出：当前牌手 = 敌方 -> 「打出前」先 mark_two（敌方）再 mark_one（我方）。
        let before = DefState::from_players(
            marking_me(&[211], omni(0)),
            PlayerDefs::new().with_hand(hand(&[311])),
        );
        let mut triggers_me = PlayTriggers::new();
        triggers_me.register(Trigger::new(
            PlayTiming::Before,
            n(201),
            EffectSource::Support,
            Nat::ZERO,
            mark_me_one,
        ));
        let mut triggers_opp = PlayTriggers::new();
        triggers_opp.register(Trigger::new(
            PlayTiming::Before,
            n(311),
            EffectSource::Support,
            Nat::ZERO,
            mark_me_two,
        ));
        let support_triggers = SupportTriggers::new();
        let equipment_triggers = EquipmentTriggers::new();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(311),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Support {
                value: SupportValue::None,
                duration: SupportDuration::WholeGame,
            },
        };
        let outcome = play_card_opp(&before, &env, &request).expect("打得出");
        // 21 = 先敌方 2、再我方 1。
        assert_eq!(
            outcome
                .single()
                .expect("一支")
                .state
                .me()
                .characters()
                .active()
                .expect("有")
                .state
                .hp(),
            Nat::new(21)
        );
    }
    #[test]
    fn card_play_timing_is_fast_unless_the_card_has_the_combat_action_rule() {
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        // 我方：普通事件牌（无「战斗行动」规则）-> 快速行动（S3.1）。
        let before = DefState::from_players(me(&[213], omni(1)), PlayerDefs::new());
        let request = PlayCardRequest {
            entity: n(213),
            cost: any_cost(1),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        };
        let fast = play_card_me(&before, &env, &request).expect("打得出");
        assert_eq!(fast.timing, Timing::Fast);
        // 带「战斗行动（行动牌）」规则 -> 战斗行动（S8.8）。
        let combat = play_card_me_with_timing(&before, &env, &request, true).expect("打得出");
        assert_eq!(combat.timing, Timing::Combat);
        // 敌方入口同样按时机判定。
        let opp_before = DefState::from_players(PlayerDefs::new(), me(&[214], omni(1)));
        let opp_request = PlayCardRequest {
            entity: n(214),
            cost: any_cost(1),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        };
        let opp_fast = play_card_opp(&opp_before, &env, &opp_request).expect("打得出");
        assert_eq!(opp_fast.timing, Timing::Fast);
        let opp_combat =
            play_card_opp_with_timing(&opp_before, &env, &opp_request, true).expect("打得出");
        assert_eq!(opp_combat.timing, Timing::Combat);
    }

    // ------------------------------------------------------------------
    // Z12 / K311：秘传揭令（前置判定 + 成功 record_play）
    // ------------------------------------------------------------------

    fn arcane_event_request(entity: u32) -> PlayCardRequest<'static> {
        PlayCardRequest {
            entity: n(entity),
            cost: Vec::new(),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        }
    }

    #[test]
    fn arcane_play_consumes_the_edict_and_rejects_the_second_play() {
        let before = DefState::from_players(me(&[211, 212], omni(0)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        // 第一次：揭令存在、未打出过 -> 成功；打出后张数 +1、揭令被消耗。
        let outcome = play_card_me_arcane(
            &before,
            &env,
            &arcane_event_request(211),
            ArcaneEdict::new(),
        )
        .expect("第一次可打");
        assert_eq!(outcome.edict.played(), Nat::new(1));
        assert!(!outcome.edict.available());
        assert_eq!(
            outcome.play.single().expect("一支").state.me().hand().len(),
            Nat::new(1)
        );
        // 第二次：用第一次交出的揭令 -> 本局已打出过秘传 -> 拒绝（入参不变）。
        let error = play_card_me_arcane(&before, &env, &arcane_event_request(212), outcome.edict)
            .expect_err("第二次被拒绝");
        assert_eq!(
            error,
            PlayCardError::ArcaneNotPlayable(ArcanePlay::AlreadyPlayed)
        );
        assert_eq!(before.me().hand().len(), Nat::new(2));
        // 揭令被其它效果消耗（未打出）-> NoEdict。
        let consumed = ArcaneEdict::new().consume().expect("新局有揭令");
        assert_eq!(
            play_card_me_arcane(&before, &env, &arcane_event_request(212), consumed)
                .expect_err("没有揭令"),
            PlayCardError::ArcaneNotPlayable(ArcanePlay::NoEdict)
        );
    }

    #[test]
    fn arcane_play_opp_binds_the_other_side_and_records_the_edict() {
        let before =
            DefState::from_players(PlayerDefs::new(), PlayerDefs::new().with_hand(hand(&[311])));
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let outcome = play_card_opp_arcane(
            &before,
            &env,
            &arcane_event_request(311),
            ArcaneEdict::new(),
        )
        .expect("第一次可打");
        assert_eq!(outcome.edict.played(), Nat::new(1));
        assert!(!outcome.edict.available());
        assert_eq!(
            outcome
                .play
                .single()
                .expect("一支")
                .state
                .opp()
                .hand()
                .len(),
            Nat::ZERO
        );
        // 我方一个字节都没动。
        assert_eq!(outcome.play.single().expect("一支").state.me(), before.me());
    }
    // ------------------------------------------------------------------
    // L1 料理 / 饱腹（S8.1）
    // ------------------------------------------------------------------

    fn food_request() -> PlayCardRequest<'static> {
        PlayCardRequest {
            entity: n(211),
            cost: any_cost(1),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        }
    }

    #[test]
    fn food_play_marks_the_target_satiated() {
        let before = DefState::from_players(me(&[211], omni(1)), PlayerDefs::new());
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let outcome =
            play_card_me_food(&before, &env, &food_request(), n(201), None).expect("料理");
        let branch = outcome.play.single().expect("一支");
        // S8.1 第 1 条：料理生效后目标进入「饱腹」。
        assert!(branch
            .state
            .me()
            .characters()
            .slot_of(n(201))
            .expect("在场")
            .state
            .satiated());
        // 纯函数：入参角色状态不变。
        assert!(!before
            .me()
            .characters()
            .slot_of(n(201))
            .expect("在场")
            .state
            .satiated());
    }

    #[test]
    fn food_play_rejects_a_satiated_target_without_changing_state() {
        let satiated = character().with_satiated();
        let characters_with_satiated = characters(&[201])
            .with_state(n(201), satiated)
            .expect("角色在区里");
        let before = DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters_with_satiated)
                .with_hand(hand(&[211]))
                .with_dice(omni(1)),
            PlayerDefs::new(),
        );
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        // S8.1 第 2 条：饱腹角色不能成为其他料理的目标。
        let error = play_card_me_food(&before, &env, &food_request(), n(201), None)
            .expect_err("饱腹目标应被拒绝");
        assert_eq!(error, PlayCardError::FoodTargetNotAllowed);
        // 拒绝不扣费 / 不离手。
        assert!(before.me().hand().contains(n(211)));
        assert_eq!(before.me().dice().total(), Nat::new(1));
        // 目标不在场同样拒绝。
        assert_eq!(
            play_card_me_food(&before, &env, &food_request(), n(999), None)
                .expect_err("目标不在场"),
            PlayCardError::FoodTargetNotAllowed
        );
    }

    #[test]
    fn opp_food_play_marks_the_opponent_target_satiated() {
        let before = DefState::from_players(
            PlayerDefs::new(),
            PlayerDefs::new()
                .with_characters(characters(&[401]))
                .with_hand(hand(&[214]))
                .with_dice(omni(1)),
        );
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let request = PlayCardRequest {
            entity: n(214),
            cost: any_cost(1),
            same_color: None,
            play_condition: None,
            placement: CardPlacement::Event { effect: None },
        };
        let outcome = play_card_opp_food(&before, &env, &request, n(401), None).expect("料理");
        let branch = outcome.play.single().expect("一支");
        assert!(branch
            .state
            .opp()
            .characters()
            .slot_of(n(401))
            .expect("在场")
            .state
            .satiated());
        // 我方没有角色，不受影响。
        assert!(branch.state.me().characters().is_empty());
    }
    // ------------------------------------------------------------------
    // L1 / L2 复苏料理（S8.1 第 4 条 + 卡面「复苏」）
    // ------------------------------------------------------------------

    fn noop_character(_zone: &mut CharacterZone, _event: &Event, _owner: EntityId) {}

    /// 我方：角色区空、击倒区有 201；手牌 211、1 个万能骰（够付料理费用）。
    fn defeated_me_state() -> DefState {
        let defeated = CharacterSlot::new(
            n(201),
            CharacterState::new(Nat::new(10), Nat::new(10), Nat::ZERO, Nat::new(2)),
        );
        DefState::from_players(
            PlayerDefs::new()
                .with_defeated(vec![defeated])
                .with_hand(hand(&[211]))
                .with_dice(omni(1)),
            PlayerDefs::new(),
        )
    }

    #[test]
    fn revive_food_puts_the_target_back_and_marks_it_satiated() {
        let before = defeated_me_state();
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        // 复苏会派发「进入角色区」（state::flows::revive）。
        let mut character_triggers = CharacterZoneTriggers::new();
        character_triggers.register(Trigger::new(
            CharacterZoneTiming::Entered,
            n(201),
            EffectSource::CharacterCard,
            Nat::ZERO,
            noop_character,
        ));
        let revive = FoodReviveEnv {
            characters: &character_triggers,
            revived_this_round: Nat::ZERO,
            hp: n(1),
            card: CardId::new(333_009),
        };
        let outcome = play_card_me_food(&before, &env, &food_request(), n(201), Some(revive))
            .expect("复苏料理");
        // 复苏计数 +1，调用方写回。
        assert_eq!(outcome.revived_this_round, Some(n(1)));
        let branch = outcome.play.single().expect("一支");
        let slot = branch
            .state
            .me()
            .characters()
            .slot_of(n(201))
            .expect("回到角色区");
        assert_eq!(slot.state.hp(), n(1));
        // S8.1 第 1 条：复苏（料理生效）后进入「饱腹」。
        assert!(slot.state.satiated());
        assert!(branch.state.me().defeated().is_empty());
        // 角色区待执行（进入角色区）随分支交回（D186）。
        assert!(branch
            .pending
            .me
            .iter()
            .any(|pending| matches!(pending, FlowPending::CharacterZone(_))));
        // 纯函数：入参不变。
        assert_eq!(before.me().characters().len(), Nat::ZERO);
        assert_eq!(before.me().defeated().len(), 1);
    }

    #[test]
    fn revive_food_is_rejected_when_the_round_count_is_used_up() {
        let before = defeated_me_state();
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let env = empty_env(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
        );
        let character_triggers = CharacterZoneTriggers::new();
        let revive = FoodReviveEnv {
            characters: &character_triggers,
            revived_this_round: n(1),
            hp: n(1),
            card: CardId::new(333_009),
        };
        // S8.1 第 4 条：本回合已通过料理复苏过 -> 拒绝，且不扣费 / 不离手。
        let error = play_card_me_food(&before, &env, &food_request(), n(201), Some(revive))
            .expect_err("复苏次数用尽");
        assert_eq!(
            error,
            PlayCardError::FoodReviveNotAllowed(ReviveError::NoReviveLeft)
        );
        assert!(before.me().hand().contains(n(211)));
        assert_eq!(before.me().dice().total(), Nat::new(1));
        assert_eq!(before.me().defeated().len(), 1);
    }

    #[test]
    fn defeat_cleanup_runs_before_the_terminal_judgement() {
        // D255：结算 -> 击倒清理 -> 判终局。对方 1 HP，事件牌一步 2 点物理后 HP=0，
        // 未清理时角色仍在角色区 -> Ongoing；注入清理后角色离场 -> MeWins。
        fn cleanup_ko(
            state: &DefState,
            side: crate::keywords::target::Perspective,
        ) -> (DefState, PendingSet) {
            let (next, pending) = {
                let player = if side == crate::keywords::target::Perspective::Me {
                    state.me().clone()
                } else {
                    state.opp().clone()
                };
                let kept: Vec<CharacterSlot> = player
                    .characters()
                    .slots()
                    .iter()
                    .filter(|slot| slot.state.hp() != Nat::ZERO)
                    .cloned()
                    .collect();
                let zone = CharacterZone::try_from_slots(kept).unwrap_or_default();
                let player = player.with_characters(zone);
                let next = if side == crate::keywords::target::Perspective::Me {
                    state.clone().with_me(player)
                } else {
                    state.clone().with_opp(player)
                };
                (next, PendingSet::new())
            };
            (next, pending)
        }
        let opp = PlayerDefs::new().with_characters(
            CharacterZone::try_from_slots(vec![CharacterSlot::new(
                n(401),
                CharacterState::new(n(1), n(10), Nat::ZERO, n(2)),
            )])
            .expect("合法槽位"),
        );
        let before = DefState::from_players(me(&[213], omni(0)), opp).with_round(n(1));
        let (triggers_me, triggers_opp, support_triggers, equipment_triggers) = play_effect_env();
        let registry = ReactionRegistry::new();
        let context = EffectContext::new(true, Nat::ZERO, Nat::ZERO);
        let mut env = env_with_keywords(
            &triggers_me,
            &triggers_opp,
            &support_triggers,
            &equipment_triggers,
            &registry,
            &context,
        );
        let step = event_step(2);
        let request = PlayCardRequest {
            entity: n(213),
            cost: Vec::new(),
            same_color: None,
            placement: CardPlacement::Event {
                effect: Some(&step),
            },
            play_condition: None,
        };
        let plain = play_card_me(&before, &env, &request).expect("打得出");
        assert_eq!(plain.single().expect("一支").outcome, Outcome::Ongoing);
        env.defeat_cleanup = cleanup_ko;
        let cleaned = play_card_me(&before, &env, &request).expect("打得出");
        let branch = cleaned.single().expect("一支");
        assert!(branch.state.opp().characters().is_empty());
        assert_eq!(branch.outcome, Outcome::MeWins);
    }
}
