//! 角色（与敌人）的**始基力标签**（一个标签一个文件）。
//!
//! D54：每个标签是零大小类型 + 一个文件；没有总枚举。
//! 数据口径：荒性 = Ousia、芒性 = Pneuma（官方英文 tagstext）。
//! 注意参考实现代码里把 pneuma 当荒、ousia 当芒，与官方英文相反；本项目以官方英文为准。

pub mod ousia;
pub mod pneuma;
