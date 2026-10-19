//! 真实随机数（决策日志 D149 / D151 / D160）：**随机状态归引擎**。
//!
//! 分工（D149）：
//! - **枚举不消费 PRNG**：[`crate::random::enumerate_random`] 是纯函数，不碰本模块（D1）；
//! - **只有采样 / 落一单才消费**：调用方先自己算好精确分布（[`crate::prob`]），
//!   再用 [`Rng::next_u128`] 取 `[0, 2^128)` 随机整数、用 [`crate::prob::pick_index`] 落一单。
//!
//! 对外接口只有 **128 位整数**（D160）：概率质量是 `ExactNat128`，抽样就是在 128 位区间上
//! 均匀取；64 位随机数要放大 / 缩放会带偏差。内部实现是 SplitMix64（两个 64 位拼一个 128 位），
//! 确定性、可复现、**零依赖**；种子由调用方传（[`Rng::from_seed`]）。

/// 引擎的随机源（D149 / D151 / D160）：内部状态用 SplitMix64，对外只给 128 位随机整数。
///
/// 枚举**不消费**它；只有采样才 `&mut` 进来。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// 从种子建随机源：**同一 seed -> 同一序列**（可复现）。
    pub const fn from_seed(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// 取一个原始 64 位随机数（SplitMix64 一步；内部积木，**不是对外随机接口**）。
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 取 `[0, 2^128)` 的均匀随机整数（两个 64 位拼成；D160 的对外随机接口）。
    pub fn next_u128(&mut self) -> u128 {
        let high = self.next_u64() as u128;
        let low = self.next_u64() as u128;
        (high << 64) | low
    }

    /// 取 `[0, n)` 的均匀随机整数（**拒绝采样**，无取模偏差；D160）。
    ///
    /// `n == 0` 是逻辑错误（`[0,0)` 无定义）-> panic，与除零同一口径。
    pub fn next_below_u128(&mut self, n: u128) -> u128 {
        assert!(n != 0, "取 [0,0) 无定义：next_below_u128 的 n 为 0（逻辑错误）");
        // 2^128 % n = (-n) % n：落在 [0, threshold) 的样本会引入偏差，丢弃重取。
        let threshold = n.wrapping_neg() % n;
        loop {
            let value = self.next_u128();
            if value >= threshold {
                return value % n;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_seed_is_deterministic() {
        let mut a = Rng::from_seed(42);
        let mut b = Rng::from_seed(42);
        for _ in 0..64 {
            assert_eq!(a.next_u128(), b.next_u128());
        }
        let mut c = Rng::from_seed(43);
        let mut d = Rng::from_seed(42);
        assert_ne!(c.next_u128(), d.next_u128());
    }

    #[test]
    fn next_u64_matches_splitmix64_reference_vectors() {
        // SplitMix64 参考输出（seed = 0），锁死算法不被无意改掉。
        let mut rng = Rng::from_seed(0);
        let expected = [
            0xE220_A839_7B1D_CDAF,
            0x6E78_9E6A_A1B9_65F4,
            0x06C4_5D18_8009_454F,
            0xF88B_B8A8_724C_81EC,
        ];
        for value in expected {
            assert_eq!(rng.next_u64(), value);
        }
    }

    #[test]
    fn next_u128_combines_two_64_bit_words() {
        // seed = 0 的前两个 64 位输出拼成 128 位（高 64 位在前）。
        let mut rng = Rng::from_seed(0);
        assert_eq!(rng.next_u128(), 0xE220_A839_7B1D_CDAF_6E78_9E6A_A1B9_65F4);
    }

    #[test]
    fn next_below_u128_stays_in_range() {
        let mut rng = Rng::from_seed(11);
        let cases = [1_u128, 2, 3, 6, 10, 12345, u64::MAX as u128, u128::MAX];
        for n in cases {
            for _ in 0..1000 {
                assert!(rng.next_below_u128(n) < n, "next_below_u128({n}) 越界");
            }
        }
        for _ in 0..16 {
            assert_eq!(rng.next_below_u128(1), 0);
        }
    }

    #[test]
    fn next_below_u128_covers_every_value_for_small_n() {
        let mut rng = Rng::from_seed(2024);
        let mut seen = [false; 6];
        for _ in 0..1000 {
            let value = rng.next_below_u128(6) as usize;
            assert!(value < 6);
            seen[value] = true;
        }
        assert!(seen.iter().all(|hit| *hit), "6 个值都应被抽到过");
    }

    #[test]
    #[should_panic(expected = "无定义")]
    fn next_below_u128_zero_panics() {
        let mut rng = Rng::from_seed(0);
        let _ = rng.next_below_u128(0);
    }

    #[test]
    fn clone_starts_from_same_state() {
        let mut original = Rng::from_seed(5);
        let _ = original.next_u128();
        let mut clone = original.clone();
        assert_eq!(original.next_u128(), clone.next_u128());
        assert_eq!(original.next_u128(), clone.next_u128());
    }
}
