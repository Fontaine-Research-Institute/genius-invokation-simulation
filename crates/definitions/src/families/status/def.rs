//! **状态定义载体**（`StatusDef`，D95 / D96 / D108 / D110 / D13）。
//!
//! 本文件只有**定义数据**，不含「状态区如何存状态」与任何结算（D108：框架只给身份 +
//! 定义数据，不解释具体语义）：
//! - 身份抽象仍是 `families::action_card::status` 的 `StateIdentity`（D95 / D96 / D110）；
//!   **本类型不实现 `StateIdentity`**，它只承载「一张状态卡是什么」的数据；
//! - 具体状态的**身份实例**在 `families/status/known.rs`（每个状态一个类型 + 一份静态实例），
//!   身份取的是本定义里的 `StateText` 指针 —— **名字 / 描述只有一份**（D110 / D64 精神）；
//! - 状态的两个类型（角色状态 / 出战状态）共用状态牌族的同一个 id 区域（D51），
//!   所以 `status_type` 是定义的一部分，不是可选属性。
//!
//! ## 字段口径
//!
//! - `id`：状态牌的卡 id（状态牌族 id 区域；数据来源 `tcgstatuseffects/<名>.json` 的 `id`）；
//! - `status_type`：由数据 `statustypetext` 决定（`角色状态` -> CharacterStatus、
//!   `阵营出战状态` -> CombatStatus；数据里还有 `角色装备` 一类，**不属于**本牌族，见待复核）；
//! - `text`：4 语名字 + 4 语描述（与 `TagText` / `StateText` 同构，D110）；本块只填中文（D120），
//!   其余语言槽位 `None`（D56 / D68 显式缺失）；
//! - `default_layers`：**默认层数**。`None` = 布尔形态（在列表里即代表「有」，D104）；
//!   `Some(n)` = 层数形态的默认层数（宿主不给层数时按它起步，D100：存储只合并不钳制）；
//! - `duration`：**持续回合数**。`None` = 不按回合清减；`Some(n)` = 回合开始时清减 n（D13）。
//!   数据里没有结构化字段，只有描述文本（如「持续到回合结束」），所以**待复核**；
//! - `tags`：标签集合。标签是**零大小类型**（D54），集合形态本轮未定（同 `CharacterDef`），
//!   先给类型参数；没有标签用 `()`。
//!
//! ## 待复核 / 待接入
//!
//! - **数据里的状态标签**：`tcgstatuseffects` 里 437 条状态带标签（`GCG_TAG_SHEILD` /
//!   `GCG_TAG_FORBIDDEN_ATTACK` / `GCG_TAG_DENDRO_PRODUCE` …），但 `tags/status/` 目前是空的，
//!   所以本块只有「已别处有唯一定义」的标签能填（护盾 -> `tags::action_card::common::shield`、
//!   草元素产物 -> `tags::summon::dendro_construct`），其余先 `()`（`待接入` 标签块）；
//! - **`statustypetext = 角色装备`（276 条）**：数据里这一类不是状态牌（更像装备态），
//!   与 D51「状态只有两个类型」冲突，本轮**不建**第三种类型（`待复核`）；
//! - `default_layers` 的具体默认值多数只能从卡面描述反推（如「提供1点护盾」），
//!   数据没有结构化字段 —— 标了 `待复核` 的条目不作为规则依据；
//! - `duration` 的「回合开始清减」由时间轴 / 状态区块实现（D13），本块只存数据。
//!
//! ## 为什么身份类型不是「字面零大小」
//!
//! `known.rs` 里每个身份类型都持有一份 `&'static StatusDef<Tags>`（指针大小，8 字节），
//! **不是** `size_of == 0`。原因：D95 / D96 的身份 = **指向定义数据的地址**，而本仓库实测
//! （rustc 1.98.1）**不同零大小静态实例的地址完全相同**，直接比 `&dyn` 地址会把不同身份
//! 判成同一个 —— 所以 `StateIdentity` 增加 `identity_address()`（默认取 `text()` 那条
//! `StateText` 的地址），`statuses::same_identity` / `state_address` 全部走它。
//! 身份实例保留「指向静态定义」的指针字段，与 `identity_address()` 同源；
//! 运行期不含可变状态、不拷贝文案。

use crate::base::CardId;
use crate::families::action_card::status::{StateForm, StateText};
use crate::families::status::StatusType;
use crate::generated::Localized;
use gi_genius_invokation_engine::Nat;

/// 一张**状态牌**的定义（纯数据；身份由 `known.rs` 的具体类型给）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
/// 本类型**不实现** `StateIdentity`：身份是「指向静态实例的指针」（D95 / D96），
/// 定义只是那份静态实例里的一份数据。
#[derive(Clone, Copy, Debug)]
pub struct StatusDef<Tags> {
    /// 状态牌的卡 id（状态牌族 id 区域内唯一）。
    ///
    /// 没有独立卡牌的共享身份（如「伤害抵消」）**不建** `StatusDef`，见
    /// `damage_chain::statuses`（那是「效果共享身份」，不是某张卡）。
    pub id: CardId,
    /// 状态类型（角色状态 / 出战状态，D51）。
    pub status_type: StatusType,
    /// 4 语名字 + 4 语描述（**唯一一份**；身份经它取，不另存）。
    pub text: StateText,
    /// 默认层数：`None` = 布尔形态；`Some(n)` = 层数形态默认 n 层。
    pub default_layers: Option<Nat>,
    /// 持续回合数：`None` = 不按回合清减；`Some(n)` = 回合开始清减 n（D13）。
    pub duration: Option<Nat>,
    /// 标签集合（D54：标签是零大小类型；集合形态本轮未定）。
    pub tags: Tags,
}

impl<Tags> StatusDef<Tags> {
    /// 建一份状态定义（常量 / 测试用；6 个参数，不再拆构造器）。
    pub const fn new(
        id: CardId,
        status_type: StatusType,
        text: StateText,
        default_layers: Option<Nat>,
        duration: Option<Nat>,
        tags: Tags,
    ) -> StatusDef<Tags> {
        StatusDef {
            id,
            status_type,
            text,
            default_layers,
            duration,
            tags,
        }
    }

    /// 静态定义数据（名字 + 描述；只有一份，不拷贝）。
    pub const fn text(&self) -> &StateText {
        &self.text
    }

    /// 4 语名字（唯一一份）。
    pub const fn name(&self) -> Localized {
        self.text.name
    }

    /// 4 语描述（唯一一份）。
    pub const fn description(&self) -> Localized {
        self.text.description
    }

    /// 状态的形态（D104）：`default_layers` 为 `None` -> 布尔；`Some` -> 层数。
    ///
    /// 形态由「默认层数」这一份数据推导，不另存第二份（D64 精神）。
    pub const fn form(&self) -> StateForm {
        match self.default_layers {
            None => StateForm::Boolean,
            Some(_) => StateForm::Layered,
        }
    }

    /// 是否布尔形态（不记录层数）。
    pub const fn is_boolean(&self) -> bool {
        self.default_layers.is_none()
    }

    /// 是否按回合清减（`duration` 有值）。
    pub const fn has_duration(&self) -> bool {
        self.duration.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::StateForm;
    use crate::generated::{Lang, Resolved};

    /// 测试用静态文本（**不是真实状态数据**）。
    static SAMPLE_TEXT: StateText = StateText {
        name: Localized::from_parts(Some("测试状态"), None, None, None),
        description: Localized::from_parts(Some("测试用描述。"), None, None, None),
    };

    /// 布尔形态的状态定义（无层数、无持续回合）。
    static BOOLEAN_DEF: StatusDef<()> = StatusDef::new(
        CardId::new(900_001),
        StatusType::CharacterStatus,
        SAMPLE_TEXT,
        None,
        None,
        (),
    );

    /// 层数形态的状态定义（默认 2 层、持续 1 回合）。
    static LAYERED_DEF: StatusDef<()> = StatusDef::new(
        CardId::new(900_002),
        StatusType::CombatStatus,
        SAMPLE_TEXT,
        Some(Nat::new(2)),
        Some(Nat::new(1)),
        (),
    );

    #[test]
    fn status_def_carries_id_type_text_layers_duration_and_tags() {
        assert_eq!(BOOLEAN_DEF.id, CardId::new(900_001));
        assert_eq!(BOOLEAN_DEF.status_type, StatusType::CharacterStatus);
        assert_eq!(BOOLEAN_DEF.default_layers, None);
        assert_eq!(BOOLEAN_DEF.duration, None);
        assert_eq!(BOOLEAN_DEF.tags, ());
        // 名字 / 描述就存在定义自己的 `text` 里（唯一一份，不在别处再存一份）。
        assert_eq!(BOOLEAN_DEF.name().get(Lang::ZhCn), Resolved::Exact("测试状态"));
        assert_eq!(
            BOOLEAN_DEF.description().get(Lang::ZhCn),
            Resolved::Exact("测试用描述。")
        );
        // 本块只填中文（D120）：其它语言槽位是 None（D56 显式缺失；取用时 get() 会报告回退）。
        assert!(BOOLEAN_DEF.name().slot(Lang::EnUs).is_none());
        assert!(!BOOLEAN_DEF.name().has(Lang::EnUs));

        assert_eq!(LAYERED_DEF.id, CardId::new(900_002));
        assert_eq!(LAYERED_DEF.status_type, StatusType::CombatStatus);
        assert_eq!(LAYERED_DEF.default_layers, Some(Nat::new(2)));
        assert_eq!(LAYERED_DEF.duration, Some(Nat::new(1)));
    }

    #[test]
    fn form_is_derived_from_default_layers_only() {
        // D104：布尔 / 层数两种形态由「默认层数」推导，不另存一份形态字段。
        assert_eq!(BOOLEAN_DEF.form(), StateForm::Boolean);
        assert!(BOOLEAN_DEF.is_boolean());
        assert_eq!(LAYERED_DEF.form(), StateForm::Layered);
        assert!(!LAYERED_DEF.is_boolean());
    }

    #[test]
    fn duration_is_explicitly_optional() {
        // D13：duration 有值才按回合清减；没有值就是「不按回合清减」。
        assert!(!BOOLEAN_DEF.has_duration());
        assert!(LAYERED_DEF.has_duration());
    }
}
