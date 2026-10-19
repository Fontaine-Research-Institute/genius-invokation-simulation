//! 手牌统计（D187）：集中放手牌这块的统计，执行方在事情发生时自己填。
//!
//! - 对外只读指针（D183）：只能拿 &Counter 读；
//! - **整体可裁剪**（feature = "stats"）：关掉时零大小空结构、更新全空操作；
//! - 统计**不参与局面等价**（HandState 的 PartialEq 手动实现里排除 stats）。

use gi_genius_invokation_engine::Nat;

/// 手牌统计（feature = "stats" 打开时才有真实字段）。
#[cfg(feature = "stats")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandStats {
    added: gi_genius_invokation_engine::stats::Counter,
    discarded: gi_genius_invokation_engine::stats::Counter,
    opening_drawn: gi_genius_invokation_engine::stats::Counter,
}

/// 手牌统计（feature 关闭时：零大小空结构，更新全是空操作）。
#[cfg(not(feature = "stats"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HandStats;

#[cfg(feature = "stats")]
impl HandStats {
    /// 全零统计（常量初始值）。
    pub const EMPTY: HandStats = HandStats {
        added: gi_genius_invokation_engine::stats::Counter::ZERO,
        discarded: gi_genius_invokation_engine::stats::Counter::ZERO,
        opening_drawn: gi_genius_invokation_engine::stats::Counter::ZERO,
    };

    /// 累计 add 入手张数 +amount。
    pub fn on_added(&mut self, amount: Nat) {
        self.added.add(amount);
    }

    /// 累计舍弃张数 +amount。
    pub fn on_discarded(&mut self, amount: Nat) {
        self.discarded.add(amount);
    }

    /// 起手阶段抓到的张数 +amount。
    pub fn on_opening_drawn(&mut self, amount: Nat) {
        self.opening_drawn.add(amount);
    }

    /// 累计 add 入手张数（只读指针，D183）。
    pub fn added(&self) -> &gi_genius_invokation_engine::stats::Counter {
        &self.added
    }

    /// 累计舍弃张数（只读指针）。
    pub fn discarded(&self) -> &gi_genius_invokation_engine::stats::Counter {
        &self.discarded
    }

    /// 起手阶段抓到的张数（只读指针）。
    pub fn opening_drawn(&self) -> &gi_genius_invokation_engine::stats::Counter {
        &self.opening_drawn
    }
}

#[cfg(not(feature = "stats"))]
impl HandStats {
    /// 空统计（常量初始值）。
    pub const EMPTY: HandStats = HandStats;

    /// 空操作（feature 关闭）。
    pub fn on_added(&mut self, _amount: Nat) {}

    /// 空操作（feature 关闭）。
    pub fn on_discarded(&mut self, _amount: Nat) {}

    /// 空操作（feature 关闭）。
    pub fn on_opening_drawn(&mut self, _amount: Nat) {}

    /// 静态 0 计数器。
    pub fn added(&self) -> &gi_genius_invokation_engine::stats::Counter {
        static ZERO: gi_genius_invokation_engine::stats::Counter =
            gi_genius_invokation_engine::stats::Counter::ZERO;
        &ZERO
    }

    /// 静态 0 计数器。
    pub fn discarded(&self) -> &gi_genius_invokation_engine::stats::Counter {
        static ZERO: gi_genius_invokation_engine::stats::Counter =
            gi_genius_invokation_engine::stats::Counter::ZERO;
        &ZERO
    }

    /// 静态 0 计数器。
    pub fn opening_drawn(&self) -> &gi_genius_invokation_engine::stats::Counter {
        static ZERO: gi_genius_invokation_engine::stats::Counter =
            gi_genius_invokation_engine::stats::Counter::ZERO;
        &ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "stats")]
    #[test]
    fn counters_read_through_shared_reference() {
        let mut stats = HandStats::EMPTY;
        stats.on_added(Nat::new(2));
        stats.on_discarded(Nat::new(1));
        stats.on_opening_drawn(Nat::new(5));
        let read: &HandStats = &stats;
        assert_eq!(read.added().get(), Nat::new(2));
        assert_eq!(read.discarded().get(), Nat::new(1));
        assert_eq!(read.opening_drawn().get(), Nat::new(5));
    }

    #[cfg(not(feature = "stats"))]
    #[test]
    fn disabled_stats_are_noops() {
        let mut stats = HandStats::EMPTY;
        stats.on_added(Nat::new(5));
        assert!(stats.added().is_zero());
        assert_eq!(std::mem::size_of::<HandStats>(), 0);
    }
}
