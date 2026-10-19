//! 召唤物的标签（D54：一个标签一个零大小类型 + 一个文件）。
//!
//! 召唤物没有类型（D51）。按游戏内文本描述，召唤物自己的标签只有
//! 「草元素产物」一个；以下标签也在召唤物上出现，但按「唯一定义」直接导入别的家庭层级的那一份：
//! - 减伤 → `crate::tags::action_card::common::damage_reduction::DamageReduction`
//! - 希穆兰卡 → `crate::tags::action_card::event::simulanka::Simulanka`
//! - 不可发现 → `crate::tags::action_card::common::undiscoverable::Undiscoverable`
//!
//! 本文件不重抄上面三个，只声明召唤物自己的标签。

pub mod dendro_construct;
