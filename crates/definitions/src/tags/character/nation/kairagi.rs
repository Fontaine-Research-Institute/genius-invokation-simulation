//! 「海乱鬼」标签（角色 · 国家/势力）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::character::nation::kairagi::Kairagi;`
//!
//! 势力标签：海乱鬼。数据里只出现在敌人牌上（2 次）；按「国家/势力」归类放在这里，敌人导入本文件。

/// 「海乱鬼」：势力标签：海乱鬼。数据里只出现在敌人牌上（2 次）；按「国家/势力」归类放在这里，敌人导入本文件。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Kairagi;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.kairagi
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
