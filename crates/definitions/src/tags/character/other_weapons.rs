//! 「其他武器」标签（角色 · 武器类型）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::character::other_weapons::OtherWeapons;`
//!
//! 其他武器（没有标准武器类型的角色/敌人；参考实现 WEAPON_TAGS 里的 otherWeapon）。角色与敌人共用，唯一定义在这里。

/// 「其他武器」：其他武器（没有标准武器类型的角色/敌人；参考实现 WEAPON_TAGS 里的 otherWeapon）。角色与敌人共用，唯一定义在这里。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct OtherWeapons;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.other_weapons
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
