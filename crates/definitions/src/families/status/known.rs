//! **正式状态定义**：反应 / 卡牌会附属的那几个状态牌（D95 / D96 / D110 / D13）。
//!
//! 本文件给 5 张**有独立卡牌**的状态（数据来源 `tcgstatuseffects`）：
//!
//! | 静态定义 | 身份实例 | 卡 id | 类型 | 数据文件 |
//! |---|---|---|---|---|
//! | `SHIELD_COMBAT_DEF` | `SHIELD_COMBAT` | 203 | 出战状态 | `shield.json` |
//! | `SHIELD_CHARACTER_DEF` | `SHIELD_CHARACTER` | 134 | 角色状态 | `shields.json` |
//! | `FROZEN_DEF` | `FROZEN` | 106 | 角色状态 | `frozen.json` |
//! | `DENDRO_CORE_DEF` | `DENDRO_CORE` | 116 | 出战状态 | `dendrocore.json` |
//! | `CATALYZING_FIELD_DEF` | `CATALYZING_FIELD` | 117 | 出战状态 | `catalyzingfield.json` |
//!
//! 三个**没有独立卡牌**的共享 / 手牌状态身份（伤害抵消 / 电击 / 费用降低）不在这里：
//! 它们的语义归行动牌（D112 / D114）或根本只是共享身份，身份留在桥模块
//! `families::character::damage_chain::statuses`。
//!
//! ## 数据口径（只读游戏内文本描述）
//!
//! - 名字 / 描述：**逐字**取数据里的 `name` / `description`（含 `\n`），只填中文（D120），
//!   其它语言槽位 `None`（D56 / D68 显式缺失）；
//! - `status_type`：数据 `statustypetext` = `角色状态` -> `CharacterStatus`、
//!   `阵营出战状态` -> `CombatStatus`；
//! - 标签：数据 `tags` 里有 `GCG_TAG_SHEILD`（护盾）-> `tags::action_card::common::shield::Shield`、
//!   `GCG_TAG_DENDRO_PRODUCE`（草元素产物）-> `tags::summon::dendro_construct::DendroConstruct`；
//!   冻结的 `GCG_TAG_FORBIDDEN_ATTACK` 还没有标签类型（`tags/status/` 本轮为空）-> `()`（待接入）；
//! - `default_layers` / `duration`：数据**没有结构化字段**，只能从卡面描述反推（D13），
//!   下面逐条标 `待复核`。
//!
//! ## 身份形态
//!
//! 每个状态一个身份类型 + 一份静态实例；实例持有 `&'static StatusDef<Tags>`（指针大小），
//! 不是字面零大小 —— 原因与证据见 `def.rs` 模块文档（零大小静态实例地址会重合，破坏
//! D95 / D96 的指针身份）。名字 / 描述只存在定义里，身份不拷贝。

use super::def::StatusDef;
use super::StatusType;
use crate::base::CardId;
use crate::families::action_card::status::{StateForm, StateIdentity, StateText};
use crate::generated::Localized;
use crate::tags::action_card::common::shield::Shield;
use crate::tags::summon::dendro_construct::DendroConstruct;
use gi_genius_invokation_engine::Nat;

/// 声明一个状态的「身份类型 + 身份静态实例」。
///
/// 身份实例 = 指向该状态静态定义的一份指针（`&'static StatusDef<Tags>`）；地址即身份
/// （D95 / D96），文本从定义里取（只有一份，D110）。
macro_rules! status_identity {
    (
        $(#[$meta:meta])*
        $identity:ident => $value:ident,
        $def:ident,
        $tags:ty
    ) => {
        $(#[$meta])*
        #[derive(Debug)]
        pub struct $identity(&'static StatusDef<$tags>);

        impl $identity {
            /// 这份身份指向的**静态定义**（数据只有一份）。
            pub const fn definition(&self) -> &'static StatusDef<$tags> {
                self.0
            }

            /// 把自身当**身份指针**用（`&'static dyn StateIdentity`；D95 / D96）。
            pub fn identity(self: &'static Self) -> &'static dyn StateIdentity {
                self
            }
        }

        impl StateIdentity for $identity {
            fn text(&self) -> &'static StateText {
                &self.0.text
            }

            fn form(&self) -> StateForm {
                self.0.form()
            }
        }

        $(#[$meta])*
        pub static $value: $identity = $identity(&$def);
    };
}

// ======================================================================
// 出战状态「护盾」（id 203）
// ======================================================================

/// 出战状态「护盾」（id 203）的定义。
///
/// 数据：`shield.json`；名字「护盾」，`statustypetext` = 阵营出战状态，
/// 描述「为我方出战角色提供1点护盾。（可叠加，没有上限）」，标签 `GCG_TAG_SHEILD`。
pub static SHIELD_COMBAT_DEF: StatusDef<Shield> = StatusDef::new(
    CardId::new(203),
    StatusType::CombatStatus,
    StateText {
        name: Localized::from_parts(Some("护盾"), None, None, None),
        description: Localized::from_parts(
            Some("为我方出战角色提供1点护盾。（可叠加，没有上限）"),
            None,
            None,
            None,
        ),
    },
    // 卡面「提供1点护盾」；待复核：数据没有结构化层数字段。
    Some(Nat::new(1)),
    None,
    Shield,
);

status_identity!(
    /// 出战状态「护盾」的身份（卡 id 203；层数 = 护盾点数，没有上限）。
    ShieldCombat => SHIELD_COMBAT,
    SHIELD_COMBAT_DEF,
    Shield
);

// ======================================================================
// 角色状态「护盾」（id 134）
// ======================================================================

/// 角色状态「护盾」（id 134）的定义。
///
/// 数据：`shields.json`；名字「护盾」，`statustypetext` = 角色状态，
/// 描述「每层抵消1点此角色受到的伤害。（可叠加，没有上限）。」，标签 `GCG_TAG_SHEILD`。
pub static SHIELD_CHARACTER_DEF: StatusDef<Shield> = StatusDef::new(
    CardId::new(134),
    StatusType::CharacterStatus,
    StateText {
        name: Localized::from_parts(Some("护盾"), None, None, None),
        description: Localized::from_parts(
            Some("每层抵消1点此角色受到的伤害。（可叠加，没有上限）。"),
            None,
            None,
            None,
        ),
    },
    // 「每层抵消1点」；待复核：默认层数由具体卡牌给。
    Some(Nat::new(1)),
    None,
    Shield,
);

status_identity!(
    /// 角色状态「护盾」的身份（卡 id 134；每层抵消 1 点，没有上限）。
    ShieldCharacter => SHIELD_CHARACTER,
    SHIELD_CHARACTER_DEF,
    Shield
);

// ======================================================================
// 角色状态「冻结」（id 106）
// ======================================================================

/// 角色状态「冻结」（id 106）的定义。
///
/// 数据：`frozen.json`；名字「冻结」，`statustypetext` = 角色状态，
/// 描述「角色无法使用技能。（持续到回合结束）\n角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。」；
/// 标签 `GCG_TAG_FORBIDDEN_ATTACK`（标签类型还没有 -> `()`，待接入）。
pub static FROZEN_DEF: StatusDef<()> = StatusDef::new(
    CardId::new(106),
    StatusType::CharacterStatus,
    StateText {
        name: Localized::from_parts(Some("冻结"), None, None, None),
        description: Localized::from_parts(
            Some("角色无法使用技能。（持续到回合结束）\n角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。"),
            None,
            None,
            None,
        ),
    },
    // 布尔形态（D104：在列表里即代表「有」）。
    None,
    // 数据只写「持续到回合结束」，按 1 回合计（D13 回合开始清减）；待复核。
    Some(Nat::new(1)),
    (),
);

status_identity!(
    /// 角色状态「冻结」的身份（卡 id 106）。
    Frozen => FROZEN,
    FROZEN_DEF,
    ()
);

// ======================================================================
// 出战状态「草原核」（id 116）
// ======================================================================

/// 出战状态「草原核」（id 116）的定义。
///
/// 数据：`dendrocore.json`；名字「草原核」，`statustypetext` = 阵营出战状态，
/// 描述「我方对敌方出战角色造成火元素伤害或雷元素伤害时，伤害值+2。\n可用次数：1」，
/// 标签 `GCG_TAG_DENDRO_PRODUCE`（草元素产物）。
pub static DENDRO_CORE_DEF: StatusDef<DendroConstruct> = StatusDef::new(
    CardId::new(116),
    StatusType::CombatStatus,
    StateText {
        name: Localized::from_parts(Some("草原核"), None, None, None),
        description: Localized::from_parts(
            Some("我方对敌方出战角色造成火元素伤害或雷元素伤害时，伤害值+2。\n可用次数：1"),
            None,
            None,
            None,
        ),
    },
    // 卡面「可用次数：1」；现有出战状态区按层数承载可用次数（待复核）。
    Some(Nat::new(1)),
    None,
    DendroConstruct,
);

status_identity!(
    /// 出战状态「草原核」的身份（卡 id 116）。
    DendroCore => DENDRO_CORE,
    DENDRO_CORE_DEF,
    DendroConstruct
);

// ======================================================================
// 出战状态「激化领域」（id 117）
// ======================================================================

/// 出战状态「激化领域」（id 117）的定义。
///
/// 数据：`catalyzingfield.json`；名字「激化领域」，`statustypetext` = 阵营出战状态，
/// 描述「我方对敌方出战角色造成雷元素伤害或草元素伤害时，伤害值+1。\n可用次数：2」，
/// 标签 `GCG_TAG_DENDRO_PRODUCE`（草元素产物）。
pub static CATALYZING_FIELD_DEF: StatusDef<DendroConstruct> = StatusDef::new(
    CardId::new(117),
    StatusType::CombatStatus,
    StateText {
        name: Localized::from_parts(Some("激化领域"), None, None, None),
        description: Localized::from_parts(
            Some("我方对敌方出战角色造成雷元素伤害或草元素伤害时，伤害值+1。\n可用次数：2"),
            None,
            None,
            None,
        ),
    },
    // 卡面「可用次数：2」；现有出战状态区按层数承载可用次数（待复核）。
    Some(Nat::new(2)),
    None,
    DendroConstruct,
);

status_identity!(
    /// 出战状态「激化领域」的身份（卡 id 117）。
    CatalyzingField => CATALYZING_FIELD,
    CATALYZING_FIELD_DEF,
    DendroConstruct
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Lang, Resolved};

    fn text_of(identity: &'static dyn StateIdentity) -> &'static StateText {
        identity.text()
    }

    #[test]
    fn known_status_defs_use_the_data_ids_and_types() {
        // 卡 id 与 reactions::entities 的那份编号表**同一个数**（数据 id）。
        assert_eq!(SHIELD_COMBAT_DEF.id, crate::reactions::entities::SHIELD);
        assert_eq!(FROZEN_DEF.id, crate::reactions::entities::FROZEN);
        assert_eq!(DENDRO_CORE_DEF.id, crate::reactions::entities::DENDRO_CORE);
        assert_eq!(
            CATALYZING_FIELD_DEF.id,
            crate::reactions::entities::CATALYZING_FIELD
        );
        assert_eq!(SHIELD_CHARACTER_DEF.id, CardId::new(134));

        // 两个护盾共用 203 / 134 两个真实卡 id，不会撞车。
        assert_ne!(SHIELD_COMBAT_DEF.id, SHIELD_CHARACTER_DEF.id);

        // statustypetext -> StatusType（D51）。
        assert_eq!(SHIELD_COMBAT_DEF.status_type, StatusType::CombatStatus);
        assert_eq!(
            SHIELD_CHARACTER_DEF.status_type,
            StatusType::CharacterStatus
        );
        assert_eq!(FROZEN_DEF.status_type, StatusType::CharacterStatus);
        assert_eq!(DENDRO_CORE_DEF.status_type, StatusType::CombatStatus);
        assert_eq!(
            CATALYZING_FIELD_DEF.status_type,
            StatusType::CombatStatus
        );
    }

    #[test]
    fn known_status_names_and_descriptions_come_from_the_data() {
        assert_eq!(
            SHIELD_COMBAT_DEF.name().get(Lang::ZhCn),
            Resolved::Exact("护盾")
        );
        assert_eq!(
            SHIELD_COMBAT_DEF.description().get(Lang::ZhCn),
            Resolved::Exact("为我方出战角色提供1点护盾。（可叠加，没有上限）")
        );
        assert_eq!(
            FROZEN_DEF.name().get(Lang::ZhCn),
            Resolved::Exact("冻结")
        );
        assert_eq!(
            DENDRO_CORE_DEF.name().get(Lang::ZhCn),
            Resolved::Exact("草原核")
        );
        assert_eq!(
            CATALYZING_FIELD_DEF.name().get(Lang::ZhCn),
            Resolved::Exact("激化领域")
        );
        assert_eq!(
            SHIELD_CHARACTER_DEF.name().get(Lang::ZhCn),
            Resolved::Exact("护盾")
        );
        // 数据里有描述的，描述不能为空（D110：定义数据 = 名字 + 描述）。
        for definition in [
            SHIELD_COMBAT_DEF.name(),
            SHIELD_CHARACTER_DEF.name(),
            FROZEN_DEF.name(),
            DENDRO_CORE_DEF.name(),
            CATALYZING_FIELD_DEF.name(),
        ] {
            assert!(!definition.get(Lang::ZhCn).is_missing());
        }
        for description in [
            SHIELD_COMBAT_DEF.description(),
            SHIELD_CHARACTER_DEF.description(),
            FROZEN_DEF.description(),
            DENDRO_CORE_DEF.description(),
            CATALYZING_FIELD_DEF.description(),
        ] {
            assert!(!description.get(Lang::ZhCn).is_missing());
        }
        // 本块只填中文（D120）：其它语言槽位是 None（D56 显式缺失；get() 取用时会报告回退）。
        assert!(SHIELD_COMBAT_DEF.name().slot(Lang::EnUs).is_none());
        assert!(!SHIELD_COMBAT_DEF.name().has(Lang::EnUs));
    }

    #[test]
    fn known_status_forms_and_durations_are_pinned() {
        // 冻结 = 布尔形态 + 1 回合（「持续到回合结束」）。
        assert_eq!(FROZEN.form(), StateForm::Boolean);
        assert!(FROZEN_DEF.is_boolean());
        assert_eq!(FROZEN_DEF.duration, Some(Nat::new(1)));
        // 其余 = 层数形态、不按回合清减。
        for definition in [
            SHIELD_COMBAT_DEF.form(),
            SHIELD_CHARACTER_DEF.form(),
            DENDRO_CORE_DEF.form(),
            CATALYZING_FIELD_DEF.form(),
        ] {
            assert_eq!(definition, StateForm::Layered);
        }
        for duration in [
            SHIELD_COMBAT_DEF.duration,
            SHIELD_CHARACTER_DEF.duration,
            DENDRO_CORE_DEF.duration,
            CATALYZING_FIELD_DEF.duration,
        ] {
            assert_eq!(duration, None);
        }
        // 层数默认值（从卡面反推，待复核）。
        assert_eq!(SHIELD_COMBAT_DEF.default_layers, Some(Nat::new(1)));
        assert_eq!(SHIELD_CHARACTER_DEF.default_layers, Some(Nat::new(1)));
        assert_eq!(DENDRO_CORE_DEF.default_layers, Some(Nat::new(1)));
        assert_eq!(CATALYZING_FIELD_DEF.default_layers, Some(Nat::new(2)));
    }

    #[test]
    fn known_status_tags_reuse_the_single_tag_definitions() {
        // D55：跨牌族共用的标签只导入定义处那一份，不新建第二份。
        assert_eq!(SHIELD_COMBAT_DEF.tags, Shield);
        assert_eq!(SHIELD_CHARACTER_DEF.tags, Shield);
        assert_eq!(DENDRO_CORE_DEF.tags, DendroConstruct);
        assert_eq!(CATALYZING_FIELD_DEF.tags, DendroConstruct);
        // 冻结的标签类型还没有（tags/status/ 本轮为空）。
        assert_eq!(FROZEN_DEF.tags, ());
    }

    #[test]
    fn identities_are_distinct_static_instances_with_one_text_copy() {
        // 身份 = 静态实例地址（D95 / D96）：五个身份两两不同。
        let identities: [&'static dyn StateIdentity; 5] =
            [&SHIELD_COMBAT, &SHIELD_CHARACTER, &FROZEN, &DENDRO_CORE, &CATALYZING_FIELD];
        for (index, left) in identities.iter().enumerate() {
            for right in identities.iter().skip(index.saturating_add(1)) {
                assert!(
                    !crate::families::action_card::status::same_state_identity(*left, *right),
                    "两个状态身份指向同一份静态实例：身份会撞"
                );
            }
        }
        // 文本只有一份：身份拿到的 &StateText 就是定义里那一份（指针相同，不拷贝）。
        assert!(std::ptr::eq(SHIELD_COMBAT.text(), SHIELD_COMBAT_DEF.text()));
        assert!(std::ptr::eq(FROZEN.text(), FROZEN_DEF.text()));
        assert_eq!(SHIELD_COMBAT.name(), "护盾");
        assert_eq!(FROZEN.name(), "冻结");
        assert_eq!(text_of(&DENDRO_CORE).name.get(Lang::ZhCn), Resolved::Exact("草原核"));
        // 定义自己不被身份改动：身份能读回定义。
        assert_eq!(CATALYZING_FIELD.definition().id, CardId::new(117));
    }
}
