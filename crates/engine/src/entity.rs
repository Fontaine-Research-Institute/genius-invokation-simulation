//! 实体表：**只发号**。
//!
//! 万物皆实体：引擎只管"发一个单调、不复用的号"，**不记录实体在哪个区**。
//! 「实体属于哪个容器」由上层自己持有容器字段（编译后就是直接取对应内存），
//! 所以引擎里**没有区号、没有区表**。

use crate::id::EntityId;
use crate::nat::Nat;

/// 实体表：**只发号**（从 0 起、单调、不复用）。
#[derive(Clone, Debug, Default)]
pub struct EntityTable {
    /// 已发放的实体数（= 下一个要发的号）。
    next: usize,
}

impl EntityTable {
    /// 一个号都还没发过。
    pub const fn new() -> Self {
        EntityTable { next: 0 }
    }

    /// 发号：从 0 起、单调、**不复用**。
    pub fn alloc(&mut self) -> EntityId {
        // id 是 u32 量级（40 亿+），实际对局不可能耗尽；
        // 真耗尽时宁可显式失败，也不发重复号（那会破坏"不复用"契约）。
        let raw = u32::try_from(self.next).expect("实体 id 空间耗尽");
        self.next += 1;
        Nat::new(raw)
    }

    /// 已发放的实体数。
    pub fn len(&self) -> usize {
        self.next
    }

    /// 一个号都还没发过。
    pub fn is_empty(&self) -> bool {
        self.next == 0
    }
}
