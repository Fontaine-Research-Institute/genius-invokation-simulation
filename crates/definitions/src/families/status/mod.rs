//! 【状态】牌族层：**类型枚举**。
//!
//! 状态的两个类型 —— 角色状态 / 出战状态 —— **共用状态牌族的同一个 id 区域**（D51），
//! 靠类型区分，所以类型是状态的一部分，不是可选属性。
//!
//! 标签属于具体类型，在顶层 `tags/status/{character_status,combat_status}/`（一个标签一个零大小类型，D54）；
//! 数据里状态**其实带标签**（`tcgstatuseffects` 里 437 条有 `tags`，如 `GCG_TAG_SHEILD` /
//! `GCG_TAG_DENDRO_PRODUCE`），但本轮 `tags/status/` 先空，具体定义只导入别处已有的那一份（D55）。
//!
//! 文件：
//! - `def.rs`：**状态定义载体** `StatusDef`（卡 id / 类型 / 名字描述 / 默认层数 / 持续回合 / 标签；
//!   duration 在回合开始清减，D13；D110 / D108）；
//! - `known.rs`：**正式状态定义**（203 出战护盾 / 134 角色护盾 / 106 冻结 / 116 草原核 / 117 激化领域）；
//!   状态区承载与 apply / layers / remove 等操作留给后续块（D182 / D183）。
//!
//! 本枚举**没有编号 / 顺序语义**：变体顺序不表示任何规则。
//!
//! 注意（D112/D113）：减费 / 赋能等「挂在行动牌上的状态」**不属于**本牌族，
//! 它们的通用状态抽象在 `families/action_card/status/`；本目录只放状态牌族的 L2 类型枚举。

pub mod arcane_edict;
pub mod def;
pub mod known;

/// 状态的类型（L2）：角色状态 / 出战状态。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StatusType {
    /// 角色状态：附在**某个角色**身上（跨回合，跟随角色）。
    CharacterStatus,
    /// 出战状态：附在**某方场上**（不跟随具体角色）。
    CombatStatus,
}

impl StatusType {
    /// 全部 2 个类型（只用于遍历；数组顺序不表示任何游戏语义）。
    pub const ALL: [StatusType; 2] = [StatusType::CharacterStatus, StatusType::CombatStatus];
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// 穷尽 match：编译器保证这里覆盖每一个变体（漏一个就编译失败）。
    fn canonical_name(ty: StatusType) -> &'static str {
        match ty {
            StatusType::CharacterStatus => "CharacterStatus",
            StatusType::CombatStatus => "CombatStatus",
        }
    }

    #[test]
    fn status_has_exactly_two_types() {
        assert_eq!(StatusType::ALL.len(), 2);
        assert_eq!(
            StatusType::ALL,
            [StatusType::CharacterStatus, StatusType::CombatStatus]
        );
    }

    #[test]
    fn status_types_are_all_distinct() {
        let mut names: Vec<&'static str> = StatusType::ALL
            .iter()
            .map(|ty| canonical_name(*ty))
            .collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), StatusType::ALL.len());

        let distinct: HashSet<StatusType> = StatusType::ALL.iter().copied().collect();
        assert_eq!(distinct.len(), StatusType::ALL.len());
    }

    #[test]
    fn status_types_share_one_id_region() {
        // 两个类型同属状态牌族的 id 区域：它们是"同一批 id 的两种解释"，
        // 所以判等/哈希互不相同，但都不带自己的 id 空间。
        assert_ne!(StatusType::CharacterStatus, StatusType::CombatStatus);
        let mut per_type: HashSet<StatusType> = HashSet::new();
        per_type.insert(StatusType::CharacterStatus);
        per_type.insert(StatusType::CombatStatus);
        assert_eq!(per_type.len(), 2);
    }
}
