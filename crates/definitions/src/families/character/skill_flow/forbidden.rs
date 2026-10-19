//! **状态禁用技能查询**（覆盖报告 R5 / S6.6 冻结 + S8.4 特技 + K55 眩晕）。
//!
//! ## 依据（规则书 + 关键字数据）
//!
//! - 规则书·元素反应·效果 **冻结**（状态 106）：「角色无法使用技能。（持续到回合结束）」；
//! - 规则书·特殊词条 **特技**：「角色因冻结、石化、眩晕等原因无法使用技能时，
//!   也会同时无法使用『特技』。」
//! - 权威关键字 **K55 眩晕**：「角色无法使用技能。（持续到回合结束）」；
//!   数据 100001「眩晕」同文；
//! - 数据 **石化**：116033「角色无法使用技能。（持续到回合结束）」/
//!   176033「角色无法使用技能。（持续到下个回合结束）」。
//!
//! ## 口径
//!
//! - **查询只看角色状态列表里有没有**（位置 = CHARACTER_STATE_POS，D105 公共列表；
//!   布尔形态，与层数无关）；
//! - 身份集合**可配置**：默认 SKILL_FORBIDDING_STATUSES（冻结 = 正式定义
//!   families::status::known::FROZEN；眩晕 / 石化正式定义归状态块，本文件先给共享身份，
//!   与 damage_chain::statuses::DAMAGE_NEGATION 同口径）；
//! - 结果 ForbiddenUses 说明「禁用了哪些**主动使用**入口」：普通攻击 / 元素战技 / 元素爆发 /
//!   特技（被动技能本来就不能主动使用，见 CannotUse::PassiveSkill，这里不重复表态）；
//! - 纯函数 / 只读：不改状态、不 panic。
//!
//! ## 待接入
//!
//! - 具体状态的正式 StatusDef（类型 / 持续回合 / 标签）归 families::status::known；
//!   接上后把默认集合换成那边的身份即可（集合形状不变）；
//! - 查询已接到 skill_flow::can_use（新增 CannotUse::ForbiddenByStatus）与特技入口
//!   skill_flow::technique；状态**何时解除**（持续回合递减）归 state/flows，不在本轮写范围；
//! - 石化两份数据（116033 / 176033）共用同一身份；持续时间由状态定义承载，本查询不读 duration。

use crate::base::CardId;
use crate::families::action_card::status::{
    same_state_identity, StateForm, StateIdentity, StateList, StateText,
};
use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
use crate::families::character::types::SkillKind;
use crate::families::status::known::FROZEN;
use crate::generated::Localized;

/// 数据（tcgstatuseffects）里「眩晕」的卡 id（100001）。
pub const STUN_CARD: CardId = CardId::new(100001);

/// 数据（tcgstatuseffects）里「石化」的卡 id（116033；176033 是另一份同效果数据）。
pub const PETRIFIED_CARD: CardId = CardId::new(116033);

static STUN_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("眩晕"), None, None, None),
    description: Localized::from_parts(Some("角色无法使用技能。（持续到回合结束）"), None, None, None),
};

static PETRIFIED_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("石化"), None, None, None),
    description: Localized::from_parts(Some("角色无法使用技能。（持续到回合结束）"), None, None, None),
};

/// 「眩晕」状态身份（K55 / 数据 100001）。
#[derive(Debug)]
pub struct Stun(&'static StateText);

impl Stun {
    /// 把自身当**身份指针**用。
    pub fn identity(&'static self) -> &'static dyn StateIdentity {
        self
    }
}

impl StateIdentity for Stun {
    fn text(&self) -> &'static StateText {
        self.0
    }

    fn form(&self) -> StateForm {
        StateForm::Boolean
    }
}

/// 「石化」状态身份（数据 116033 / 176033）。
#[derive(Debug)]
pub struct Petrified(&'static StateText);

impl Petrified {
    /// 把自身当**身份指针**用。
    pub fn identity(&'static self) -> &'static dyn StateIdentity {
        self
    }
}

impl StateIdentity for Petrified {
    fn text(&self) -> &'static StateText {
        self.0
    }

    fn form(&self) -> StateForm {
        StateForm::Boolean
    }
}

/// 「眩晕」的唯一静态身份实例。
pub static STUN: Stun = Stun(&STUN_TEXT);

/// 「石化」的唯一静态身份实例。
pub static PETRIFIED: Petrified = Petrified(&PETRIFIED_TEXT);

/// 默认的「禁用技能」状态身份集合：冻结（正式定义）+ 眩晕 / 石化（共享身份）。
pub const SKILL_FORBIDDING_STATUSES: &[&'static dyn StateIdentity] =
    &[&FROZEN, &STUN, &PETRIFIED];

/// 被禁用的**主动使用入口**（哪些动作不能做）。
///
/// 规则书里「冻结 / 石化 / 眩晕」都是「角色无法使用技能」，特技也一并禁用（S8.4 特技）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ForbiddenUses {
    /// 普通攻击不能用。
    pub normal_attack: bool,
    /// 元素战技不能用。
    pub elemental_skill: bool,
    /// 元素爆发不能用。
    pub elemental_burst: bool,
    /// 特技不能用（S8.4：「也会同时无法使用『特技』」）。
    pub technique: bool,
}

impl ForbiddenUses {
    /// 全部主动入口都禁用（冻结 / 石化 / 眩晕的实际取值）。
    pub const ALL: ForbiddenUses = ForbiddenUses {
        normal_attack: true,
        elemental_skill: true,
        elemental_burst: true,
        technique: true,
    };

    /// 一个都不禁用（没有状态时用）。
    pub const NONE: ForbiddenUses = ForbiddenUses {
        normal_attack: false,
        elemental_skill: false,
        elemental_burst: false,
        technique: false,
    };

    /// 这个技能种类是否被禁用（被动技能不在「主动使用」口径里 -> false）。
    pub fn forbids_skill_kind(self, kind: SkillKind) -> bool {
        match kind {
            SkillKind::NormalAttack(_) => self.normal_attack,
            SkillKind::ElementalSkill => self.elemental_skill,
            SkillKind::ElementalBurst => self.elemental_burst,
            SkillKind::Passive => false,
        }
    }

    /// 是否一个入口都没禁。
    pub fn is_empty(self) -> bool {
        !self.normal_attack && !self.elemental_skill && !self.elemental_burst && !self.technique
    }
}

impl Default for ForbiddenUses {
    fn default() -> ForbiddenUses {
        ForbiddenUses::NONE
    }
}

/// 一条查询结果：**哪个状态**（身份）+ 它禁用了**哪些入口**。
#[derive(Clone, Copy, Debug)]
pub struct ForbiddenState {
    /// 导致禁用的状态身份（第一个命中的；判定顺序 = 身份集合顺序）。
    pub state: &'static dyn StateIdentity,
    /// 被禁用的入口。
    pub uses: ForbiddenUses,
}

impl PartialEq for ForbiddenState {
    fn eq(&self, other: &ForbiddenState) -> bool {
        same_state_identity(self.state, other.state) && self.uses == other.uses
    }
}

impl Eq for ForbiddenState {}

/// 某个状态身份在角色状态列表里是否存在（位置 = 角色状态口径）。
fn state_present(states: &StateList, identity: &'static dyn StateIdentity) -> bool {
    states.contains(CHARACTER_STATE_POS, identity)
}

/// 在**给定的身份集合**里找第一个「存在且禁用技能」的状态（只读）。
///
/// - 顺序 = 传入的身份集合顺序（默认集合：冻结 -> 眩晕 -> 石化）；
/// - 都不存在 -> None。
pub fn forbidding_status(
    states: &StateList,
    identities: &[&'static dyn StateIdentity],
) -> Option<&'static dyn StateIdentity> {
    identities
        .iter()
        .copied()
        .find(|identity| state_present(states, *identity))
}

/// 用**默认身份集合**查询：角色身上有没有导致禁用技能的状态。
///
/// 有 -> Some（状态身份 + 被禁用的入口）；没有 -> None。
pub fn forbidden_by(states: &StateList) -> Option<ForbiddenState> {
    forbidding_status(states, SKILL_FORBIDDING_STATUSES).map(|state| ForbiddenState {
        state,
        // 冻结 / 石化 / 眩晕的原文都是「无法使用技能」+ S8.4 连特技一起禁 -> 全部主动入口。
        uses: ForbiddenUses::ALL,
    })
}

/// 这个技能种类现在**还能不能用**（只看状态禁用；费用 / 次数 / 准备技能归 can_use）。
pub fn skill_allowed(states: &StateList, kind: SkillKind) -> bool {
    match forbidden_by(states) {
        None => true,
        Some(forbidden) => !forbidden.uses.forbids_skill_kind(kind),
    }
}

/// 特技现在**还能不能用**（S8.4：冻结 / 石化 / 眩晕时同时无法使用特技）。
pub fn technique_allowed(states: &StateList) -> bool {
    match forbidden_by(states) {
        None => true,
        Some(forbidden) => !forbidden.uses.technique,
    }
}

impl super::SkillUseContext<'_> {
    /// 这个角色现在被哪些状态禁用技能（只读；can_use 的 ForbiddenByStatus 用它）。
    pub fn forbidden(&self) -> Option<ForbiddenState> {
        forbidden_by(&self.character.states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::StateList;
    use crate::generated::{Lang, Resolved};

    fn states_with(identity: &'static dyn StateIdentity) -> StateList {
        let mut states = StateList::new();
        states.add(CHARACTER_STATE_POS, identity, None);
        states
    }

    #[test]
    fn forbidden_identities_come_from_keyword_and_data() {
        assert_eq!(STUN.name(), "眩晕");
        assert_eq!(STUN.form(), StateForm::Boolean);
        assert_eq!(STUN.text().name.get(Lang::ZhCn), Resolved::Exact("眩晕"));
        assert_eq!(
            STUN.text().description.get(Lang::ZhCn).text(),
            Some("角色无法使用技能。（持续到回合结束）")
        );
        assert_eq!(PETRIFIED.name(), "石化");
        assert_eq!(PETRIFIED.form(), StateForm::Boolean);
        assert_eq!(
            PETRIFIED.text().description.get(Lang::ZhCn).text(),
            Some("角色无法使用技能。（持续到回合结束）")
        );
        // 默认集合 = 冻结（正式定义）+ 眩晕 + 石化。
        assert_eq!(SKILL_FORBIDDING_STATUSES.len(), 3);
        assert!(SKILL_FORBIDDING_STATUSES
            .iter()
            .any(|identity| same_state_identity(*identity, &FROZEN)));
        assert!(SKILL_FORBIDDING_STATUSES
            .iter()
            .any(|identity| same_state_identity(*identity, &STUN)));
        assert!(SKILL_FORBIDDING_STATUSES
            .iter()
            .any(|identity| same_state_identity(*identity, &PETRIFIED)));
    }

    #[test]
    fn frozen_blocks_every_active_entry_and_technique() {
        // 规则书冻结「角色无法使用技能」+ S8.4「也会同时无法使用特技」。
        let states = states_with(&FROZEN);
        let forbidden = forbidden_by(&states).expect("冻结应命中");
        assert!(same_state_identity(forbidden.state, &FROZEN));
        assert_eq!(forbidden.uses, ForbiddenUses::ALL);
        assert!(!forbidden.uses.is_empty());
        // 普通攻击 / 战技 / 爆发 / 特技都被禁；被动不在主动口径里。
        assert!(!skill_allowed(&states, SkillKind::normal_attack()));
        assert!(!skill_allowed(&states, SkillKind::ElementalSkill));
        assert!(!skill_allowed(&states, SkillKind::ElementalBurst));
        assert!(!technique_allowed(&states));
        assert!(!forbidden.uses.forbids_skill_kind(SkillKind::Passive));
        // 没有状态 -> 全都能用。
        let empty = StateList::new();
        assert!(forbidden_by(&empty).is_none());
        assert!(skill_allowed(&empty, SkillKind::normal_attack()));
        assert!(technique_allowed(&empty));
    }

    #[test]
    fn stun_and_petrified_use_the_same_query() {
        for identity in [&STUN as &'static dyn StateIdentity, &PETRIFIED] {
            let states = states_with(identity);
            let forbidden = forbidden_by(&states).expect("应命中");
            assert_eq!(forbidden.uses, ForbiddenUses::ALL);
            assert!(!skill_allowed(&states, SkillKind::ElementalSkill));
            assert!(!technique_allowed(&states));
        }
    }

    #[test]
    fn custom_identity_set_can_replace_the_default() {
        // 具体状态的正式定义落地后 / 测试用：换一组身份即可，不新建承载。
        let states = states_with(&STUN);
        const STUN_ONLY: [&'static dyn StateIdentity; 1] = [&STUN];
        // 默认集合命中 STUN。
        assert!(forbidding_status(&states, SKILL_FORBIDDING_STATUSES).is_some());
        // 自定义集合：冻结 -> 不命中；只含眩晕 -> 命中。
        const FROZEN_ONLY: [&'static dyn StateIdentity; 1] = [&FROZEN];
        assert!(forbidding_status(&states, &FROZEN_ONLY).is_none());
        assert!(forbidding_status(&states, &STUN_ONLY).is_some());
    }
}
