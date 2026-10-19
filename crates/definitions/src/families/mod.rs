//! **牌族层（L2 类型）**：只放每个牌族的**类型枚举**。
//!
//! 层级归属（D51 + 最新结构：tags 已提到与 families 同级的顶层目录）：
//! - **base**：L1 牌族枚举 `CardFamily`（也是 id 区域）；
//! - **families（本目录）**：只放**类型枚举** —— 行动牌 `ActionCardType`、
//!   状态 `StatusType`；其余牌族（角色 / 召唤物 / 敌人）**没有类型**；
//! - **tags**（`src/tags/`，与 families 同级的顶层目录）：标签按 牌族 / 类型 分组，
//!   一个标签一个零大小类型（D54）；没有类型的牌族，其标签直接挂在 `tags/<牌族>/`。
//!
//! 本目录的枚举**没有编号 / 顺序语义**：它们只是"属于哪个类型"的分类，
//! 不参与 id 运算、不做位编码。类型决定 id 区域里的一段（状态的两个类型共用状态牌族的 id 区域）。
//!
//! 「没实现对应标签的牌族不能被调用」由模块与类型系统保证：某牌族在 `tags/` 下
//! 没有对应模块/枚举，别的牌族就引用不到它，不需要运行时检查。
//!
//! 本块只含类型枚举与牌族骨架；区域状态（手牌 / 牌库 / 召唤物区 / 角色区）、
//! 操作函数、具体卡牌不在本块。

pub mod action_card;
pub mod character;
pub mod enemy;
pub mod status;
pub mod summon;

#[cfg(test)]
mod tests {
    use super::action_card::ActionCardType;
    use super::status::StatusType;
    use crate::base::CardFamily;

    /// L2 类型层归属的**唯一口径**（D51）：只有行动牌与状态有类型。
    ///
    /// 这里用穷尽 match 把口径钉在代码里：以后 `CardFamily` 新增变体，
    /// 这个函数**编译不过**，必须回来重新回答"新牌族有没有类型"。
    fn has_type_layer(family: CardFamily) -> bool {
        match family {
            CardFamily::ActionCard | CardFamily::Status => true,
            CardFamily::Summon | CardFamily::Character | CardFamily::Enemy => false,
        }
    }

    const ALL_FAMILIES: [CardFamily; 5] = [
        CardFamily::ActionCard,
        CardFamily::Summon,
        CardFamily::Character,
        CardFamily::Enemy,
        CardFamily::Status,
    ];

    #[test]
    fn only_action_card_and_status_have_types() {
        // 有类型的牌族恰好两个：行动牌、状态（其余三个牌族没有类型）
        let with_types: Vec<CardFamily> = ALL_FAMILIES
            .iter()
            .copied()
            .filter(|family| has_type_layer(*family))
            .collect();
        assert_eq!(with_types, [CardFamily::ActionCard, CardFamily::Status]);

        let without_types: Vec<CardFamily> = ALL_FAMILIES
            .iter()
            .copied()
            .filter(|family| !has_type_layer(*family))
            .collect();
        assert_eq!(
            without_types,
            [
                CardFamily::Summon,
                CardFamily::Character,
                CardFamily::Enemy,
            ]
        );
    }

    #[test]
    fn type_enums_belong_to_their_own_family() {
        // 类型枚举是各自牌族的：行动牌的类型枚举不覆盖状态，反之亦然。
        assert_eq!(
            ActionCardType::ALL.len(),
            3,
            "行动牌只有 支援 / 装备 / 事件 三个类型"
        );
        assert_eq!(
            StatusType::ALL.len(),
            2,
            "状态只有 角色状态 / 出战状态 两个类型"
        );
    }
}
