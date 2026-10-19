//! **行动牌定义载体**（`ActionCardDef`）+ **打出条件**（D217）。
//!
//! ## 这一层解决什么
//!
//! 一张行动牌 = 一个 [`ActionCardType`] + 若干标签（见 `families/action_card/mod.rs`）。
//! 本文件把「一张行动牌是什么」收成一份**纯数据**：卡 id、类型、名字 / 描述、标签、
//! **基础费用**，以及 —— D217 —— **它自己的打出条件**（函数指针）。
//!
//! 用户口径（D217）：「行动牌持定义自己的打出条件，尝试打出(比较是否付得起前)先调用这个」。
//! 所以 `flow/play_card.rs` 在**比较是否付得起之前**调用 [`PlayConditionFn`]：
//! 不通过就直接拒绝、**状态一个字节不变**；通过后才走 支付 -> 离手 -> 落位 -> 派发。
//!
//! ## 依据（唯一权威）
//!
//! - 规则书·进行行动 打出手牌：「用元素骰支付所需的费用，打出一张手牌」——「能不能打出」
//!   与「付不付得起」是两件事，条件在**付费之前**；卡面上写这类条件（「…时才能打出 /
//!   只能…时打出」的句式）；
//! - 规则书·游戏区域 II(1) / III / VI：装备牌附属给角色、支援牌进支援区、事件牌结算完离场
//!   —— 落位由 `flow::CardPlacement` 表达，本文件只提供 [`ActionCardDef::card_type`]；
//! - D217（Lead 拍板）：条件在付费判定前调用；拒绝给原因；状态不变；
//! - D186：效果 / 条件一律**登记函数指针**，不登记闭包、不捕获环境（可静态存放、可比较）；
//! - D120：本块只填**中文**名字 / 描述，其余语言槽位 `None`（D56 / D68 显式缺失；
//!   取值走 `Localized::get` 会如实报告回退，绝不静默顶替）。
//!
//! ## 口径
//!
//! - **纯数据 + 只读访问器**：字段私有，读一律走访问器（D182 / D183 精神：对外只读）；
//!   `new` 与访问器全是 `const fn`，所以一份定义可以写成 `static` 常量（卡牌文件用）；
//! - **`cost` 是静态切片**（`&'static [CostComponent]`）：基础费用写在卡牌文件里，
//!   不每次分配。费用**修正链**（少花费 / 加费用）还没接，见「待接入」；
//! - **`play_condition` 是函数指针** [`PlayConditionFn`]：吃 `&DefState` + 视角方、
//!   返回 [`PlayCondition`]。签名里只有**只读**引用 ⇒ 条件改不了任何状态（纯函数由签名保证），
//!   也不许采样 / 展开不确定量（D190：条件必须确定）；
//! - **视角方不是敌我编号**（D29 / D34）：[`ViewSide`] 是关键字层 [`Perspective`] 的别名，
//!   由成对入口（`play_card_me` / `play_card_opp`）绑定；条件函数里不写实体号；
//! - 拒绝原因是**调试文案**（`&'static str`，中文短句），**不是**给玩家看的本地化文本：
//!   正式文案将来由本地化文本给（见「待接入」）。
//!
//! ## 谁调用（顺序契约）
//!
//! `flow/play_card.rs` 的 `play`：
//! 「手牌里有没有这张牌」-> **打出条件**（本文件的 [`ActionCardDef::check_play`]）->
//! 「打出前」派发 / 执行 -> 付费 -> 离手 -> 落位 -> 「打出后」派发。
//! 顺序里「条件在付费之前」由 D217 钉死；「条件相对『打出前』触发的位置」见「待复核」。
//!
//! ## 待接入
//!
//! 1. **卡牌文件**：gi-genius-invokation-cards 的 `src/cards/**`（D240）里每张行动牌写一份 `static ...: ActionCardDef<Tags>`，
//!    费用填卡面基础费用（卡 id 由 D215 编译期自动绑定，不在本文件写 id）；
//! 2. **费用表来源**：`PlayCardRequest::cost` 现在仍由调用方给（费用修正链未接），
//!    将来应改为由本文件的 `cost()` 派生（D184 第 3 步）；
//! 3. **本地化的拒绝文案**：现在只有中文调试串；
//! 4. **本文件的模块声明**：非卡牌模块按 D216 还要 `module_source!` 自声明并登记进
//!    `source::SOURCE_FILES`（手写清单）；本任务的可写路径不含 `source/mod.rs`，
//!    所以与其它 `def.rs`（`status/def.rs` / `summon/def.rs`）一样先不声明。
//!
//! ## 待复核
//!
//! 1. **条件相对「打出前」触发的位置**：本层把条件放在「打出前」派发**之前**（条件看到的是
//!    触发前的局面）。D217 只规定「在比较是否付得起之前」；若规则要求「打出前」效果先改局面，
//!    应改成派发之后求值（`flow/play_card.rs` 的测试钉住了当前选择）；
//! 2. **条件与「在不在手牌」的先后**：不知道是哪张牌就问不了它的条件，所以 `NotInHand`
//!    优先于「条件不通过」；
//! 3. **`ViewSide` 用别名复用 `Perspective`**：若将来条件要区分「观战视角」或要独立演化，
//!    应改成独立类型；
//! 4. **`cost` 与 `PlayCardRequest::cost` 暂时重复**：等费用修正链接入后统一；
//! 5. **装备门 / 场地门**：装备牌「能不能装到这个角色」现在是装备栏的 `EquipReject`（落位阶段），
//!    它依赖**目标**而不是只依赖局面；是否也算「打出条件」搬进本字段，待复核。

use crate::base::CardId;
use crate::families::action_card::ActionCardType;
use crate::generated::Localized;
use crate::keywords::target::Perspective;
use crate::resources::dice::CostComponent;
use crate::state::DefState;

/// **视角方**：相对哪一侧看局面（**不是**敌我编号 / 不是下标，D29 / D34）。
///
/// 就是关键字层的 [`Perspective`]：由成对入口（`play_card_me` / `play_card_opp`）绑定，
/// 条件函数里因此不出现「第几个玩家」这种写法。
pub type ViewSide = Perspective;

/// **一次打出条件的结论**（D217）。
///
/// `待复核`：拒绝原因目前是 `&'static str` 调试文案；正式对玩家展示的文案应由
/// 本地化文本（`generated::Localized`）给，等本地化文案块接入。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayCondition {
    /// 条件通过：可以继续「比较是否付得起」。
    Ok,
    /// 条件不通过：**不许打出**，带调试文案（中文短句，不是给玩家看的本地化文本）。
    Rejected(&'static str),
}

impl PlayCondition {
    /// 条件是否通过。
    pub const fn is_ok(self) -> bool {
        matches!(self, PlayCondition::Ok)
    }

    /// 拒绝原因（调试文案）；通过 -> `None`。
    pub const fn reason(self) -> Option<&'static str> {
        match self {
            PlayCondition::Ok => None,
            PlayCondition::Rejected(reason) => Some(reason),
        }
    }
}

/// **打出条件函数**（D217 / D186）：吃一局状态 + 视角方，返回结论。
///
/// 用**函数指针**（不是闭包）：签名里没有可变引用，也没有捕获环境 ——
/// 条件**改不了任何状态**、也不依赖调用顺序（纯函数由签名保证）。
pub type PlayConditionFn = fn(&DefState, ViewSide) -> PlayCondition;

/// 一张**行动牌**的定义（纯数据；D217）。
///
/// 泛型参数 `Tags` 是标签集合（形态本轮未定，见 `lib.rs`；没有标签用 `()`）。
/// 一份定义可以写成 `static` 常量：`new` 与全部访问器都是 `const fn`。
#[derive(Clone, Copy, Debug)]
pub struct ActionCardDef<Tags> {
    /// 行动牌的卡 id（行动牌牌族 id 区域内唯一）。
    id: CardId,
    /// 类型（支援 / 装备 / 事件，D51）；决定落位落点。
    card_type: ActionCardType,
    /// 4 语名字（本块只填中文，D120）。
    name: Localized,
    /// 4 语描述（本块只填中文，D120）。
    description: Localized,
    /// 标签集合（D54：标签是零大小类型；集合形态本轮未定）。
    tags: Tags,
    /// **基础费用**（静态切片；费用修正链未接，见「待接入」）。
    cost: &'static [CostComponent],
    /// **打出条件**（D217）：尝试打出时、**比较是否付得起之前**调用。
    play_condition: PlayConditionFn,
}

impl<Tags> ActionCardDef<Tags> {
    /// 建一份行动牌定义（常量 / 测试用；7 个参数，不再拆构造器）。
    pub const fn new(
        id: CardId,
        card_type: ActionCardType,
        name: Localized,
        description: Localized,
        tags: Tags,
        cost: &'static [CostComponent],
        play_condition: PlayConditionFn,
    ) -> ActionCardDef<Tags> {
        ActionCardDef {
            id,
            card_type,
            name,
            description,
            tags,
            cost,
            play_condition,
        }
    }

    /// 卡 id（只读）。
    pub const fn id(&self) -> CardId {
        self.id
    }

    /// 类型（只读；落位落点由它决定）。
    pub const fn card_type(&self) -> ActionCardType {
        self.card_type
    }

    /// 4 语名字（只读；唯一一份，不拷贝）。
    pub const fn name(&self) -> Localized {
        self.name
    }

    /// 4 语描述（只读；唯一一份，不拷贝）。
    pub const fn description(&self) -> Localized {
        self.description
    }

    /// 标签集合（只读）。
    pub const fn tags(&self) -> &Tags {
        &self.tags
    }

    /// 基础费用（只读；静态切片，不分配）。
    pub const fn cost(&self) -> &'static [CostComponent] {
        self.cost
    }

    /// 是否有费用（空切片 = 免费）。
    pub const fn has_cost(&self) -> bool {
        !self.cost.is_empty()
    }

    /// 打出条件函数（只读；调用它请用 [`ActionCardDef::check_play`]）。
    pub const fn play_condition(&self) -> PlayConditionFn {
        self.play_condition
    }

    /// **问一次打出条件**（D217）：把本定义的函数指针套用到「局面 + 视角方」上。
    ///
    /// 纯读取：只借 `&DefState`，不改任何区、不采样（D190）。
    pub fn check_play(&self, state: &DefState, viewer: ViewSide) -> PlayCondition {
        (self.play_condition)(state, viewer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;
    use crate::generated::{Lang, Resolved};
    use crate::state::testing::{characters, hand};
    use crate::state::PlayerDefs;
    use gi_genius_invokation_engine::Nat;

    /// 测试用名字（**不是真实卡牌数据**）。
    static SAMPLE_NAME: Localized = Localized::from_parts(Some("测试行动牌"), None, None, None);

    /// 测试用描述（**不是真实卡牌数据**）。
    static SAMPLE_DESCRIPTION: Localized =
        Localized::from_parts(Some("测试用描述（仅中文）。"), None, None, None);

    /// 测试用基础费用：2 个火骰 + 1 个任意骰。
    static SAMPLE_COST: [CostComponent; 2] = [
        CostComponent::SpecificElement(Element::Pyro, Nat::new(2)),
        CostComponent::AnyColor(Nat::new(1)),
    ];

    /// 条件样本：只有**视角方**场上至少有 2 个角色才通过（看视角方绑定）。
    fn needs_two_characters(state: &DefState, viewer: ViewSide) -> PlayCondition {
        let side = match viewer {
            ViewSide::Me => state.me(),
            ViewSide::Opp => state.opp(),
        };
        let count = side.characters().len();
        if count < Nat::new(2) {
            PlayCondition::Rejected("测试：视角方角色不足 2 个")
        } else {
            PlayCondition::Ok
        }
    }

    /// 条件样本：总是拒绝。
    fn always_rejected(_state: &DefState, _viewer: ViewSide) -> PlayCondition {
        PlayCondition::Rejected("测试：这次不允许打出")
    }

    /// 一份完整的测试用行动牌定义（**不是真实卡牌数据**）。
    static SAMPLE_DEF: ActionCardDef<()> = ActionCardDef::new(
        CardId::new(900_101),
        ActionCardType::Event,
        SAMPLE_NAME,
        SAMPLE_DESCRIPTION,
        (),
        &SAMPLE_COST,
        needs_two_characters,
    );

    /// 我方 2 角色 / 敌方 1 角色的局面。
    fn state() -> DefState {
        DefState::from_players(
            PlayerDefs::new()
                .with_characters(characters(&[201, 202]))
                .with_hand(hand(&[211])),
            PlayerDefs::new().with_characters(characters(&[301])),
        )
    }

    #[test]
    fn action_card_def_carries_id_type_text_tags_cost_and_condition() {
        assert_eq!(SAMPLE_DEF.id(), CardId::new(900_101));
        assert_eq!(SAMPLE_DEF.card_type(), ActionCardType::Event);
        assert_eq!(SAMPLE_DEF.tags(), &());
        assert!(SAMPLE_DEF.has_cost());
        assert_eq!(SAMPLE_DEF.cost().len(), 2);
        assert_eq!(
            SAMPLE_DEF.cost()[0],
            CostComponent::SpecificElement(Element::Pyro, Nat::new(2))
        );
        assert_eq!(SAMPLE_DEF.cost()[1], CostComponent::AnyColor(Nat::new(1)));
        // 名字 / 描述就在定义里（唯一一份，不另存）
        assert_eq!(
            SAMPLE_DEF.name().get(Lang::ZhCn),
            Resolved::Exact("测试行动牌")
        );
        assert_eq!(
            SAMPLE_DEF.description().get(Lang::ZhCn),
            Resolved::Exact("测试用描述（仅中文）。")
        );
        // D120：只填中文；其它语言槽位是 None（D56 / D68 显式缺失，不静默顶替）
        assert!(!SAMPLE_DEF.name().has(Lang::EnUs));
        assert!(!SAMPLE_DEF.description().has(Lang::JaJp));
        // 条件函数存的是函数指针（不是闭包）：同一张牌的两次取用是同一个地址。
        assert!(std::ptr::eq(
            SAMPLE_DEF.play_condition() as *const (),
            needs_two_characters as *const ()
        ));
    }

    #[test]
    fn cost_is_a_borrowed_static_slice_not_a_copy() {
        // 静态切片：拿到的是卡牌文件里那份数组本身（同一块内存），没有克隆 / 分配。
        assert!(std::ptr::eq(
            SAMPLE_DEF.cost().as_ptr(),
            SAMPLE_COST.as_ptr()
        ));
        assert_eq!(SAMPLE_DEF.cost().len(), SAMPLE_COST.len());
    }

    #[test]
    fn play_condition_ok_and_rejected_are_distinguishable() {
        let ok = PlayCondition::Ok;
        assert!(ok.is_ok());
        assert_eq!(ok.reason(), None);

        let rejected = PlayCondition::Rejected("测试：不允许");
        assert!(!rejected.is_ok());
        assert_eq!(rejected.reason(), Some("测试：不允许"));
        assert_ne!(ok, rejected);
    }

    #[test]
    fn check_play_binds_the_viewer_side() {
        // 我方 2 角色、敌方 1 角色；同一个定义换个视角方，结论相反。
        let state = state();
        assert_eq!(
            SAMPLE_DEF.check_play(&state, ViewSide::Me),
            PlayCondition::Ok
        );
        assert_eq!(
            SAMPLE_DEF.check_play(&state, ViewSide::Opp),
            PlayCondition::Rejected("测试：视角方角色不足 2 个")
        );
    }

    #[test]
    fn check_play_is_a_pure_read_only_function() {
        // 条件函数的签名只有 &DefState / ViewSide -> 改不了局面（纯函数由签名保证）；
        // 这里再把「两次调用结论相同、局面不变」钉住。
        let before = state();
        let snapshot = before.clone();
        let first = SAMPLE_DEF.check_play(&before, ViewSide::Me);
        let second = SAMPLE_DEF.check_play(&before, ViewSide::Me);
        assert_eq!(first, second);
        assert_eq!(before, snapshot);

        // 拒绝路径同样只读
        let third = SAMPLE_DEF.check_play(&before, ViewSide::Opp);
        let fourth = SAMPLE_DEF.check_play(&before, ViewSide::Opp);
        assert_eq!(third, fourth);
        assert_eq!(before, snapshot);
    }

    #[test]
    fn definition_is_copy_and_can_be_a_static() {
        // 定义是纯数据 + Copy：可以按值传、可以写进 static（卡牌文件就是这么用）。
        let copied = SAMPLE_DEF;
        assert_eq!(copied.id(), SAMPLE_DEF.id());
        assert!(std::ptr::eq(
            copied.cost().as_ptr(),
            SAMPLE_DEF.cost().as_ptr()
        ));

        // 换一份条件 / 类型 / 费用 = 另一张牌（同一份文案，仅测试）
        let other: ActionCardDef<()> = ActionCardDef::new(
            CardId::new(900_102),
            ActionCardType::Support,
            SAMPLE_NAME,
            SAMPLE_DESCRIPTION,
            (),
            &[],
            always_rejected,
        );
        assert_ne!(other.id(), SAMPLE_DEF.id());
        assert_ne!(other.card_type(), SAMPLE_DEF.card_type());
        assert!(!other.has_cost());
        assert_eq!(
            other.check_play(&state(), ViewSide::Me),
            PlayCondition::Rejected("测试：这次不允许打出")
        );
    }
}
