//! **一局对局的根状态**：`DefState`（D34 / D42）。
//!
//! 字段：敌我两个具名实例 `me` / `opp`（下面两节）+ 回合数 `round`（规则书·对局回合上限）
//! + 先手牌手 `first_player`（规则书·宣布结束 / 结算顺序「当前牌手优先」）。
//!
//! ## 为什么只有两个具名字段（D34 / D42）
//!
//! 用户口径：「不要用数字编码敌我，而是用像建两个实例这样的」。所以：
//! - 没有 `Side` 枚举、没有 `[PlayerDefs; 2]`、没有「第 0 个是我方」这种下标语义；
//! - 就是 `me` / `opp` 两个**具名字段**，代码写 `state.me()` / `state.opp()`；
//! - 需要「对某一方做同样的事」时，按 D29 的落地形态写**成对入口**（`flows::switch_me` /
//!   `flows::switch_opp`、`defeat_me` / `defeat_opp`…），共用实现写一份
//!   `#[inline]` 私有函数（D35），两个入口各自把那一侧的数据引用传进去 —— **不传 side 参数**。
//!
//! ## 构造 / 克隆 / 相等
//!
//! - 构造：[`DefState::new`] 两个完全空的一方；建局用 [`DefState::from_players`]
//!   一次给两方（例如已经搭好卡组 / 角色牌的两方）；
//! - 克隆：深拷贝（整局快照），与各区纯函数风格配套（见 `state/mod.rs`）；
//! - 相等：`PartialEq` 逐字段比较（两侧 + 回合数 + 先手牌手）；**统计不参与**
//!   （各容器自己的 `PartialEq` 已忽略 stats，D187）。不实现 `Eq`（骰池带精确概率质量，
//!   只有 `PartialEq`）。回合数 / 先手牌手**参与相等**：它们是规则语义（回合上限、行动权），
//!   不是统计。
//!
//! ## 回合数
//!
//! `round` 是「这一局走到第几回合」：**0 = 第一回合还没开始**（建局后 / 开局准备阶段），
//! 第 1 回合起为 1、2、…。依据是规则书·其他规则「对局回合上限」—— 判定「第 15 回合开始时
//! 是否仍未分出胜负」必须先知道现在是第几回合。
//!
//! - **推进点唯一**：回合开始流程 `state::flows::round_begin::begin_round`（+1，到 15 封顶）；
//! - **判定**：终局判定见 `state::terminal`（只读本字段，不改它）；
//! - 回合数参与 `PartialEq`：只差回合数的两局是**不同的局面**（回合上限是规则语义；
//!   D187「统计不参与相等」不适用于它）。
//!
//! ## 先手牌手
//!
//! `first_player` 是**本回合的先手牌手**（= 本回合行动阶段 / 结束阶段的处理起始方）：
//!
//! - 依据 1：规则书·进行行动 战斗行动【宣布结束】「先『宣布回合结束』的牌手会成为『先手牌手』，
//!   并在下回合的行动阶段优先行动」；
//! - 依据 2：规则书·其他规则 结算顺序「1、先结算当前牌手的所有卡牌效果，再结算对方牌手的
//!   卡牌效果」—— 这个「当前牌手」就是先手牌手（配合同样条文决定同侧内的来源类别顺序）；
//! - **[`None`] = 还没裁定**：规则书没有规定第一回合先手归谁（待实测 / 由外部裁定），
//!   所以不编默认值；写入方只有**开局 / 加载流程**；
//! - 与「本回合谁已经宣布结束」的区别：宣布记账是 flow 层的事（宣布结束流程），
//!   本字段只存**结果**（本回合谁先手），供行动权 / 结束阶段顺序读取。
//!
//! ## 只读指针（D183）
//!
//! `me()` / `opp()` 返回 `&PlayerDefs`；可变入口 `me_mut()` / `opp_mut()` 是
//! `pub(crate)`，只给 `state::flows` 用，crate 外拿不到可变引用。

use crate::base::Perspective;
use crate::state::player::PlayerDefs;
use gi_genius_invokation_engine::Nat;

/// **一局对局的完整状态**：我方全部区 + 敌方全部区（两个具名实例，D34）+ 回合数。
///
/// 容器没有身份（D42）：它就是「这一局」这个变量本身，不出现对局号、不登记。
#[derive(Clone, Debug, PartialEq)]
pub struct DefState {
    /// 我方的全部区。
    me: PlayerDefs,
    /// 敌方的全部区。
    opp: PlayerDefs,
    /// 回合数（0 = 还没开始第一回合；每回合开始 +1；上限见
    /// `crate::state::terminal::ROUND_LIMIT`）。
    ///
    /// 依据：规则书·其他规则 对局回合上限「第15回合开始时，如果仍未分出胜负，则双方都会输掉
    /// 这场对局」—— 判定需要知道「现在是第几回合」，所以回合数由根状态持有。
    round: Nat,
    /// **本回合的先手牌手**（行动阶段 / 结束阶段的处理起始方；[`None`] = 还没裁定）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】「先『宣布回合结束』的牌手会成为
    /// 『先手牌手』，并在下回合的行动阶段优先行动」；其他规则·结算顺序第 1 条
    /// 「先结算当前牌手的所有卡牌效果」。
    ///
    /// 写入方：开局 / 加载流程（第一回合由外部裁定；之后由上一回合的先宣布者接任）。
    /// 读取方：引擎求解接口 `state::terminal::DefRules::to_move`（当前轮到谁行动）。
    first_player: Option<Perspective>,
}

impl Default for DefState {
    fn default() -> DefState {
        DefState::new()
    }
}

impl DefState {
    /// 空的一局（双方都是完全空的一方；回合数 0 = 第一回合还没开始）。
    pub const fn new() -> DefState {
        DefState {
            me: PlayerDefs::new(),
            opp: PlayerDefs::new(),
            round: Nat::ZERO,
            first_player: None,
        }
    }

    /// 用两方各自的全部区建一局（建局 / 测试注入初始状态用；回合数从 0 开始、先手未知）。
    pub const fn from_players(me: PlayerDefs, opp: PlayerDefs) -> DefState {
        DefState {
            me,
            opp,
            round: Nat::ZERO,
            first_player: None,
        }
    }

    /// 我方的全部区（**只读指针**，D183）。
    pub fn me(&self) -> &PlayerDefs {
        &self.me
    }

    /// 敌方的全部区（**只读指针**，D183）。
    pub fn opp(&self) -> &PlayerDefs {
        &self.opp
    }

    /// 当前回合数（0 = 还没开始第一回合；1 = 第 1 回合）。
    ///
    /// 推进点只有一个：回合开始流程 `crate::state::flows::round_begin::begin_round`。
    pub fn round(&self) -> Nat {
        self.round
    }

    /// 换掉回合数（纯函数：返回新的一局，不改入参；建局 / 测试注入用）。
    ///
    /// 正式流程不要直接调它 —— 回合推进走
    /// `crate::state::flows::round_begin::begin_round`（+1 并处理第 15 回合边界）。
    pub fn with_round(self, round: Nat) -> DefState {
        DefState { round, ..self }
    }

    /// **本回合的先手牌手**（[`None`] = 还没裁定；只读指针，D183）。
    ///
    /// 依据：规则书·进行行动 战斗行动【宣布结束】+ 其他规则·结算顺序第 1 条。
    pub fn first_player(&self) -> Option<Perspective> {
        self.first_player
    }

    /// 换掉先手牌手（纯函数：返回新的一局，不改入参）。
    ///
    /// - 写入方：**只有开局 / 加载流程**（第一回合由外部裁定；之后由上一回合的先宣布者接任，
    ///   见 flow 层的宣布结束流程）；
    /// - [`None`] = 还没裁定（建局后、第一回合先手确定之前）；
    /// - 用 [`Option`] 而不是「默认我方」：规则书没有规定第一回合先手归谁，不自行编一个
    ///   默认值（默认值只出现在引擎适配 `to_move` 的**占位**里，见 `state::terminal`）。
    pub fn with_first_player(self, first_player: Option<Perspective>) -> DefState {
        DefState {
            first_player,
            ..self
        }
    }

    /// 换掉我方的全部区（纯函数：返回新的一局，不改入参）。
    pub fn with_me(self, me: PlayerDefs) -> DefState {
        DefState { me, ..self }
    }

    /// 换掉敌方的全部区（纯函数：返回新的一局，不改入参）。
    pub fn with_opp(self, opp: PlayerDefs) -> DefState {
        DefState { opp, ..self }
    }

    /// 我方的可变入口（`pub(crate)`；只有 `state::flows` 用）。
    pub(crate) fn me_mut(&mut self) -> &mut PlayerDefs {
        &mut self.me
    }

    /// 敌方的可变入口（`pub(crate)`；只有 `state::flows` 用）。
    pub(crate) fn opp_mut(&mut self) -> &mut PlayerDefs {
        &mut self.opp
    }
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::zones::hand::HandState;
    use crate::resources::dice::{DicePool, DieFace};
    use crate::state::testing::{characters, hand, n, pile};
    use gi_genius_invokation_engine::Nat;

    fn side_with_contents() -> PlayerDefs {
        PlayerDefs::new()
            .with_pile(pile(&[101, 102, 103]))
            .with_hand(hand(&[201]))
            .with_characters(characters(&[301, 302]))
            .with_dice(DicePool::empty().add(DieFace::Omni, n(2)))
    }

    #[test]
    fn new_state_has_two_empty_sides() {
        let state = DefState::new();
        assert_eq!(state.me(), &PlayerDefs::new());
        assert_eq!(state.opp(), &PlayerDefs::new());
        assert_eq!(DefState::default(), DefState::new());
        // 两侧是两个独立实例（不是同一个值的别名）
        assert_eq!(state.me().hand().len(), Nat::ZERO);
        assert_eq!(state.opp().hand().len(), Nat::ZERO);
    }

    #[test]
    fn round_starts_at_zero_and_with_round_is_pure() {
        let base = DefState::new();
        // 建局后还没开始第一回合：0
        assert_eq!(base.round(), Nat::ZERO);
        // 换回合数：纯函数、不改两方任何区
        let third = base.clone().with_round(n(3));
        assert_eq!(third.round(), n(3));
        assert_eq!(third.me(), &PlayerDefs::new());
        assert_eq!(third.opp(), &PlayerDefs::new());
        // 回合数是局面语义的一部分（规则书·对局回合上限）：只差回合数 = 不同局面
        assert_ne!(third, base);
        // 纯函数：入参一个字节都不动
        assert_eq!(base.round(), Nat::ZERO);
        // 用 from_players 建局也从 0 开始
        assert_eq!(
            DefState::from_players(side_with_contents(), PlayerDefs::new()).round(),
            Nat::ZERO
        );
    }

    #[test]
    fn first_player_starts_unknown_and_with_first_player_is_pure() {
        use crate::base::Perspective;

        let base = DefState::new();
        // 建局：先手未知（规则书没规定第一回合先手归谁，不编默认值）
        assert_eq!(base.first_player(), None);

        let me_first = base.clone().with_first_player(Some(Perspective::Me));
        assert_eq!(me_first.first_player(), Some(Perspective::Me));
        // 换先手不动两方任何区 / 回合数
        assert_eq!(me_first.me(), &PlayerDefs::new());
        assert_eq!(me_first.opp(), &PlayerDefs::new());
        assert_eq!(me_first.round(), Nat::ZERO);
        // 纯函数：入参不变
        assert_eq!(base.first_player(), None);

        // 先手是局面语义的一部分（宣布结束 S3.6 / 结算顺序「当前牌手优先」）：
        // 只差先手 = 不同的局面
        let opp_first = base.clone().with_first_player(Some(Perspective::Opp));
        assert_eq!(opp_first.first_player(), Some(Perspective::Opp));
        assert_ne!(me_first, base);
        assert_ne!(opp_first, me_first);

        // 可以清回「未知」
        assert_eq!(me_first.with_first_player(None).first_player(), None);
        // 与回合数互不干扰
        let both = opp_first.clone().with_round(n(3));
        assert_eq!(both.first_player(), Some(Perspective::Opp));
        assert_eq!(both.round(), n(3));

        // 用 from_players 建局也从「未知」开始
        assert_eq!(
            DefState::from_players(side_with_contents(), PlayerDefs::new()).first_player(),
            None
        );
    }

    #[test]
    fn with_me_and_with_opp_are_pure_replacers() {
        let base = DefState::new();
        let me = side_with_contents();
        let next = base.clone().with_me(me.clone());
        assert_eq!(next.me(), &me);
        assert_eq!(next.opp(), &PlayerDefs::new());
        // 纯函数：入参不变
        assert_eq!(base, DefState::new());
        // 换 opp 同理
        let next = base.clone().with_opp(me.clone());
        assert_eq!(next.opp(), &me);
        assert_eq!(next.me(), &PlayerDefs::new());
        // from_players 一次给两方
        let both = DefState::from_players(me.clone(), side_with_contents());
        assert_eq!(both.me(), &me);
        assert_eq!(both.opp(), &side_with_contents());
    }

    #[test]
    fn two_sides_are_independent() {
        // 只改我方：敌方必须一个字节都不动（D34：两个具名实例，互不影响）
        let before = DefState::from_players(side_with_contents(), side_with_contents());
        let opp_snapshot = before.opp().clone();
        let me_changed = before
            .me()
            .clone()
            .with_hand(HandState::new())
            .with_dice(DicePool::empty());
        let after = before.clone().with_me(me_changed);
        assert!(after.me().hand().is_empty());
        assert_eq!(after.me().dice().total(), Nat::ZERO);
        // 敌方完全没变
        assert_eq!(after.opp(), &opp_snapshot);
        // 入参也没变
        assert_eq!(before.me().hand().len(), Nat::new(1));
        assert_eq!(before.me().dice().total(), Nat::new(2));
    }

    #[test]
    fn me_and_opp_are_distinct_addresses() {
        let state = DefState::from_players(side_with_contents(), PlayerDefs::new());
        // 两个具名实例是两块独立内存（不是同一个引用）
        assert!(!std::ptr::addr_eq(state.me(), state.opp()));
        // 只读指针：两次拿到同一块（&PlayerDefs）
        let me: &PlayerDefs = state.me();
        assert!(std::ptr::addr_eq(me, state.me()));
        // 读到的值正确
        assert_eq!(me.hand().len(), Nat::new(1));
        assert_eq!(state.opp().hand().len(), Nat::ZERO);
    }

    #[test]
    fn clone_is_a_deep_snapshot() {
        let before = DefState::from_players(side_with_contents(), side_with_contents());
        let snapshot = before.clone();
        let changed = before
            .clone()
            .with_me(before.me().clone().with_hand(HandState::new()));
        assert!(changed.me().hand().is_empty());
        // 克隆体与快照都不受改动影响
        assert_eq!(snapshot.me().hand().len(), Nat::new(1));
        assert_eq!(snapshot, before);
    }

    #[test]
    fn equality_compares_both_sides_and_ignores_nothing_structural() {
        let a = DefState::from_players(side_with_contents(), side_with_contents());
        let b = DefState::from_players(side_with_contents(), side_with_contents());
        // 同样的构造 -> 相等（统计不参与，构造顺序不影响）
        assert_eq!(a, b);
        // 任一侧任何一处不同 -> 不等
        let different_me = DefState::from_players(
            side_with_contents().with_hand(HandState::new()),
            side_with_contents(),
        );
        assert_ne!(a, different_me);
        let different_opp = DefState::from_players(
            side_with_contents(),
            side_with_contents().with_hand(HandState::new()),
        );
        assert_ne!(a, different_opp);
    }
}
