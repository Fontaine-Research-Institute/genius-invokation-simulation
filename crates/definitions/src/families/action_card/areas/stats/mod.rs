//! 行动牌区域承载的统计（D187）：支援区 / 手牌卡状态 / 当前费用各自的统计，
//! 执行方在事情发生时自己填。
//!
//! - 对外只读指针（D183）：只能拿 `&Counter` 读；
//! - **整体可裁剪**（feature = "stats"）：关掉时零大小空结构、更新全空操作；
//! - 统计**不参与局面等价**（各区 `PartialEq` 手动实现里排除 stats）。

/// 定义一组「区统计」：feature 打开时是若干 `Counter` 字段，关闭时是零大小结构。
macro_rules! zone_stats {
    (
        $(
            $(#[$meta:meta])*
            $name:ident {
                $( $field:ident => ($on:ident, $get:ident) : $doc:literal ),* $(,)?
            }
        )*
    ) => {
        $(
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
        )*
    };
}

zone_stats!(
    /// 支援区统计（规则书·游戏区域 III）。
    SupportStats {
        entered => (on_entered, entered) : "累计「进入支援区」的支援牌张数 +amount。",
        discarded => (on_discarded, discarded) : "累计「离开支援区（弃置）」的支援牌张数 +amount（含满 4 时选择弃置的那张）。",
    }
    /// 手牌卡状态统计。
    HandCardStateStats {
        added => (on_added, added) : "累计「给手牌卡加上状态」的层数 / 次数 +amount。",
        removed => (on_removed, removed) : "累计「从手牌卡移除状态」的次数 +amount。",
    }
);

#[cfg(test)]
mod tests {
    use gi_genius_invokation_engine::Nat;

    use super::*;

    #[test]
    fn stats_read_as_counters() {
        let mut support = SupportStats::EMPTY;
        support.on_entered(Nat::new(2));
        support.on_discarded(Nat::new(1));

        let mut hand = HandCardStateStats::EMPTY;
        hand.on_added(Nat::new(3));
        // 只读指针不改原值。
        let read: &HandCardStateStats = &hand;
        assert_eq!(read.removed().get(), Nat::ZERO);

        if cfg!(feature = "stats") {
            assert_eq!(support.entered().get(), Nat::new(2));
            assert_eq!(support.discarded().get(), Nat::new(1));
            assert_eq!(hand.added().get(), Nat::new(3));
        } else {
            assert!(support.entered().is_zero());
            assert!(support.discarded().is_zero());
            assert!(hand.added().is_zero());
        }
    }

    #[test]
    fn stats_size_follows_feature_flag() {
        if cfg!(feature = "stats") {
            assert!(std::mem::size_of::<SupportStats>() > 0);
        } else {
            assert_eq!(std::mem::size_of::<SupportStats>(), 0);
            let mut stats = SupportStats::EMPTY;
            stats.on_entered(Nat::new(9));
            assert!(stats.entered().is_zero());
        }
    }
}
