//! 牌库统计（D187）：集中放牌库这块的统计，执行方在事情发生时自己填。
//!
//! - 对外只读指针（D183）：只能拿 &Counter 读，拿不到 &mut、拿不到字段；
//! - **整体可裁剪**（feature = "stats"）：关掉时本类型是零大小空结构，
//!   所有 on_* 更新都是空操作（不产生运算），读接口返回静态的 0 计数器；
//! - 统计**不参与局面等价**（PileState 的 PartialEq 手动实现里排除 stats）。

#[cfg(feature = "stats")]
use gi_genius_invokation_engine::stats::Counter;
#[cfg(not(feature = "stats"))]
use gi_genius_invokation_engine::stats::Counter;
use gi_genius_invokation_engine::Nat;

/// 牌库统计（feature = "stats" 打开时才有真实字段）。
#[cfg(feature = "stats")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PileStats {
    drawn: Counter,
    discarded: Counter,
    inserted: Counter,
    put_bottom: Counter,
}

/// 牌库统计（feature 关闭时：零大小空结构，更新全是空操作）。
#[cfg(not(feature = "stats"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PileStats;

#[cfg(feature = "stats")]
impl PileStats {
    /// 全零统计（常量初始值）。
    pub const EMPTY: PileStats = PileStats {
        drawn: Counter::ZERO,
        discarded: Counter::ZERO,
        inserted: Counter::ZERO,
        put_bottom: Counter::ZERO,
    };

    /// 累计抽到张数 +amount。
    pub fn on_drawn(&mut self, amount: Nat) {
        self.drawn.add(amount);
    }

    /// 累计舍弃张数 +amount。
    pub fn on_discarded(&mut self, amount: Nat) {
        self.discarded.add(amount);
    }

    /// 累计置入 / 洗入张数 +amount。
    pub fn on_inserted(&mut self, amount: Nat) {
        self.inserted.add(amount);
    }

    /// 累计置底次数 +1。
    pub fn on_put_bottom(&mut self) {
        self.put_bottom.add(Nat::new(1));
    }

    /// 累计抽到张数（只读指针，D183）。
    pub fn drawn(&self) -> &Counter {
        &self.drawn
    }

    /// 累计舍弃张数（只读指针）。
    pub fn discarded(&self) -> &Counter {
        &self.discarded
    }

    /// 累计置入 / 洗入张数（只读指针）。
    pub fn inserted(&self) -> &Counter {
        &self.inserted
    }

    /// 累计置底次数（只读指针）。
    pub fn put_bottom(&self) -> &Counter {
        &self.put_bottom
    }
}

#[cfg(not(feature = "stats"))]
impl PileStats {
    /// 空统计（常量初始值）。
    pub const EMPTY: PileStats = PileStats;

    /// 空操作（feature 关闭）。
    pub fn on_drawn(&mut self, _amount: Nat) {}

    /// 空操作（feature 关闭）。
    pub fn on_discarded(&mut self, _amount: Nat) {}

    /// 空操作（feature 关闭）。
    pub fn on_inserted(&mut self, _amount: Nat) {}

    /// 空操作（feature 关闭）。
    pub fn on_put_bottom(&mut self) {}

    /// 静态 0 计数器（只读指针；feature 关闭时没有真实存储）。
    pub fn drawn(&self) -> &Counter {
        static ZERO: Counter = Counter::ZERO;
        &ZERO
    }

    /// 静态 0 计数器。
    pub fn discarded(&self) -> &Counter {
        static ZERO: Counter = Counter::ZERO;
        &ZERO
    }

    /// 静态 0 计数器。
    pub fn inserted(&self) -> &Counter {
        static ZERO: Counter = Counter::ZERO;
        &ZERO
    }

    /// 静态 0 计数器。
    pub fn put_bottom(&self) -> &Counter {
        static ZERO: Counter = Counter::ZERO;
        &ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "stats")]
    #[test]
    fn counters_read_through_shared_reference() {
        let mut stats = PileStats::EMPTY;
        stats.on_drawn(Nat::new(2));
        stats.on_drawn(Nat::new(1));
        stats.on_discarded(Nat::new(1));
        stats.on_inserted(Nat::new(3));
        stats.on_put_bottom();
        let read: &PileStats = &stats;
        assert_eq!(read.drawn().get(), Nat::new(3));
        assert_eq!(read.discarded().get(), Nat::new(1));
        assert_eq!(read.inserted().get(), Nat::new(3));
        assert_eq!(read.put_bottom().get(), Nat::new(1));
    }

    #[cfg(feature = "stats")]
    #[test]
    fn real_counters_accumulate() {
        let mut stats = PileStats::EMPTY;
        stats.on_drawn(Nat::new(5));
        assert_eq!(stats.drawn().get(), Nat::new(5));
    }

    #[cfg(not(feature = "stats"))]
    #[test]
    fn disabled_stats_are_noops() {
        let mut stats = PileStats::EMPTY;
        stats.on_drawn(Nat::new(5));
        stats.on_discarded(Nat::new(5));
        stats.on_inserted(Nat::new(5));
        stats.on_put_bottom();
        assert!(stats.drawn().is_zero());
        assert!(stats.put_bottom().is_zero());
        assert_eq!(std::mem::size_of::<PileStats>(), 0);
    }
}
