//! **游戏版本同步**（D244 / D245 / D246）：卡牌 / 模块声明自己已同步（核对）到的最新游戏版本。
//!
//! 口径（用户原话）：
//! 「不写最新版本号，只写同步了的最新版本」
//! 「新版本没调整是直接改这个宏定义的数值改成最新的，没改内容的同步也算」。
//!
//! - 本模块没有「全仓最新版本」常量（D245 删除）：不维护第二份版本号；
//! - 定义处只写一个值 card_game_version!(6, 2); -> pub const GAME_VERSION: GameVersion，
//!   含义 = 本卡已同步（核对）到的最新游戏版本 = 6.2；
//! - 「已同步到最新版本」的唯一表示方式 = 把宏里的版本号改成最新版本号（D246）；
//! - 不要求卡牌内容有任何改动：新版本没调整这张卡，照样只改数字，未调整的卡同样算同步（D246）；
//! - 卡牌实现与版本数值解耦：数值只表示「核对过，且该卡在该版本未调整」（D246）。
//!
//! 升级流程（D246）：**改数字 -> 不动卡牌内容**。
//! 例：最新版变成 6.3 时，把 card_game_version!(6, 2) 改成 card_game_version!(6, 3)，
//! 卡牌实现保持原样即算同步（未调整的卡同样算同步）。
//!
//! 本模块纯 const / 纯数据：零浮点、只用 Nat、无分配、无 IO（no_std 风格，只用 core）。

use core::fmt;

pub use gi_genius_invokation_engine::Nat;

mod macros;

/// 游戏版本号（D244）：major.minor，如 6.2。
///
/// 派生 Ord / PartialOrd 用于普通比较；构造与访问器是 const。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct GameVersion {
    /// 主版本号，如 6。
    major: Nat,
    /// 次版本号，如 2。
    minor: Nat,
}

impl GameVersion {
    /// const 构造（写常量用）。
    pub const fn new(major: Nat, minor: Nat) -> GameVersion {
        GameVersion { major, minor }
    }

    /// 主版本号。
    pub const fn major(self) -> Nat {
        self.major
    }

    /// 次版本号。
    pub const fn minor(self) -> Nat {
        self.minor
    }
}

impl fmt::Display for GameVersion {
    /// 输出 major.minor（如 6.2）。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.major, f)?;
        f.write_str(".")?;
        fmt::Display::fmt(&self.minor, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 宏生成常量可读（D245）：本测试模块声明一张「已同步到 6.2」的卡。
    crate::card_game_version!(6, 2);

    fn v(major: u32, minor: u32) -> GameVersion {
        GameVersion::new(Nat::new(major), Nat::new(minor))
    }

    #[test]
    fn version_compares_and_displays() {
        let v62 = v(6, 2);
        let v61 = v(6, 1);
        let v70 = v(7, 0);
        assert_eq!(v62.major(), Nat::new(6));
        assert_eq!(v62.minor(), Nat::new(2));
        // 派生 Ord：先比 major，再比 minor。
        assert!(v62 > v61);
        assert!(v62 < v70);
        // Display 输出 major.minor。
        assert_eq!(format!("{v62}"), "6.2");
        assert_eq!(format!("{v70}"), "7.0");
    }

    #[test]
    fn macro_generates_a_readable_constant() {
        assert_eq!(GAME_VERSION, v(6, 2));
        assert_eq!(GAME_VERSION.major(), Nat::new(6));
        assert_eq!(GAME_VERSION.minor(), Nat::new(2));
        assert_eq!(format!("{GAME_VERSION}"), "6.2");
    }

    #[test]
    fn different_cards_can_declare_different_versions() {
        // 多个卡 / 模块各自声明自己的「已同步到的最新版本」，互不影响。
        mod card_a {
            // 本卡已同步（核对）到 6.2。
            crate::card_game_version!(6, 2);
        }
        mod card_b {
            // 本卡已同步（核对）到 5.0（新版本没调整它，不必改）。
            crate::card_game_version!(5, 0);
        }
        assert_eq!(card_a::GAME_VERSION, v(6, 2));
        assert_eq!(card_b::GAME_VERSION, v(5, 0));
        assert_ne!(card_a::GAME_VERSION, card_b::GAME_VERSION);
        assert!(card_a::GAME_VERSION > card_b::GAME_VERSION);
    }
}
