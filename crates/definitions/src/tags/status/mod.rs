//! 状态的标签（按**类型**分组，本轮先空）。
//!
//! 状态有两个类型（`StatusType`：角色状态 / 出战状态，D51），标签属于具体类型：
//! - `character_status`：角色状态的标签；
//! - `combat_status`：出战状态的标签。
//!
//! D54：若写，也是一个标签一个零大小类型 + 一个文件，没有总枚举。
//! 本轮**先空**（状态标签与具体状态实现强相关，留到 status 块）。注意：
//! `结构.md` 里「状态没有 types」的旧写法与 D51 冲突，**以 D51 为准**。

pub mod character_status;
pub mod combat_status;
