//! L2 类型规则行（5 个）：支援 / 装备 / 事件 / 角色状态 / 出战状态。
//!
//! 口径：类型层只声明「这个类型的基础操作」；具体卡牌效果不在本层（gi-genius-invokation-cards
//! 的 src/cards/** 不动，D240）。

use super::ops::equip::{
    ARTIFACT_SLOT, TALENT_SLOT, TECHNIQUE_SLOT, WEAPON_SLOT,
};
use super::ops::support::SUPPORT_CAP;
use super::{
    Evidence, Layer, NumRule, OpeningSpec, RuleKind, RuleOp, RuleRow, Stage,
};
use crate::families::action_card::ActionCardType;
use crate::families::status::StatusType;

pub static ROWS: &[RuleRow] = &[
    // ------------------------------------------------------------------
    // L2 支援
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::CardType,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Support, RuleOp::Order, RuleOp::Opening],
        stages: &[Stage::Opening, Stage::ActionPhase],
        evidence: &[
            Evidence::new(302213, "随机生成「伙伴」到场上，直到填满双方支援区（支援区有上限）"),
            Evidence::new(322014, "如果对方的支援区未满，则将此牌转移到对方的支援区"),
        ],
        basis: &[
            "规则书·游戏区域 III：支援区最多 4 张；占满后打出新支援牌需先选择一张弃置",
            "规则书·其他规则 结算顺序：支援牌效果按放置先后顺序结算",
            "支援的值 = None / Flag(bool) / Count(Nat) 三选一互斥（分工计划 §4.4.15）",
            "元素幻变是支援的特例：行动阶段开始时那套开局（见 ops::opening）",
        ],
        action_card_type: Some(ActionCardType::Support),
        nums: &[NumRule::new("支援区上限", 4)],
        opening: Some(OpeningSpec::ElementalTransmutation),
        support_cap: Some(SUPPORT_CAP),
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L2 装备
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::CardType,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip, RuleOp::Order],
        stages: &[Stage::ActionPhase],
        evidence: &[
            Evidence::new(311204, "「弓」角色才能装备。角色最多装备 1 件「武器」"),
            Evidence::new(312001, "角色最多装备 1 件「圣遗物」"),
            Evidence::new(313010, "角色最多装备 1 个「特技」"),
            Evidence::new(217051, "牌组中包含白术，才能加入牌组；装备此牌后立刻使用一次愈气全形论"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器、1 张圣遗物、1 张特技，和任意张其他类型装备",
            "卡面：「『X』角色才能装备」——武器类型门来自卡面，规则书未写",
            "规则书未规定同槽替换细节与天赋同种覆盖 -> Lead 裁定 2/3 待实测、待用户复核",
        ],
        action_card_type: Some(ActionCardType::Equipment),
        slots: &[WEAPON_SLOT, ARTIFACT_SLOT, TALENT_SLOT, TECHNIQUE_SLOT],
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L2 事件
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::CardType,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Timing, RuleOp::Arcane],
        stages: &[Stage::Opening, Stage::ActionPhase],
        evidence: &[
            Evidence::new(330001, "整局游戏只能打出一张「秘传」卡牌；这张牌一定在你的起始手牌中"),
            Evidence::new(116121, "战斗行动：选一个敌方角色，对其造成 1 点岩元素伤害"),
        ],
        basis: &[
            "规则书·进行行动 快速行动【打出手牌】：打出手牌默认快速行动；带「战斗行动」规则时改为战斗行动",
            "规则书·其他规则 结算顺序",
            "规则书·特殊词条 秘传：起手必带；D17/D90 起手实现",
        ],
        action_card_type: Some(ActionCardType::Event),
        opening: Some(OpeningSpec::ArcaneLegendOpeningHand),
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L2 角色状态
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::CardType,
        kind: RuleKind::Mechanism,
        stages: &[Stage::ActionPhase],
        basis: &[
            "规则书·游戏区域 II：状态附属在角色牌上",
            "规则书·特殊词条 持续回合：每个结束阶段 -1，耗尽立刻弃置",
        ],
        status_type: Some(StatusType::CharacterStatus),
        nums: &[NumRule::new("持续回合每结束阶段递减", 1)],
        ops: &[RuleOp::Lifetime, RuleOp::Order],
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L2 出战状态
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::CardType,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Lifetime, RuleOp::Order],
        stages: &[Stage::ActionPhase, Stage::Damage],
        basis: &[
            "规则书·游戏区域 II(3)：出战状态附属在出战角色上；出战角色切换时转移给新出战角色",
            "规则书·其他规则 结算顺序：阵营出战状态位于角色装备和状态之后、召唤物之前",
            "规则书·特殊词条 持续回合：每个结束阶段 -1",
        ],
        status_type: Some(StatusType::CombatStatus),
        nums: &[NumRule::new("持续回合每结束阶段递减", 1)],
        ..RuleRow::EMPTY
    },
];
