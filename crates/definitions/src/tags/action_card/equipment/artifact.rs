//! 「圣遗物」标签（行动牌 · 装备）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::action_card::equipment::artifact::Artifact;`
//!
//! 装备牌中的圣遗物；与「武器」在装备上互斥（D16）。

/// 「圣遗物」：装备牌中的圣遗物；与「武器」在装备上互斥（D16）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Artifact;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.artifact
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
