//! 卡牌 / 技能编号类型（基于 Nat 的新类型）。
//!
//! **唯一性口径**：CardId 只是一个数字，在**所属牌族的 id 区域内**唯一；
//! 跨牌族**不保证**不重复。所以引用一张卡必须带牌族：(CardFamily, CardId)。
//! 按名字找卡只属于用户层（UI / 搜索框），游戏逻辑里不出现"名字 → 卡"的查找。
//!
//! SkillId 由角色 id 派生（角色 id × 10 + 序号），同样是 Nat 新类型。
//! 两者都只提供 const fn new：卡牌 / 技能代码里**不写裸数字**，一律写具名常量。

use gi_genius_invokation_engine::Nat;

/// 卡牌 id：某个牌族 id 空间内的编号（跨牌族不保证唯一）。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CardId(pub Nat);

impl CardId {
    /// **const 构造**（用于写常量）。
    pub const fn new(v: u32) -> CardId {
        CardId(Nat::new(v))
    }
}

/// 技能 id：由角色 id 派生（角色 id × 10 + 序号）。
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SkillId(pub Nat);

impl SkillId {
    /// **const 构造**（用于写常量）。
    pub const fn new(v: u32) -> SkillId {
        SkillId(Nat::new(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::CardFamily;
    use std::collections::HashMap;

    /// 测试用示例编号（**不是真实卡牌数据**；真实编号由具体卡牌常量提供）。
    const SAMPLE_CARD_A: CardId = CardId::new(1418);
    const SAMPLE_CARD_B: CardId = CardId::new(1419);
    /// 技能 id 派生示例：角色 1418 的第 0 个技能 = 1418 × 10 + 0。
    const SAMPLE_SKILL: SkillId = SkillId::new(14180);

    #[test]
    fn id_is_transparent_over_nat() {
        assert_eq!(std::mem::size_of::<CardId>(), std::mem::size_of::<Nat>());
        assert_eq!(std::mem::size_of::<SkillId>(), std::mem::size_of::<Nat>());
        // 公开字段可直接取到底层 Nat（同一块内存，无额外开销）
        assert_eq!(SAMPLE_CARD_A.0, Nat::new(1418));
        assert_eq!(SAMPLE_SKILL.0, Nat::new(14180));
    }

    #[test]
    fn ids_are_ordered_and_hashable() {
        let mut ids = [SAMPLE_CARD_B, SAMPLE_CARD_A];
        ids.sort();
        assert_eq!(ids, [SAMPLE_CARD_A, SAMPLE_CARD_B]);

        let mut seen: HashMap<CardId, &'static str> = HashMap::new();
        assert_eq!(seen.insert(SAMPLE_CARD_A, "A"), None);
        // 同一个 id 是同一个键：再次插入是覆盖，不是新增
        assert_eq!(seen.insert(SAMPLE_CARD_A, "A2"), Some("A"));
        assert_eq!(seen.len(), 1);
    }

    #[test]
    fn uniqueness_is_per_family_not_global() {
        // 口径：查找键 = (牌族, id)，不是 id 单值 —— 不同牌族可以复用同一个数字。
        let mut cards: HashMap<(CardFamily, CardId), &'static str> = HashMap::new();
        assert_eq!(
            cards.insert((CardFamily::ActionCard, SAMPLE_CARD_A), "行动牌"),
            None
        );
        assert_eq!(
            cards.insert((CardFamily::Character, SAMPLE_CARD_A), "角色牌"),
            None
        );
        assert_eq!(cards.len(), 2);

        // 同一牌族内同一 id 只能有一张：重注册是覆盖（唯一性口径的落地）
        assert_eq!(
            cards.insert((CardFamily::ActionCard, SAMPLE_CARD_A), "行动牌（重注册）"),
            Some("行动牌")
        );
        assert_eq!(cards.len(), 2);
        assert_eq!(
            cards.get(&(CardFamily::ActionCard, SAMPLE_CARD_A)),
            Some(&"行动牌（重注册）")
        );
    }

    #[test]
    fn skill_id_is_a_nat_newtype() {
        // 派生公式由角色牌族实现；这里只验证 SkillId 是 Nat 上的独立新类型。
        assert_eq!(SAMPLE_SKILL, SkillId(Nat::new(14180)));
        assert!(SkillId::new(1) < SkillId::new(2));
        // 技能 id 与卡牌 id 是不同类型：不能混用（下面两行若去掉注释会编译失败）
        // let _: SkillId = SAMPLE_CARD_A;
        // let _: CardId = SAMPLE_SKILL;
    }
}
