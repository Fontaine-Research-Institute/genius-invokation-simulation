//! 求解接口 `GameRules`。
//!
//! **这个模块默认不编译**：需要求解时构建加 `--features rules`（决策日志 D24）。
//! 默认的 engine 仍然只有实体 / 区 / 随机与选择。
//!
//! 这里只放**接口**：引擎不知道任何语义（也**不分敌我**），类型参数 `S` 完全由调用方决定。
//!
//! **怎么表达「谁」**：由定义层用关联类型 [`GameRules::Player`] 自己给出；
//! 「我方 / 敌方」是它的**两个具名实例**（不是编号、不是枚举、不是数组下标，D34）。
//! 引擎这边不提供任何「玩家标识」，也**不提供区编号/区表**（区就是上层自己持有的变量）。
//!
//! 估值用精确族 `ExactNat128`（D159：不出现浮点）。
//!
//! 终局结果见 [`GameResult`]：除 Win / Draw / RoundLimit 外还有 `BothLose`（**双负**，D267）。

use crate::nat::ExactNat128;

/// 对局规则接口：求解器只通过这四个函数看局面。
pub trait GameRules<S> {
    /// 「谁」的类型：由定义层给出，最小约束只要 `Copy + Eq`。
    /// 「我方 / 敌方」是它的两个**具名实例**，不是编号。
    type Player: Copy + Eq;

    /// 现在轮到谁行动。
    fn to_move(&self, st: &S) -> Self::Player;

    /// 局面是否已结束（`None` = 未结束）。
    fn terminal(&self, st: &S) -> Option<GameResult<Self::Player>>;

    /// `who` 在当前局面下的全部选择列表。
    fn actions(&self, st: &S, who: Self::Player) -> Vec<ActionList>;

    /// 局面估值（越大越偏向「我方」那个具名实例；谁是「我方」由调用方约定）。
    ///
    /// 用精确族 `ExactNat128`（D159）：求解器内部一律精确整数，不出现浮点。
    fn evaluate(&self, st: &S) -> ExactNat128;
}

/// 终局结果（`P` = 定义层给的「谁」）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum GameResult<P> {
    /// 某一方获胜。
    Win(P),
    /// 平局。
    Draw,
    /// 达到回合上限。
    RoundLimit,
    /// **双方都输（双负）**：终局判定时双方角色区都为空。
    ///
    /// - 不是平局（`Draw`），也不是某一方获胜（`Win`）；
    /// - 依据 **D267**（用户 2026-10-19 明确）：终局判定在**步边界**（本步效果全部执行完才判，步内不提前终止）；
    ///   实例：芙宁娜（白芙）天赋牌 + 普通攻击 可造成双负。
    BothLose,
}

/// 选择列表的**来源描述**：具体形状由定义层决定，引擎不预设。
///
/// v1 只是占位结构：能让 [`GameRules`] 编译通过即可，不实现任何逻辑。
/// 后续形状（列表来源 / 项的数量 / 每个项怎么解析）由各块自己的 `parse.rs` 定义（D26）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ActionList {
    _placeholder: (),
}

impl ActionList {
    /// 占位构造（v1 唯一用法；本身不代表任何语义）。
    pub const fn new() -> Self {
        ActionList { _placeholder: () }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 「我方 / 敌方」= 同一个玩家类型的**两个具名实例**（不是枚举判别值、不是编号）。
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    struct Player {
        name: &'static str,
    }

    const ME: Player = Player { name: "我方" };
    const OPP: Player = Player { name: "敌方" };

    /// 用一个不涉及任何上层语义的 stub 证明 trait 可用。
    struct Stub;

    impl GameRules<u32> for Stub {
        type Player = Player;

        fn to_move(&self, _st: &u32) -> Player {
            ME
        }

        fn terminal(&self, st: &u32) -> Option<GameResult<Player>> {
            match *st {
                // 终局只在「步边界」判定（D267）：这里用 stub 的数值模拟三种已定的终局。
                2 => Some(GameResult::RoundLimit),
                3 => Some(GameResult::BothLose),
                _ => None,
            }
        }

        fn actions(&self, _st: &u32, _who: Player) -> Vec<ActionList> {
            Vec::new()
        }

        fn evaluate(&self, st: &u32) -> ExactNat128 {
            ExactNat128::new(*st as u128)
        }
    }

    #[test]
    fn stub_rules_implements_trait() {
        // 具名实例：两个实例各自的「名字」可读，不是数字编码。
        assert_eq!(ME.name, "我方");
        assert_eq!(OPP.name, "敌方");
        assert_ne!(ME, OPP);

        let rules = Stub;
        assert_eq!(rules.to_move(&0), ME);
        assert_eq!(rules.terminal(&0), None);
        assert_eq!(rules.terminal(&2), Some(GameResult::RoundLimit));
        assert_eq!(rules.terminal(&3), Some(GameResult::BothLose));
        assert!(rules.actions(&0, OPP).is_empty());
        assert_eq!(rules.evaluate(&3), ExactNat128::new(3));
    }

    #[test]
    fn game_result_is_comparable() {
        assert_eq!(GameResult::Win(ME), GameResult::Win(ME));
        assert_ne!(GameResult::Win(ME), GameResult::Win(OPP));
        assert_ne!(
            GameResult::<Player>::Draw,
            GameResult::<Player>::RoundLimit
        );
        let mut set: std::collections::HashSet<GameResult<Player>> = std::collections::HashSet::new();
        set.insert(GameResult::Draw);
        assert!(set.contains(&GameResult::Draw));
    }

    #[test]
    fn both_lose_is_distinct_from_draw_and_win() {
        // 构造 + 穷尽 match（新增变体必须被显式处理）。
        let result: GameResult<Player> = GameResult::BothLose;
        let name = match result {
            GameResult::Win(_) => "win",
            GameResult::Draw => "draw",
            GameResult::RoundLimit => "round_limit",
            GameResult::BothLose => "both_lose",
        };
        assert_eq!(name, "both_lose");

        // 语义（D267）：双负 ≠ 平局、≠ 某一方胜。
        assert_eq!(GameResult::<Player>::BothLose, GameResult::BothLose);
        assert!(matches!(
            GameResult::<Player>::BothLose,
            GameResult::BothLose
        ));
        assert_ne!(GameResult::<Player>::BothLose, GameResult::Draw);
        assert_ne!(GameResult::<Player>::BothLose, GameResult::RoundLimit);
        assert_ne!(GameResult::<Player>::BothLose, GameResult::Win(ME));
        assert_ne!(GameResult::<Player>::BothLose, GameResult::Win(OPP));

        // 原有三个变体的语义不变。
        assert_eq!(GameResult::Win(ME), GameResult::Win(ME));
        assert_ne!(GameResult::Win(ME), GameResult::Win(OPP));
        assert_eq!(GameResult::<Player>::Draw, GameResult::Draw);
        assert_eq!(GameResult::<Player>::RoundLimit, GameResult::RoundLimit);
        assert_ne!(GameResult::<Player>::Draw, GameResult::RoundLimit);
    }

    #[test]
    fn action_list_placeholder() {
        let l = ActionList::new();
        assert_eq!(l, ActionList::default());
        let v: Vec<ActionList> = vec![l];
        assert_eq!(v.len(), 1);
    }
}
