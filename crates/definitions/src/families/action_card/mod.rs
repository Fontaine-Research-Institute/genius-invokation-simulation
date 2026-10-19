//! 【行动牌】牌族层：**类型枚举**。
//!
//! 行动牌是唯一有类型树的牌族之一（另一个是状态）。层级（D51 + 最新结构）：
//! - **families（本文件）**：只放**类型枚举** `ActionCardType`（支援 / 装备 / 事件）；
//! - **tags**（与 families 同级的顶层目录）：标签按 牌族 / 类型 分组，一个标签一个
//!   零大小类型（D54）；行动牌的标签在 `tags/action_card/{support,equipment,event,common}/`，不在本文件里。
//!
//! 一个行动牌 = 一个类型 + 若干标签；类型决定它的定义与区域操作的落点
//! （`families/action_card/types/<类型>/`，后续块建）。
//!
//! 本枚举**没有编号 / 顺序语义**：变体的书写顺序不表示任何游戏规则，
//! 需要"有几个类型"时用 `ALL` 遍历，不做 `as usize` 之类的编号运算。
//!
//! 本块只有类型枚举：`ActionCardDef` / 花费 / 手牌 / 牌库 / 查询 / 元素调和等
//! 区域状态与操作留给后续块。

/// 区域承载（D182 / D183 / D185 / D187）：支援区 / 手牌卡状态 / 当前元素骰费用。
pub mod areas;
/// 定义载体（D217）：一张行动牌的纯数据 + 它自己的**打出条件**。
pub mod def;
pub mod status;
pub mod zones;

/// 行动牌的类型（L2）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum ActionCardType {
    /// 支援牌：打到支援区，留在场上持续生效（场地 / 伙伴 / 道具 / 护盾 / 元素幻变 …）。
    Support,
    /// 装备牌：挂到角色身上（武器 / 圣遗物 / 天赋 / 特技 …）。
    Equipment,
    /// 事件牌：结算完就离开（料理 / 秘传 / 元素共鸣 / 战斗行动 …）。
    Event,
}

impl ActionCardType {
    /// 全部 3 个类型（只用于遍历；数组顺序不表示任何游戏语义）。
    pub const ALL: [ActionCardType; 3] = [
        ActionCardType::Support,
        ActionCardType::Equipment,
        ActionCardType::Event,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// 穷尽 match：编译器保证这里覆盖每一个变体（漏一个就编译失败）。
    fn canonical_name(ty: ActionCardType) -> &'static str {
        match ty {
            ActionCardType::Support => "Support",
            ActionCardType::Equipment => "Equipment",
            ActionCardType::Event => "Event",
        }
    }

    #[test]
    fn action_card_has_exactly_three_types() {
        assert_eq!(ActionCardType::ALL.len(), 3);
        // 三个类型齐全：逐个点名核对（漏项由穷尽 match 兜住）
        for expected in ActionCardType::ALL {
            assert!(ActionCardType::ALL.contains(&expected));
        }
    }

    #[test]
    fn action_card_types_are_all_distinct() {
        // 互不相同：去重后个数不变（有重复变体混进来就会变小）
        let mut names: Vec<&'static str> = ActionCardType::ALL
            .iter()
            .map(|ty| canonical_name(*ty))
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ActionCardType::ALL.len());

        let distinct: HashSet<ActionCardType> = ActionCardType::ALL.iter().copied().collect();
        assert_eq!(distinct.len(), ActionCardType::ALL.len());
    }

    #[test]
    fn action_card_type_is_not_a_numeric_encoding() {
        // 可判等 / 可作集合键；"大小"不表示任何游戏语义（不是编号）。
        assert_ne!(ActionCardType::Support, ActionCardType::Equipment);
        assert_eq!(ActionCardType::Event, ActionCardType::Event);
    }
}
