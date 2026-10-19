//! 支援区（types=支援；tags=场地 / 伙伴 / 道具 / 元素幻变 / 冒险地点 / 未变身）。
//!
//! **权威依据（规则书·游戏区域 III 支援区）**：
//! 「『支援牌』打出后，会放置在支援区。支援区最多放置 4 张『支援牌』。
//! 占满后打出新的支援牌时，需要先选择一张支援区内的支援牌弃置。
//! （同时结算多个支援牌效果时，会按照支援区的排列顺序依次结算。）」
//! English: "The Support Zone can hold up to 4 Support Cards. If you want to play a new Support Card
//! when this zone is full, you must discard a card in it first."
//!
//! 卡面：322014 鲸井小弟「如果对方的支援区未满，则将此牌转移到对方的支援区」；
//! 302213 芙佳的声援「随机生成『伙伴』到场上，直到填满双方支援区」。
//! Lead 裁定 6（待用户复核）：场地 / 伙伴 / 道具仅分类 + 支援区上限 4；场地不额外限制唯一。
//!
use gi_genius_invokation_engine::Nat;

/// 一方支援区上限（分工计划 §4.4.15 已定）。
pub const SUPPORT_CAP: Nat = Nat::new(4);

/// 打出一张支援牌、支援区已有 `occupied` 张时的结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupportEnter {
    /// 直接进场。
    Accept,
    /// 已满：必须先由玩家选择弃置一张（走选择原语）。
    MustChooseDiscard,
}

/// 进场判定（纯函数）。
pub fn support_enter(occupied: Nat) -> SupportEnter {
    if occupied < SUPPORT_CAP {
        SupportEnter::Accept
    } else {
        SupportEnter::MustChooseDiscard
    }
}

/// 支援区离场 = **弃置**（移除，无弃牌堆；D 术语）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SupportExit {
    /// 移除即完成。
    Discard,
}

/// 离场操作形状（具体移除由支援区落点实现）。
pub fn support_exit() -> SupportExit {
    SupportExit::Discard
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_zone_cap_is_four() {
        assert_eq!(SUPPORT_CAP, Nat::new(4));
        assert_eq!(support_enter(Nat::ZERO), SupportEnter::Accept);
        assert_eq!(support_enter(Nat::new(1)), SupportEnter::Accept);
        assert_eq!(support_enter(Nat::new(3)), SupportEnter::Accept);
        // 第 5 张：必须先选择弃置一张
        assert_eq!(support_enter(Nat::new(4)), SupportEnter::MustChooseDiscard);
        assert_eq!(support_enter(Nat::new(5)), SupportEnter::MustChooseDiscard);
    }

    #[test]
    fn leaving_support_zone_is_discard() {
        assert_eq!(support_exit(), SupportExit::Discard);
    }
}