//! 秘传与秘传揭令（规则书·特殊词条「秘传」+ 权威关键字 #70 秘传揭令）。
//!
//! **权威原文（中文）**：
//! 1. 在一场对局中，每位牌手只能使用一张「秘传」卡牌。
//! 2. 「秘传」卡牌一定在你的起始手牌中。
//!    （当牌组中的「秘传」卡牌数量大于起始手牌数时，秘传卡牌将随机出现在起始手牌中。）
//! 3. 权威关键字 #70：只有在「秘传揭令」仍存在时才能打出秘传；打出后消耗使用机会。
//!
//! **English（消除歧义）**：Arcane Legend —— "The Arcane Legend card will be in your starting hand
//! (If you have more Arcane Legend cards in your deck than there are starting hand cards,
//! random Arcane Legend cards will appear in your starting hand.)"
//!
//! D17 / D90：起手先按「秘传」标签抓，再补足到 5 张（已实现 HandState::draw_opening）。

use gi_genius_invokation_engine::Nat;

/// 整局每位牌手最多打出 1 张秘传（规则书第 1 条 + 关键字 #70）。
pub const ARCANE_LEGEND_PER_GAME: Nat = Nat::new(1);

/// 秘传一定在起始手牌中（规则书第 2 条；数量超过起始手牌数时只随机出现一部分）。
pub const ARCANE_LEGEND_IN_OPENING_HAND: bool = true;

/// 是否「所有秘传都在起始手牌中」：牌组里的秘传数不超过起始手牌数时成立（规则书第 2 条）。
pub fn all_arcane_guaranteed(arcane_in_deck: Nat, opening_hand: Nat) -> bool {
    arcane_in_deck <= opening_hand
}

/// 起始手牌中出现的秘传张数：等于秘传总数；超过起始手牌数时取起始手牌数（随机抽）。
pub fn arcane_in_opening_hand(arcane_in_deck: Nat, opening_hand: Nat) -> Nat {
    if arcane_in_deck < opening_hand {
        arcane_in_deck
    } else {
        opening_hand
    }
}

/// 秘传：起手一定在手上（用于把「秘传」标签转成起手抓牌谓词）。
pub fn guaranteed_in_opening_hand() -> bool {
    ARCANE_LEGEND_IN_OPENING_HAND
}

/// 打出判定结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArcanePlay {
    /// 可以打出。
    Allowed,
    /// 本局已经打出过秘传（每人 1 张）。
    AlreadyPlayed,
    /// 秘传揭令已消耗 / 不存在。
    NoEdict,
}

/// 打出判定：played_this_game = 本局该牌手已打出的秘传张数，
/// edict_available = 秘传揭令是否仍存在。
pub fn can_play(played_this_game: Nat, edict_available: bool) -> ArcanePlay {
    if played_this_game >= ARCANE_LEGEND_PER_GAME {
        ArcanePlay::AlreadyPlayed
    } else if !edict_available {
        ArcanePlay::NoEdict
    } else {
        ArcanePlay::Allowed
    }
}

/// 打出后是否消耗秘传揭令（关键字 #70：会消耗）。
pub fn consumes_edict() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_arcane_legend_per_game() {
        assert_eq!(ARCANE_LEGEND_PER_GAME, Nat::new(1));
        assert_eq!(can_play(Nat::ZERO, true), ArcanePlay::Allowed);
        assert_eq!(can_play(Nat::new(1), true), ArcanePlay::AlreadyPlayed);
    }

    #[test]
    fn requires_live_edict() {
        assert_eq!(can_play(Nat::ZERO, false), ArcanePlay::NoEdict);
        assert_eq!(can_play(Nat::new(1), false), ArcanePlay::AlreadyPlayed);
        assert!(consumes_edict());
    }

    #[test]
    fn arcane_is_guaranteed_only_when_it_fits() {
        assert!(guaranteed_in_opening_hand());
        assert_eq!(super::super::opening::OPENING_HAND_SIZE, Nat::new(5));
        // 不超过起始手牌数：全部保证
        assert!(all_arcane_guaranteed(Nat::new(3), Nat::new(5)));
        assert_eq!(arcane_in_opening_hand(Nat::new(3), Nat::new(5)), Nat::new(3));
        // 超过起始手牌数：只随机出现 5 张
        assert!(!all_arcane_guaranteed(Nat::new(6), Nat::new(5)));
        assert_eq!(arcane_in_opening_hand(Nat::new(6), Nat::new(5)), Nat::new(5));
    }
}
