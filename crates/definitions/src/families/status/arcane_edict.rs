//! **秘传揭令**：状态身份 + 「本局该牌手是否仍可打出秘传」的容器（Z12）。
//!
//! ## 依据与权威顺序（RULES §三.3）
//!
//! **规则书·特殊词条「秘传」**（第一权威）：「在一场对局中，每位牌手只能使用一张『秘传』卡牌。『秘传』卡牌一定在你的起始手牌中。……」
//!
//! **权威关键字 #70 秘传揭令（K311）**（第二权威；规则书无专条）：「在一场对局中，每位牌手只能使用一张『秘传』卡牌。 / 只有在『秘传揭令』仍存在时，才能打出『秘传』卡牌。打出『秘传』卡牌后，会消耗掉当前对局的『秘传揭令』使用机会。」
//!
//! **卡面 / 关键字数据交叉验证**：generated/data/keywords.json 的 K311 描述与上文字面一致（见 rules::ops::arcane 与本文件测试）；没有找到「秘传揭令」的独立状态卡数据行（tcgstatuseffects 无此条），所以**不建 StatusDef / 不伪造卡 id**。
//!
//! ## 这个容器表示什么
//!
//! ArcaneEdict = **每位牌手一个**的「秘传揭令使用机会」+ 「本局已打出秘传张数」：
//! - 开局 ArcaneEdict::new：揭令存在（可打出）、已打出 0 张；
//! - ArcaneEdict::can_play 复用 rules::ops::arcane::can_play（判定顺序与关键字层一致）；
//! - ArcaneEdict::record_play：允许时 -> 已打出 +1，并按 K311 消耗揭令；
//! - ArcaneEdict::consume：只消耗揭令（不打牌；供「其他效果移除揭令」类卡面使用，待实测）；
//! - 容器本身**不改外部状态**（纯函数返回新值），也不做派发。
//!
//! ARCANE_EDICT 是「秘传揭令」这个**状态的身份**（名字 / 描述中文；布尔形态：存在即代表有），
//! 供状态列表 / 查询按静态身份指针使用（D95 / D96）；容器与身份是同一语义的两种视角
//! （存在性 vs 使用机会记账）。
//!
//! ## 待实测 / TODO（规则书没写清，不自行编排）
//!
//! - **按牌手还是全局**：规则书写「每位牌手只能使用一张秘传」，K311 写「消耗当前对局的
//!   秘传揭令使用机会」；本容器按**每位牌手一个实例**建模（我方 / 敌方各一份）。若实测是
//!   双方共享一份揭令，需要把两个实例合并为一方持有一份 —— 标 TODO，待实测。
//! - **揭令是否存在是否会显示成状态**：数据里没有该状态卡；ARCANE_EDICT 只提供身份与中文
//!   文本，是否要显示在状态区 / 是否可被效果移除，**待实测**，本块不做接线。
//! - **消耗时机**：本块按「打出秘传后立刻消耗」；若实测有「打出失败不消耗」等边界，
//!   由调用方只在 record_play 返回 Some 时采用。

use crate::families::action_card::status::{StateForm, StateIdentity, StateText};
use crate::generated::Localized;
use crate::rules::ops::arcane::{can_play, consumes_edict, ArcanePlay, ARCANE_LEGEND_PER_GAME};
use gi_genius_invokation_engine::{nadd, Nat};

/// 「秘传揭令」的静态文本（名字 / 描述；中文按关键字数据 K311 逐字）。
///
/// 其它语言槽位没有数据（D56 / D68 显式缺失）。
pub static ARCANE_EDICT_TEXT: StateText = StateText {
    name: Localized::from_parts(Some("秘传揭令"), None, None, None),
    description: Localized::from_parts(
        Some(
            "在一场对局中，每位牌手只能使用一张「秘传」卡牌。\n只有在「秘传揭令」仍存在时，才能打出「秘传」卡牌。打出「秘传」卡牌后，会消耗掉当前对局的「秘传揭令」使用机会。",
        ),
        None,
        None,
        None,
    ),
};

/// 「秘传揭令」状态的身份类型（没有独立卡牌数据 -> 不建 StatusDef；身份 = 静态文本地址）。
#[derive(Debug)]
pub struct ArcaneEdictStatus;

/// 「秘传揭令」的状态身份（布尔形态：存在即代表有）。
pub static ARCANE_EDICT: ArcaneEdictStatus = ArcaneEdictStatus;

impl StateIdentity for ArcaneEdictStatus {
    fn text(&self) -> &'static StateText {
        &ARCANE_EDICT_TEXT
    }

    fn form(&self) -> StateForm {
        StateForm::Boolean
    }
}

impl ArcaneEdictStatus {
    /// 把自身当状态身份指针用（D95 / D96）。
    pub const fn identity(&'static self) -> &'static dyn StateIdentity {
        self
    }
}

/// 每位牌手一份的**秘传揭令容器**（使用机会 + 已打出张数）。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ArcaneEdict {
    /// 秘传揭令是否仍存在（存在 = 还能打出秘传）。
    available: bool,
    /// 本局该牌手已打出的秘传张数。
    played: Nat,
}

impl ArcaneEdict {
    /// 整局每位牌手最多打出 1 张秘传（规则书 + K311；复用规则层常量）。
    pub const PER_GAME: Nat = ARCANE_LEGEND_PER_GAME;

    /// 开局状态：揭令存在、还没有打出过秘传。
    pub const fn new() -> ArcaneEdict {
        ArcaneEdict {
            available: true,
            played: Nat::ZERO,
        }
    }

    /// 秘传揭令是否仍存在。
    pub const fn available(&self) -> bool {
        self.available
    }

    /// 本局已打出的秘传张数。
    pub const fn played(&self) -> Nat {
        self.played
    }

    /// 现在能不能打出秘传（复用规则层的判定顺序：先看已打出张数，再看揭令）。
    pub fn can_play(&self) -> ArcanePlay {
        can_play(self.played, self.available)
    }

    /// 只**消耗**揭令（不打牌；供「移除揭令」类效果使用，待实测）。
    ///
    /// 本来就不存在 -> None（状态不变）。
    pub const fn consume(&self) -> Option<ArcaneEdict> {
        if self.available {
            Some(ArcaneEdict {
                available: false,
                played: self.played,
            })
        } else {
            None
        }
    }

    /// 打出秘传：允许时记录「已打出 +1」并按 K311 消耗揭令；不允许 -> None（状态不变）。
    pub fn record_play(&self) -> Option<ArcaneEdict> {
        match self.can_play() {
            ArcanePlay::Allowed => Some(ArcaneEdict {
                available: if consumes_edict() {
                    false
                } else {
                    self.available
                },
                played: nadd!(self.played, Nat::new(1)),
            }),
            ArcanePlay::AlreadyPlayed | ArcanePlay::NoEdict => None,
        }
    }

    /// 这个容器的状态身份（供状态列表 / 查询按静态身份指针使用）。
    pub fn status_identity(&self) -> &'static dyn StateIdentity {
        &ARCANE_EDICT
    }
}

impl Default for ArcaneEdict {
    fn default() -> ArcaneEdict {
        ArcaneEdict::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::families::action_card::status::same_state_identity;
    use crate::generated::Lang;

    fn n(value: u32) -> Nat {
        Nat::new(value)
    }

    #[test]
    fn edict_identity_has_name_and_boolean_form() {
        assert_eq!(ARCANE_EDICT.name(), "秘传揭令");
        assert_eq!(ARCANE_EDICT.form(), StateForm::Boolean);
        assert!(same_state_identity(&ARCANE_EDICT, &ARCANE_EDICT));
        assert_eq!(
            ARCANE_EDICT_TEXT.name.get(Lang::ZhCn).text(),
            Some("秘传揭令")
        );
    }

    #[test]
    fn fresh_container_allows_arcane_and_records_play() {
        let edict = ArcaneEdict::new();
        assert!(edict.available());
        assert_eq!(edict.played(), n(0));
        assert_eq!(edict.can_play(), ArcanePlay::Allowed);
        assert_eq!(edict.status_identity().name(), "秘传揭令");

        // 打出秘传：已打出 +1，揭令被消耗（K311）。
        let played = edict.record_play().expect("允许打出");
        assert!(!played.available());
        assert_eq!(played.played(), n(1));
        assert_eq!(played.can_play(), ArcanePlay::AlreadyPlayed);
        // 再打一次 -> 拒绝，状态不变。
        assert!(played.record_play().is_none());
    }

    #[test]
    fn consumed_edict_blocks_arcane_with_no_edict() {
        let consumed = ArcaneEdict::new().consume().expect("存在可消耗");
        assert!(!consumed.available());
        assert_eq!(consumed.played(), n(0));
        assert_eq!(consumed.can_play(), ArcanePlay::NoEdict);
        assert!(consumed.consume().is_none());
    }
}
