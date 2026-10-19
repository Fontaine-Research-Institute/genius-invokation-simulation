//! random.rs（随机原语 + 概率去重）的集成测试。
//!
//! 默认 feature 与 `--features dedup` 两套构建都要跑同一批「分布不变」的断言；
//! 只有「分支数」会因去重而不同。概率是精确分数 [`Fraction`]（D158 / D159）。

use gi_genius_invokation_engine::nat::{ExactNat128, Nat};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::random::{enumerate_random, DedupFn};
use gi_genius_invokation_engine::select::SelectionSource;

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

fn prob_sum(branches: &[(State, Fraction)]) -> Fraction {
    branches.iter().map(|(_, probability)| *probability).sum()
}

fn assert_sums_to_one(branches: &[(State, Fraction)]) {
    assert!(prob_sum(branches).is_one(), "Σ != 1");
}

// ------------------------------------------------------------ 基本边界行为

#[test]
fn times_one_without_repeat_sums_to_one() {
    let branches = enumerate_random(&empty_state(), &Choices(3), false, false, n(1), None);

    assert_eq!(branches.len(), 3);
    for (index, (branch, probability)) in branches.iter().enumerate() {
        assert_eq!(branch.picked, vec![index]);
        assert_eq!(*probability, frac(1, 3));
    }
    assert_sums_to_one(&branches);
}

#[test]
fn all_same_gives_exactly_len_branches() {
    let branches = enumerate_random(&empty_state(), &Choices(3), false, true, n(2), None);

    assert_eq!(branches.len(), 3);
    for (index, (branch, _)) in branches.iter().enumerate() {
        assert_eq!(branch.picked, vec![index, index]);
    }
    assert_sums_to_one(&branches);
}

#[test]
fn all_same_wins_over_non_repeatable_times_over_len() {
    let branches = enumerate_random(&empty_state(), &Choices(2), false, true, n(3), None);

    assert_eq!(branches.len(), 2, "all_same -> 条数 = list.len()");
    for (index, (branch, _)) in branches.iter().enumerate() {
        assert_eq!(branch.picked, vec![index, index, index]);
    }
    assert_sums_to_one(&branches);
}

#[test]
fn times_two_without_repeat_over_short_list_is_empty() {
    let branches = enumerate_random(&empty_state(), &Choices(1), false, false, n(2), None);
    assert!(branches.is_empty());
}

#[test]
fn times_zero_is_empty() {
    let branches = enumerate_random(&empty_state(), &Choices(3), true, false, n(0), None);
    assert!(branches.is_empty());
}

#[test]
fn empty_list_is_empty() {
    let branches = enumerate_random(&empty_state(), &Choices(0), true, false, n(1), None);
    assert!(branches.is_empty());
}

// ------------------------------------------------------- 多次枚举（展开顺序）

#[test]
fn repeatable_times_two_enumerates_ordered_tuples() {
    let branches = enumerate_random(&empty_state(), &Choices(2), true, false, n(2), None);

    assert_eq!(branches.len(), 4, "可重复：len^times = 4");
    let choices: Vec<Vec<usize>> = branches.iter().map(|(branch, _)| branch.picked.clone()).collect();
    assert_eq!(
        choices,
        vec![vec![0, 0], vec![0, 1], vec![1, 0], vec![1, 1]]
    );
    assert_sums_to_one(&branches);
}

#[test]
fn non_repeatable_times_two_enumerates_ordered_distinct_tuples() {
    let branches = enumerate_random(&empty_state(), &Choices(3), false, false, n(2), None);

    assert_eq!(branches.len(), 6, "不重复：P(3, 2) = 6");
    let choices: Vec<Vec<usize>> = branches.iter().map(|(branch, _)| branch.picked.clone()).collect();
    assert_eq!(
        choices,
        vec![
            vec![0, 1],
            vec![0, 2],
            vec![1, 0],
            vec![1, 2],
            vec![2, 0],
            vec![2, 1]
        ]
    );
    assert_sums_to_one(&branches);
}

// ---------------------------------------------------------------- 去重测试

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

// feature 打开：去重真的生效 -> 分支数变少、Σ 仍为 1。
#[cfg(feature = "dedup")]
#[test]
fn dedup_feature_merges_equivalent_branches() {
    let branches = enumerate_random(
        &empty_state(),
        &Choices(2),
        false,
        false,
        n(1),
        Some(merge_branch_0_and_1 as DedupFn),
    );

    assert_eq!(branches.len(), 1, "0 与 1 合并后只剩 1 支");
    let (branch, probability) = &branches[0];
    assert_eq!(branch.picked, vec![1], "代表元是原始分支 1");
    assert_eq!(*probability, Fraction::ONE, "权重相加后是 1");
    assert_sums_to_one(&branches);
}

// feature 打开但 dedup == None：原样返回，分支数不变。
#[cfg(feature = "dedup")]
#[test]
fn dedup_feature_without_fn_is_identity() {
    let branches = enumerate_random(&empty_state(), &Choices(2), false, false, n(1), None);
    assert_eq!(branches.len(), 2);
    assert_sums_to_one(&branches);
}

// feature 关闭：去重参数被忽略 -> 分支数不变、分布不变。
#[cfg(not(feature = "dedup"))]
#[test]
fn dedup_fn_is_ignored_without_feature() {
    let branches = enumerate_random(
        &empty_state(),
        &Choices(2),
        false,
        false,
        n(1),
        Some(merge_branch_0_and_1 as DedupFn),
    );

    assert_eq!(branches.len(), 2, "没有 dedup feature 时不合并");
    let choices: Vec<Vec<usize>> = branches.iter().map(|(branch, _)| branch.picked.clone()).collect();
    assert_eq!(choices, vec![vec![0], vec![1]]);
    assert_sums_to_one(&branches);
}
