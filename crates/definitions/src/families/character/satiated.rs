//! **饱腹**（覆盖报告 L1 / 规则书·特殊词条 料理）—— 状态身份 + 赋予 / 解除的纯操作。
//!
//! ## 依据（权威顺序：规则书 > 卡面 / 关键字数据 > 参考实现）
//!
//! 规则书·特殊词条 **料理**（S8.1，原文）：
//! 1. 当「料理」事件牌对角色生效后，角色会进入「饱腹」状态。
//! 2. 处于「饱腹」状态下的角色，无法成为其他「料理」事件牌的目标，
//!    也无法受到其他「料理」事件牌的效果影响。
//! 3. 「饱腹」状态会在回合结束或角色被击倒时解除。
//!
//! 数据（权威第 2 档，tcgstatuseffects id **303300**「饱腹」）：
//! 描述「本回合无法食用更多「料理」」—— 名字 / 描述逐字取数据。
//!
//! D17：料理**只负责赋予饱腹**，限制由饱腹自身决定 —— 本模块给饱腹的身份 + 状态操作；
//! 「能不能被料理选为目标 / 受料理效果」的纯判定仍在 rules::ops::food（吃 satiated: bool）。
//!
//! ## 口径（D95 / D96 / D104 / D105）
//!
//! - 身份 = **指向静态文本数据的指针**（StateIdentity::identity_address，D57 / D58）；
//! - 状态挂在角色的 CharacterState::states 上，位置 = CHARACTER_STATE_POS
//!   （角色状态口径，与反应执行器 / 关键字层同口径，D105：所有状态共用一张列表）；
//! - **布尔形态**（D104：在列表里即代表「有」，不存 bool、不记层数），重复赋予幂等；
//! - **纯函数**：吃 &CharacterState、返回新状态，不改入参；失败不 panic。
//!
//! ## 待接入 / 待复核
//!
//! - **正式 StatusDef 归 families::status**（状态牌类型 / 持续回合 = 无 / 标签 / 文案表）：
//!   本轮写范围只到角色牌族，本文件先给**共享身份**（与 damage_chain::statuses::DAMAGE_NEGATION
//!   同口径）；状态块落地后把 satiated_in 用的身份换成那边正式实例即可（接口不变）；
//! - 「回合结束 / 被击倒」两个解除时机的**调度接线**在 state/flows/{end_phase,defeat}.rs
//!   （本轮写范围不含 state/**）：本文件给按时机命名的纯操作 clear_at_round_end /
//!   clear_on_defeat，接线方直接调用；
//! - 规则书说「回合结束」解除，数据说「本回合无法食用更多料理」：解除发生在结束阶段哪一步
//!   （持续回合递减前 / 后）规则书未写 -> 待实测；
//! - 料理「生效后」进入饱腹：本模块只给 CharacterState::with_satiated；
//!   哪张料理牌生效 / 何时调用归 cards/ 与 flow/。

use crate::base::CardId;
use crate::families::action_card::status::{StateForm, StateIdentity, StateList, StateText};
use crate::families::character::character::CharacterState;
use crate::families::character::damage_chain::executor::CHARACTER_STATE_POS;
use crate::generated::Localized;

/// 状态数据里的卡 id（tcgstatuseffects **303300**「饱腹」）。
///
/// 用于「按卡 id 找身份」的桥（damage_chain::statuses::status_of_card）；
/// 正式状态定义归状态块后这个 id 仍从那边取（本常量只作本族的引用）。
pub const SATIATED_CARD: CardId = CardId::new(303300);

/// 饱腹的静态文本（名字 / 描述逐字取自数据 303300；其余语言槽位显式缺失，D56 / D68）。
static SATIATED_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("饱腹"), None, None, None),
    description: Localized::from_parts(Some("本回合无法食用更多「料理」"), None, None, None),
};

/// 「饱腹」状态身份（D95 / D96：静态实例指针即身份）。
#[derive(Debug)]
pub struct Satiated(&'static StateText);

impl Satiated {
    /// 把自身当**身份指针**用（&'static dyn StateIdentity）。
    pub fn identity(&'static self) -> &'static dyn StateIdentity {
        self
    }
}

impl StateIdentity for Satiated {
    fn text(&self) -> &'static StateText {
        self.0
    }

    fn form(&self) -> StateForm {
        // 规则书只有「有 / 没有」，没有层数 -> 布尔形态（D104）。
        StateForm::Boolean
    }
}

/// 「饱腹」身份的唯一静态实例。
pub static SATIATED: Satiated = Satiated(&SATIATED_TEXT);

/// 角色状态列表里有没有「饱腹」（位置 = 角色状态口径）。
///
/// 只读；不关心层数（布尔形态）。
pub fn satiated_in(states: &StateList) -> bool {
    states.contains(CHARACTER_STATE_POS, &SATIATED)
}

impl CharacterState {
    /// 这个角色是不是**饱腹**（= 不能成为其他料理的目标 / 不受其效果）。
    pub fn satiated(&self) -> bool {
        satiated_in(&self.states)
    }

    /// **料理生效后**：让角色进入饱腹（规则书 S8.1 第 1 条）。
    ///
    /// - 布尔形态：重复调用幂等（不会叠层）；
    /// - 纯函数：返回新状态，不改入参。
    pub fn with_satiated(&self) -> CharacterState {
        let mut next = self.clone();
        next.states.add(CHARACTER_STATE_POS, &SATIATED, None);
        next
    }

    /// 解除饱腹（回合结束 / 被击倒都走这个纯操作；不存在也算成功）。
    ///
    /// 纯函数：返回新状态，不改入参。
    pub fn clear_satiated(&self) -> CharacterState {
        let mut next = self.clone();
        next.states.remove(CHARACTER_STATE_POS, &SATIATED);
        next
    }

    /// 这个角色**能不能成为料理事件牌的目标**（规则书 S8.1 第 2 条）。
    ///
    /// = !satiated；直接喂 rules::ops::food::can_be_targeted_by_food。
    pub fn food_targetable(&self) -> bool {
        !self.satiated()
    }
}

/// 这个角色**能不能被料理事件牌选为目标**（规则书 S8.1 第 2 条）。
///
/// 组合：本族状态判定 -> rules::ops::food::can_be_targeted_by_food（唯一一份限制口径）。
pub fn can_choose_as_food_target(state: &CharacterState) -> bool {
    crate::rules::ops::food::can_be_targeted_by_food(state.satiated())
}

/// 料理事件牌的效果**能不能作用于**这个角色（规则书 S8.1 第 2 条）。
///
/// 组合：本族状态判定 -> rules::ops::food::can_be_affected_by_food（唯一一份限制口径）。
pub fn can_apply_food_effect(state: &CharacterState) -> bool {
    crate::rules::ops::food::can_be_affected_by_food(state.satiated())
}

/// **回合结束**时的饱腹解除（规则书 S8.1 第 3 条）。
///
/// 纯操作；调度接线在 state/flows/end_phase.rs（不在本族写范围）。
pub fn clear_at_round_end(state: &CharacterState) -> CharacterState {
    state.clear_satiated()
}

/// **角色被击倒**时的饱腹解除（规则书 S8.1 第 3 条）。
///
/// 纯操作；调度接线在 state/flows/defeat.rs（不在本族写范围）。
pub fn clear_on_defeat(state: &CharacterState) -> CharacterState {
    state.clear_satiated()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated::{Lang, Resolved};
    use gi_genius_invokation_engine::Nat;

    fn character() -> CharacterState {
        CharacterState::new(Nat::new(10), Nat::new(10), Nat::ZERO, Nat::new(2))
    }

    #[test]
    fn satiated_identity_comes_from_the_status_data() {
        // 数据 303300：名字「饱腹」、描述「本回合无法食用更多「料理」」。
        assert_eq!(SATIATED.name(), "饱腹");
        assert_eq!(SATIATED.form(), StateForm::Boolean);
        assert_eq!(
            SATIATED.text().name.get(Lang::ZhCn),
            Resolved::Exact("饱腹")
        );
        assert_eq!(
            SATIATED.text().description.get(Lang::ZhCn).text(),
            Some("本回合无法食用更多「料理」")
        );
        // 身份指针稳定（D95 / D96：同一份静态实例 = 同一身份）。
        let left: &'static dyn StateIdentity = SATIATED.identity();
        let right: &'static dyn StateIdentity = &SATIATED;
        assert!(crate::families::action_card::status::same_state_identity(
            left, right
        ));
        // 数据 id 就是桥表里登记的那个。
        assert_eq!(SATIATED_CARD, CardId::new(303300));
    }

    #[test]
    fn food_effect_grants_satiated_once_and_blocks_food_targeting() {
        // 规则书 S8.1(1)(2)：料理生效后进入饱腹；饱腹角色不能成为其他料理的目标。
        let state = character();
        assert!(!state.satiated());
        assert!(state.food_targetable());
        let after = state.with_satiated();
        assert!(after.satiated());
        assert!(!after.food_targetable());
        // 喂给 rules::ops::food 的唯一一份限制口径：饱腹 -> 不能选为目标 / 不受效果。
        assert!(can_choose_as_food_target(&state));
        assert!(can_apply_food_effect(&state));
        assert!(!can_choose_as_food_target(&after));
        assert!(!can_apply_food_effect(&after));
        // 布尔形态：重复赋予幂等（不叠层、不 panic）。
        let twice = after.with_satiated();
        assert!(twice.satiated());
        assert_eq!(twice.states.entries().len(), 1);
        // 纯函数：入参没变。
        assert!(!state.satiated());
        assert!(!satiated_in(&state.states));
        // 只动状态列表，不动生命 / 充能。
        assert_eq!(after.hp(), state.hp());
        assert_eq!(after.energy(), state.energy());
    }

    #[test]
    fn satiated_is_cleared_at_round_end_and_on_defeat() {
        // 规则书 S8.1(3)：回合结束或角色被击倒时解除。
        let satiated = character().with_satiated();
        assert!(satiated.satiated());
        let at_end = clear_at_round_end(&satiated);
        assert!(!at_end.satiated());
        assert!(at_end.food_targetable());
        let on_defeat = clear_on_defeat(&satiated);
        assert!(!on_defeat.satiated());
        // 解除不存在的情况也算成功（幂等、不 panic）。
        assert!(!character().clear_satiated().satiated());
        // 纯函数：原值不变。
        assert!(satiated.satiated());
    }
}
