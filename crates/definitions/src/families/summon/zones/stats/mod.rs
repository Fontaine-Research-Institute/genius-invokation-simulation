//! 召唤物区统计（D187）：集中放召唤物区的统计，执行方在事情发生时自己填。
//!
//! - 对外只读指针（D183）：只能拿 `&Counter` 读；
//! - **整体可裁剪**（feature = "stats"）：关掉时零大小空结构、更新全空操作；
//! - 统计**不参与局面等价**（`SummonZone` 的 `PartialEq` 手动实现里排除 stats）。

/// 定义一组「区统计」：feature 打开时是若干 `Counter` 字段，关闭时是零大小结构。
macro_rules! zone_stats {
    (
        $(#[$meta:meta])*
        $name:ident {
            $( $field:ident => ($on:ident, $get:ident) : $doc:literal ),* $(,)?
        }
    ) => {
        /// 统计（feature = "stats" 打开时才有真实字段）。
        #[cfg(feature = "stats")]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        $(#[$meta])*
        pub struct $name {
            $( $field: gi_genius_invokation_engine::stats::Counter, )*
        }

        #[cfg(feature = "stats")]
        impl $name {
            /// 全零统计（常量初始值）。
            pub const EMPTY: $name = $name {
                $( $field: gi_genius_invokation_engine::stats::Counter::ZERO, )*
            };

            $(
                #[doc = $doc]
                pub fn $on(&mut self, amount: gi_genius_invokation_engine::Nat) {
                    self.$field.add(amount);
                }

                #[doc = $doc]
                pub fn $get(&self) -> &gi_genius_invokation_engine::stats::Counter {
                    &self.$field
                }
            )*
        }

        /// 统计（feature 关闭时：零大小空结构，更新全是空操作）。
        #[cfg(not(feature = "stats"))]
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        $(#[$meta])*
        pub struct $name;

        #[cfg(not(feature = "stats"))]
        impl $name {
            /// 空统计（常量初始值）。
            pub const EMPTY: $name = $name;

            $(
                #[doc = $doc]
                pub fn $on(&mut self, _amount: gi_genius_invokation_engine::Nat) {}

                #[doc = $doc]
                pub fn $get(&self) -> &gi_genius_invokation_engine::stats::Counter {
                    static ZERO: gi_genius_invokation_engine::stats::Counter =
                        gi_genius_invokation_engine::stats::Counter::ZERO;
                    &ZERO
                }
            )*
        }
    };
}

zone_stats!(
    /// 召唤物区统计（规则书·游戏区域 IV）。
    SummonZoneStats {
        summoned => (on_summoned, summoned) : "累计成功生成（占新位置）的召唤物数量 +amount。",
        merged => (on_merged, merged) : "累计「同名召唤物合并」的次数 +amount。",
        rejected_full => (on_rejected_full, rejected_full) : "累计「召唤物区已满、无法生成」的次数 +amount。",
        triggered => (on_triggered, triggered) : "累计召唤物效果触发（可用次数递减）的次数 +amount。",
        depleted => (on_depleted, depleted) : "累计「可用次数耗尽被弃置」的召唤物数量 +amount。",
        discarded => (on_discarded, discarded) : "累计被指定弃置的召唤物数量 +amount。",
    }
);

#[cfg(test)]
mod tests {
    use gi_genius_invokation_engine::Nat;

    use super::*;

    #[test]
    fn stats_read_as_counters() {
        let mut stats = SummonZoneStats::EMPTY;
        stats.on_summoned(Nat::new(2));
        stats.on_triggered(Nat::new(3));
        // 只读指针不改原值。
        let read: &SummonZoneStats = &stats;
        assert_eq!(read.merged().get(), Nat::ZERO);
        if cfg!(feature = "stats") {
            assert_eq!(stats.summoned().get(), Nat::new(2));
            assert_eq!(stats.triggered().get(), Nat::new(3));
        } else {
            assert!(stats.summoned().is_zero());
            assert!(stats.triggered().is_zero());
        }
    }

    #[test]
    fn stats_size_follows_feature_flag() {
        if cfg!(feature = "stats") {
            assert!(std::mem::size_of::<SummonZoneStats>() > 0);
        } else {
            assert_eq!(std::mem::size_of::<SummonZoneStats>(), 0);
            let mut stats = SummonZoneStats::EMPTY;
            stats.on_summoned(Nat::new(9));
            assert!(stats.summoned().is_zero());
        }
    }
}
