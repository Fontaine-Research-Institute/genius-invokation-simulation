//! L3 角色标签规则行（24 个：元素 7 / 国家 7 / 势力 7 / 其他武器 1 / 始基力 2）。
//!
//! 角色标签**绝大多数仅分类、无操作**：它们的唯一用途是被别的机制当筛选条件
//! （元素共鸣组牌、元素幻变队伍条件、武器类型门、国家 / 势力组牌门槛、荒芒湮灭判定）。
//! 例外：始基力（荒 / 芒）提供一条对立关系（ops::arkhe）；元素附着 / 反应 / 伤害的实现在
//! families/character/{attachment,damage}.rs（D133 / D164）。

use super::{Evidence, Layer, QueryRole, RuleKind, RuleOp, RuleRow, Stage};
use crate::tags::character::arkhe::{ousia, pneuma};
use crate::tags::character::element::{anemo, cryo, dendro, electro, geo, hydro, pyro};
use crate::tags::character::nation::{
    consecrated_beast, cosmic_calamity, eremites, fatui, fontaine, hilichurl, inazuma, kairagi,
    liyue, mondstadt, monster, natlan, nodkrai, sumeru,
};
use crate::tags::character::other_weapons;

/// 元素标签共用的查询角色：元素共鸣 / 元素幻变条件 / 角色元素。
const ELEMENT_ROLES: &[QueryRole] = &[
    QueryRole::TeamElement,
    QueryRole::ResonanceElement,
    QueryRole::TransmutationCondition,
];

/// 国家标签共用的查询角色：国家共鸣组牌门槛。
const NATION_ROLES: &[QueryRole] = &[QueryRole::CampDeckGate];

/// 势力标签共用的查询角色：势力组牌门槛 + 敌人分类。
const CAMP_ROLES: &[QueryRole] = &[QueryRole::CampDeckGate, QueryRole::EnemyClass];

pub static ROWS: &[RuleRow] = &[
    // ==================================================================
    // 元素（7）
    // ==================================================================
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: ELEMENT_ROLES,
        evidence: &[
            Evidence::new(1304, "安柏（火元素角色）"),
            Evidence::new(2302, "深渊咏者·渊火（火元素角色）"),
        ],
        basis: &[
            "规则书·元素反应·规则 元素附着：冰 / 水 / 火 / 雷 / 草伤害会附着对应元素；风 / 岩不能附着",
            "仅分类：被元素共鸣、元素幻变队伍条件引用（元素标签不提供操作）",
        ],
        tag: Some(pyro::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: ELEMENT_ROLES,
        evidence: &[
            Evidence::new(1216, "爱诺（水元素角色）"),
            Evidence::new(2203, "深渊使徒·激流（水元素、其他武器、魔物）"),
        ],
        basis: &[
            "规则书·元素反应·规则 元素附着：冰 / 水 / 火 / 雷 / 草伤害会附着对应元素；风 / 岩不能附着",
            "仅分类：被元素共鸣、元素幻变队伍条件引用（元素标签不提供操作）",
        ],
        tag: Some(hydro::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: ELEMENT_ROLES,
        evidence: &[
            Evidence::new(1405, "北斗（雷元素角色）"),
            Evidence::new(2406, "深渊咏者·紫电（雷元素角色）"),
        ],
        basis: &[
            "规则书·元素反应·规则 元素附着：冰 / 水 / 火 / 雷 / 草伤害会附着对应元素；风 / 岩不能附着",
            "仅分类：被元素共鸣、元素幻变队伍条件引用（元素标签不提供操作）",
        ],
        tag: Some(electro::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: ELEMENT_ROLES,
        evidence: &[
            Evidence::new(1706, "艾尔海森（草元素角色）"),
            Evidence::new(1705, "白术（草元素角色）"),
        ],
        basis: &[
            "规则书·元素反应·规则 元素附着：冰 / 水 / 火 / 雷 / 草伤害会附着对应元素；风 / 岩不能附着",
            "仅分类：被元素共鸣、元素幻变队伍条件引用（元素标签不提供操作）",
        ],
        tag: Some(dendro::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: ELEMENT_ROLES,
        evidence: &[
            Evidence::new(1104, "重云（冰元素角色）"),
            Evidence::new(1110, "夏洛蒂（冰元素角色）"),
        ],
        basis: &[
            "规则书·元素反应·规则 元素附着：冰 / 水 / 火 / 雷 / 草伤害会附着对应元素；风 / 岩不能附着",
            "仅分类：被元素共鸣、元素幻变队伍条件引用（元素标签不提供操作）",
        ],
        tag: Some(cryo::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: ELEMENT_ROLES,
        evidence: &[
            Evidence::new(1511, "恰斯卡（风元素角色）"),
            Evidence::new(2504, "黑蛇骑士·斩风之剑（风元素角色）"),
        ],
        basis: &[
            "规则书·元素反应·规则 元素附着：冰 / 水 / 火 / 雷 / 草伤害会附着对应元素；风 / 岩不能附着",
            "仅分类：被元素共鸣、元素幻变队伍条件引用（元素标签不提供操作）",
        ],
        tag: Some(anemo::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: ELEMENT_ROLES,
        evidence: &[
            Evidence::new(1604, "阿贝多（岩元素角色）"),
            Evidence::new(1605, "荒泷一斗（岩元素角色）"),
        ],
        basis: &[
            "规则书·元素反应·规则 元素附着：冰 / 水 / 火 / 雷 / 草伤害会附着对应元素；风 / 岩不能附着",
            "仅分类：被元素共鸣、元素幻变队伍条件引用（元素标签不提供操作）",
        ],
        tag: Some(geo::data),
        ..RuleRow::EMPTY
    },
    // ==================================================================
    // 国家（7）
    // ==================================================================
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: NATION_ROLES,
        evidence: &[
            Evidence::new(1604, "阿贝多（蒙德角色）"),
            Evidence::new(331801, "风与自由：（牌组包含至少 2 个「蒙德」角色，才能加入牌组）"),
        ],
        basis: &[
            "卡面国家共鸣（如 331801 风与自由）：牌组包含至少 2 个同国家角色，才能加入牌组",
            "规则书无国家篇目；仅分类 + 组牌门槛 / 卡效果按国家筛选",
        ],
        tag: Some(mondstadt::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: NATION_ROLES,
        evidence: &[
            Evidence::new(1705, "白术（璃月角色）"),
            Evidence::new(311402, "千岩长枪：入场时我方队伍中每有 1 名「璃月」角色，提供 1 点护盾"),
            Evidence::new(331802, "岩与契约：（牌组包含至少 2 个「璃月」角色，才能加入牌组）"),
        ],
        basis: &[
            "卡面国家共鸣（如 331801 风与自由）：牌组包含至少 2 个同国家角色，才能加入牌组",
            "规则书无国家篇目；仅分类 + 组牌门槛 / 卡效果按国家筛选",
        ],
        tag: Some(liyue::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: NATION_ROLES,
        evidence: &[
            Evidence::new(1605, "荒泷一斗（稻妻角色）"),
            Evidence::new(331803, "雷与永恒：（牌组包含至少 2 个「稻妻」角色，才能加入牌组）"),
        ],
        basis: &[
            "卡面国家共鸣（如 331801 风与自由）：牌组包含至少 2 个同国家角色，才能加入牌组",
            "规则书无国家篇目；仅分类 + 组牌门槛 / 卡效果按国家筛选",
        ],
        tag: Some(inazuma::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: NATION_ROLES,
        evidence: &[
            Evidence::new(1706, "艾尔海森（须弥角色）"),
            Evidence::new(331804, "草与智慧：（牌组包含至少 2 个「须弥」角色，才能加入牌组）"),
        ],
        basis: &[
            "卡面国家共鸣（如 331801 风与自由）：牌组包含至少 2 个同国家角色，才能加入牌组",
            "规则书无国家篇目；仅分类 + 组牌门槛 / 卡效果按国家筛选",
        ],
        tag: Some(sumeru::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: NATION_ROLES,
        evidence: &[
            Evidence::new(1110, "夏洛蒂（枫丹角色）"),
            Evidence::new(331805, "水与正义：（牌组包含至少 2 个「枫丹」角色，才能加入牌组）"),
        ],
        basis: &[
            "卡面国家共鸣（如 331801 风与自由）：牌组包含至少 2 个同国家角色，才能加入牌组",
            "规则书无国家篇目；仅分类 + 组牌门槛 / 卡效果按国家筛选",
        ],
        tag: Some(fontaine::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: NATION_ROLES,
        evidence: &[
            Evidence::new(1511, "恰斯卡（纳塔角色）"),
            Evidence::new(331806, "火与战争：（牌组包含至少 2 个「纳塔」角色，才能加入牌组）"),
        ],
        basis: &[
            "卡面国家共鸣（如 331801 风与自由）：牌组包含至少 2 个同国家角色，才能加入牌组",
            "规则书无国家篇目；仅分类 + 组牌门槛 / 卡效果按国家筛选",
        ],
        tag: Some(natlan::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: NATION_ROLES,
        evidence: &[
            Evidence::new(1216, "爱诺（挪德卡莱角色）"),
            Evidence::new(331807, "月与故乡：（牌组包含至少 2 个「挪德卡莱」角色，才能加入牌组）"),
            Evidence::new(331721, "月兆·满辉：（牌组包含至少 2 个「挪德卡莱」角色，才能加入牌组）"),
        ],
        basis: &[
            "卡面国家共鸣（如 331801 风与自由）：牌组包含至少 2 个同国家角色，才能加入牌组",
            "规则书无国家篇目；仅分类 + 组牌门槛 / 卡效果按国家筛选",
        ],
        tag: Some(nodkrai::data),
        ..RuleRow::EMPTY
    },
    // ==================================================================
    // 势力（7）
    // ==================================================================
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: CAMP_ROLES,
        evidence: &[
            Evidence::new(2101, "愚人众·冰萤术士（愚人众角色）"),
            Evidence::new(332016, "愚人众的阴谋：（牌组包含至少 2 个「愚人众」角色，才能加入牌组）"),
            Evidence::new(3301, "愚人众伏兵（关键字 #77：在自身所在阵营的角色使用技能后触发）"),
        ],
        basis: &[
            "规则书无势力篇目；仅分类 + 组牌门槛 / 卡效果筛选（不做战斗机制）",
            "卡面 332016 愚人众的阴谋：牌组包含至少 2 个「愚人众」角色；权威关键字 #77 愚人众伏兵",
        ],
        tag: Some(fatui::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: CAMP_ROLES,
        evidence: &[
            Evidence::new(2203, "深渊使徒·激流（魔物角色）"),
            Evidence::new(332015, "深渊的呼唤：（牌组包含至少 2 个「魔物」角色，才能加入牌组）"),
        ],
        basis: &[
            "规则书无势力篇目；仅分类 + 组牌门槛 / 卡效果筛选（不做战斗机制）",
            "卡面 332015 深渊的呼唤：牌组包含至少 2 个「魔物」角色",
        ],
        tag: Some(monster::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: CAMP_ROLES,
        evidence: &[
            Evidence::new(2601, "丘丘岩盔王（丘丘人角色）"),
            Evidence::new(422001, "丘丘人箭塔：我方出战中的射手丘丘人 / 冰箭丘丘人 / 雷箭丘丘人受到的伤害 -2"),
        ],
        basis: &[
            "规则书无势力篇目；仅分类 + 卡效果筛选（不做战斗机制）",
            "卡面 422001 丘丘人箭塔；权威关键字 #74 随机「丘丘人」召唤物共 4 种",
        ],
        tag: Some(hilichurl::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: CAMP_ROLES,
        evidence: &[
            Evidence::new(2703, "镀金旅团·叶轮舞者（镀金旅团角色）"),
            Evidence::new(136051, "厄灵·岩之魔蜥：此召唤物在场，镀金旅团·魔岩役使受到伤害时抵消 2 点伤害"),
            Evidence::new(423067, "机巧符术·茶歇时刻：行动阶段开始时生成 1 张镀金旅团的茶歇"),
        ],
        basis: &[
            "规则书无势力篇目；仅分类 + 卡效果筛选（不做战斗机制）",
            "卡面 423067 机巧符术·茶歇时刻；136051 厄灵·岩之魔蜥",
        ],
        tag: Some(eremites::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: CAMP_ROLES,
        evidence: &[
            Evidence::new(3401, "海乱鬼·雷腾（敌人牌）"),
            Evidence::new(3303, "海乱鬼·炎威（敌人牌）"),
        ],
        basis: &[
            "规则书无势力篇目；纯分类",
            "数据里 2 张敌人牌，没有任何卡文引用该标签",
        ],
        tag: Some(kairagi::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: CAMP_ROLES,
        evidence: &[
            Evidence::new(2705, "圣骸牙兽（圣骸兽角色）"),
            Evidence::new(182071, "噬骸能量块：被弃置时我方全体圣骸兽 +1 攻击力"),
        ],
        basis: &[
            "规则书无势力篇目；仅分类 + 卡效果筛选（不做战斗机制）",
            "卡面 182071 噬骸能量块：被弃置时我方全体圣骸兽 +1 攻击力",
        ],
        tag: Some(consecrated_beast::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: CAMP_ROLES,
        evidence: &[
            Evidence::new(2204, "吞星之鲸（寰宇劫灭角色）"),
            Evidence::new(6605, "丝柯克（寰宇劫灭角色）"),
        ],
        basis: &[
            "规则书无势力篇目；纯分类",
            "数据里 5 张（角色 3 + 敌人 2），没有任何卡文引用该标签",
        ],
        tag: Some(cosmic_calamity::data),
        ..RuleRow::EMPTY
    },
    // ==================================================================
    // 其他武器（1）
    // ==================================================================
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::WeaponTypeGate],
        evidence: &[
            Evidence::new(2203, "深渊使徒·激流：水元素 / 其他武器 / 魔物"),
            Evidence::new(2302, "深渊咏者·渊火：火元素 / 其他武器 / 魔物"),
        ],
        basis: &[
            "仅分类：180 张（44 角色 + 136 敌人）没有武器类型",
            "派生结论（不硬造操作）：卡面武器牌要求「『X』角色才能装备」，其他武器角色没有匹配子类 -> 装不了武器",
        ],
        tag: Some(other_weapons::data),
        ..RuleRow::EMPTY
    },
    // ==================================================================
    // 始基力（2）
    // ==================================================================
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Arkhe],
        stages: &[Stage::Damage],
        query_roles: &[QueryRole::Arkhe],
        evidence: &[
            Evidence::new(1313, "夏沃蕾（始基力：荒性）"),
            Evidence::new(1412, "克洛琳德（始基力：荒性）"),
            Evidence::new(423045, "机巧符术·爆发攻势·荒性（带荒性的行动牌）"),
        ],
        basis: &[
            "权威关键字 #87 / #88：荒芒湮灭效应（受到相反始基力的伤害后触发）；规则书无始基力篇目",
            "机制：只提供一条对立关系（荒 <-> 芒），供伤害侧判定荒芒湮灭效应（见 ops::arkhe）；湮灭效果由角色 / 伤害块实现",
        ],
        tag: Some(ousia::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Mechanism,
        ops: &[RuleOp::Arkhe],
        stages: &[Stage::Damage],
        query_roles: &[QueryRole::Arkhe],
        evidence: &[
            Evidence::new(1110, "夏洛蒂（始基力：芒性）"),
            Evidence::new(1212, "芙宁娜（始基力：芒性）"),
        ],
        basis: &[
            "权威关键字 #87 / #88：荒芒湮灭效应（受到相反始基力的伤害后触发）；规则书无始基力篇目",
            "机制：只提供一条对立关系（芒 <-> 荒），供伤害侧判定荒芒湮灭效应（见 ops::arkhe）；湮灭效果由角色 / 伤害块实现",
        ],
        tag: Some(pneuma::data),
        ..RuleRow::EMPTY
    },
];
