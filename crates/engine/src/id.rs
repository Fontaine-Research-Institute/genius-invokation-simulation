//! 标识类型（引擎只认识"标识"，不解释它代表什么）。
//!
//! **引擎只认实体**：唯一的全局标识是 [`EntityId`]（单调流水号）。
//! **区没有编号，也没有区表**：区就是上层自己持有的变量（"谁要哪个区自己持有"），
//! 编译后直接落到对应内存地址；引擎不发号、不登记任何区标识。
//! "谁"（敌我）同样不在引擎里：由定义层用**两个具名实例**表达（D34），不是编号/枚举/下标。
//!
//! - `EntityId`：万物皆实体，实体 id 只是一个 `Nat`（从 0 起、单调、不复用）；
//! - `ActionId`：列表下标，不是数值。

use crate::nat::Nat;

/// 实体 id：从 0 起、单调、不复用的流水号（由实体表发放）。
pub type EntityId = Nat;

/// 列表下标（**不是数值**）。
///
/// 注意：本 crate 不参与上层"编译期裸整型扫描"，所以这里保留 `u32` 下标；
/// `ActionId` 只用于"列表第几项"，**不得参与数值运算**。
pub type ActionId = u32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_is_nat() {
        // EntityId 就是 Nat：同类型、可直接比较与排序
        let a: EntityId = Nat::new(3);
        let b: EntityId = Nat::new(1);
        assert_eq!(a, Nat::new(3));
        assert!(b < a);

        let mut ids: Vec<EntityId> = vec![a, b, Nat::new(2)];
        ids.sort();
        assert_eq!(ids, vec![Nat::new(1), Nat::new(2), Nat::new(3)]);

        // 保证二者是同一个类型（写成同一行即可编译）
        let _same: Nat = ids[0];
    }

    #[test]
    fn action_id_is_index() {
        // 只是列表下标：可以比较/做 usize 转换，但不参与数值运算
        let idx: ActionId = 2;
        assert_eq!(idx as usize, 2);
        assert_eq!(ActionId::default(), 0);
    }

    #[test]
    fn engine_has_no_area_id_concept() {
        // 引擎没有区编号，也没有区表：全局标识只有 EntityId（与列表下标 ActionId）。
        // 这里只验证实体标识可直接比较/排序，不涉及任何区标识。
        let a: EntityId = Nat::new(2);
        let b: EntityId = Nat::new(1);
        assert!(b < a);
        assert_eq!(a, Nat::new(2));
    }
}
