//! 「冒险地点」标签（行动牌 · 支援）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::action_card::support::adventure_spot::AdventureSpot;`
//!
//! 支援牌中的冒险地点（对应参考实现 adventureSpot）。冒险/奇遇机制本轮不做（对比-gi-tcg P2-5），标签先建。

/// 「冒险地点」：支援牌中的冒险地点（对应参考实现 adventureSpot）。冒险/奇遇机制本轮不做（对比-gi-tcg P2-5），标签先建。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct AdventureSpot;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.adventure_spot
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
