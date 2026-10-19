//! 角色牌**定义载体**（D121 / D162 迁移）。
//!
//! `CharacterDef` 原先临时放在 `cards/character/mod.rs`（D162 的 TODO(迁移, P0)），
//! 现在迁到这里；具体卡牌数据（原先的 `cards/character/*`）在 D240 起独立到
//! gi-genius-invokation-cards crate，本 crate 只保留定义载体。
//!
//! 本文件只有**定义数据**，不含运行期状态与结算逻辑（D133）。

use crate::base::CardId;
use crate::families::character::skill::SkillDef;
use crate::generated::Localized;
use gi_genius_invokation_engine::Nat;

/// 一张**角色牌**的定义（D162）。
///
/// 泛型参数 `Tags` 是**标签集合**：标签集合形态本轮未定（见 `lib.rs`），
/// 样板用**类型级零大小元组**（如 `(Hydro, Claymore, NodKrai)`）——
/// 标签就是类型本身（D54），组合起来仍然零大小、无分配、编译期可断言。
///
/// 上限口径（用户裁决）：`max_hp` / `max_energy` 是**定义数据**（上限来源）；
/// 运行期当前值在 `CharacterState`（用 [crate::families::character::character::CharacterState::from_def]
/// 按本定义建初始状态）。
///
/// TODO(待定)：标签集合形态（元组 / Vec / 位图）正式确定后改这一处。
#[derive(Clone, Copy, Debug)]
pub struct CharacterDef<Tags> {
    /// 卡牌 id（角色牌族 id 区域内唯一）。
    pub id: CardId,
    /// 角色名（样板只填中文，D120）。
    pub name: Localized,
    /// 标签集合。
    pub tags: Tags,
    /// 生命上限。
    pub max_hp: Nat,
    /// 充能上限。
    pub max_energy: Nat,
    /// 技能列表（顺序 = 使用 / 展示顺序：普攻、战技、爆发、被动）。
    pub skills: &'static [&'static SkillDef],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::SkillId;

    /// 测试用定义：无技能、标签集合为空元组。
    static EMPTY_DEF: CharacterDef<()> = CharacterDef {
        id: CardId::new(1216),
        name: Localized::EMPTY,
        tags: (),
        max_hp: Nat::new(10),
        max_energy: Nat::new(2),
        skills: &[],
    };

    fn noop() {}

    static SKILL: SkillDef = SkillDef {
        id: SkillId::new(12161),
        name: Localized::EMPTY,
        description: Localized::EMPTY,
        kind: crate::families::character::types::SkillKind::normal_attack(),
        cost: crate::families::character::skill::SkillCost::FREE,
        effect: noop,
    };

    static ONE_SKILL_DEF: CharacterDef<()> = CharacterDef {
        id: CardId::new(1216),
        name: Localized::EMPTY,
        tags: (),
        max_hp: Nat::new(10),
        max_energy: Nat::new(2),
        skills: &[&SKILL],
    };

    #[test]
    fn definition_carries_id_tags_and_limits() {
        assert_eq!(EMPTY_DEF.id, CardId::new(1216));
        assert_eq!(EMPTY_DEF.tags, ());
        assert_eq!(EMPTY_DEF.max_hp, Nat::new(10));
        assert_eq!(EMPTY_DEF.max_energy, Nat::new(2));
        assert!(EMPTY_DEF.skills.is_empty());
    }

    #[test]
    fn definition_carries_skill_list_in_order() {
        assert_eq!(ONE_SKILL_DEF.skills.len(), 1);
        let skill = ONE_SKILL_DEF.skills.first().expect("有一个技能");
        assert_eq!(skill.id, SkillId::new(12161));
        assert_eq!(
            skill.kind,
            crate::families::character::types::SkillKind::normal_attack()
        );
    }
}
