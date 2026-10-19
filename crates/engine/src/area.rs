//! 实体区：放实体的**容器类型**。
//!
//! 只有两种容器类型（它们是上层状态里的**字段类型**）：
//! - [`Area`]：**多实体容器**（不搬动、不设上限）；
//! - [`SingleArea`]：**单实体容器**（= 一个变量，只能放一个实体）。
//!
//! 容器**没有身份、没有号**，也不登记到任何地方：
//! 「谁从哪个容器取」由上层自己持有这些字段（编译后就是直接取对应内存）。

use crate::id::EntityId;

/// 多实体容器：只保证"放/取不频繁搬动 + 不设容量上限"。
///
/// 不保证顺序：[`Area::take_at`] 走 `swap_remove` 语义（末尾元素填补空位）。
#[derive(Clone, Debug, Default)]
pub struct Area {
    items: Vec<EntityId>,
}

impl Area {
    /// 空容器。
    pub const fn new() -> Self {
        Area { items: Vec::new() }
    }

    /// 放进来，返回它所在的下标（末尾）。
    pub fn push(&mut self, id: EntityId) -> usize {
        let index = self.items.len();
        self.items.push(id);
        index
    }

    /// 实体个数。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 里面一个实体都没有。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 只读切片（顺序 = 当前存放顺序，不保证稳定）。
    pub fn as_slice(&self) -> &[EntityId] {
        &self.items
    }

    /// 按当前顺序遍历。
    pub fn iter(&self) -> std::slice::Iter<'_, EntityId> {
        self.items.iter()
    }

    /// 是否装着这个实体。
    pub fn contains(&self, id: EntityId) -> bool {
        self.items.contains(&id)
    }

    /// 这个实体的下标（重复放入同一实体时给第一个）。
    pub fn index_of(&self, id: EntityId) -> Option<usize> {
        self.items.iter().position(|item| *item == id)
    }

    /// O(1) 取出：**swap_remove 语义**（不保证顺序，末尾元素填补空位）。
    ///
    /// 下标越界 -> `None`，容器不变。
    pub fn take_at(&mut self, index: usize) -> Option<EntityId> {
        if index < self.items.len() {
            Some(self.items.swap_remove(index))
        } else {
            None
        }
    }

    /// 按实体摘除（swap_remove 语义）；返回是否真的摘到了。
    pub fn remove(&mut self, id: EntityId) -> bool {
        match self.index_of(id) {
            Some(index) => {
                self.items.swap_remove(index);
                true
            }
            None => false,
        }
    }
}

/// 单实体容器 = 一个"变量"：要么没有，要么只有一个实体。
///
/// 用途举例（上层）：像"要么没有、要么只有一张"的那种位置。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SingleArea {
    item: Option<EntityId>,
}

impl SingleArea {
    /// 空变量。
    pub const fn new() -> Self {
        SingleArea { item: None }
    }

    /// 当前装着的实体。
    pub const fn get(&self) -> Option<EntityId> {
        self.item
    }

    /// 放入（**替换**旧的），返回被替换掉的。
    pub fn set(&mut self, id: EntityId) -> Option<EntityId> {
        self.item.replace(id)
    }

    /// 清空，返回原来的。
    pub fn clear(&mut self) -> Option<EntityId> {
        self.item.take()
    }

    /// 里面没有实体。
    pub fn is_empty(&self) -> bool {
        self.item.is_none()
    }
}
