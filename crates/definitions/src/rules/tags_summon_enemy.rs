//! L3 召唤物 / 敌人标签规则行（2 个：草元素产物 / 无元素类型）。
//!
//! 两者都**仅分类、无操作**：只作为效果筛选条件（召唤物侧）与敌人分类（敌人侧）。

use super::{Evidence, Layer, QueryRole, RuleKind, RuleRow};
use crate::tags::enemy::no_elemental_type;
use crate::tags::summon::dendro_construct;

pub static ROWS: &[RuleRow] = &[
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::SummonClass],
        evidence: &[
            Evidence::new(115, "燃烧烈焰（草元素产物召唤物）：结束阶段造成 1 点火元素伤害，可用次数 1"),
            Evidence::new(
                331702,
                "元素共鸣：蔓生之草：若我方场上存在燃烧烈焰 / 草原核或丰穰之核 / 激化领域，则对对方出战角色造成 1 点伤害",
            ),
        ],
        basis: &[
            "仅分类：数据里只有 1 张召唤物带此标签；作为效果筛选条件（草元素产物集合）",
            "卡面 331702 元素共鸣：蔓生之草按「燃烧烈焰 / 草原核 / 丰穰之核 / 激化领域」筛选；规则书无该分类",
        ],
        tag: Some(dendro_construct::data),
        ..RuleRow::EMPTY
    },
    RuleRow {
        layer: Layer::Tag,
        kind: RuleKind::Classification,
        query_roles: &[QueryRole::EnemyClass],
        evidence: &[
            Evidence::new(3011, "歼灭特化型机关（敌人牌，无元素类型）"),
            Evidence::new(3001, "打手丘丘人（敌人牌，无元素类型）"),
        ],
        basis: &[
            "仅分类：11 张敌人牌，数据里没有 description 文本",
            "规则书·元素反应·规则 只规定冰 / 水 / 火 / 雷 / 草附着与风 / 岩不附着；对「无元素类型」敌人牌无专门条目",
            "Lead 裁定 10（待用户复核）：是否「不可附着元素 / 不参与反应」数据无依据 -> 记入待确认，不实现",
        ],
        tag: Some(no_elemental_type::data),
        ..RuleRow::EMPTY
    },
];
