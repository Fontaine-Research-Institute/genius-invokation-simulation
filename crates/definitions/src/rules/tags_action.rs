//! L3 行动牌标签规则行（25 个：支援 6 / 装备 10 / 事件 4 / 公共 5）。
//!
//! 每条规则的依据都写在该行的 evidence（卡 id + 一句话卡文）与 basis（规格 / 关键字）里。
//! 「自走牌」按任务要求排除（不建行、不实现）。

use super::ops::equip::{
    ARTIFACT_SLOT, BOW_SLOT, CATALYST_SLOT, CLAYMORE_SLOT, POLEARM_SLOT, SWORD_SLOT,
    TALENT_SLOT, TECHNIQUE_SLOT, WEAPON_SLOT,
};
use super::ops::query::{SCRAPPED, UNDISCOVERABLE};
use super::{
    DamageRole, Evidence, Layer, NumRule, OpeningSpec, QueryRole, RuleKind, RuleOp, RuleRow, Stage,
};

use crate::tags::action_card::common::{
    combat_action, damage_reduction, scrapped, shield, undiscoverable,
};
use crate::tags::action_card::equipment::{
    artifact, bow, catalyst, claymore, hexerei, polearm, sword, talent, technique, weapon,
};
use crate::tags::action_card::event::{arcane_legend, food, resonance, simulanka};
use crate::tags::action_card::support::{
    adventure_spot, companion, elemental_transmutation, item, location, untransformed,
};

pub static ROWS: &[RuleRow] = &[
    // ==================================================================
    // 支援类（6）
    // ==================================================================
    // 场地：仅分类 + 支援区上限 4（Lead 裁定 6）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::CardKindGate],
        evidence: &[
            Evidence::new(
                332040,
                "如果我方存在相同元素类型的角色，则从 3 张「场地」中挑选 1 张加入手牌",
            ),
            Evidence::new(321029, "墨色酒馆（场地）：入场时随机生成 1 张希穆兰卡伙伴手牌"),
        ],
        basis: &[
            "规则书·游戏区域 III：支援区最多 4 张；规则书与卡面均无「场地」单独上限",
            "Lead 裁定 6（待用户复核）：场地 / 伙伴 / 道具仅分类 + 支援区上限 4；场地不额外限制最多 1 张",
        ],
        tag: Some(location::data),
        ..RuleRow::EMPTY
    },
    // 伙伴：仅分类 + 支援区上限 4（Lead 裁定 6）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::CardKindGate],
        evidence: &[
            Evidence::new(322016, "打出「伙伴」支援牌时：少花费 1 个元素骰（每回合 1 次）"),
            Evidence::new(322021, "我方打出「玛梅赫」以外的「料理」/「场地」/「伙伴」/「道具」行动牌后…"),
        ],
        basis: &[
            "规则书·游戏区域 III：支援区最多 4 张；规则书与卡面均无「伙伴」单独上限",
            "Lead 裁定 6（待用户复核）：场地 / 伙伴 / 道具仅分类 + 支援区上限 4",
        ],
        tag: Some(companion::data),
        ..RuleRow::EMPTY
    },
    // 道具：仅分类 + 支援区上限 4（Lead 裁定 6）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::CardKindGate],
        evidence: &[
            Evidence::new(323002, "入场时：从牌组中随机抽取 1 张「料理」事件"),
            Evidence::new(323001, "双方角色使用技能后：如果造成了元素伤害，此牌累积 1 个「质变进度」"),
        ],
        basis: &[
            "规则书·游戏区域 III：支援区最多 4 张；规则书与卡面均无「道具」单独上限",
            "Lead 裁定 6（待用户复核）：场地 / 伙伴 / 道具仅分类 + 支援区上限 4",
        ],
        tag: Some(item::data),
        ..RuleRow::EMPTY
    },
    // 元素幻变：开局那一套（全表唯一完整的开局机制）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Opening],
        stages: &[Stage::Opening, Stage::RollPhase, Stage::ActionPhase],
        query_roles: &[QueryRole::TransmutationCondition],
        evidence: &[
            Evidence::new(
                331004,
                "元素幻变：冰元素雷元素；投掷阶段：总是投出 2 个冰元素骰和 2 个雷元素骰；触发超导后弃置此牌并挑 1",
            ),
            Evidence::new(
                331010,
                "元素幻变：水元素风元素；投掷阶段：总是投出 2 个水元素骰和 2 个风元素骰",
            ),
            Evidence::new(
                331008,
                "元素幻变：冰元素草元素；我方选择行动前，如果存在敌方角色同时附着冰元素与草元素：弃置此牌并挑 1",
            ),
            Evidence::new(331006, "元素幻变：水元素草元素；触发绽放或月绽放反应后弃置此牌并挑 1"),
            Evidence::new(331012, "元素幻变：雷元素火元素；触发超载反应后弃置此牌并挑 1"),
        ],
        basis: &[
            "权威关键字 #23 元素幻变：行动阶段开始时，若队伍中包含两种元素类型的角色且不包含其他元素，则将此牌抓起、弃置并生成至场上、将 4 个骰子转化为幻变的元素类型",
            "卡面 9 张（331004-331012）：投掷阶段总是投出 2+2；触发对应反应后弃置此牌并从两张祝佑中挑 1",
            "规则书·回合流程 投掷阶段：每回合投 8 个元素骰、1 次重投；行动阶段从先手开始轮流行动",
            "Lead 裁定 1（待用户复核）：2+2 每轮；首次仅第 1 轮行动阶段；转化固定 2+2；只处理牌库；条件恰等于 {X,Y}（待实测）",
        ],
        tag: Some(elemental_transmutation::data),
        nums: &[
            NumRule::new("投掷阶段固定每种元素骰", 2),
            NumRule::new("开局转化骰子总数", 4),
            NumRule::new("触发时候选祝佑数", 2),
        ],
        opening: Some(OpeningSpec::ElementalTransmutation),
        ..RuleRow::EMPTY
    },
    // 冒险地点：冒险动作接口（Lead 裁定 7）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Adventure],
        stages: &[Stage::ActionPhase],
        query_roles: &[QueryRole::CardKindGate],
        evidence: &[
            Evidence::new(321032, "沉玉谷：冒险经历达到 2 / 4 / 8 时触发不同效果"),
            Evidence::new(321040, "层岩巨渊：入场时在我方牌组中随机生成 5 张事件牌"),
            Evidence::new(321033, "自体自身之塔：入场时对我方所有角色造成 1 点穿透伤害"),
            Evidence::new(321034, "天蛇船：冒险经历增加时，将 1 个元素骰转换为万能元素"),
        ],
        basis: &[
            "权威关键字 #22 冒险：从冒险地点中挑选 1 个生成；如果我方已存在冒险地点，则改为使该地点的冒险经历 +1",
            "规则书无「冒险」篇目；冒险经历阈值效果属具体卡（不实现）",
            "Lead 裁定 7（待用户复核）：只做冒险动作接口 + 已有地点则经历 +1；不额外限制每方数量",
        ],
        tag: Some(adventure_spot::data),
        ..RuleRow::EMPTY
    },
    // 未变身：仅分类（Lead 裁定 5）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::CardKindGate],
        evidence: &[Evidence::new(322033, "乐平波琳（伙伴）：入场时挑选 1 个投资计划")],
        basis: &[
            "规则书·其他规则 转换形态：转换后名称、元素类型、标签、技能可能改变",
            "数据仅 1 张（乐平波琳，伙伴），无变身规则文本 -> Lead 裁定 5：仅分类（待用户复核）",
        ],
        tag: Some(untransformed::data),
        ..RuleRow::EMPTY
    },
    // ==================================================================
    // 装备类（10）
    // ==================================================================
    // 武器：每角色 1 件 + 子类匹配
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        evidence: &[
            Evidence::new(
                311204,
                "阿莫斯之弓：「弓」角色才能装备。角色最多装备 1 件「武器」",
            ),
            Evidence::new(311503, "风鹰剑：「单手剑」角色才能装备。角色最多装备 1 件「武器」"),
            Evidence::new(311104, "千夜浮梦：「法器」角色才能装备。角色最多装备 1 件「武器」"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器",
            "卡面：「『X』角色才能装备。角色最多装备 1 件『武器』」（54 张同句式）——类型门与替换细节来自卡面 / 推断",
            "Lead 裁定 2（待用户复核）：同槽已有 -> 强制替换，旧装备弃置（待实测）",
        ],
        tag: Some(weapon::data),
        slots: &[WEAPON_SLOT],
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        query_roles: &[QueryRole::WeaponTypeGate],
        evidence: &[
            Evidence::new(311503, "风鹰剑：「单手剑」角色才能装备"),
            Evidence::new(311505, "西风剑：「单手剑」角色才能装备"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器",
            "卡面：「『X』角色才能装备」——子类门来自卡面（规则书未写武器类型门）",
        ],
        tag: Some(sword::data),
        slots: &[SWORD_SLOT],
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        query_roles: &[QueryRole::WeaponTypeGate],
        evidence: &[
            Evidence::new(311204, "阿莫斯之弓：「弓」角色才能装备"),
            Evidence::new(311208, "若水：「弓」角色才能装备"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器",
            "卡面：「『X』角色才能装备」——子类门来自卡面（规则书未写武器类型门）",
        ],
        tag: Some(bow::data),
        slots: &[BOW_SLOT],
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        query_roles: &[QueryRole::WeaponTypeGate],
        evidence: &[
            Evidence::new(311306, "苇海信标：「双手剑」角色才能装备"),
            Evidence::new(311310, "拾慧铸熔：「双手剑」角色才能装备"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器",
            "卡面：「『X』角色才能装备」——子类门来自卡面（规则书未写武器类型门）",
        ],
        tag: Some(claymore::data),
        slots: &[CLAYMORE_SLOT],
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        query_roles: &[QueryRole::WeaponTypeGate],
        evidence: &[
            Evidence::new(311104, "千夜浮梦：「法器」角色才能装备"),
            Evidence::new(311109, "金流监督：「法器」角色才能装备"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器",
            "卡面：「『X』角色才能装备」——子类门来自卡面（规则书未写武器类型门）",
        ],
        tag: Some(catalyst::data),
        slots: &[CATALYST_SLOT],
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        query_roles: &[QueryRole::WeaponTypeGate],
        evidence: &[
            Evidence::new(311402, "千岩长枪：「长柄武器」角色才能装备"),
            Evidence::new(311405, "薙草之稻光：「长柄武器」角色才能装备"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器",
            "卡面：「『X』角色才能装备」——子类门来自卡面（规则书未写武器类型门）",
        ],
        tag: Some(polearm::data),
        slots: &[POLEARM_SLOT],
        ..RuleRow::EMPTY
    },
    // 圣遗物：每角色 1 件，无类型门
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        evidence: &[
            Evidence::new(312001, "冒险家头带：（角色最多装备 1 件「圣遗物」）"),
            Evidence::new(312027, "紫晶的花冠：（角色最多装备 1 件「圣遗物」）"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张圣遗物（无类型门）",
            "卡面 63 张：（角色最多装备 1 件「圣遗物」）",
        ],
        tag: Some(artifact::data),
        slots: &[ARTIFACT_SLOT],
        ..RuleRow::EMPTY
    },
    // 天赋：牌组门槛 + 每角色多张、同种覆盖
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        evidence: &[
            Evidence::new(
                217051,
                "在地为化：（牌组中包含白术，才能加入牌组）战斗行动：我方出战角色为白术时，装备此牌",
            ),
            Evidence::new(
                212041,
                "深渊之灾·凝水盛放：（牌组中包含达达利亚，才能加入牌组）装备此牌后立刻使用一次魔王武装·狂澜",
            ),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张武器、1 张圣遗物、1 张特技，和任意张其他类型装备（天赋属其他类型 -> 不限张数）",
            "卡面 155 张：「牌组中包含 <角色>，才能加入牌组」并把牌装到该角色上",
            "分工计划 §4.4.14 + Lead 裁定 3（待用户复核）：同种 = 同卡 id / 同名覆盖；规则书未规定 -> 待实测",
        ],
        tag: Some(talent::data),
        slots: &[TALENT_SLOT],
        ..RuleRow::EMPTY
    },
    // 特技：每角色 1 个；使用特技 = 战斗行动
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Equip],
        stages: &[Stage::ActionPhase],
        evidence: &[
            Evidence::new(313010, "膨膨兽：（角色最多装备 1 个「特技」）特技：膨膨音波"),
            Evidence::new(112142, "咬咬鲨鱼：「特技：鲨鲨冲浪板」"),
        ],
        basis: &[
            "规则书·特殊词条 特技：装备有『特技』卡牌时才能使用对应特技；使用特技是战斗行动、不视为使用技能、伤害不算角色造成",
            "规则书·游戏区域 II(1)：每个角色最多装备 1 张特技",
            "卡面 22 张：（角色最多装备 1 个「特技」）",
        ],
        tag: Some(technique::data),
        slots: &[TECHNIQUE_SLOT],
        ..RuleRow::EMPTY
    },
    // 魔导：仅分类、并入天赋（Lead 裁定 4）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::CardKindGate],
        evidence: &[
            Evidence::new(216042, "白芒之书：（牌组中包含阿贝多，才能加入牌组）快速行动：装备给我方的阿贝多"),
            Evidence::new(212032, "天步真原：（牌组中包含莫娜，才能加入牌组）战斗行动：装备此牌"),
            Evidence::new(215012, "七循之理：我方附属了「天赋」的角色造成的伤害 +1"),
        ],
        basis: &[
            "规则书·游戏区域 II(1)：装备只区分武器 / 圣遗物 / 特技 / 其他类型；魔导属「其他类型装备」",
            "7 张全是「天赋+魔导」装备牌；卡文与规则书均无「魔导」专有条目 -> Lead 裁定 4：仅分类并入天赋（待用户复核）",
        ],
        tag: Some(hexerei::data),
        ..RuleRow::EMPTY
    },
    // ==================================================================
    // 事件类（4）
    // ==================================================================
    // 料理：饱腹（每角色每回合 1 次）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Food],
        stages: &[Stage::ActionPhase],
        evidence: &[
            Evidence::new(
                333027,
                "纵声欢唱：所有我方角色获得饱腹，抓 2 张牌（每回合每个角色最多食用 1 次「料理」）",
            ),
            Evidence::new(
                333009,
                "提瓦特煎蛋：（每回合中，最多通过「料理」复苏 1 个角色，并且每个角色最多食用 1 次「料理」）",
            ),
            Evidence::new(333003, "莲花酥：（每回合中每个角色最多食用 1 次「料理」）"),
        ],
        basis: &[
            "规则书·特殊词条 料理：料理对角色生效后进入「饱腹」；饱腹角色不能成为其他料理目标、不受其效果；饱腹在回合结束或角色被击倒时解除；本回合打出复苏料理后不能再打其他复苏料理",
            "卡面 30 张：（每回合每个角色最多食用 1 次「料理」）",
            "D17：料理只负责赋予饱腹，限制由饱腹自身决定",
        ],
        tag: Some(food::data),
        nums: &[
            NumRule::new("每角色每回合料理次数", 1),
            NumRule::new("每回合料理复苏角色数", 1),
        ],
        ..RuleRow::EMPTY
    },
    // 秘传：起手必带 + 整局 1 张 + 秘传揭令
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Arcane, RuleOp::Opening],
        stages: &[Stage::Opening, Stage::ActionPhase],
        evidence: &[
            Evidence::new(
                330001,
                "旧时庭园：（整局游戏只能打出一张「秘传」卡牌；这张牌一定在你的起始手牌中）",
            ),
            Evidence::new(
                330002,
                "磐岩盟契：（整局游戏只能打出一张「秘传」卡牌；这张牌一定在你的起始手牌中）",
            ),
        ],
        basis: &[
            "规则书·特殊词条 秘传：一场对局每位牌手只能使用 1 张；秘传一定在起始手牌中（牌组中秘传多于起始手牌数时随机出现）",
            "权威关键字 #70 秘传揭令：只有揭令存在时才能打出，打出后消耗使用机会",
            "D17 / D90：起手先按「秘传」标签抓，再补足到 5 张（已实现 HandState::draw_opening）",
        ],
        tag: Some(arcane_legend::data),
        nums: &[NumRule::new("整局秘传张数", 1)],
        opening: Some(OpeningSpec::ArcaneLegendOpeningHand),
        ..RuleRow::EMPTY
    },
    // 元素共鸣：组牌门槛（至少 2 名同元素角色）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Resonance],
        stages: &[Stage::DeckBuilding],
        query_roles: &[QueryRole::ResonanceElement],
        evidence: &[
            Evidence::new(
                331602,
                "元素共鸣：坚定之岩（牌组包含至少 2 个岩元素角色，才能加入牌组）",
            ),
            Evidence::new(
                331301,
                "元素共鸣：交织之火（牌组包含至少 2 个火元素角色，才能加入牌组）",
            ),
            Evidence::new(
                331721,
                "月兆·满辉（牌组包含至少 2 个「挪德卡莱」角色，才能加入牌组）",
            ),
        ],
        basis: &[
            "卡面 15 张：（牌组包含至少 2 个 <元素/国家> 角色，才能加入牌组）",
            "规则书无组牌门槛条目（元素共鸣 / 国家共鸣属卡面规则）",
        ],
        tag: Some(resonance::data),
        nums: &[NumRule::new("同元素角色数门槛", 2)],
        ..RuleRow::EMPTY
    },
    // 希穆兰卡：固定集合 + 随机生成范围
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Simulanka],
        stages: &[Stage::ActionPhase],
        query_roles: &[QueryRole::CardKindGate, QueryRole::SummonClass],
        evidence: &[
            Evidence::new(332051, "「邪龙」的苏醒：召唤「邪龙」；我方支援区每弃置 1 张牌，则「邪龙」可用次数 +1"),
            Evidence::new(332053, "破碎之海：使我方所有「希穆兰卡」召唤物的可用次数和效果量 +1"),
            Evidence::new(321029, "墨色酒馆：我方宣布结束时随机触发 1 个「希穆兰卡」召唤物的结束阶段效果"),
            Evidence::new(332054, "「魔女M的祝福」：选择并弃置 1 个我方召唤物，转化至多 2 个基础元素骰"),
        ],
        basis: &[
            "权威关键字 #109：「希穆兰卡」卡牌是固定集合（墨色酒馆 / 星轨王城 / 森林的祝福 / 预言女神的礼物 / 「邪龙」的苏醒 / 旁白的注脚 / 「魔女M的祝福」/（test）恶龙的地点）",
            "权威关键字 #107：「希穆兰卡」召唤物共有 4 种；随机生成 = 从这 4 种中取 1",
            "规则书无希穆兰卡篇目",
        ],
        tag: Some(simulanka::data),
        nums: &[NumRule::new("希穆兰卡召唤物种类", 4)],
        ..RuleRow::EMPTY
    },
    // ==================================================================
    // 公共类（5）
    // ==================================================================
    // 战斗行动：行动权
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Timing],
        stages: &[Stage::ActionPhase],
        evidence: &[
            Evidence::new(116121, "阿咚：战斗行动：选一个敌方角色，对其造成 1 点岩元素伤害"),
            Evidence::new(
                217051,
                "在地为化：战斗行动：我方出战角色为白术时，装备此牌",
            ),
        ],
        basis: &[
            "规则书·进行行动：行动分战斗行动与快速行动；使用技能 / 切换角色 / 宣布结束是战斗行动；打出手牌 / 元素调和是快速行动",
            "规则书·特殊词条 战斗行动（行动牌）：打出带此规则的手牌视为进行战斗行动，而非通常的快速行动",
            "数据里 125 张卡带「战斗行动」标签",
        ],
        tag: Some(combat_action::data),
        ..RuleRow::EMPTY
    },
    // 护盾：先吸收伤害
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Damage],
        stages: &[Stage::Damage],
        evidence: &[
            Evidence::new(422021, "地脉异常·守定之势·完备：行动阶段开始时提供 6 点护盾，保护我方出战角色"),
            Evidence::new(422036, "地脉异常·重重防御·高阶：提供 6 点护盾；行动阶段开始时恢复此牌提供的护盾"),
            Evidence::new(422025, "地脉异常·守定之势·轻装：行动阶段开始时提供 3 点护盾"),
        ],
        basis: &[
            "规则书·特殊词条 护盾：护盾所保护的角色受到伤害时，消耗护盾值来抵消伤害",
            "规则书·特殊词条 穿透伤害：不会被护盾等效果抵消",
            "Lead 裁定 8（待用户复核）：先护盾吸收再减伤扣减；规则书未写先后 -> 待实测",
        ],
        tag: Some(shield::data),
        damage: Some(DamageRole::Shield),
        ..RuleRow::EMPTY
    },
    // 减伤：护盾之后再扣减
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Damage],
        stages: &[Stage::Damage],
        evidence: &[
            Evidence::new(
                422001,
                "丘丘人箭塔：我方出战中的射手丘丘人、冰箭丘丘人、雷箭丘丘人受到的伤害 -2（每回合 1 次）",
            ),
            Evidence::new(113041, "兔兔伯爵：我方出战角色受到伤害时：抵消 2 点伤害"),
            Evidence::new(122043, "黑色幻影：我方出战角色受到伤害时：抵消 1 点伤害，然后此牌可用次数 -2"),
        ],
        basis: &[
            "规则书·特殊词条 穿透伤害：不会被护盾等效果抵消（减伤同属抵消类效果）",
            "规则书没有「减伤」词条；效果来自卡面（抵消 N 点 / 受到的伤害 -N）",
            "Lead 裁定 8（待用户复核）：护盾之后再减伤；先后与叠加待实测",
        ],
        tag: Some(damage_reduction::data),
        damage: Some(DamageRole::Reduction),
        ..RuleRow::EMPTY
    },
    // 不可发现：检索策略（不重复实现，与 query.rs 同口径）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Query],
        stages: &[Stage::Query],
        evidence: &[
            Evidence::new(331004, "元素幻变：超导祝佑（不可发现：检索类效果不允许发现这张牌）"),
            Evidence::new(303041, "超导祝佑·极寒（不可发现）"),
        ],
        basis: &[
            "分工计划 §4.4.15：三类不进检索池（不可发现 / 弃用 / 自走牌）；规则书无检索条目",
            "D101：两个过滤都没给时才排除；显式查询该标签仍能查到；实现见 zones/query.rs",
        ],
        tag: Some(undiscoverable::data),
        query_policy: Some(UNDISCOVERABLE),
        ..RuleRow::EMPTY
    },
    // 弃用：排除（不重复实现）
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Query],
        stages: &[Stage::Query],
        evidence: &[Evidence::new(532117, "（test）千岩长枪（弃用：测试用类型，不进入检索池）")],
        basis: &[
            "分工计划 §4.4.15：三类不进检索池（不可发现 / 弃用 / 自走牌）；规则书无检索条目",
            "D101：两个过滤都没给时才排除；显式查询该标签仍能查到；实现见 zones/query.rs",
        ],
        tag: Some(scrapped::data),
        query_policy: Some(SCRAPPED),
        ..RuleRow::EMPTY
    },
];
