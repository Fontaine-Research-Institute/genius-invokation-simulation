//! 「元素幻变」标签（行动牌 · 支援）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::action_card::support::elemental_transmutation::ElementalTransmutation;`
//!
//! 支援牌中的元素幻变（两种元素）。定义与注册在 families/action_card/types/support/elemental_transmutation.rs（后续块）。

/// 「元素幻变」：支援牌中的元素幻变（两种元素）。定义与注册在 families/action_card/types/support/elemental_transmutation.rs（后续块）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ElementalTransmutation;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.elemental_transmutation
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
