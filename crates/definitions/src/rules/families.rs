//! L1 牌族规则行（5 个）：行动牌 / 角色 / 召唤物 / 状态 / 敌人。
//!
//! 口径：牌族层只**声明**该族的基础操作与开局钩子；区域状态与操作由各区域块实现
//! （D20：框架只给接口），本层不重复实现、也不新建状态容器。

use super::{Layer, NumRule, OpeningSpec, RuleKind, RuleOp, RuleRow, Stage};
use crate::base::CardFamily;

pub static ROWS: &[RuleRow] = &[
    // ------------------------------------------------------------------
    // L1 行动牌
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::Family,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Opening],
        stages: &[Stage::Opening, Stage::ActionPhase],
        basis: &[
            "规则书·《七圣召唤》游戏准备：双方各抓 5 张起始手牌，之后各有 1 次机会替换任意数量的手牌",
            "规则书·游戏区域 V/VI：起手从牌堆抓 5 张；手牌上限 10；每回合结束时双方各抓 2 张",
            "D17 / D90：起手先按「秘传」标签抓，再补足到 5 张（HandState::draw_opening 已实现；换牌 / 结束阶段抓牌待接入）",
        ],
        family: Some(CardFamily::ActionCard),
        nums: &[
            NumRule::new("起手张数", 5),
            NumRule::new("换牌机会", 1),
            NumRule::new("结束阶段抓牌", 2),
        ],
        opening: Some(OpeningSpec::OpeningHand),
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L1 角色
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::Family,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Opening],
        stages: &[Stage::Opening, Stage::ActionPhase, Stage::Damage],
        basis: &[
            "规则书·游戏区域 I/II：角色区、出战角色、角色附属（装备 / 状态 / 出战状态）",
            "规则书·其他规则 击倒：生命值降为 0 后，在技能或效果结算完成时才被击倒（弃置装备与状态、清空充能）",
            "规则书·其他规则 免于被击倒：不视为被击倒过（装备 / 状态 / 充能不清理）",
            "规则书·其他规则 转换形态：转换后名称、元素类型、标签、技能可能改变",
            "families/character/{character,attachment,damage}.rs 已有生命 / 充能 / 元素附着 / 反应注册；击倒与形态转换待接入",
        ],
        family: Some(CardFamily::Character),
        opening: Some(OpeningSpec::FirstCharacterChoice),
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L1 召唤物
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::Family,
        kind: RuleKind::Mechanism,
        stages: &[Stage::ActionPhase],
        basis: &[
            "规则书·游戏区域 IV：召唤物区最多放置 4 个召唤物；占满后无法生成新的召唤物（与支援区不同，不能选择弃置）",
            "规则书·特殊词条 可用次数：每次触发消耗 1 次，耗尽立刻弃置",
            "规则书·其他规则 结算顺序：召唤物效果按召唤先后顺序结算",
        ],
        family: Some(CardFamily::Summon),
        nums: &[NumRule::new("召唤区上限", 4)],
        ops: &[RuleOp::Lifetime, RuleOp::Order],
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L1 状态
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::Family,
        kind: RuleKind::Mechanism,
        stages: &[Stage::ActionPhase],
        basis: &[
            "规则书·特殊词条 持续回合：每个结束阶段 -1，耗尽立刻弃置",
            "规则书·特殊词条 可用次数：每次触发消耗 1 次，耗尽立刻弃置",
            "规则书·游戏区域 II：状态与出战状态附属在角色牌上",
            "（原 D13「duration 在回合开始递减」与规则书冲突，已按规则书改为结束阶段）",
        ],
        family: Some(CardFamily::Status),
        nums: &[NumRule::new("持续回合每结束阶段递减", 1)],
        ops: &[RuleOp::Lifetime, RuleOp::Order],
        ..RuleRow::EMPTY
    },
    // ------------------------------------------------------------------
    // L1 敌人
    // ------------------------------------------------------------------
    RuleRow {
        layer: Layer::Family,
        kind: RuleKind::Mechanism,
        stages: &[Stage::ActionPhase],
        query_roles: &[super::QueryRole::EnemyClass],
        basis: &[
            "规则书·游戏区域 I：角色区中的「角色牌」；敌人牌按同一套区域规则摆放（无类型、只有标签）",
            "分工计划 §4.4.0.0：敌人牌族没有 L2 类型",
        ],
        family: Some(CardFamily::Enemy),
        ..RuleRow::EMPTY
    },
];
