//! 角色牌三个区的统计（D187）：集中放「角色区 / 出战状态区 / 装备栏」的统计，
//! 执行方在事情发生时自己填（各区的操作里已经埋好调用点）。
//!
//! - 对外只读指针（D183）：只能拿 `&Counter` 读；
//! - **整体可裁剪**（feature = "stats"）：关掉时零大小空结构、更新全空操作；
//! - 统计**不参与局面等价**：三个区的 `PartialEq` 都手动实现、排除 stats。
//!
//! 归属（D182 / D187）：统计是**实体区的属性**，由各区自己作为字段持有；
//! 同一个区可以有多个统计属性、不同的区各自独立；这里只是把它们的**类型**收在一处，
//! 不建「区 -> 统计」绑定、没有 id、没有全局表。

/// 定义一组「区统计」：feature 打开时是若干 `Counter` 字段，关闭时是零大小结构。
///
/// 用法（每个统计属性给：字段名 => (更新方法名, 只读方法名) : 文档）：
/// ```text
/// zone_stats!(FooStats {
///     entered => (on_entered, entered) : "累计进入次数。",
/// });
/// ```
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
    /// 角色区统计（规则书·游戏区域 I）。
    CharacterZoneStats {
        entered => (on_entered, entered) : "累计「进入角色区」的角色数 +amount（开局放置 / 后续加入）。",
        left => (on_left, left) : "累计「离开角色区」的角色数 +amount（被击倒 / 移出比赛）。",
        switches => (on_switched, switches) : "累计成功执行的「切换角色」次数 +amount（不含被击倒后的强制补位）。",
    }
    /// 出战状态区统计（规则书·游戏区域 II(3)）。
    CombatStatusStats {
        attached => (on_attached, attached) : "累计「出战状态进入出战状态区」的次数 +amount。",
        transferred => (on_transferred, transferred) : "累计「出战状态随切换转移」的次数 +amount。",
        depleted => (on_depleted, depleted) : "累计「持续回合耗尽被弃置」的出战状态数量 +amount。",
    }
    /// 装备栏统计（规则书·游戏区域 II(1)）。
    EquipmentZoneStats {
        equipped => (on_equipped, equipped) : "累计「装备进入装备栏」的张数 +amount（含替换时的新装备）。",
        discarded => (on_discarded, discarded) : "累计「装备离开装备栏（弃置）」的张数 +amount（含替换被弃置的旧装备）。",
    }
);

#[cfg(test)]
mod tests {
    use gi_genius_invokation_engine::Nat;

    use super::*;

    #[test]
    fn stats_types_exist_and_read_as_counters() {
        let mut character = CharacterZoneStats::EMPTY;
        character.on_entered(Nat::new(3));
        character.on_switched(Nat::new(1));

        let mut combat = CombatStatusStats::EMPTY;
        combat.on_attached(Nat::new(2));

        let mut equipment = EquipmentZoneStats::EMPTY;
        equipment.on_equipped(Nat::new(1));
        equipment.on_discarded(Nat::new(1));

        if cfg!(feature = "stats") {
            assert_eq!(character.entered().get(), Nat::new(3));
            assert_eq!(combat.attached().get(), Nat::new(2));
            assert_eq!(equipment.equipped().get(), Nat::new(1));
            assert_eq!(equipment.discarded().get(), Nat::new(1));
        } else {
            // feature 关闭：更新全是空操作，读到的都是 0。
            assert!(character.entered().is_zero());
            assert!(combat.attached().is_zero());
            assert!(equipment.equipped().is_zero());
            assert!(equipment.discarded().is_zero());
        }
    }

    #[test]
    fn stats_are_readable_through_shared_references() {
        let mut stats = CharacterZoneStats::EMPTY;
        stats.on_left(Nat::new(1));
        let read: &CharacterZoneStats = &stats;
        // 只读指针不改原值（没有内部可变性）。
        assert_eq!(stats.left().get(), read.left().get());
        if cfg!(feature = "stats") {
            assert_eq!(read.left().get(), Nat::new(1));
        } else {
            assert!(read.left().is_zero());
        }
    }

    #[test]
    fn stats_size_follows_feature_flag() {
        if cfg!(feature = "stats") {
            assert!(std::mem::size_of::<CharacterZoneStats>() > 0);
        } else {
            assert_eq!(std::mem::size_of::<CharacterZoneStats>(), 0);
            let mut stats = CharacterZoneStats::EMPTY;
            stats.on_entered(Nat::new(9));
            assert!(stats.entered().is_zero());
        }
    }
}
