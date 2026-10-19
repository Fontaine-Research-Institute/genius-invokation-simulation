//! 「芒性」标签（角色 · 始基力）。
//!
//! D54：一个标签 = 一个**零大小类型** + 一个文件；没有"总标签枚举"，也没有数字/字符串编号。
//! 卡牌只能导入具体标签类型：`use crate::tags::character::arkhe::pneuma::Pneuma;`
//!
//! 始基力：芒性（官方英文 tagstext = Arkhe: Pneuma）。注意：参考实现代码里把 pneuma 当荒、ousia 当芒，与官方英文相反；本项目以数据里的官方英文为准。

/// 「芒性」：始基力：芒性（官方英文 tagstext = Arkhe: Pneuma）。注意：参考实现代码里把 pneuma 当荒、ousia 当芒，与官方英文相反；本项目以数据里的官方英文为准。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pneuma;

/// 本标签的静态文本（D67：数据唯一一份在 generated::TAGS；这里只给字段引用，不写字面量）。
pub fn data() -> &'static crate::generated::TagText {
    &crate::generated::TAGS.pneuma
}
// 运行期身份（D67）：data() 返回的字段地址（&TAGS.<字段>）；按指针比较/筛选，不用数字编号、不用 TypeId。
