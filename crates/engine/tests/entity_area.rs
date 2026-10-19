//! entity.rs（只发号）/ area.rs（两个容器类型）的集成测试。

use gi_genius_invokation_engine::area::{Area, SingleArea};
use gi_genius_invokation_engine::entity::EntityTable;
use gi_genius_invokation_engine::{EntityId, Nat};

fn n(v: u32) -> Nat {
    Nat::new(v)
}

// ------------------------------------------------------- EntityTable：只发号

#[test]
fn new_table_is_empty() {
    let table = EntityTable::new();
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);
}

#[test]
fn alloc_1000_ids_are_strictly_increasing_and_unique() {
    let mut table = EntityTable::new();

    let mut ids: Vec<EntityId> = Vec::new();
    for _ in 0..1000 {
        ids.push(table.alloc());
    }
    assert_eq!(table.len(), 1000);
    assert!(!table.is_empty());

    // 从 0 起、连号
    for (i, id) in ids.iter().enumerate() {
        assert_eq!(id.get(), i as u32, "id 必须从 0 起单调发放");
    }
    // 严格递增
    for pair in ids.windows(2) {
        assert!(pair[0] < pair[1], "id 必须严格递增");
    }
    // 不重复
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), 1000, "1000 个 id 必须互不相同");
}

#[test]
fn alloc_keeps_growing_and_never_reuses() {
    // 引擎不再记录"实体在哪"：只要保证每次 alloc 都拿到更大的新号。
    let mut table = EntityTable::new();
    let first = table.alloc();
    let second = table.alloc();
    let third = table.alloc();

    assert!(first < second);
    assert!(second < third);
    assert_eq!(table.len(), 3);
}

// ------------------------------------------------------------- Area：多实体容器

#[test]
fn area_push_returns_index_and_reflects_in_slice() {
    let mut area = Area::new();
    assert!(area.is_empty());
    assert_eq!(area.len(), 0);
    assert_eq!(area.as_slice(), &[] as &[EntityId]);

    assert_eq!(area.push(n(10)), 0);
    assert_eq!(area.push(n(11)), 1);
    assert_eq!(area.push(n(12)), 2);

    assert_eq!(area.len(), 3);
    assert!(!area.is_empty());
    assert_eq!(area.as_slice(), &[n(10), n(11), n(12)]);
    assert_eq!(
        area.iter().copied().collect::<Vec<_>>(),
        vec![n(10), n(11), n(12)]
    );
}

#[test]
fn area_contains_and_index_of() {
    let mut area = Area::new();
    area.push(n(10));
    area.push(n(11));

    assert!(area.contains(n(10)));
    assert!(area.contains(n(11)));
    assert!(!area.contains(n(99)));

    assert_eq!(area.index_of(n(10)), Some(0));
    assert_eq!(area.index_of(n(11)), Some(1));
    assert_eq!(area.index_of(n(99)), None);
}

#[test]
fn take_at_uses_swap_remove_and_keeps_remaining_set() {
    let mut area = Area::new();
    for i in 0..4 {
        area.push(n(i));
    }

    // 取走下标 1（值 1）：末尾的 3 被 swap 到下标 1
    assert_eq!(area.take_at(1), Some(n(1)));
    assert_eq!(area.len(), 3);
    assert_eq!(area.as_slice(), &[n(0), n(3), n(2)]);

    // 剩余元素集合正确
    let mut left: Vec<u32> = area.iter().map(|e| e.get()).collect();
    left.sort();
    assert_eq!(left, vec![0, 2, 3]);

    // 越界不 panic、容器不变
    assert_eq!(area.take_at(99), None);
    assert_eq!(area.len(), 3);

    // 继续取空
    assert_eq!(area.take_at(0), Some(n(0)));
    assert_eq!(area.as_slice(), &[n(2), n(3)]);
    assert_eq!(area.take_at(0), Some(n(2)));
    assert_eq!(area.take_at(0), Some(n(3)));
    assert!(area.is_empty());
    assert_eq!(area.take_at(0), None);
}

#[test]
fn area_remove_by_entity() {
    let mut area = Area::new();
    area.push(n(1));
    area.push(n(2));

    assert!(area.remove(n(2)));
    assert_eq!(area.as_slice(), &[n(1)]);
    assert!(!area.remove(n(2)), "已经摘过一次");
    assert_eq!(area.len(), 1);
}

// --------------------------------------------------------- SingleArea：单实体容器

#[test]
fn single_area_set_returns_old_value_and_clear_empties() {
    let mut slot = SingleArea::new();
    assert!(slot.is_empty());
    assert_eq!(slot.get(), None);

    // 从空放入 -> 没有旧值
    assert_eq!(slot.set(n(1)), None);
    assert_eq!(slot.get(), Some(n(1)));
    assert!(!slot.is_empty());

    // 替换 -> 返回旧值
    assert_eq!(slot.set(n(2)), Some(n(1)));
    assert_eq!(slot.get(), Some(n(2)));

    // 清空 -> 返回原来的，之后为空
    assert_eq!(slot.clear(), Some(n(2)));
    assert!(slot.is_empty());
    assert_eq!(slot.get(), None);

    // 空上再清 -> None
    assert_eq!(slot.clear(), None);
}
