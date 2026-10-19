//! 随机原语 + 概率去重（决策日志 D1 / D3 / D150 / D158 / D159）。
//!
//! 唯一入口是 [`enumerate_random`]（等概率）与 [`enumerate_random_weighted`]（可选分数列表），
//! 调用链**写死**：
//!
//! 1. 先枚举**原始分支**（只算「依次取了列表的哪些项」，不碰状态）；
//! 2. feature `dedup` 打开时调用去重函数（`None` 则原样），合并等价分支、权重相加；
//! 3. **只对代表元调用 `list.apply` 建状态**（这就是去重的全部好处）。
//!
//! 概率来源（D150 / D158 / D159）：
//! - 不传分数列表 -> **等概率**（每支 `1 / 分支数`，精确分数）；
//! - 传了分数列表 -> 单次抽取内各选项按分数占比，分支概率 = 各次抽取的条件概率之积；
//! - 概率一律用精确分数 [`Fraction`]（`ExactNat128`），**不经过浮点**；
//! - 权重长度不等 / 权重之和为 0 -> 返回空（不 panic，与其它边界行为同风格）。
//!
//! 计算优化开关（**只为性能，不改概率分布**）：
//! - `dedup`：**只**启用「概率去重」；
//! - `opt-all`：**启用全部计算优化**（`opt-all = ["dedup"]`，即必然包含去重）。
//!   以后新增的计算优化开关都挂到 `opt-all` 下面，`dedup` 只是其中一个。
//!
//! feature 关掉时 `dedup` 参数被忽略（`let _ = dedup;`），
//! **结果分布必须与开启时一致**，只是分支数可能更多。

use crate::nat::{ExactNat128, Nat};
use crate::prob::Fraction;
use crate::select::SelectionSource;

/// 去重函数指针（D1：去重**在调用点声明**，引擎只收函数指针）。
///
/// 输入 `Vec<(原始分支序号, 权重)>`，输出 `Vec<(代表分支序号, 合并后的权重)>`。
/// 权重是**精确分数** [`Fraction`]（D158），不是浮点。
///
/// **原始分支序号的约定**（去重函数必须认识它所服务的这次枚举）：
/// - `all_same = true`：第 `i` 支 = 所有次数都取列表第 `i` 项（共 `len` 支）；
/// - 其他情况：按「依次取了哪些项」的字典序编号（末位变化最快）：
///   `repeatable = true` 时是**可重复**元组，共 `len^times` 支；
///   `repeatable = false` 时是**不重复有序**元组，共 `P(len, times)` 支。
///
/// 契约：只合并等价分支、权重相加，**Σ 不变**（分工计划 §4.2「边界行为」表）。
pub type DedupFn = fn(Vec<(usize, Fraction)>) -> Vec<(usize, Fraction)>;

/// ③-a 随机：是否可以重复选择某项 / 是否所有次数都选择同一项 / 选择次数。
///
/// 边界行为（分工计划 §4.2「边界行为（写死）」表，判定顺序即下面代码的顺序）：
/// - `times == 0` -> 空；
/// - `list.len() == 0` -> 空；
/// - `all_same == true` -> 先选 1 项、其余次数都取它；结果条数 = `list.len()`。
///   **优先于下面的「不重复且次数超长」判定**：全同本来就是重复取同一项，不该被它判空；
/// - `repeatable == false` 且 `times > list.len()` -> 空（不 panic）；
/// - 其余 -> 均匀枚举全部原始分支，每支概率 `1 / 分支数`（精确分数）；
/// - 每条分支 `prob` 相加 = 1（精确分数，可直接用 [`Fraction::is_one`] 判断）；
///   去重只合并权重，Σ 不变。
///
/// ⚠️ **不做分支数上限保护**：`repeatable == true` 时原始分支数是 `len^times`，会指数爆炸。
/// 枚举语义本来就是「全枚举」，`times` 要小由调用方（定义层）保证。
pub fn enumerate_random<S, L>(
    state: &S,
    list: &L,
    repeatable: bool,
    all_same: bool,
    times: Nat,
    dedup: Option<DedupFn>,
) -> Vec<(S, Fraction)>
where
    S: Clone,
    L: SelectionSource<S>,
{
    enumerate_random_weighted(state, list, repeatable, all_same, times, dedup, None)
}

/// ③-a 随机（**可选分数列表**，D150）：在 [`enumerate_random`] 上多收一个权重列表。
///
/// `weights` 与候选列表**一一对应**（`weights.len() == list.len()`）：
/// - `None`：与 [`enumerate_random`] **完全一致**（等概率）；
/// - `Some(weights)`：单次抽取内各选项按 `weights[i]` 的占比抽；
///   分支概率 = **各次抽取的选项条件概率之积**（精确分数）；
/// - 权重长度不等 -> 返回空 `Vec`（不 panic，与 `times == 0` / `list.len() == 0` 同风格）；
/// - 权重之和为 0（所有分子都是 0）-> 归一化无定义，返回空 `Vec`（不 panic）。
///
/// **各模式的加权语义**（与等概率语义一致地推导）：
/// - `all_same == true`：只抽 **1 次**，第 `i` 支概率 = `w_i / Σw`
///   （不是 `(w_i / Σw)^times`；等权重时 = `1 / len`，与等概率一致）；
/// - `repeatable == true`：每次都在**全列表**里独立抽，概率 = 各次占比之积
///   （等权重时 = `1 / len^times`，与等概率一致）；
/// - `repeatable == false`：**不放回**抽样，第 k 次在「剩余项」里按占比抽，
///   概率 = 各次**条件**占比之积（等权重时 = `1 / P(len, times)`，与等概率一致）。
///   若某次「剩余项权重之和为 0」 -> **panic**（概率模型自相矛盾；不静默退化成均匀抽）。
///
/// 概率为 0 的分支**仍然列出**（不剪枝）；去重只合并等价分支、权重相加，**Σ 不变**。
pub fn enumerate_random_weighted<S, L>(
    state: &S,
    list: &L,
    repeatable: bool,
    all_same: bool,
    times: Nat,
    dedup: Option<DedupFn>,
    weights: Option<&[Fraction]>,
) -> Vec<(S, Fraction)>
where
    S: Clone,
    L: SelectionSource<S>,
{
    let times = times.get() as usize;
    let len = list.len();

    // —— 边界行为（§4.2 表，顺序即优先级）——
    if times == 0 || len == 0 {
        return Vec::new();
    }
    // 分数列表必须与候选列表一一对应；长度不等 -> 空（不 panic）。
    if let Some(weights) = weights {
        if weights.len() != len {
            return Vec::new();
        }
    }
    // all_same 优先：全同本来就是「重复取同一项」，不能用「不重复且次数超长」判空。
    if !all_same && !repeatable && times > len {
        return Vec::new();
    }

    // —— ① 先枚举原始分支（只算下标序列，不碰状态）——
    let raw_choices = raw_branches(len, times, repeatable, all_same);
    if raw_choices.is_empty() {
        return Vec::new();
    }

    // —— ①' 原始分支的权重：不传分数 -> 均匀；传了 -> 精确按分数占比 ——
    let raw: Vec<(usize, Fraction)> = match weights {
        None => {
            // 原始分支**均匀**：每支 1/分支数（精确分数）。
            let each = Fraction::new(
                ExactNat128::new(1),
                ExactNat128::new(raw_choices.len() as u128),
            );
            (0..raw_choices.len())
                .map(|ordinal| (ordinal, each))
                .collect()
        }
        Some(weights) => {
            let Some(probabilities) =
                weighted_probabilities(len, repeatable, all_same, &raw_choices, weights)
            else {
                // 权重之和为 0：归一化无定义 -> 空（不 panic）。
                return Vec::new();
            };
            probabilities.into_iter().enumerate().collect()
        }
    };

    // —— ② 去重（条件编译；关闭时参数被忽略，分布不变、只是分支更多）——
    let representatives = merge_branches(raw, dedup);

    // —— ③ 只对代表元调用 apply 建状态 ——
    let mut out = Vec::with_capacity(representatives.len());
    for (ordinal, probability) in representatives {
        let Some(choices) = raw_choices.get(ordinal) else {
            // 去重函数违约（返回了不存在的原始分支序号）：跳过，不让它把枚举带崩。
            debug_assert!(false, "去重函数返回了不存在的原始分支序号");
            continue;
        };
        let mut next = state.clone();
        for &index in choices {
            let (after, _weight) = list.apply(&next, index);
            next = after;
        }
        out.push((next, probability));
    }
    out
}

/// 按分数列表计算**每条原始分支**的概率（精确分数，D150 / D158）。
///
/// - `all_same`：每条分支只对应 1 次抽取；
/// - `repeatable`：每次独立抽，概率 = 各次占比之积；
/// - 不重复：不放回抽样，概率 = 各次**条件**占比之积（等权重时退化为均匀的 `1/P(len,times)`）。
///
/// 返回 `None` 表示权重之和为 0（归一化无定义）。
fn weighted_probabilities(
    len: usize,
    repeatable: bool,
    all_same: bool,
    choices: &[Vec<usize>],
    weights: &[Fraction],
) -> Option<Vec<Fraction>> {
    let total = crate::prob::total(weights);
    if total.is_zero() {
        return None;
    }
    // 单次抽取的占比（等权重 -> 1/len）。
    let single: Vec<Fraction> = weights.iter().map(|weight| weight.exact_div(total)).collect();

    if all_same {
        // 只抽 1 次：第 i 支 = w_i / Σw。
        return Some(choices.iter().map(|choice| single[choice[0]]).collect());
    }

    if repeatable {
        // 每次都在全列表里独立抽：各次占比之积。
        return Some(
            choices
                .iter()
                .map(|choice| {
                    choice
                        .iter()
                        .fold(Fraction::ONE, |mass, &index| mass * single[index])
                })
                .collect(),
        );
    }

    // 不放回：第 k 次在「剩余项」里按占比抽；等权重时 = 1/P(len, times)。
    Some(
        choices
            .iter()
            .map(|choice| {
                let mut mass = Fraction::ONE;
                let mut used: Vec<usize> = Vec::with_capacity(choice.len());
                for &index in choice {
                    let remaining = (0..len)
                        .filter(|candidate| !used.contains(candidate))
                        .fold(Fraction::ZERO, |sum, candidate| {
                            sum + weights[candidate]
                        });
                    // 剩余项权重全为 0 意味着概率模型自相矛盾（应为不可能状态）：
                    // 直接 panic，不用「均匀抽」把 bug 掩盖掉。
                    if remaining.is_zero() {
                        panic!(
                            "概率模型自相矛盾：不重复加权抽样时「剩余项权重全为 0」，无法计算条件概率"
                        );
                    }
                    mass = mass * weights[index].exact_div(remaining);
                    used.push(index);
                }
                mass
            })
            .collect(),
    )
}

/// 枚举「原始分支」：每支 = 依次取了列表的哪些项（只算下标，不碰状态）。
fn raw_branches(len: usize, times: usize, repeatable: bool, all_same: bool) -> Vec<Vec<usize>> {
    debug_assert!(times > 0, "times == 0 已在调用前返回空");
    debug_assert!(len > 0, "len == 0 已在调用前返回空");

    if all_same {
        // 先选 1 项，其余次数都取它；结果条数 = len。
        return (0..len).map(|index| vec![index; times]).collect();
    }

    let mut branches = Vec::new();
    let mut current = Vec::with_capacity(times);
    extend_choices(len, times, repeatable, &mut current, &mut branches);
    branches
}

/// 递归展开所有长度 `times` 的下标序列（`repeatable = false` 时不重复）。
fn extend_choices(
    len: usize,
    times: usize,
    repeatable: bool,
    current: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if current.len() == times {
        out.push(current.clone());
        return;
    }
    for index in 0..len {
        if !repeatable && current.contains(&index) {
            continue;
        }
        current.push(index);
        extend_choices(len, times, repeatable, current, out);
        current.pop();
    }
}

/// feature `dedup` 打开：有去重函数就调，`None` 原样返回。
#[cfg(feature = "dedup")]
fn merge_branches(
    raw: Vec<(usize, Fraction)>,
    dedup: Option<DedupFn>,
) -> Vec<(usize, Fraction)> {
    match dedup {
        Some(dedup) => dedup(raw),
        None => raw,
    }
}

/// feature `dedup` 关闭：**完全忽略**去重函数（只为计算优化的开关，不改分布）。
#[cfg(not(feature = "dedup"))]
fn merge_branches(
    raw: Vec<(usize, Fraction)>,
    dedup: Option<DedupFn>,
) -> Vec<(usize, Fraction)> {
    let _ = dedup;
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用状态：记录依次被选中项的下标序列。
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct State {
        picked: Vec<usize>,
    }

    /// 测试用列表：`len` 个「项」，取第 `i` 项就是把 `i` 记进状态。
    struct Choices(usize);

    impl SelectionSource<State> for Choices {
        fn len(&self) -> usize {
            self.0
        }

        fn apply(&self, state: &State, index: usize) -> (State, Fraction) {
            let mut next = state.clone();
            next.picked.push(index);
            (next, Fraction::ONE)
        }
    }

    fn n(v: u32) -> Nat {
        Nat::new(v)
    }

    fn frac(num: u128, den: u128) -> Fraction {
        Fraction::new(ExactNat128::new(num), ExactNat128::new(den))
    }

    fn empty_state() -> State {
        State { picked: Vec::new() }
    }

    fn probability_sum(branches: &[(State, Fraction)]) -> Fraction {
        branches.iter().map(|(_, probability)| *probability).sum()
    }

    fn assert_sums_to_one(branches: &[(State, Fraction)]) {
        let sum = probability_sum(branches);
        assert!(sum.is_one(), "Σ = {sum} != 1");
    }

    // ------------------------------------------------- 等概率回归（None 逐位一致）

    #[test]
    fn none_weights_is_bit_identical_to_plain_enumerate_random() {
        let cases = [
            (3_usize, false, false, 1_u32),
            (2, true, false, 2),
            (3, false, false, 2),
            (2, false, true, 3),
            (3, true, false, 2),
            (3, false, false, 3),
        ];
        for (len, repeatable, all_same, times) in cases {
            let plain = enumerate_random(
                &empty_state(),
                &Choices(len),
                repeatable,
                all_same,
                n(times),
                None,
            );
            let weighted = enumerate_random_weighted(
                &empty_state(),
                &Choices(len),
                repeatable,
                all_same,
                n(times),
                None,
                None,
            );
            assert_eq!(plain, weighted, "None 必须与旧行为逐位一致");
        }
    }

    #[test]
    fn none_weights_keeps_uniform_probabilities() {
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(3),
            false,
            false,
            n(1),
            None,
            None,
        );
        assert_eq!(branches.len(), 3);
        for (_, probability) in &branches {
            assert_eq!(*probability, frac(1, 3));
        }
        assert_sums_to_one(&branches);
    }

    // --------------------------------------------------------- 分数权重：单次抽取

    #[test]
    fn weighted_single_draw_uses_ratio() {
        let weights = [frac(1, 2), frac(1, 4), frac(1, 4)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(3),
            false,
            false,
            n(1),
            None,
            Some(&weights),
        );
        assert_eq!(branches.len(), 3);
        assert_eq!(branches[0].0.picked, vec![0]);
        assert_eq!(branches[1].0.picked, vec![1]);
        assert_eq!(branches[2].0.picked, vec![2]);
        assert_eq!(branches[0].1, frac(1, 2));
        assert_eq!(branches[1].1, frac(1, 4));
        assert_eq!(branches[2].1, frac(1, 4));
        assert_sums_to_one(&branches);
    }

    #[test]
    fn weighted_all_same_uses_single_draw() {
        // all_same：先选 1 项 -> 概率是 w_i/Σw（一次），不是幂。
        let weights = [frac(1, 4), frac(3, 4)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(2),
            false,
            true,
            n(3),
            None,
            Some(&weights),
        );
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[0].0.picked, vec![0, 0, 0]);
        assert_eq!(branches[1].0.picked, vec![1, 1, 1]);
        assert_eq!(branches[0].1, frac(1, 4));
        assert_eq!(branches[1].1, frac(3, 4));
        assert_sums_to_one(&branches);
    }

    // ---------------------------------------------------- 分数权重：多次（可重复）

    #[test]
    fn weighted_repeatable_multiplies_draw_probabilities() {
        // 2/3 与 1/3 各抽两次 -> 4/9, 2/9, 2/9, 1/9（字典序）。
        let weights = [frac(2, 3), frac(1, 3)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(2),
            true,
            false,
            n(2),
            None,
            Some(&weights),
        );
        assert_eq!(branches.len(), 4);
        let expected = [frac(4, 9), frac(2, 9), frac(2, 9), frac(1, 9)];
        for ((_, probability), wanted) in branches.iter().zip(expected) {
            assert_eq!(*probability, wanted);
        }
        assert_sums_to_one(&branches);
    }

    // -------------------------------------------------- 分数权重：多次（不重复）

    #[test]
    fn weighted_non_repeatable_uses_conditional_probabilities() {
        // 不放回：第一次 1/2, 1/4, 1/4；之后在剩余项里按占比抽。
        let weights = [frac(1, 2), frac(1, 4), frac(1, 4)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(3),
            false,
            false,
            n(2),
            None,
            Some(&weights),
        );
        assert_eq!(branches.len(), 6, "P(3, 2) = 6");
        let expected = [
            (vec![0, 1], frac(1, 4)),
            (vec![0, 2], frac(1, 4)),
            (vec![1, 0], frac(1, 6)),
            (vec![1, 2], frac(1, 12)),
            (vec![2, 0], frac(1, 6)),
            (vec![2, 1], frac(1, 12)),
        ];
        for ((branch, probability), (picked, wanted)) in branches.iter().zip(expected) {
            assert_eq!(branch.picked, picked);
            assert_eq!(*probability, wanted);
        }
        assert_sums_to_one(&branches);
    }

    #[test]
    #[should_panic(expected = "剩余项权重全为 0")]
    fn weighted_non_repeatable_zero_remaining_weight_panics() {
        // 不放回抽样时某次「剩余项权重全为 0」 = 概率模型自相矛盾 -> panic（不静默均匀化）。
        let weights = [frac(1, 1), frac(0, 1)];
        let _ = enumerate_random_weighted(
            &empty_state(),
            &Choices(2),
            false,
            false,
            n(2),
            None,
            Some(&weights),
        );
    }

    // --------------------------------------------------------- 等分数 = 等概率

    #[test]
    fn equal_weights_match_uniform_distribution() {
        let cases = [
            (3_usize, false, false, 1_u32),
            (2, true, false, 2),
            (3, false, false, 2),
            (2, false, true, 3),
            (3, true, false, 2),
        ];
        for (len, repeatable, all_same, times) in cases {
            let weights: Vec<Fraction> = (0..len).map(|_| frac(1, 1)).collect();
            let plain = enumerate_random(
                &empty_state(),
                &Choices(len),
                repeatable,
                all_same,
                n(times),
                None,
            );
            let weighted = enumerate_random_weighted(
                &empty_state(),
                &Choices(len),
                repeatable,
                all_same,
                n(times),
                None,
                Some(&weights),
            );
            assert_eq!(plain, weighted, "等权重必须与等概率逐位一致");
            assert_sums_to_one(&weighted);
        }
    }

    // ----------------------------------------------------------- 权重边界行为

    #[test]
    fn weights_length_mismatch_is_empty() {
        let weights = [frac(1, 2), frac(1, 2)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(3),
            false,
            false,
            n(1),
            None,
            Some(&weights),
        );
        assert!(branches.is_empty(), "长度不等 -> 空（不 panic）");

        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(0),
            true,
            false,
            n(1),
            None,
            Some(&weights),
        );
        assert!(branches.is_empty());
    }

    #[test]
    fn zero_total_weights_is_empty() {
        let weights = [frac(0, 3), frac(0, 4)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(2),
            false,
            false,
            n(1),
            None,
            Some(&weights),
        );
        assert!(branches.is_empty(), "Σ = 0 -> 归一化无定义 -> 空（不 panic）");
    }

    #[test]
    fn times_zero_or_bad_times_still_empty_with_weights() {
        let weights = [frac(1, 2), frac(1, 2)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(2),
            false,
            false,
            n(0),
            None,
            Some(&weights),
        );
        assert!(branches.is_empty());

        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(2),
            false,
            false,
            n(3),
            None,
            Some(&weights),
        );
        assert!(branches.is_empty());
    }

    #[test]
    #[should_panic(expected = "溢出")]
    fn weighted_branch_mass_overflow_panics() {
        // D153：精确族连乘溢出 panic（不是饱和/静默失真）。
        // 单次占比 5/8 与 3/8；5/8 连乘 46 次的分母 8^46 > 2^128。
        let weights = [frac(1, 3), frac(1, 5)];
        let choices = vec![vec![0_usize; 46]];
        let _ = weighted_probabilities(2, true, false, &choices, &weights);
    }

    // ------------------------------------------------------------ 去重 + 权重

    /// 测试用去重函数：把原始分支 0 与 1 合并成代表元 1（其余原样）。
    fn merge_branch_0_and_1(branches: Vec<(usize, Fraction)>) -> Vec<(usize, Fraction)> {
        let mut merged = Fraction::ZERO;
        let mut rest = Vec::new();
        for (ordinal, weight) in branches {
            if ordinal == 0 || ordinal == 1 {
                merged = merged + weight;
            } else {
                rest.push((ordinal, weight));
            }
        }
        if !merged.is_zero() {
            rest.insert(0, (1, merged));
        }
        rest
    }

    #[cfg(feature = "dedup")]
    #[test]
    fn dedup_preserves_total_with_weights() {
        let weights = [frac(1, 2), frac(1, 4), frac(1, 4)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(3),
            false,
            false,
            n(1),
            Some(merge_branch_0_and_1 as DedupFn),
            Some(&weights),
        );
        assert_eq!(branches.len(), 2, "0 与 1 合并后只剩 2 支");
        // 代表元是原始分支 1（权重 1/4），合并了分支 0（1/2）-> 3/4。
        assert_eq!(branches[0].0.picked, vec![1]);
        assert_eq!(branches[0].1, frac(3, 4));
        assert_eq!(branches[1].0.picked, vec![2]);
        assert_eq!(branches[1].1, frac(1, 4));
        assert_sums_to_one(&branches);
    }

    #[cfg(not(feature = "dedup"))]
    #[test]
    fn dedup_fn_is_ignored_with_weights_without_feature() {
        let weights = [frac(1, 2), frac(1, 4), frac(1, 4)];
        let branches = enumerate_random_weighted(
            &empty_state(),
            &Choices(3),
            false,
            false,
            n(1),
            Some(merge_branch_0_and_1 as DedupFn),
            Some(&weights),
        );
        assert_eq!(branches.len(), 3, "没有 dedup feature 时不合并");
        assert_eq!(branches[0].1, frac(1, 2));
        assert_eq!(branches[1].1, frac(1, 4));
        assert_eq!(branches[2].1, frac(1, 4));
        assert_sums_to_one(&branches);
    }

    #[test]
    fn weighted_probabilities_sum_to_one_for_all_modes() {
        let cases = [
            (3_usize, false, false, 1_u32),
            (3, false, false, 2),
            (3, false, false, 3),
            (2, true, false, 4),
            (3, true, false, 3),
            (2, false, true, 5),
        ];
        for (len, repeatable, all_same, times) in cases {
            let weights: Vec<Fraction> = (0..len)
                .map(|index| frac(1 + index as u128 * 2, 3 + index as u128))
                .collect();
            let branches = enumerate_random_weighted(
                &empty_state(),
                &Choices(len),
                repeatable,
                all_same,
                n(times),
                None,
                Some(&weights),
            );
            assert!(!branches.is_empty());
            assert_sums_to_one(&branches);
        }
    }
}
