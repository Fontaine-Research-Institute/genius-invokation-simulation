//! 反应会「生成 / 附属」的状态与手牌状态的**身份表**（D95 / D96 / D186）。
//!
//! ## 身份来源（本块已把占位身份换成正式定义）
//!
//! | 身份 | 定义来源 |
//! |---|---|
//! | 出战状态「护盾」203 / 角色状态「护盾」134 / 角色状态「冻结」106 / 出战状态「草原核」116 / 出战状态「激化领域」117 | **`families::status::known`** 的正式 `StatusDef`（数据 = `tcgstatuseffects`；卡 id / 类型 / 名字描述 / 默认层数 / 持续回合 / 标签） |
//! | 共享身份「伤害抵消」（数据里没有独立卡牌实体） | 本文件（规则书·特殊词条；`待复核`） |
//! | 共享身份「伤害免疫」（卡面「免疫所有伤害」的抽象身份） | 本文件（权威关键字 K5 + 卡面；`待复核`） |
//! | 手牌状态「电击」K3003 / 「费用降低」K3005（属**行动牌**，D112 / D114） | 本文件（名字来自关键字表；说明数据为空，D118；`待接入` 行动牌状态块） |
//!
//! 本文件保留的只是两者之间的**唯一桥**：`reactions::effect::EffectOp` 用 `CardId` 表达
//! 「要附属 / 生成什么」，而各区（出战状态区 / 角色状态列表 / 手牌卡状态表 / 召唤物区）的操作
//! 要的是 `&'static dyn StateIdentity`（D95 / D96：身份 = 指向静态实例的指针）。
//! 一份静态表「卡 id -> 身份」，没有运行期状态、没有第二份承载
//! （槽位 / 层数仍然只存在各区里，D64 / D182 / D183）。
//!
//! ## 依据（只读游戏内文本描述 + 规则书）
//!
//! - 护盾（出战状态 203，`tcgstatuseffects/shield.json`）：「为我方出战角色提供1点护盾。
//!   （可叠加，没有上限）」`statustypetext = 阵营出战状态`；
//! - 护盾（角色状态 134，`shields.json`）：「每层抵消1点此角色受到的伤害。（可叠加，没有上限）。」
//!   `statustypetext = 角色状态`；
//! - 冻结（角色状态 106，`frozen.json`）：「角色无法使用技能。（持续到回合结束）\
//!   角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。」
//! - 草原核（出战状态 116，`dendrocore.json`）/ 激化领域（出战状态 117，`catalyzingfield.json`）：
//!   规则书·元素反应·效果 绽放 / 原激化；
//! - 电击（K3003）/ 费用降低（K3005）：关键字表 · 手牌状态（月感电 / 月绽放用），数据只有名字、没有说明（D118）。
//!
//! ## 口径
//!
//! - 身份 = **指向定义数据的地址**（D57 / D58；见 `StateIdentity::identity_address`）。
//!   比较一律用 [`same_identity`]：ZST 身份自身没有独立地址（不同 ZST 静态实例地址可能相同），
//!   所以不能拿 `&dyn StateIdentity` 的地址比较；`StateList` / `CombatStatusZone` 已同口径；
//! - `GameStatus` 这个旧名保留为 [`StateIdentity`] 的 trait object 别名（`damage_chain::mod` 的
//!   `pub use` 不断链）；具体的 8 个身份现在各有自己的类型；
//! - 「减伤」（卡面统一写法「受到伤害时：抵消 N 点伤害」，官方式叫法「伤害抵消」）**没有独立卡牌实体**
//!   （数据里只有带 `GCG_TAG_SUB_HURT` 的各张卡，没有「伤害抵消」这一张），所以给一份**共享身份**
//!   [`DAMAGE_NEGATION`]：具体卡牌定义引用同一份实例即可（不新建编号、不新建承载）；
//! - 本模块**不解释状态语义**（D108）：只给身份 + 名字描述 + 形态（布尔 / 层数）。
//!
//! ## 待接入 / 待复核
//!
//! - **手牌状态的正式定义**（电击 / 费用降低）按 D112 / D114 应归 `families/action_card/status/`；
//!   本轮只能在桥模块给身份（名字来自关键字表、说明为空），等行动牌状态块落地后把这两份
//!   静态实例换成那边的正式实例即可（表形状 [`StatusEntity`] 不变）；
//! - **伤害抵消**：数据里 id 210「抗性」（角色状态，`GCG_TAG_SUB_HURT`）是语义最接近的独立状态
//!   （「受到伤害时：抵消1点伤害。（可叠加，没有上限）」），但「伤害抵消」是**效果**不是某张卡；
//!   是否把 210 当成卡面实体 —— **待复核**；
//! - 形态（布尔 / 层数）**待复核**：护盾按层数（层数 = 护盾点数）、冻结按布尔；
//!   草原核 / 激化领域卡面写「可用次数」，本模块按层数承载（可用次数归召唤物区 / 后续状态定义）；
//! - 电击按布尔、费用降低按层数（每层少花 1 个元素骰）——**待复核**；
//! - 5 张状态卡的 `default_layers` / `duration` 从卡面反推（数据无结构化字段），见 `known.rs`。

use crate::base::CardId;
use crate::families::action_card::areas::hand_state::{
    HAND_COST_INCREASE, HAND_EMPOWERMENT, HAND_INVALIDATION, HAND_UNTUNABLE,
};
use crate::families::action_card::status::{StateForm, StateIdentity, StateText};
use crate::families::character::satiated::{SATIATED, SATIATED_CARD};
use crate::families::character::skill_flow::forbidden::{
    PETRIFIED, PETRIFIED_CARD, STUN, STUN_CARD,
};
use crate::generated::Localized;
use crate::reactions::effect::HandCardStatus;
use crate::reactions::entities;

// ======================================================================
// 正式定义（families/status/known.rs）：保留原来的导出名，调用方一行不改
// ======================================================================

pub use crate::families::status::known::{
    CatalyzingField, DendroCore, Frozen, ShieldCharacter, ShieldCombat, CATALYZING_FIELD,
    DENDRO_CORE, FROZEN, SHIELD_CHARACTER, SHIELD_COMBAT,
};

/// 状态身份的**旧名兼容别名**（原占位类型 `GameStatus` 已删除）。
///
/// 正式身份现在各有自己的具体类型（都实现 [`StateIdentity`]）；这个别名只保证
/// `damage_chain::mod` 的既有 `pub use statuses::GameStatus` 不断链，
/// 语义 = 「任意状态身份」（`dyn StateIdentity`）。
pub type GameStatus = dyn StateIdentity;

/// 两个状态身份是否**同一份身份**（比较 `identity_address()` = 指向定义数据的地址；见模块文档）。
pub fn same_identity(left: &'static dyn StateIdentity, right: &'static dyn StateIdentity) -> bool {
    crate::families::action_card::status::same_state_identity(left, right)
}

/// 这条身份是否在给定的身份集合里（按地址比较）。
pub fn identity_in(
    identity: &'static dyn StateIdentity,
    set: &[&'static dyn StateIdentity],
) -> bool {
    set.iter()
        .copied()
        .any(|candidate| same_identity(candidate, identity))
}

// ======================================================================
// 没有 StatusDef 承载的身份（共享身份 / 手牌状态）：本文件给
// ======================================================================

/// 「伤害抵消」（减伤，没有独立卡牌实体）的文本。
///
/// 数据里没有这一张卡，所以文案来自规则书·特殊词条 伤害抵消 / 卡面统一句式（`待复核`：
/// 数据 id 210「抗性」语义最接近，但是否算同一实体未定）。
static DAMAGE_NEGATION_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("伤害抵消"), None, None, None),
    description: Localized::from_parts(
        Some("所附属角色受到伤害时，抵消与层数相等的伤害"),
        None,
        None,
        None,
    ),
};

/// 手牌状态「电击」（K3003）的文本。
///
/// 名字来自关键字数据（K3003 = 电击）；关键字类状态**没有说明**（D118 / 数据里说明为空），
/// 所以描述保持空（D56 显式缺失）。`待接入`：正式定义归行动牌状态块后换来源。
static ELECTRIC_SHOCK_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("电击"), None, None, None),
    description: Localized::EMPTY,
};

/// 手牌状态「费用降低」（K3005）的文本。
///
/// 名字来自关键字数据（K3005 = 费用降低）；关键字类状态没有说明（D118），描述保持空。
/// `待接入`：正式定义归行动牌状态块后换来源。
static COST_REDUCTION_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("费用降低"), None, None, None),
    description: Localized::EMPTY,
};

/// 声明一个「桥模块自己给」的状态身份（静态文本 + 形态；无 `StatusDef`）。
macro_rules! bridge_identity {
    (
        $(#[$meta:meta])*
        $identity:ident => $value:ident,
        $text:ident,
        $form:expr
    ) => {
        $(#[$meta])*
        #[derive(Debug)]
        pub struct $identity(&'static StateText);

        impl $identity {
            /// 把自身当**身份指针**用（`&'static dyn StateIdentity`；D95 / D96）。
            pub fn identity(self: &'static Self) -> &'static dyn StateIdentity {
                self
            }
        }

        impl StateIdentity for $identity {
            fn text(&self) -> &'static StateText {
                self.0
            }

            fn form(&self) -> StateForm {
                $form
            }
        }

        $(#[$meta])*
        pub static $value: $identity = $identity(&$text);
    };
}

bridge_identity!(
    /// 「伤害抵消」（减伤；共享身份，没有独立卡牌实体）。
    DamageNegation => DAMAGE_NEGATION,
    DAMAGE_NEGATION_TEXT,
    StateForm::Layered
);

/// 「伤害免疫」的文本。
///
/// 规则书正文**没有**「伤害免疫」词条；权威依据是**关键字数据 K5 穿透伤害**
/// （「穿透伤害无法受到伤害加成，但是也无法被护盾、伤害免疫等效果抵消」）
/// 与**卡面**（护罩类状态：「所附属角色**免疫所有伤害**」）。
///
/// 描述取卡面原文语义（不编造数值 / 持续时间）；具体卡牌（护罩）的正式定义归状态块，
/// 本文件只给**共享身份**，供伤修链按身份判定「本次伤害为 0」。
static DAMAGE_IMMUNITY_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("伤害免疫"), None, None, None),
    description: Localized::from_parts(Some("所附属角色免疫所有伤害"), None, None, None),
};

bridge_identity!(
    /// 「伤害免疫」（本次伤害为 0 / 不结算；共享身份，卡面「免疫所有伤害」）。
    DamageImmunity => DAMAGE_IMMUNITY,
    DAMAGE_IMMUNITY_TEXT,
    StateForm::Boolean
);

bridge_identity!(
    /// 手牌状态「电击」（K3003；布尔）。
    ElectricShock => ELECTRIC_SHOCK,
    ELECTRIC_SHOCK_TEXT,
    StateForm::Boolean
);

bridge_identity!(
    /// 手牌状态「费用降低」（K3005；层数，每层少花 1 个元素骰）。
    CostReduction => COST_REDUCTION,
    COST_REDUCTION_TEXT,
    StateForm::Layered
);

// ======================================================================
// 静态表：卡 id -> 状态身份（D186「静态数据 + 指针表」，数据只有一份）
// ======================================================================

/// 一条「卡 id -> 状态身份」登记。
#[derive(Clone, Copy, Debug)]
pub struct StatusEntity {
    /// 状态卡的卡 id（状态牌族 id 区域）。
    pub card: CardId,
    /// 状态身份（类型化静态实例指针，D95 / D96）。
    pub identity: &'static dyn StateIdentity,
}

impl StatusEntity {
    /// 一条登记。
    pub const fn new(card: CardId, identity: &'static dyn StateIdentity) -> StatusEntity {
        StatusEntity { card, identity }
    }
}

/// 反应会附属 / 生成的状态**静态登记表**（顺序 = 规则书条目顺序，不参与判定）。
///
/// 后三项不是反应产物，而是**关键字 / 卡牌会按卡 id 附属**的状态（R5 / L1）：
/// 眩晕 100001、石化 116033、饱腹 303300 —— 登记后 `keywords::attach_status`
/// 用同一张表就能查到身份（状态块落地前先由本族提供共享身份）。
pub const STATUS_ENTITIES: [StatusEntity; 8] = [
    StatusEntity::new(entities::FROZEN, &FROZEN),
    StatusEntity::new(entities::SHIELD, &SHIELD_COMBAT),
    StatusEntity::new(entities::DENDRO_CORE, &DENDRO_CORE),
    StatusEntity::new(entities::CATALYZING_FIELD, &CATALYZING_FIELD),
    StatusEntity::new(CardId::new(134), &SHIELD_CHARACTER),
    StatusEntity::new(STUN_CARD, &STUN),
    StatusEntity::new(PETRIFIED_CARD, &PETRIFIED),
    StatusEntity::new(SATIATED_CARD, &SATIATED),
];

/// 按卡 id 查状态身份（没登记 -> `None`）。
pub fn status_of_card(card: CardId) -> Option<&'static dyn StateIdentity> {
    STATUS_ENTITIES
        .iter()
        .find(|entry| entry.card == card)
        .map(|entry| entry.identity)
}

/// 算作**护盾**的全部状态身份（出战状态 203 + 角色状态 134）。
///
/// 两种写法（「为我方出战角色提供 N 点护盾」/「每层抵消 1 点此角色受到的伤害」）在规则上
/// 都是护盾，所以伤害修正链把两处承载的点数**相加**后一起抵消。
pub const SHIELD_STATUSES: &[&'static dyn StateIdentity] = &[&SHIELD_COMBAT, &SHIELD_CHARACTER];

/// 算作**减伤**（伤害抵消）的状态身份。
///
/// 数据里没有独立的「伤害抵消」实体，所以只有一份共享身份 [`DAMAGE_NEGATION`]；
/// 具体卡牌定义引用同一份实例，链按地址比较即可命中。
pub const REDUCTION_STATUSES: &[&'static dyn StateIdentity] = &[&DAMAGE_NEGATION];

/// 算作**伤害免疫**的状态身份（本次伤害归 0 / 不结算）。
///
/// 依据：权威关键字 K5（穿透伤害「无法被护盾、伤害免疫等效果抵消」）+ 卡面
/// 「所附属角色免疫所有伤害」。具体护罩类状态的正式定义归状态块（本块不新建卡牌身份），
/// 链路只要求它们引用同一份 [`DAMAGE_IMMUNITY`] 或由调用方给自定义集合
/// （[`crate::families::character::damage_chain::chain::DamageChainEnv::with_immunities`]）。
pub const IMMUNITY_STATUSES: &[&'static dyn StateIdentity] = &[&DAMAGE_IMMUNITY];

/// 手牌状态枚举 -> 状态身份（月感电 / 月绽放用；K3003–K3008 共 6 种）。
pub fn hand_status_identity(status: HandCardStatus) -> &'static dyn StateIdentity {
    match status {
        HandCardStatus::ElectricShock => &ELECTRIC_SHOCK,
        HandCardStatus::CostIncrease => &HAND_COST_INCREASE,
        HandCardStatus::CostReduction => &COST_REDUCTION,
        HandCardStatus::Empowerment => &HAND_EMPOWERMENT,
        HandCardStatus::Untunable => &HAND_UNTUNABLE,
        HandCardStatus::Invalidation => &HAND_INVALIDATION,
    }
}

/// 状态身份 -> 手牌状态枚举（反向查询；不是这 6 种之一 -> None）。
pub fn hand_status_of_identity(identity: &'static dyn StateIdentity) -> Option<HandCardStatus> {
    HandCardStatus::ALL
        .iter()
        .copied()
        .find(|status| same_identity(hand_status_identity(*status), identity))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Lang, Resolved};
    use gi_genius_invokation_engine::Nat;

    #[test]
    fn reaction_status_cards_map_to_their_identities() {
        // 反应模块产出的 `EffectOp::Status { card }` 都能查到身份（执行器才不会丢操作）。
        for (card, expected) in [
            (entities::FROZEN, &FROZEN as &'static dyn StateIdentity),
            (entities::SHIELD, &SHIELD_COMBAT),
            (entities::DENDRO_CORE, &DENDRO_CORE),
            (entities::CATALYZING_FIELD, &CATALYZING_FIELD),
        ] {
            let found = status_of_card(card).expect("反应会生成的状态必须登记");
            assert!(same_identity(found, expected));
        }
        // 未登记的卡 id -> None（不 panic、不猜）。
        assert!(status_of_card(CardId::new(999_999)).is_none());
    }

    #[test]
    fn keyword_and_card_statuses_are_registered_by_their_data_ids() {
        // R5 / L1：眩晕 100001、石化 116033、饱腹 303300 都能按卡 id 查到身份。
        let stun = status_of_card(STUN_CARD).expect("眩晕已登记");
        assert!(same_identity(stun, &STUN));
        let petrified = status_of_card(PETRIFIED_CARD).expect("石化已登记");
        assert!(same_identity(petrified, &PETRIFIED));
        let satiated = status_of_card(SATIATED_CARD).expect("饱腹已登记");
        assert!(same_identity(satiated, &SATIATED));
        // 与正式定义的状态互不混淆。
        assert!(!same_identity(stun, &FROZEN));
        assert!(!same_identity(satiated, &PETRIFIED));
    }

    #[test]
    fn identity_comparison_is_by_address_not_vtable() {
        // 同一份静态实例 -> 同身份；不同实例 -> 不同身份（D95 / D96 指针身份）。
        assert!(same_identity(&FROZEN, &FROZEN));
        assert!(!same_identity(&FROZEN, &DENDRO_CORE));
        // 经 `identity()` 拿到胖指针后仍然按地址比。
        let left: &'static dyn StateIdentity = SHIELD_COMBAT.identity();
        let right: &'static dyn StateIdentity = &SHIELD_COMBAT;
        assert!(same_identity(left, right));
    }

    #[test]
    fn damage_immunity_identity_is_boolean_and_named_from_the_keyword_data() {
        // K5 原文点名「伤害免疫」；卡面写「免疫所有伤害」。
        assert_eq!(DAMAGE_IMMUNITY.name(), "伤害免疫");
        assert_eq!(DAMAGE_IMMUNITY.form(), StateForm::Boolean);
        assert_eq!(
            DAMAGE_IMMUNITY.text().description.get(Lang::ZhCn).text(),
            Some("所附属角色免疫所有伤害")
        );
        // 与护盾 / 减伤是**不同**身份（互不混淆）。
        assert!(!same_identity(&DAMAGE_IMMUNITY, &SHIELD_COMBAT));
        assert!(!same_identity(&DAMAGE_IMMUNITY, &DAMAGE_NEGATION));
        // 默认免疫集合只有它自己。
        assert_eq!(IMMUNITY_STATUSES.len(), 1);
        assert!(identity_in(&DAMAGE_IMMUNITY, IMMUNITY_STATUSES));
        assert!(!identity_in(&SHIELD_COMBAT, IMMUNITY_STATUSES));
    }

    #[test]
    fn shield_and_reduction_identity_sets_are_the_rulebook_ones() {
        assert_eq!(SHIELD_STATUSES.len(), 2);
        assert!(identity_in(&SHIELD_COMBAT, SHIELD_STATUSES));
        assert!(identity_in(&SHIELD_CHARACTER, SHIELD_STATUSES));
        assert!(!identity_in(&DAMAGE_NEGATION, SHIELD_STATUSES));
        assert_eq!(REDUCTION_STATUSES.len(), 1);
        assert!(identity_in(&DAMAGE_NEGATION, REDUCTION_STATUSES));
    }

    #[test]
    fn hand_status_identities_are_distinct_and_layered_correctly() {
        let shock = hand_status_identity(HandCardStatus::ElectricShock);
        let cheaper = hand_status_identity(HandCardStatus::CostReduction);
        assert!(!same_identity(shock, cheaper));
        // 电击 = 布尔（在列表里即代表有）、费用降低 = 层数（每层 -1 费）。
        assert_eq!(shock.form(), StateForm::Boolean);
        assert_eq!(cheaper.form(), StateForm::Layered);
    }

    #[test]
    fn all_six_hand_statuses_map_to_identities_and_back() {
        let expected = [
            (HandCardStatus::ElectricShock, "电击", Nat::new(3003)),
            (HandCardStatus::CostIncrease, "费用增加", Nat::new(3004)),
            (HandCardStatus::CostReduction, "费用降低", Nat::new(3005)),
            (HandCardStatus::Empowerment, "赋能", Nat::new(3006)),
            (HandCardStatus::Untunable, "不可调和", Nat::new(3007)),
            (HandCardStatus::Invalidation, "无效化", Nat::new(3008)),
        ];
        for (status, name, k_code) in expected {
            assert_eq!(status.k_code(), k_code);
            assert_eq!(HandCardStatus::from_k_code(k_code), Some(status));
            let identity = hand_status_identity(status);
            assert_eq!(identity.name(), name);
            assert_eq!(hand_status_of_identity(identity), Some(status));
        }
        assert_eq!(HandCardStatus::ALL.len(), 6);
        assert_eq!(HandCardStatus::from_k_code(Nat::new(3002)), None);
        // 非手牌状态的身份查不到反向枚举。
        assert_eq!(hand_status_of_identity(&SHIELD_COMBAT), None);
        assert_eq!(hand_status_of_identity(&FROZEN), None);
    }

    #[test]
    fn identities_expose_their_rulebook_names() {
        // 5 张状态卡的名字逐字来自数据（known.rs 的 StatusDef）；只有一份。
        assert_eq!(SHIELD_COMBAT.name(), "护盾");
        assert_eq!(SHIELD_CHARACTER.name(), "护盾");
        assert_eq!(FROZEN.name(), "冻结");
        assert_eq!(DENDRO_CORE.name(), "草原核");
        assert_eq!(CATALYZING_FIELD.name(), "激化领域");
        // 共享 / 手牌状态：伤害抵消来自规则书、电击 / 费用降低来自关键字数据。
        assert_eq!(DAMAGE_NEGATION.name(), "伤害抵消");
        assert_eq!(ELECTRIC_SHOCK.name(), "电击");
        assert_eq!(COST_REDUCTION.name(), "费用降低");
    }

    #[test]
    fn formal_definitions_drive_identity_text_without_copying() {
        // 5 张状态卡的文本来自 known.rs 的 StatusDef（同一份，不拷贝）。
        assert!(std::ptr::eq(
            FROZEN.text(),
            crate::families::status::known::FROZEN_DEF.text()
        ));
        assert!(std::ptr::eq(
            SHIELD_COMBAT.text(),
            crate::families::status::known::SHIELD_COMBAT_DEF.text()
        ));
        assert_eq!(
            FROZEN.text().description.get(Lang::ZhCn).text(),
            Some("角色无法使用技能。（持续到回合结束）\n角色受到火元素伤害或物理伤害时，移除此效果，使该伤害+2。")
        );
        assert_eq!(
            SHIELD_COMBAT.text().name.get(Lang::ZhCn),
            Resolved::Exact("护盾")
        );
        // 身份能读回定义（卡 id / 类型）。
        assert_eq!(
            SHIELD_COMBAT.definition().id,
            crate::reactions::entities::SHIELD
        );
    }

    #[test]
    fn hand_statuses_have_names_but_no_descriptions_in_data() {
        // D118：关键字类手牌状态没有说明 -> 描述保持空（D56 显式缺失），不编造。
        assert_eq!(
            ELECTRIC_SHOCK.text().name.get(Lang::ZhCn),
            Resolved::Exact("电击")
        );
        assert!(ELECTRIC_SHOCK
            .text()
            .description
            .get(Lang::ZhCn)
            .is_missing());
        assert_eq!(
            COST_REDUCTION.text().name.get(Lang::ZhCn),
            Resolved::Exact("费用降低")
        );
        assert!(COST_REDUCTION
            .text()
            .description
            .get(Lang::ZhCn)
            .is_missing());
        // 伤害抵消没有卡面数据，只有规则书文案（描述非空）。
        assert!(!DAMAGE_NEGATION
            .text()
            .description
            .get(Lang::ZhCn)
            .is_missing());
    }

    #[test]
    fn game_status_alias_still_names_a_status_identity() {
        // 旧名 GameStatus 保留为 trait object 别名（damage_chain::mod 的既有导出不断链）。
        let identity: &'static GameStatus = &FROZEN;
        assert_eq!(identity.name(), "冻结");
    }
}
