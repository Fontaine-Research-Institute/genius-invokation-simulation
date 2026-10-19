//! 【行动牌】状态（D112/D113）：通用状态抽象 + **一个公共状态列表**。
//!
//! 归属说明（用户已定，D112/D113）：
//! - 减费 / 赋能等「挂在行动牌（牌库位置 / 手牌）上的状态」属于**行动牌**，不是状态牌族；
//! - 所以通用状态放这里（action_card/status/），而 families/status/ 只留**状态牌族的 L2 类型枚举**
//!   StatusType（D51），两者不是一回事。
//!
//! 依据：D95/D96（状态身份 = 类型化静态实例指针）、D104（通用状态同时支持必须带层数 / 布尔值）、
//! D105（所有状态共用一个列表；布尔类不额外存 bool，在列表里即代表有）、
//! D108（通用层不解释减费 / 赋能语义）、D110（静态数据与 TagText 同构：名字 + 描述）、
//! D64（数据只有一份，条目只存身份指针 + 位置 + 层数）。

pub mod state;

pub use state::{
    Mark, StateEntry, StateForm, StateIdentity, StateKey, StateList, StateText, same_state_identity,
    state_address,
};
