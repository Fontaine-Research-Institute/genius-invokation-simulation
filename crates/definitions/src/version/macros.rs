//! D245 / D246 的版本声明宏（#[macro_export] 暴露到 crate 根）。
//!
//! 宏只生成一个常量，不参与任何运行期逻辑。

/// 声明「本卡 / 本模块已同步（核对）到的最新游戏版本」（D245 / D246）。
///
/// 用法：crate::card_game_version!(6, 2); 生成
/// pub const GAME_VERSION: GameVersion = GameVersion::new(Nat::new(6), Nat::new(2));
///
/// 语义（D245 / D246）：
/// - 这个值 = 本卡已同步（核对）到的最新游戏版本；
/// - 「已同步到最新版本」的唯一表示方式 = 把宏里的版本号改成最新版本号；
/// - 不要求卡牌内容有任何改动：新版本没调整这张卡，同样算同步（未调整也算同步）；
/// - 卡牌实现与版本数值解耦：数值只表示「核对过，且该卡在该版本未调整」。
///
/// 升级流程（D246）：**改数字 -> 不动卡牌内容**。
/// 例：最新版变成 6.3 时，把 crate::card_game_version!(6, 2); 改成
/// crate::card_game_version!(6, 3);，卡牌实现保持原样即算已同步。
#[macro_export]
macro_rules! card_game_version {
    ($major:expr, $minor:expr $(,)?) => {
        /// 本卡 / 本模块已同步（核对）到的最新游戏版本（D245 / D246；由 card_game_version! 生成）。
        pub const GAME_VERSION: $crate::version::GameVersion = $crate::version::GameVersion::new(
            $crate::version::Nat::new($major),
            $crate::version::Nat::new($minor),
        );
    };
}
