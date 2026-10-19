//! 选择原语：**从列表的项里选 count 个**（决策日志 D25：**没有 who**）。
//!
//! 列表（[`SelectionSource`]）由各块**自己的 parse.rs** 定义（D26）：引擎只要求
//! 「能数出多少项」和「取第 index 项后状态变成什么、权重多少」，不预设任何语义。
//!
//! 概率一律用精确分数 [`Fraction`]（D158 / D159），不经过浮点。
//!
//! 边界行为（分工计划 §4.2「边界行为（写死）」表）：
//! - `count == 0` -> 空；
//! - `list.len() == 0` -> 空；
//! - `count == 1` -> `list.len()` 个分支，每支概率 `1 / len`（精确分数）；
//! - `count > 1` -> **语义尚未拍板**（见下），先返回空（不 panic）。

use crate::nat::{ExactNat128, Nat};
use crate::prob::Fraction;

/// 定义层提供的「选择列表」：引擎只要求能数出多少项、以及「取第 i 项后状态变成什么」。
pub trait SelectionSource<S> {
    /// 列表有多少项。
    fn len(&self) -> usize;

    /// 列表是否为空（默认由 `len` 推出，实现方可以覆盖）。
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 返回：取第 `index` 项后的新状态 + 该项权重（精确分数，D158）。
    fn apply(&self, state: &S, index: usize) -> (S, Fraction);
}

/// ③-b 选择：**只收列表与数量，没有 who**（D25）。
///
/// 语义：从列表的项里选 `count` 个。
/// 「谁在选」「每项数据怎么解析表达」由**列表类型自己**表达（D26）。
///
/// v1 规则（写死）：
/// - `count == 0` 或 `list.len() == 0` -> 返回空 `Vec`；
/// - `count == 1` -> `list.len()` 个分支，每支概率 `1 / len`（精确分数，Σ = 1）；
/// - `count > 1` -> **暂时返回空 `Vec`（不 panic）**。
///
/// ⚠️ **多选 / 连抽的实现语义尚未拍板，见决策日志 D25**。
/// 这里**不擅自实现成「看起来合理」的样子**（先选后选、是否放回、概率怎么乘都还没定），
/// 先把入口留空并把 TODO 标在代码里；拍板后只改这一个分支。
/// debug 构建下也**不 panic**：调用方只是传了个值，不该因此炸掉。
pub fn enumerate_select<S, L>(state: &S, list: &L, count: Nat) -> Vec<(S, Fraction)>
where
    S: Clone,
    L: SelectionSource<S>,
{
    let count = count.get() as usize;
    let len = list.len();

    if count == 0 || len == 0 {
        return Vec::new();
    }

    if count > 1 {
        // 多选/连抽的实现语义尚未拍板，见 D25：先返回空，**不 panic**。
        return Vec::new();
    }

    // count == 1：均匀取一项。概率完全由列表长度决定（§4.2 写死为 1/len）。
    let probability = Fraction::new(ExactNat128::new(1), ExactNat128::new(len as u128));
    let mut branches = Vec::with_capacity(len);
    for index in 0..len {
        let (next, _weight) = list.apply(state, index);
        branches.push((next, probability));
    }
    branches
}
