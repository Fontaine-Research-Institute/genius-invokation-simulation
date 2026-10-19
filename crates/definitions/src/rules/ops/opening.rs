//! 开局流程（统一钩子）：元素幻变的「开局那一套」+ 游戏准备流程。
//!
//! **权威依据：规则书 + 权威关键字 + 卡面（不读参考实现，D122）**：
//! - 规则书·《七圣召唤》游戏准备：
//!   「(1) 双方牌手各抓 5 张起始手牌。然后，双方各有一次机会替换任意数量的手牌。
//!   (2) 双方各自选择一张自己的『角色牌』作为『出战角色』。」
//!   English: "Both sides will draw 5 cards ... 1 chance to switch out any number of cards in their Hand."
//! - 规则书·游戏区域 VI（手牌）：「游戏开始后，从『牌堆』中抓 5 张牌，作为初始的『手牌』。」
//! - 规则书·回合流程 投掷阶段：「每回合的投掷阶段中，会投掷 8 个『元素骰』；
//!   投掷完成后，还有 1 次机会选择任意数量元素骰进行『重投』。」
//! - 权威关键字 #23「元素幻变」：「行动阶段开始时，若队伍中包含元素幻变的两种元素类型的角色
//!   且不包含其他元素的角色，则将此牌抓起，然后弃置并生成至场上，然后将 4 个骰子转化为
//!   幻变的元素类型。」
//! - 元素幻变 9 张（331004-331012）卡面三行：绑定两种元素 / 「投掷阶段：总是投出 2 个 X 和 2 个 Y」/
//!   「触发对应反应后：弃置此牌并从两张祝佑中挑 1 加入手牌」。
//! - 秘传（如 330001）：规则书·特殊词条 秘传「秘传卡牌一定在你的起始手牌中」→ D17 / D90，
//!   已有实现 HandState::draw_opening（先按秘传标签抓，再补足到 5 张）。
//!
//! **Lead 裁定（2026-10，待用户复核；规则书未直接规定这些时序细节）**：
//! 1. 「投掷阶段 2+2」每轮都生效（前提：牌已在场）；
//! 2. 「行动阶段开始时」的抓牌 -> 弃置 -> 生成上场 -> 4 骰转 2+2 **只在首次（第 1 轮）**；
//!    第 1 轮投掷阶段时牌还在牌库（未上场），所以第 1 轮不产 2+2，从第 2 轮起每轮投掷阶段 2+2（待实测）；
//! 3. 转化 = 固定 2+2（待实测）；
//! 4. 只处理**从牌库**；牌已在手牌时不自动上场；
//! 5. 条件 = 队伍元素集合**恰等于** {X, Y}（各至少 1 名，且没有第三种元素）。
use crate::base::Element;
use crate::resources::dice::{DicePool, DieFace};
use gi_genius_invokation_engine::Nat;

/// 起手张数（规则书·游戏区域 VI：从牌堆抓 5 张；与 HandState::OPENING_SIZE 同口径；D17 / D90）。
pub const OPENING_HAND_SIZE: Nat = Nat::new(5);

/// 游戏准备阶段的换牌机会（规则书·游戏准备：双方各有 1 次机会替换任意数量的手牌）。
pub const MULLIGAN_CHANCES: Nat = Nat::new(1);

/// 游戏准备步骤（规则书·《七圣召唤》游戏准备；顺序即执行顺序）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PregameStep {
    /// 双方各抓 5 张起始手牌（秘传优先，D17 / D90）。
    DrawOpeningHand,
    /// 双方各有 1 次机会替换任意数量的手牌（HandState 尚无换牌原语，// 待接入）。
    Mulligan,
    /// 双方各自选择一张角色牌作为出战角色（// 待接入 角色块）。
    ChooseFirstCharacter,
}

/// 游戏准备的执行顺序。
pub fn pregame_steps() -> [PregameStep; 3] {
    [
        PregameStep::DrawOpeningHand,
        PregameStep::Mulligan,
        PregameStep::ChooseFirstCharacter,
    ]
}

/// 元素幻变：一张卡绑定的两种元素。
///
/// 具体是哪两种元素由**具体卡**给出（如 331004 = [冰, 雷]）；规则层只做通用判定，
/// 不把 9 张卡硬编码进来。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ElementalTransmutation {
    /// 绑定的两种元素（顺序不影响规则）。
    pub elements: [Element; 2],
}

impl ElementalTransmutation {
    /// 用两种元素构造（不从 `new` 起名：本类型没有「默认值」，不实现 Default）。
    pub const fn from_elements(elements: [Element; 2]) -> ElementalTransmutation {
        ElementalTransmutation { elements }
    }

    /// 条件（关键字 #23）：队伍元素集合**恰等于** {X, Y}。
    ///
    /// 「恰等于」= 两种元素各至少 1 名、且没有第三种元素的角色。
    pub fn team_matches(&self, team: &[Element]) -> bool {
        let mut has_first = false;
        let mut has_second = false;
        for element in team {
            if *element == self.elements[0] {
                has_first = true;
            } else if *element == self.elements[1] {
                has_second = true;
            } else {
                return false;
            }
        }
        has_first && has_second
    }

    /// 投掷阶段（每轮，牌已在场）：总是投出 2 个 X + 2 个 Y。
    pub fn roll_phase_dice(&self) -> [(Element, Nat); 2] {
        [
            (self.elements[0], Nat::new(2)),
            (self.elements[1], Nat::new(2)),
        ]
    }

    /// 开局步骤（首次、第 1 轮行动阶段开始时；顺序即结算顺序）。
    pub fn opening_steps(&self) -> [OpeningStep; 5] {
        [
            OpeningStep::CheckTeamElements,
            OpeningStep::DrawFromPile,
            OpeningStep::DiscardActionCardForm,
            OpeningStep::GenerateToSupportZone,
            OpeningStep::ConvertFourDice,
        ]
    }

    /// 开局瞬间的骰子转化：把 4 颗骰子转化为该两元素各 2 颗（总数不变）。
    ///
    /// 实现复用 `resources::dice::DicePool::convert`（确定性扣减，D144；选哪颗骰的分支由上层做）：
    /// 先把 4 颗**任意来源**的骰子转成第一种元素，再把其中 2 颗转成第二种元素。
    /// **为什么分两步**：`convert` 一次只能给一个目标；两步都写「任意来源」会把刚转出来的
    /// 骰子当成第二轮的来源（`take_any` 先扣具体色），净效果就不是 2+2 —— 第二步显式指定来源。
    /// 骰子不足 4 颗 -> `None`（不改状态）。
    /// 待实测：转化固定 2+2（Lead 裁定 3）、只处理牌库中的牌（裁定 4）。
    pub fn convert_opening_dice(&self, pool: &DicePool) -> Option<DicePool> {
        let first = DieFace::from_element(self.elements[0]);
        let second = DieFace::from_element(self.elements[1]);
        let after_first = pool.convert(None, Nat::new(4), first)?;
        after_first.convert(Some(first), Nat::new(2), second)
    }

    /// 投掷阶段（每轮）的固定骰：与开局转化同一套运算（「总是投出 2+2」⇔「4 颗骰转成 2+2」）。
    pub fn apply_roll_phase(&self, pool: &DicePool) -> Option<DicePool> {
        self.convert_opening_dice(pool)
    }

    /// 触发（第三行）：弃置此牌，并从 2 张指定「祝佑」中挑 1 张加入手牌。
    ///
    /// 候选具体是哪两张由**具体卡**给出（规则层不硬编码具体卡），这里只给操作形状。
    pub fn on_reaction_triggered(&self) -> TransmutationTrigger {
        TransmutationTrigger {
            discard_self: true,
            candidate_count: Nat::new(2),
            to_hand: Nat::new(1),
        }
    }
}

/// 开局步骤（顺序 = 结算顺序）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpeningStep {
    /// 条件判定：队伍元素集合恰等于 {X, Y}（关键字 #23）。
    CheckTeamElements,
    /// 从牌库抓起这张牌（只处理牌库中的；Lead 裁定 4）。
    DrawFromPile,
    /// 弃置「行动牌形态」（同一张卡换区，不新建卡；分工计划 §4.4.13）。
    DiscardActionCardForm,
    /// 以**同一 id** 生成到支援区（支援牌就是行动牌本身）。
    GenerateToSupportZone,
    /// 把 4 颗骰子转化为两种元素各 2 颗（Lead 裁定 3）。
    ConvertFourDice,
    /// 投掷阶段固定 2+2（**每轮**，牌在场时；不属于「首次」步骤，单独列）。
    RollPhaseFixedDice,
}

/// 触发结果（弃置自身 + 从 2 张候选中挑 1 加入手牌）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TransmutationTrigger {
    /// 是否弃置此牌（卡面：「弃置此牌」）。
    pub discard_self: bool,
    /// 候选张数（卡面：「从 A 和 B 中挑选一项」= 2）。
    pub candidate_count: Nat,
    /// 加入手牌张数（卡面：「加入手牌」= 1）。
    pub to_hand: Nat,
}

/// 是不是第 1 轮（「首次生效」的判据；Lead 裁定 2）。
///
/// 轮次从 1 起算；第 1 轮投掷阶段牌还在牌库，所以开局那套只在第 1 轮的行动阶段开始时执行。
pub fn is_opening_round(round: Nat) -> bool {
    round == Nat::new(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Element;

    fn superconduct() -> ElementalTransmutation {
        // 331004 元素幻变：超导祝佑 = 冰 + 雷
        ElementalTransmutation::from_elements([Element::Cryo, Element::Electro])
    }

    #[test]
    fn team_must_be_exactly_the_two_elements() {
        let rule = superconduct();
        // 两种都有、没有第三种 -> 命中
        assert!(rule.team_matches(&[Element::Cryo, Element::Electro, Element::Cryo]));
        // 顺序不影响
        assert!(rule.team_matches(&[Element::Electro, Element::Cryo]));
        // 少一种 -> 不命中
        assert!(!rule.team_matches(&[Element::Cryo]));
        assert!(!rule.team_matches(&[Element::Electro, Element::Electro]));
        // 多第三种元素 -> 不命中（关键字 #23「且不包含其他元素的角色」）
        assert!(!rule.team_matches(&[
            Element::Cryo,
            Element::Electro,
            Element::Pyro
        ]));
        // 空队伍 -> 不命中
        assert!(!rule.team_matches(&[]));
    }

    #[test]
    fn roll_phase_is_fixed_two_plus_two() {
        let rule = superconduct();
        assert_eq!(
            rule.roll_phase_dice(),
            [(Element::Cryo, Nat::new(2)), (Element::Electro, Nat::new(2))]
        );
    }

    #[test]
    fn opening_steps_are_ordered() {
        let steps = superconduct().opening_steps();
        assert_eq!(
            steps,
            [
                OpeningStep::CheckTeamElements,
                OpeningStep::DrawFromPile,
                OpeningStep::DiscardActionCardForm,
                OpeningStep::GenerateToSupportZone,
                OpeningStep::ConvertFourDice,
            ]
        );
        // 「抓牌→弃置→生成」顺序固定：先抓到手，再弃置行动牌形态，再以同一 id 生成到支援区。
        let draw = steps
            .iter()
            .position(|step| *step == OpeningStep::DrawFromPile)
            .expect("有抓牌步骤");
        let discard = steps
            .iter()
            .position(|step| *step == OpeningStep::DiscardActionCardForm)
            .expect("有弃置步骤");
        let generate = steps
            .iter()
            .position(|step| *step == OpeningStep::GenerateToSupportZone)
            .expect("有生成步骤");
        assert!(draw < discard && discard < generate);
    }

    #[test]
    fn opening_only_on_first_round() {
        assert!(is_opening_round(Nat::new(1)));
        assert!(!is_opening_round(Nat::new(2)));
        assert!(!is_opening_round(Nat::ZERO));
    }

    #[test]
    fn opening_dice_conversion_is_two_plus_two() {
        use crate::resources::dice::CostComponent;
        let rule = superconduct();
        // 8 颗万能骰 -> 转 4 颗成 冰 2 + 雷 2（转化不改变骰子总数）。
        let pool = DicePool::new(Nat::new(8), Vec::new(), Vec::new());
        let converted = rule
            .convert_opening_dice(&pool)
            .expect("8 颗骰足够转 4 颗");
        assert_eq!(converted.total(), pool.total());
        // 转化后既能付 2 冰、也能付 2 雷：说明两种元素各转出了 2 颗。
        assert!(converted.can_afford(&[CostComponent::SpecificElement(Element::Cryo, Nat::new(2))]));
        assert!(converted.can_afford(&[CostComponent::SpecificElement(Element::Electro, Nat::new(2))]));
    }

    #[test]
    fn opening_dice_conversion_takes_any_source() {
        let rule = superconduct();
        // 4 颗已知火骰也能被转成 冰 2 + 雷 2（「转化为幻变的元素类型」不要求来源是万能骰）。
        let pool = DicePool::new(Nat::ZERO, Vec::new(), vec![(Element::Pyro, Nat::new(4))]);
        let converted = rule.convert_opening_dice(&pool).expect("4 颗骰足够");
        assert_eq!(converted.total(), pool.total());
    }

    #[test]
    fn opening_dice_conversion_needs_four_dice() {
        let rule = superconduct();
        let pool = DicePool::new(Nat::new(2), Vec::new(), Vec::new());
        assert_eq!(rule.convert_opening_dice(&pool), None);
    }

    #[test]
    fn trigger_discards_and_offers_two_candidates() {
        let trigger = superconduct().on_reaction_triggered();
        assert!(trigger.discard_self);
        assert_eq!(trigger.candidate_count, Nat::new(2));
        assert_eq!(trigger.to_hand, Nat::new(1));
    }

    #[test]
    fn pregame_is_draw_then_mulligan_then_first_character() {
        assert_eq!(OPENING_HAND_SIZE, Nat::new(5));
        assert_eq!(MULLIGAN_CHANCES, Nat::new(1));
        assert_eq!(
            pregame_steps(),
            [
                PregameStep::DrawOpeningHand,
                PregameStep::Mulligan,
                PregameStep::ChooseFirstCharacter,
            ]
        );
    }

    #[test]
    fn opening_hand_size_is_five() {
        assert_eq!(OPENING_HAND_SIZE, Nat::new(5));
    }
}