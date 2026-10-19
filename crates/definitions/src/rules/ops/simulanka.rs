//! 希穆兰卡（tags=希穆兰卡；types=事件 / 支援；召唤物也带该标签）。
//!
//! **依据**：
//! - 权威关键字 #109「『希穆兰卡』卡牌」：「『希穆兰卡』卡牌包括：墨色酒馆、星轨王城、
//!   森林的祝福、预言女神的礼物、『邪龙』的苏醒、旁白的注脚、『魔女M的祝福』、
//!   （test）恶龙的地点。」——即**固定集合**，检索时按该标签筛选。
//! - 权威关键字 #107「『希穆兰卡』召唤物」：「可能出现的『希穆兰卡』召唤物共有 4 种…」
//!   ——即**固定 4 种**，随机生成 = 从这 4 种中取 1。
//! - 12 张带该标签的数据：`332051` 邪龙的苏醒、`332053` 破碎之海、`332054` 魔女M的祝福、
//!   `321029` 墨色酒馆、`322030` 预言女神的礼物 等 8 张行动牌 + 4 个召唤物。
//!
//! 规则层只给**集合范围**；真正的等概率抽样走引擎随机接口（D20：框架只给接口）。

use gi_genius_invokation_engine::Nat;

/// 「希穆兰卡」召唤物共有 4 种（关键字 #107）。
pub const SUMMON_KIND_COUNT: Nat = Nat::new(4);

/// 随机生成的取值范围。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RandomPickScope {
    /// 按「希穆兰卡」标签筛出的固定集合，等概率取 1。
    TaggedSet,
}

/// 召唤物种类数。
pub fn summon_kind_count() -> Nat {
    SUMMON_KIND_COUNT
}

/// 随机生成的取值范围（具体集合由调用方按标签查询得到）。
pub fn random_pick_scope() -> RandomPickScope {
    RandomPickScope::TaggedSet
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulanka_has_four_summon_kinds() {
        assert_eq!(summon_kind_count(), Nat::new(4));
        assert_eq!(random_pick_scope(), RandomPickScope::TaggedSet);
    }
}
