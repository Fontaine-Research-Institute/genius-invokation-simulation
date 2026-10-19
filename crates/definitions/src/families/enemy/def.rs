//! **敌人牌定义载体**（`EnemyDef`）。
//!
//! 本文件只有定义数据，不含敌人牌的任何结算 / 流程（D133 精神：本层只做类型与数据）。
//! 敌人牌是**没有 L2 类型**的牌族（D51）；标签在 `tags/enemy/` 与共用处（元素 / 国家·势力 /
//! 武器子类 / 其他武器按「唯一定义」导入，D55）。
//!
//! ## 数据口径（只读游戏内文本描述）
//!
//! - `id` / `name`：逐字取数据（中文；D120，其它语言槽位 `None`，D56 / D68）；
//! - `max_hp`：数据 `hp` 字段（定义数据 = 上限来源；运行期当前值在角色区 / 后续状态块）；
//! - `skills`：数据 `skills` 数组，复用角色的 [`SkillDef`]（D121 / D162：不另造第二份技能定义）；
//!   顺序 = 数据里的技能顺序（普攻 / 战技 / 爆发 / 被动…），不参与判定；
//! - `tags`：数据 `tags`（如 `GCG_TAG_ELEMENT_HYDRO` / `GCG_TAG_WEAPON_NONE` / `GCG_TAG_CAMP_MONSTER`）；
//!   标签是零大小类型（D54），集合形态本轮未定 -> 类型参数 `Tags`，同 `CharacterDef`。
//!
//! ## 待复核 / 待接入
//!
//! - **描述**：`tcgenemycards` **没有顶层 `description` 字段**（只有 `skills[].description`），
//!   所以 `description` 保持 `Localized::EMPTY`（D56 显式缺失），**不从技能文案拼**（那会是第二份数据）；
//!   将来若要敌人牌面描述，等数据源补字段或由展示层拼技能文案（`待接入`）；
//! - **充能上限**：数据有 `maxenergy` 字段，但本块按任务清单只放生命上限；充能上限归充能块
//!   （`待接入`，届时 `EnemyDef` 增加字段即可，不影响本文件其它字段）；
//! - **技能效果体**：`SkillDef.effect` 现在是占位 `fn()`（D162），具体敌人技能实现不在本块。

use crate::base::CardId;
use crate::families::character::skill::SkillDef;
use crate::generated::Localized;
use gi_genius_invokation_engine::Nat;

/// 一张**敌人牌**的定义（纯数据）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；与 `CharacterDef` 同一做法）。
#[derive(Clone, Copy, Debug)]
pub struct EnemyDef<Tags> {
    /// 敌人牌卡 id（敌人牌族 id 区域内唯一）。
    pub id: CardId,
    /// 敌人名（本块只填中文，D120）。
    pub name: Localized,
    /// 敌人牌描述（数据没有顶层描述 -> `Localized::EMPTY`，见模块文档）。
    pub description: Localized,
    /// 生命上限（定义数据；运行期当前值不在本层）。
    pub max_hp: Nat,
    /// 技能列表（复用 [`SkillDef`]；顺序 = 数据里的技能顺序）。
    pub skills: &'static [&'static SkillDef],
    /// 标签集合（D54 / D55）。
    pub tags: Tags,
}

impl<Tags> EnemyDef<Tags> {
    /// 建一张敌人牌定义（常量 / 测试用）。
    pub const fn new(
        id: CardId,
        name: Localized,
        description: Localized,
        max_hp: Nat,
        skills: &'static [&'static SkillDef],
        tags: Tags,
    ) -> EnemyDef<Tags> {
        EnemyDef {
            id,
            name,
            description,
            max_hp,
            skills,
            tags,
        }
    }

    /// 是否有技能（数据里每个敌人都有技能，但定义层不假设至少一个）。
    pub fn has_skills(&self) -> bool {
        !self.skills.is_empty()
    }

    /// 技能个数（用 `Nat` 承载数值，§8.3）。
    pub fn skill_count(&self) -> Nat {
        Nat::new(self.skills.len() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::SkillId;
    use crate::families::character::skill::SkillCost;
    use crate::families::character::types::SkillKind;
    use crate::generated::{Lang, Resolved};
    use crate::tags::character::element::hydro::Hydro;
    use crate::tags::character::nation::monster::Monster;
    use crate::tags::character::other_weapons::OtherWeapons;

    fn noop() {}

    /// 测试用技能（**不是真实技能数据**）。
    static SAMPLE_SKILL: SkillDef = SkillDef {
        id: SkillId::new(900_201),
        name: Localized::from_parts(Some("测试普通攻击"), None, None, None),
        description: Localized::from_parts(Some("造成2点物理伤害。"), None, None, None),
        kind: SkillKind::normal_attack(),
        cost: SkillCost::FREE,
        effect: noop,
    };

    /// 测试用被动技能（无消耗）。
    static PASSIVE_SKILL: SkillDef = SkillDef {
        id: SkillId::new(900_202),
        name: Localized::from_parts(Some("测试被动"), None, None, None),
        description: Localized::from_parts(Some("【被动】测试。"), None, None, None),
        kind: SkillKind::normal_attack(),
        cost: SkillCost::FREE,
        effect: noop,
    };

    /// 无技能、无标签的敌人定义。
    static EMPTY_ENEMY: EnemyDef<()> = EnemyDef::new(
        CardId::new(900_301),
        Localized::from_parts(Some("测试敌人"), None, None, None),
        Localized::EMPTY,
        Nat::new(10),
        &[],
        (),
    );

    /// 带 2 个技能、3 个标签的敌人定义（测试夹具，**不是真实敌人**）。
    static SAMPLE_ENEMY: EnemyDef<(Hydro, OtherWeapons, Monster)> = EnemyDef::new(
        CardId::new(900_302),
        Localized::from_parts(Some("测试敌人·水"), None, None, None),
        Localized::EMPTY,
        Nat::new(6),
        &[&SAMPLE_SKILL, &PASSIVE_SKILL],
        (Hydro, OtherWeapons, Monster),
    );

    #[test]
    fn enemy_def_carries_id_name_description_hp_tags_and_skills() {
        assert_eq!(EMPTY_ENEMY.id, CardId::new(900_301));
        assert_eq!(
            EMPTY_ENEMY.name.get(Lang::ZhCn),
            Resolved::Exact("测试敌人")
        );
        // 数据没有敌人顶层描述（D56：显式缺失，不编造）。
        assert!(EMPTY_ENEMY.description.get(Lang::ZhCn).is_missing());
        assert_eq!(EMPTY_ENEMY.max_hp, Nat::new(10));
        assert_eq!(EMPTY_ENEMY.tags, ());
        assert!(EMPTY_ENEMY.skills.is_empty());
        assert!(!EMPTY_ENEMY.has_skills());
        assert_eq!(EMPTY_ENEMY.skill_count(), Nat::ZERO);
    }

    #[test]
    fn enemy_def_reuses_skill_defs_in_data_order() {
        // 技能列表复用 SkillDef（D121 / D162），顺序 = 数据里的技能顺序。
        assert_eq!(SAMPLE_ENEMY.skills.len(), 2);
        assert!(SAMPLE_ENEMY.has_skills());
        assert_eq!(SAMPLE_ENEMY.skill_count(), Nat::new(2));
        let first = SAMPLE_ENEMY.skills.first().copied().expect("有一个技能");
        let second = SAMPLE_ENEMY.skills.get(1).copied().expect("有两个技能");
        assert_eq!(first.id, SkillId::new(900_201));
        assert_eq!(second.id, SkillId::new(900_202));
        assert_eq!(first.kind, SkillKind::normal_attack());
        // 效果体是占位 fn()：可以取用 / 调用（现在不做任何事）。
        let effect = first.effect;
        effect();
    }

    #[test]
    fn enemy_def_carries_hp_and_tags() {
        assert_eq!(SAMPLE_ENEMY.max_hp, Nat::new(6));
        assert_eq!(
            SAMPLE_ENEMY.tags,
            (Hydro, OtherWeapons, Monster),
            "标签集合 = 类型级零大小元组（D54）"
        );
        // 标签类型都是零大小（D54）。
        assert_eq!(std::mem::size_of::<Hydro>(), 0);
        assert_eq!(std::mem::size_of::<OtherWeapons>(), 0);
        assert_eq!(std::mem::size_of::<Monster>(), 0);
        assert_eq!(
            std::mem::size_of::<(Hydro, OtherWeapons, Monster)>(),
            0
        );
    }
}
