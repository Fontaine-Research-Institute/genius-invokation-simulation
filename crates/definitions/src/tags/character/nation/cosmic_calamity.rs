//! 「寰宇劫灭」标签（角色 · 国家/势力）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::character::nation::cosmic_calamity::CosmicCalamity;`
//!
//! 势力标签：寰宇劫灭。角色与敌人共用，唯一定义在角色处。

/// 「寰宇劫灭」：势力标签：寰宇劫灭。角色与敌人共用，唯一定义在角色处。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CosmicCalamity;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.cosmic_calamity
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
