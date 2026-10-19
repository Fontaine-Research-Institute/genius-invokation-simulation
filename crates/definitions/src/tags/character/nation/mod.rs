//! 角色（与敌人）的**国家 / 势力标签**（一个标签一个文件）。
//!
//! D54：每个标签是零大小类型 + 一个文件；没有总枚举。
//! 全集来自游戏内文本描述（角色牌 + 敌人牌）：
//! 蒙德 / 璃月 / 稻妻 / 须弥 / 枫丹 / 纳塔 / 挪德卡莱 / 愚人众 / 镀金旅团 / 魔物 /
//! 丘丘人 / 圣骸兽 / 寰宇劫灭 / 海乱鬼。
//! 角色与敌人共用；唯一定义在这里，敌人导入这一份。

pub mod consecrated_beast;
pub mod cosmic_calamity;
pub mod eremites;
pub mod fatui;
pub mod fontaine;
pub mod hilichurl;
pub mod inazuma;
pub mod kairagi;
pub mod liyue;
pub mod mondstadt;
pub mod monster;
pub mod natlan;
pub mod nodkrai;
pub mod sumeru;
