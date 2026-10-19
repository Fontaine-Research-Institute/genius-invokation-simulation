//! 骰子这块自己的**选择列表 + 折叠（去重）函数**（D91/D92）。
//!
//! - 选择列表：随机骰面列表 RandomFaceList（当前生成用组合枚举避免 8^n 爆炸；列表留给引擎随机原语接入）。
//! - 折叠：按「骰池表示的等价类」合并分支、权重相加（D148）；本块自己的实现，不复用牌库的。
//! - 随机生成：给定 n，直接按**组合 / 多重集**枚举，权重 = 排列数（等价于先枚所有有序分支再折叠，
//!   但不会指数爆炸）；概率 = 权重 / 原始分支数，总概率必须为 1。
//! - D139/D140/D148：随机可限定基础 / 不限定；可要求 n 颗互不相同（先检测 n 再枚举）。
//! - D141：指定内容用有序表达式，数量 = 长度，顺序当前无语义。
//! - D149/D151：本文件只算概率；真实随机数归引擎（采样调用点只有 mod.rs 的 sample_color 一处）。
//! - D156/D158/D159：概率 / 权重一律用精确族 ExactNat128 与引擎的 Fraction，**全程不经过浮点**
//!   （求解器 / UI 拿到的也是精确分数）。

use super::{min_nat, DicePool, DiceSlot, DieFace, DICE_CAP};
use crate::base::Element;
use gi_genius_invokation_engine::nat::ExactNat128;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::{nadd, ndiv, nmul, nsub, Nat, SelectionSource};

// ======================================================================
// 选择列表（D92）
// ======================================================================

/// 随机骰面列表：限定基础 = 七元素（不含万能）；不限定 = 七元素 + 万能（D140）。
pub(super) fn random_faces(limited_to_basic: bool) -> &'static [DieFace] {
    if limited_to_basic {
        &DieFace::BASIC
    } else {
        &DieFace::ALL
    }
}

/// 骰子这块自己的选择列表（D92）：每项 = 一种随机骰面。
///
/// 当前生成走组合枚举（multiset_branches，避免 8^n 爆炸）；这个列表供引擎随机原语 /
/// 将来接入使用，所以先保留（本文件内暂无调用）。
#[allow(dead_code)]
pub struct RandomFaceList {
    /// 是否限定基础元素。
    pub limited_to_basic: bool,
}

impl SelectionSource<DicePool> for RandomFaceList {
    fn len(&self) -> usize { // bare-int-ok: 引擎列表下标契约，非业务数值
        random_faces(self.limited_to_basic).len()
    }

    fn apply(&self, state: &DicePool, index: usize) -> (DicePool, Fraction) { // bare-int-ok: 引擎列表下标契约，非业务数值
        let faces = random_faces(self.limited_to_basic);
        let Some(face) = faces.get(index) else {
            return (state.clone(), Fraction::ONE);
        };
        (state.add(*face, Nat::new(1)), Fraction::ONE)
    }
}

// ======================================================================
// 生成入口
// ======================================================================

/// 随机生成（D139/D140/D148）：n 颗骰，返回折叠后的分支（精确分数概率）。
pub(super) fn generate_random(
    pool: &DicePool,
    count: Nat,
    limited_to_basic: bool,
    distinct: bool,
) -> Vec<(DicePool, Fraction)> {
    if count.get() == 0 {
        return Vec::new();
    }
    let faces = random_faces(limited_to_basic);
    let face_total = Nat::new(faces.len() as _);
    if distinct && count > face_total {
        return Vec::new();
    }
    let capacity = nsub!(DICE_CAP, pool.total());
    let take = min_nat(count, capacity);
    if take.get() == 0 {
        return vec![(pool.clone(), Fraction::ONE)];
    }
    let branches = if distinct {
        distinct_branches(pool, faces, take)
    } else {
        multiset_branches(pool, faces, take)
    };
    fold_branches(branches)
}

/// 按有序内容表达式生成（D141）：数量 = 长度；顺序当前无语义。
pub(super) fn generate_content(
    pool: &DicePool,
    content: &[DiceSlot],
    limited_to_basic: bool,
) -> Vec<(DicePool, Fraction)> {
    if content.is_empty() {
        return Vec::new();
    }
    let capacity = nsub!(DICE_CAP, pool.total());
    let take = min_nat(Nat::new(content.len() as _), capacity);
    if take.get() == 0 {
        return vec![(pool.clone(), Fraction::ONE)];
    }
    let mut explicit = pool.clone();
    let mut any_count = Nat::ZERO;
    for slot in content.iter().take(take.get() as _) {
        match *slot {
            DiceSlot::Element(element) => {
                explicit = explicit.add(DieFace::from_element(element), Nat::new(1));
            }
            DiceSlot::Omni => explicit = explicit.add(DieFace::Omni, Nat::new(1)),
            DiceSlot::Any => any_count = nadd!(any_count, Nat::new(1)),
        }
    }
    if any_count.get() == 0 {
        return vec![(explicit, Fraction::ONE)];
    }
    fold_branches(multiset_branches(
        &explicit,
        random_faces(limited_to_basic),
        any_count,
    ))
}

// ======================================================================
// 折叠（去重）—— 本块自己的实现（D91/D148）
// ======================================================================

/// 表示等价键：万能数 + 折叠组 (source, size) + 具体色。
///
/// 注意：键里**不含**折叠组的 color_mass。本块的随机生成只产出 known/omni（D148），
/// 所以折叠不会遇到「个数相同但质量不同」的情形；将来 folded 参与折叠时要扩展键并混合质量。
type PoolKey = (Nat, Vec<(Nat, Nat)>, Vec<(Element, Nat)>);

fn pool_key(pool: &DicePool) -> PoolKey {
    let folded: Vec<(Nat, Nat)> = pool
        .folded
        .iter()
        .map(|group| (group.source, group.size))
        .collect();
    (pool.omni, folded, pool.known.clone())
}

/// 按表示等价类合并分支、权重相加（本块自己的折叠入口，D91/D92/D148）。
///
/// 权重是精确 Fraction（D156/D158）。
pub(super) fn fold_branches(branches: Vec<(DicePool, Fraction)>) -> Vec<(DicePool, Fraction)> {
    use std::collections::BTreeMap;
    let mut classes: BTreeMap<PoolKey, (DicePool, Vec<Fraction>)> = BTreeMap::new();
    for (branch, probability) in branches {
        let key = pool_key(&branch);
        match classes.get_mut(&key) {
            Some(entry) => entry.1.push(probability),
            None => {
                classes.insert(key, (branch, vec![probability]));
            }
        }
    }
    classes
        .into_values()
        .map(|(branch, weights)| {
            let total: Fraction = weights.iter().sum();
            (branch, total)
        })
        .collect()
}

// ======================================================================
// 组合枚举（权重 = 排列数；概率 = 权重 / 原始分支数）
// ======================================================================

/// 可重复、不区分顺序：枚举 n 颗骰的多重集。
///
/// 概率 = 多重集系数 / m^n = (k! / ∏c_i!) / m^k（精确 Fraction）。
fn multiset_branches(pool: &DicePool, faces: &[DieFace], count: Nat) -> Vec<(DicePool, Fraction)> {
    let mut out = Vec::new();
    let face_total = Nat::new(faces.len() as _);
    let denominator = pow_exact(face_total, count);
    extend_multisets(
        pool,
        faces,
        count,
        ExactNat128::new(1),
        denominator,
        &mut out,
    );
    out
}

fn extend_multisets(
    pool: &DicePool,
    faces: &[DieFace],
    remaining: Nat,
    multinomial: ExactNat128,
    denominator: ExactNat128,
    out: &mut Vec<(DicePool, Fraction)>,
) {
    let Some((face, rest)) = faces.split_first() else {
        if remaining.get() == 0 {
            out.push((pool.clone(), Fraction::new(multinomial, denominator)));
        }
        return;
    };
    let mut count = Nat::ZERO;
    loop {
        let next_multinomial = nmul!(multinomial, choose_exact(remaining, count));
        let next_pool = pool.add(*face, count);
        extend_multisets(
            &next_pool,
            rest,
            nsub!(remaining, count),
            next_multinomial,
            denominator,
            out,
        );
        if count == remaining {
            break;
        }
        count = nadd!(count, Nat::new(1));
    }
}

/// 不重复、不区分顺序：枚举 C(m, k) 个组合，每个组合概率 1 / C(m, k)（精确 Fraction）。
fn distinct_branches(pool: &DicePool, faces: &[DieFace], count: Nat) -> Vec<(DicePool, Fraction)> {
    let combinations = choose_exact(Nat::new(faces.len() as _), count);
    if combinations == ExactNat128::ZERO {
        return Vec::new();
    }
    let each = Fraction::new(ExactNat128::new(1), combinations);
    let mut out = Vec::new();
    extend_distinct(pool, faces, count, each, &mut out);
    out
}

fn extend_distinct(
    pool: &DicePool,
    faces: &[DieFace],
    need: Nat,
    probability: Fraction,
    out: &mut Vec<(DicePool, Fraction)>,
) {
    if need.get() == 0 {
        out.push((pool.clone(), probability));
        return;
    }
    let Some((face, rest)) = faces.split_first() else {
        return;
    };
    let rest_total = Nat::new(rest.len() as _);
    let after_take = nsub!(need, Nat::new(1));
    if after_take <= rest_total {
        let next = pool.add(*face, Nat::new(1));
        extend_distinct(&next, rest, after_take, probability, out);
    }
    if need <= rest_total {
        extend_distinct(pool, rest, need, probability, out);
    }
}

/// 组合数 C(n, k)（精确族；n <= 16 不会溢出）。
///
/// 递推 C(n, i) = C(n, i-1) * (n-i+1) / i：每一步的除法都是精确整除。
fn choose_exact(n: Nat, k: Nat) -> ExactNat128 {
    if k > n {
        return ExactNat128::ZERO;
    }
    let n_minus_k = nsub!(n, k);
    let mut result = ExactNat128::new(1);
    let mut index = Nat::new(1);
    while index <= k {
        let numerator = ExactNat128::new(nadd!(n_minus_k, index).get() as _);
        let divisor = ExactNat128::new(index.get() as _);
        result = ndiv!(nmul!(result, numerator), divisor);
        index = nadd!(index, Nat::new(1));
    }
    result
}

/// base^exponent（精确族）。
fn pow_exact(base: Nat, exponent: Nat) -> ExactNat128 {
    let base = ExactNat128::new(base.get() as _);
    let mut result = ExactNat128::new(1);
    let mut index = Nat::ZERO;
    while index < exponent {
        result = nmul!(result, base);
        index = nadd!(index, Nat::new(1));
    }
    result
}

// ======================================================================
// 测试
// ======================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use gi_genius_invokation_engine::rng::Rng;

    fn view(pool: &DicePool) -> crate::resources::dice::DiceSnapshot {
        pool.all_dice(&mut Rng::from_seed(0))
    }

    fn frac(num: Nat, den: Nat) -> Fraction {
        Fraction::new(
            ExactNat128::new(num.get() as _),
            ExactNat128::new(den.get() as _),
        )
        .exact_div(Fraction::ONE)
    }

    fn probability_total(branches: &[(DicePool, Fraction)]) -> Fraction {
        branches.iter().map(|(_, probability)| *probability).sum()
    }

    #[test]
    fn random_two_basic_folds_into_count_vectors() {
        let branches = DicePool::empty().generate_random(Nat::new(2), true, false);
        assert_eq!(branches.len(), 28);
        assert_eq!(probability_total(&branches), Fraction::ONE);
        for (pool, probability) in &branches {
            let snapshot = view(pool);
            let expected = if snapshot.known.len() == 1 {
                frac(Nat::new(1), Nat::new(49))
            } else {
                frac(Nat::new(2), Nat::new(49))
            };
            assert_eq!(*probability, expected);
        }
    }

    #[test]
    fn random_one_unrestricted_can_be_omni() {
        let branches = DicePool::empty().generate_random(Nat::new(1), false, false);
        assert_eq!(branches.len(), 8);
        for (_, probability) in &branches {
            assert_eq!(*probability, frac(Nat::new(1), Nat::new(8)));
        }
        assert!(branches
            .iter()
            .any(|(pool, _)| view(pool).omni == Nat::new(1)));
    }

    #[test]
    fn random_distinct_uses_combinations_and_checks_n() {
        let branches = DicePool::empty().generate_random(Nat::new(2), true, true);
        assert_eq!(branches.len(), 21);
        for (_, probability) in &branches {
            assert_eq!(*probability, frac(Nat::new(1), Nat::new(21)));
        }
        assert!(DicePool::empty()
            .generate_random(Nat::new(8), true, true)
            .is_empty());
        let all = DicePool::empty().generate_random(Nat::new(8), false, true);
        assert_eq!(all.len(), 1);
        assert_eq!(all.first().expect("一支").1, Fraction::ONE);
        let snapshot = view(&all.first().expect("一支").0);
        assert_eq!(snapshot.omni, Nat::new(1));
        assert_eq!(snapshot.known.len(), 7);
    }

    #[test]
    fn content_any_slot_is_random() {
        let content = [
            DiceSlot::Element(Element::Pyro),
            DiceSlot::Element(Element::Pyro),
            DiceSlot::Any,
        ];
        let branches = DicePool::empty().generate(&content, true);
        assert_eq!(branches.len(), 7);
        let mut any_turned_into_pyro = Nat::ZERO;
        for (pool, probability) in &branches {
            assert_eq!(*probability, frac(Nat::new(1), Nat::new(7)));
            assert_eq!(pool.total(), Nat::new(3));
            let snapshot = view(pool);
            let pyro = snapshot
                .known
                .iter()
                .find(|entry| entry.0 == Element::Pyro)
                .expect("有火");
            assert!(pyro.1 == Nat::new(2) || pyro.1 == Nat::new(3));
            if pyro.1 == Nat::new(3) {
                any_turned_into_pyro = nadd!(any_turned_into_pyro, Nat::new(1));
            }
        }
        assert_eq!(any_turned_into_pyro, Nat::new(1));
    }

    #[test]
    fn content_two_any_slots_fold() {
        let branches = DicePool::empty().generate(&[DiceSlot::Any, DiceSlot::Any], true);
        assert_eq!(branches.len(), 28);
        assert_eq!(probability_total(&branches), Fraction::ONE);
    }

    #[test]
    fn content_any_slot_respects_unrestricted_range() {
        let branches = DicePool::empty().generate(&[DiceSlot::Any], false);
        assert_eq!(branches.len(), 8);
        assert!(branches
            .iter()
            .any(|(pool, _)| view(pool).omni == Nat::new(1)));
    }

    #[test]
    fn content_slot_order_does_not_change_result() {
        let left = DicePool::empty().generate(
            &[
                DiceSlot::Element(Element::Pyro),
                DiceSlot::Any,
                DiceSlot::Any,
            ],
            true,
        );
        let right = DicePool::empty().generate(
            &[
                DiceSlot::Any,
                DiceSlot::Element(Element::Pyro),
                DiceSlot::Any,
            ],
            true,
        );
        assert_eq!(left.len(), right.len());
        for (pool, probability) in &left {
            let found = right
                .iter()
                .find(|(other, _)| *other == *pool)
                .expect("同一支");
            assert_eq!(*probability, found.1);
        }
    }

    #[test]
    fn generation_is_truncated_at_cap() {
        let pool = DicePool::empty().add(DieFace::Pyro, Nat::new(15));
        let branches = pool.generate_random(Nat::new(2), true, false);
        assert_eq!(branches.len(), 7);
        for (branch, probability) in &branches {
            assert_eq!(*probability, frac(Nat::new(1), Nat::new(7)));
            assert_eq!(branch.total(), DICE_CAP);
        }
    }

    #[test]
    fn generation_on_full_pool_is_single_noop_branch() {
        let pool = DicePool::empty().add(DieFace::Pyro, DICE_CAP);
        let branches = pool.generate_random(Nat::new(3), true, false);
        assert_eq!(branches.len(), 1);
        assert_eq!(branches.first().expect("一支").1, Fraction::ONE);
        assert_eq!(branches.first().expect("一支").0, pool);
    }

    #[test]
    fn zero_or_empty_generation_is_empty() {
        assert!(DicePool::empty()
            .generate_random(Nat::ZERO, true, false)
            .is_empty());
        assert!(DicePool::empty().generate(&[], true).is_empty());
    }

    #[test]
    fn fold_branches_merges_equivalent_pools() {
        let pyro = DicePool::empty().add(DieFace::Pyro, Nat::new(1));
        let hydro = DicePool::empty().add(DieFace::Hydro, Nat::new(1));
        let folded = fold_branches(vec![
            (pyro.clone(), frac(Nat::new(1), Nat::new(4))),
            (hydro, frac(Nat::new(1), Nat::new(2))),
            (pyro.clone(), frac(Nat::new(1), Nat::new(4))),
        ]);
        assert_eq!(folded.len(), 2);
        let merged = folded
            .iter()
            .find(|(pool, _)| *pool == pyro)
            .expect("有合并支");
        assert_eq!(merged.1, frac(Nat::new(1), Nat::new(2)));
    }
}