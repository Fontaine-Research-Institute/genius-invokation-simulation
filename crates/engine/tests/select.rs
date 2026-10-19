//! select.rs（选择原语）的集成测试。
//!
//! 覆盖 v1 写死的行为：count == 1 均匀分支且 Σprob = 1（精确分数）；
//! count == 0 / len == 0 为空；count > 1 语义未定（返回空）。

use gi_genius_invokation_engine::nat::{ExactNat128, Nat};
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::select::{enumerate_select, SelectionSource};

/// 测试用状态：记录被选中项的下标序列。
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    picked: Vec<usize>,
}

/// 测试用选择列表：`len` 个「项」，取第 `i` 项就是把 `i` 记进状态。
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

fn empty_state() -> State {
    State { picked: Vec::new() }
}

fn prob_sum(branches: &[(State, Fraction)]) -> Fraction {
    branches.iter().map(|(_, probability)| *probability).sum()
}

#[test]
fn count_one_gives_one_branch_per_item_with_uniform_probability() {
    let state = empty_state();
    let list = Choices(3);

    let branches = enumerate_select(&state, &list, n(1));

    assert_eq!(branches.len(), 3, "len = 3、count = 1 -> 3 支");
    let expected = Fraction::new(ExactNat128::new(1), ExactNat128::new(3));
    for (index, (branch, probability)) in branches.iter().enumerate() {
        assert_eq!(branch.picked, vec![index], "第 {index} 支取第 {index} 项");
        assert_eq!(*probability, expected, "每支概率必须是 1/len");
    }
    assert!(prob_sum(&branches).is_one(), "概率之和必须是 1");
}

#[test]
fn count_one_does_not_mutate_the_input_state() {
    let state = State { picked: vec![42] };
    let list = Choices(2);

    let branches = enumerate_select(&state, &list, n(1));

    assert_eq!(state.picked, vec![42], "原状态不能被改动");
    assert_eq!(branches.len(), 2);
    for (branch, _) in &branches {
        assert_eq!(branch.picked.len(), 2);
        assert_eq!(branch.picked[0], 42);
    }
}

#[test]
fn count_zero_is_empty() {
    let branches = enumerate_select(&empty_state(), &Choices(3), n(0));
    assert!(branches.is_empty(), "count == 0 -> 空");
}

#[test]
fn empty_list_is_empty() {
    let branches = enumerate_select(&empty_state(), &Choices(0), n(1));
    assert!(branches.is_empty(), "list.len() == 0 -> 空");
}

// 多选/连抽语义尚未拍板（D25）：现在**直接返回空，不 panic**（debug 与 release 都一样）。
#[test]
fn count_two_is_empty_and_does_not_panic() {
    let branches = enumerate_select(&empty_state(), &Choices(3), n(2));
    assert!(branches.is_empty(), "多选语义未拍板 -> 暂时返回空");
}
