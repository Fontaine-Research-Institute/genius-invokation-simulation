//! 分数（决策日志 D150 / D152 / D153 / D156 / D158 / D159）：概率的「分子 / 分母」。
//!
//! 口径：
//! - 分子 / 分母都是 [`ExactNat128`]（D152：连乘 / 通分不乱饱和；D153：精确族，溢出一律 panic）；
//! - **分母为 0 是逻辑错误**：与引擎除法（`Div` / `ndiv!`）同一口径，[`Fraction::new`] 直接 panic；
//! - 加 / 乘 / 除**全部精确**、自动约分（避免精确累加时分子分母指数膨胀）；溢出一律 panic；
//! - **概率相关的数值一律精确**（权重、概率质量、累计量）。引擎内部与跨 crate 接口都用
//!   `Fraction`，**不使用浮点**（D158 / D159）。
//!
//! 字段私有：**唯一构造入口是 [`Fraction::new`]**，这样「分母不为 0」是类型不变量，
//! 任何调用方都造不出分母为 0 的分数。

use crate::nat::ExactNat128;
use std::fmt;
use std::iter::Sum;
use std::ops::{Add, Div, Mul};

/// 求最大公约数（给分数约分用；`gcd(a, 0) = a`）。
const fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a
}

/// 约分：分子分母同除 `gcd`。分母恒 > 0，所以 `gcd` 不会为 0。
const fn reduce(num: ExactNat128, den: ExactNat128) -> Fraction {
    let divisor = gcd(num.get(), den.get());
    Fraction {
        num: ExactNat128::new(num.get() / divisor),
        den: ExactNat128::new(den.get() / divisor),
    }
}

/// 两个 `u128` 相乘的**高 128 位**（用于把「比例比较」放大到 128 位区间，不经过浮点）。
///
/// 只算高位：`hi = floor(a*b / 2^128)`，用 64 位分块做精确乘法。
const fn mul_u128_hi(a: u128, b: u128) -> u128 {
    const MASK: u128 = (1_u128 << 64) - 1;
    let a_low = a & MASK;
    let a_high = a >> 64;
    let b_low = b & MASK;
    let b_high = b >> 64;
    let p0 = a_low * b_low;
    let p1 = a_low * b_high;
    let p2 = a_high * b_low;
    let p3 = a_high * b_high;
    let middle = (p0 >> 64) + (p1 & MASK) + (p2 & MASK);
    p3 + (p1 >> 64) + (p2 >> 64) + (middle >> 64)
}

/// 分数：`num / den`（分子 / 分母，均 `ExactNat128`；D150 / D152 / D153 / D158）。
///
/// 用 [`Fraction::new`] 构造；分母为 0 会 panic。
/// 加 / 乘 / 除的结果**自动约分**（如 `2/4` 会变成 `1/2`），所以可直接用结构化相等比较值。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fraction {
    num: ExactNat128,
    den: ExactNat128,
}

impl Fraction {
    /// 零：`0 / 1`（加法单位元）。
    pub const ZERO: Fraction = Fraction {
        num: ExactNat128::ZERO,
        den: ExactNat128::new(1),
    };

    /// 一：`1 / 1`（乘法单位元）。
    pub const ONE: Fraction = Fraction {
        num: ExactNat128::new(1),
        den: ExactNat128::new(1),
    };

    /// 构造 `num / den`（**不约分**；需要规范化时用加 / 乘 / 除的结果）。
    ///
    /// **分母为 0 -> panic**（与引擎除法一致：除零是逻辑错误，不是饱和业务）。
    pub const fn new(num: ExactNat128, den: ExactNat128) -> Fraction {
        assert!(
            den.get() != 0,
            "除零：分数分母为 0（与引擎除法一致，这是逻辑错误）"
        );
        Fraction { num, den }
    }

    /// 分子。
    pub const fn num(self) -> ExactNat128 {
        self.num
    }

    /// 分母（保证 > 0）。
    pub const fn den(self) -> ExactNat128 {
        self.den
    }

    /// 数值是否为 0。
    pub const fn is_zero(self) -> bool {
        self.num.get() == 0
    }

    /// 数值是否为 1（分母恒 > 0，所以 `num == den` 就是值 1）。
    pub const fn is_one(self) -> bool {
        self.num.get() == self.den.get()
    }

    /// 精确加：`a/b + c/d = (a*d + c*b) / (b*d)`，结果**约分**；**上溢 panic**（D153）。
    pub const fn exact_add(self, rhs: Fraction) -> Fraction {
        let num = self
            .num
            .exact_mul(rhs.den)
            .exact_add(rhs.num.exact_mul(self.den));
        let den = self.den.exact_mul(rhs.den);
        reduce(num, den)
    }

    /// 精确乘：`(a/b) * (c/d) = (a*c) / (b*d)`，结果**约分**；**上溢 panic**（D153）。
    pub const fn exact_mul(self, rhs: Fraction) -> Fraction {
        reduce(
            self.num.exact_mul(rhs.num),
            self.den.exact_mul(rhs.den),
        )
    }

    /// 精确除：`(a/b) / (c/d) = (a*d) / (b*c)`，结果**约分**。
    ///
    /// **除数分子为 0 -> panic**（除零是逻辑错误）；上溢同样 panic（D153）。
    pub const fn exact_div(self, rhs: Fraction) -> Fraction {
        assert!(
            rhs.num.get() != 0,
            "除零：分数除数的分子为 0（与引擎除法一致，这是逻辑错误）"
        );
        reduce(
            self.num.exact_mul(rhs.den),
            self.den.exact_mul(rhs.num),
        )
    }

    /// 精确比较两个分数的**值**（交叉相乘；乘积用 128×128 -> 256 位，不会溢出）。
    pub fn cmp_value(self, other: Fraction) -> std::cmp::Ordering {
        compare_scaled(self.num.get(), self.den.get(), other.num.get(), other.den.get())
    }
}

/// 比较 `left_num / left_den` 与 `right_num / right_den`（全部 > 0），用 256 位精确乘对比。
fn compare_scaled(left_num: u128, left_den: u128, right_num: u128, right_den: u128) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // left_num * right_den  vs  right_num * left_den
    let left_hi = mul_u128_hi(left_num, right_den);
    let left_lo = left_num.wrapping_mul(right_den);
    let right_hi = mul_u128_hi(right_num, left_den);
    let right_lo = right_num.wrapping_mul(left_den);
    match left_hi.cmp(&right_hi) {
        Ordering::Equal => left_lo.cmp(&right_lo),
        other => other,
    }
}

impl Add for Fraction {
    type Output = Fraction;

    #[inline]
    fn add(self, rhs: Fraction) -> Fraction {
        self.exact_add(rhs)
    }
}

impl Mul for Fraction {
    type Output = Fraction;

    #[inline]
    fn mul(self, rhs: Fraction) -> Fraction {
        self.exact_mul(rhs)
    }
}

impl Div for Fraction {
    type Output = Fraction;

    /// 分数除法：**除数分子为 0 -> panic**；结果约分。
    #[inline]
    fn div(self, rhs: Fraction) -> Fraction {
        self.exact_div(rhs)
    }
}

impl Sum for Fraction {
    /// 累加：空迭代器 -> [`Fraction::ZERO`]。
    fn sum<I: Iterator<Item = Fraction>>(iter: I) -> Fraction {
        iter.fold(Fraction::ZERO, Fraction::exact_add)
    }
}

impl<'a> Sum<&'a Fraction> for Fraction {
    /// 按引用累加（方便直接对切片求和）。
    fn sum<I: Iterator<Item = &'a Fraction>>(iter: I) -> Fraction {
        iter.fold(Fraction::ZERO, |acc, fraction| acc.exact_add(*fraction))
    }
}

impl fmt::Display for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.num, self.den)
    }
}

impl fmt::Debug for Fraction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fraction({}/{})", self.num, self.den)
    }
}

/// 一组分数的和（精确族累加，溢出一律 panic；空切片 -> [`Fraction::ZERO`]）。
pub fn total(fractions: &[Fraction]) -> Fraction {
    fractions.iter().sum()
}

/// 一组分数的**归一化**（D150 / D156）：按各自占比变成**精确分数**（Σ = 1）。
///
/// 返回 `None` 的情况（**不 panic**，与引擎其它边界行为一致的「返回空」风格）：
/// - 空切片；
/// - 分数之和为 0（所有分子都是 0：归一化无定义）。
///
/// 内部用 [`Fraction::exact_div`] 精确相除并约分；**不经过浮点**（D158 / D159）。
pub fn normalize(fractions: &[Fraction]) -> Option<Vec<Fraction>> {
    let total = total(fractions);
    if total.is_zero() {
        return None;
    }
    Some(
        fractions
            .iter()
            .map(|fraction| fraction.exact_div(total))
            .collect(),
    )
}

/// 按分数占比选一个下标：`unit` 是 `[0, 2^128)` 的均匀随机整数（由 [`crate::rng::Rng`] 取）。
///
/// 语义：把归一化后的累计概率映射到 128 位整数区间（`unit / 2^128`），取第一个累计占比
/// 严格大于该点的小标。全程整数比较（把比例放大到 256 位做交叉相乘），不经过浮点。
/// 量化误差 < `2^-128`（128 位区间的必然精度；实际可忽略）。
///
/// 返回 `None` 的情况与 [`normalize`] 相同（空列表 / 分数之和为 0）。
/// 概率为 0 的项永远不会被选中（累计值不变）。
pub fn pick_index(fractions: &[Fraction], unit: u128) -> Option<usize> {
    let total = total(fractions);
    if total.is_zero() {
        return None;
    }
    let mut cumulative = Fraction::ZERO;
    for (index, fraction) in fractions.iter().enumerate() {
        cumulative = cumulative.exact_add(*fraction);
        // 比较「累计占比 p = cumulative / total」与「unit / 2^128」：
        //   p > unit / 2^128  <=>  p.num * 2^128 > unit * p.den。
        // 左边 = (高 128 位 p.num, 低 128 位 0)；右边 = unit * p.den 的 256 位乘积。
        // 高位比较即可（低位左边是 0，永远不会更大）。
        let probability = cumulative.exact_div(total);
        if probability.num().get() > mul_u128_hi(unit, probability.den().get()) {
            return Some(index);
        }
    }
    Some(fractions.len() - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frac(num: u128, den: u128) -> Fraction {
        Fraction::new(ExactNat128::new(num), ExactNat128::new(den))
    }

    #[test]
    fn new_keeps_num_and_den() {
        let fraction = frac(3, 4);
        assert_eq!(fraction.num(), ExactNat128::new(3));
        assert_eq!(fraction.den(), ExactNat128::new(4));
        assert_eq!(Fraction::ZERO, frac(0, 1));
        assert_eq!(Fraction::ONE, frac(1, 1));
        assert!(Fraction::ZERO.is_zero());
        assert!(!Fraction::ONE.is_zero());
        assert!(Fraction::ONE.is_one());
        assert!(frac(3, 3).is_one());
        assert!(!frac(2, 3).is_one());
    }

    #[test]
    #[should_panic(expected = "除零")]
    fn zero_denominator_panics() {
        let _ = frac(1, 0);
    }

    #[test]
    fn add_is_exact_and_reduced() {
        assert_eq!(frac(1, 2).exact_add(frac(1, 3)), frac(5, 6));
        assert_eq!(frac(1, 2) + Fraction::ZERO, frac(1, 2));
        assert_eq!(frac(1, 4) + frac(1, 4), frac(1, 2));
        let third = frac(1, 3);
        assert!((third + third + third).is_one());
    }

    #[test]
    fn mul_is_exact_and_reduced() {
        assert_eq!(frac(1, 2).exact_mul(frac(2, 3)), frac(1, 3));
        assert_eq!(frac(3, 7) * Fraction::ONE, frac(3, 7));
        assert!((frac(6, 9) * frac(3, 2)).is_one());
    }

    #[test]
    fn div_is_exact_and_reduced() {
        assert_eq!(frac(1, 2).exact_div(frac(3, 4)), frac(2, 3));
        assert!((frac(1, 2) / frac(1, 2)).is_one());
    }

    #[test]
    #[should_panic(expected = "除零")]
    fn div_by_zero_numerator_panics() {
        let _ = frac(1, 2).exact_div(frac(0, 3));
    }

    #[test]
    #[should_panic(expected = "溢出")]
    fn add_overflow_panics() {
        let big = Fraction::new(ExactNat128::MAX, ExactNat128::new(1));
        let _ = big + big;
    }

    #[test]
    #[should_panic(expected = "溢出")]
    fn mul_overflow_panics() {
        let big = Fraction::new(ExactNat128::MAX, ExactNat128::new(2));
        let _ = big * big;
    }

    #[test]
    fn nat128_does_not_overflow_where_nat64_would() {
        let wide = u64::MAX as u128;
        let product = frac(wide, 1) * frac(wide, 1);
        assert_eq!(product, frac(wide * wide, 1));
        assert_eq!(product.num(), ExactNat128::new(wide * wide));
    }

    #[test]
    fn cmp_value_is_exact() {
        use std::cmp::Ordering;
        assert_eq!(frac(1, 2).cmp_value(frac(1, 3)), Ordering::Greater);
        assert_eq!(frac(1, 3).cmp_value(frac(2, 6)), Ordering::Equal);
        assert_eq!(frac(1, 4).cmp_value(frac(1, 3)), Ordering::Less);
        assert_eq!(frac(2, 3).cmp_value(frac(1, 2)), Ordering::Greater);
    }

    #[test]
    fn sum_and_total() {
        let fractions = [frac(1, 2), frac(1, 3)];
        assert_eq!(total(&fractions), frac(5, 6));
        assert_eq!(total(&[]), Fraction::ZERO);
        let by_iter: Fraction = fractions.into_iter().sum();
        assert_eq!(by_iter, frac(5, 6));
        let by_ref: Fraction = fractions.iter().sum();
        assert_eq!(by_ref, frac(5, 6));
    }

    #[test]
    fn normalize_gives_exact_proportions_summing_to_one() {
        let probabilities = normalize(&[frac(1, 4), frac(3, 4)]).expect("非零分数");
        assert_eq!(probabilities, vec![frac(1, 4), frac(3, 4)]);
        assert!((probabilities.iter().sum::<Fraction>()).is_one());

        let probabilities = normalize(&[frac(1, 2), frac(1, 3)]).expect("非零分数");
        assert_eq!(probabilities, vec![frac(3, 5), frac(2, 5)]);
        assert!((probabilities.iter().sum::<Fraction>()).is_one());

        let probabilities = normalize(&[frac(1, 7), frac(1, 7), frac(1, 7)]).expect("非零分数");
        assert_eq!(probabilities, vec![frac(1, 3); 3]);
        assert!((probabilities.iter().sum::<Fraction>()).is_one());
    }

    #[test]
    fn normalize_rejects_empty_and_zero_totals() {
        assert_eq!(normalize(&[]), None);
        assert_eq!(normalize(&[frac(0, 5), frac(0, 7)]), None);
    }

    #[test]
    fn pick_index_follows_cumulative_distribution() {
        use std::cmp::Ordering;
        let fractions = [frac(1, 4), frac(3, 4)];
        assert_eq!(pick_index(&fractions, 0), Some(0));
        // 累计 1/4 的边界：unit = 2^126-1 -> 0.2499...；unit = 2^126 -> 正好 0.25
        let boundary = 1_u128 << 126;
        assert_eq!(pick_index(&fractions, boundary - 1), Some(0));
        assert_eq!(pick_index(&fractions, boundary), Some(1));
        assert_eq!(pick_index(&fractions, u128::MAX), Some(1));
        assert_eq!(pick_index(&[], 0), None);
        assert_eq!(pick_index(&[frac(0, 3)], 0), None);

        // 概率为 0 的项永远不会被选中
        let with_zero = [frac(0, 1), frac(1, 1)];
        assert_eq!(pick_index(&with_zero, 0), Some(1));

        // 分数之和不是 1 时也按占比：1/2、1/4 -> 2/3、1/3
        let uneven = [frac(1, 2), frac(1, 4)];
        let half = 1_u128 << 127;
        assert_eq!(pick_index(&uneven, half), Some(0));
        let three_quarters = (1_u128 << 127) + (1_u128 << 126);
        assert_eq!(pick_index(&uneven, three_quarters), Some(1));

        // cmp_value 与 pick_index 用的是同一套精确比较
        assert_eq!(frac(1, 2).cmp_value(frac(2, 3)), Ordering::Less);
    }

    #[test]
    fn display_and_debug() {
        assert_eq!(frac(3, 4).to_string(), "3/4");
        assert_eq!(format!("{:?}", frac(3, 4)), "Fraction(3/4)");
    }
}
