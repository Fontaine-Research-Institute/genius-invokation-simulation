//! 【行动牌】区域状态与操作：**手牌 + 牌库**。
//!
//! 依据（决策日志）：D42（区 = 状态里的字段，没有区号/区表）、D52（牌库标记跟着位置走）、
//! D80/D82（标记表口径）、D83/D84/D85（折叠的早期口径）、D86/D89（舍弃 / 置底的标记语义）、
//! D87（引擎 dedup 无上下文 -> definitions 侧后置合并）、D90（秘传起手归手牌）、
//! D91/D92（每块自己的选择列表 + 去重函数，放该块文件夹的 parse.rs）、
//! D93/D94（位置为键的统一列表 + 段区间 + 同源池分组 id）、
//! D95/D96（状态身份 = 类型化静态实例指针，不用 Nat）、
//! D97/D98（按组合折叠、顺序一律不区分）、D103/D105（状态存成**一个公共列表**，
//! 布尔类不额外存 bool；通用状态在 families/action_card/status/state.rs，D112/D113）、
//! D64/D65（字段访问、不用下标取数）。
//!
//! **容器没有身份**：本目录里的 HandState / PileState 就是「区」本身，
//! 它们是 DefState.me / DefState.opp 里各一个字段（由状态层持有，两层实例 = 敌我），
//! 这里**不出现区号、不做区登记、不建「实体 -> 区」表**。
//!
//! **为什么不直接用引擎的 Area**：手牌 / 牌库的元素不是裸 EntityId，而是
//! 「实体号 + 卡 id 一对」（用户已定：**不建全局 实体 -> CardId 映射表**）。
//! 所以本目录自己用 Vec<CardInstance> / Vec<PileEntry> 做容器。
//!
//! **纯函数风格**：所有游戏操作都吃 &self、返回新状态（或分支列表），不改动入参；
//! 失败语义统一为「不改状态 + 返回表示没做」（false / None / 空列表），不 panic。
//!
//! 结构（D92：逻辑块建文件夹）：
//! - hand/mod.rs：手牌（上限 10 / 加入 / 舍弃 / 起手流程 D90）；
//! - pile/mod.rs：牌库（分段 / 同源随机池 / 位置为键的 entries / 公共状态列表 / 置底 / 均匀置入 / 按位置舍弃）；
//! - pile/parse.rs：牌库这块自己的**选择列表 + 去重（折叠）函数**（D26 / D91）；
//! - hand/parse.rs：手牌这块自己的折叠入口。
//!
//! 查询（按类型 / 标签筛选、PileQuery 按段分组）在后续的查询块里；
//! 本目录只提供结构访问接口（segments() / entry() / entries() / marks()）。

pub mod hand;
pub mod pile;
pub mod query;

use crate::base::CardId;
use gi_genius_invokation_engine::prob::Fraction;
use gi_genius_invokation_engine::EntityId;

/// 卡牌实例 = **实体号 + 卡 id 一对**（用户已定：不建全局映射表）。
///
/// - entity：本局唯一、单调不复用的实体号（由引擎发号器发放）；
/// - card：这张牌是哪个定义（CardId，行动牌 id 区域内唯一）。
///
/// 运行时身份靠 entity；规则语义（类型 / 标签 / 费用）靠 card 去查定义。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct CardInstance {
    /// 本局唯一的实体号。
    pub entity: EntityId,
    /// 这张实体对应的卡牌定义 id。
    pub card: CardId,
}

impl CardInstance {
    /// **const 构造**（建局 / 测试写常量用）。
    pub const fn new(entity: EntityId, card: CardId) -> CardInstance {
        CardInstance { entity, card }
    }
}

// ======================================================================
// 折叠（去重）—— 行动牌这一族内共享（D97 / D98 / D105）
// ======================================================================
//
// 最终口径（D98 / D105）：**按抽取结果的组合折叠，顺序永远不区分**。
// - 一张牌的键 = (卡牌 id, 标记键)；
// - **标记键（D105）= 该位置在公共状态列表里按 pos 过滤出来的状态键**：
//   布尔条目进 booleans（在列表即代表有，不存 bool）、层数条目进 layered；身份都是指针（D95）。
// - 一条序列的等价类键 = 把上面这些键**排序后的列表（多重集）**；
// - 两支序列等价 ⟺ 组合相同；等价类权重 = 组内原始有序序列条数；
//   概率 = 权重 / 原始有序序列总数（不是均匀）。
//
// 每个块自己的 parse.rs 提供入口（D91/D92）；这里只放 key / compose / 分组实现。

/// 折叠用的标记键：就是通用状态的折叠键（D105）。
pub(crate) use crate::families::action_card::status::StateKey as FoldMark;

/// 折叠看到的一张牌：卡 id + 位置状态（手牌 / 牌库各自转换进来）。
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct FoldCard {
    /// 卡牌 id。
    pub card: CardId,
    /// 位置状态（公共状态列表按 pos 过滤出来的键）。
    pub mark: FoldMark,
}

/// 子函数一：一张牌的折叠键 = (卡牌 id, 标记键)。
pub(crate) fn fold_key(card: FoldCard) -> (CardId, FoldMark) {
    (card.card, card.mark)
}

/// 折叠组合键：排序后的 (卡牌 id, 标记键) 多重集。
pub(crate) type FoldCombo = Vec<(CardId, FoldMark)>;

/// 子函数二：把一条序列压成**排序后的键多重集**（D98：顺序不区分）。
///
/// 用排序 vec 当键（不用 HashMap：遍历序不稳定）。
pub(crate) fn compose(cards: &[FoldCard]) -> FoldCombo {
    let mut key: FoldCombo = cards.iter().map(|card| fold_key(card.clone())).collect();
    key.sort();
    key
}

/// 通用折叠入口：按组合分组、权重相加、每组只留代表元（D87 后置合并 + D98 组合键）。
///
/// 各块自己的 parse.rs 调用它（判据共享、入口留在各自块，D91/D92）。
pub(crate) fn fold_equivalent<B>(
    branches: Vec<(B, Fraction)>,
    cards_of: impl Fn(&B) -> Vec<FoldCard>,
) -> Vec<(B, Fraction)> {
    let mut classes: Vec<(FoldCombo, B, Vec<Fraction>)> = Vec::new();
    for (branch, probability) in branches {
        let key = compose(&cards_of(&branch));
        if let Some(entry) = classes.iter_mut().find(|entry| entry.0 == key) {
            entry.2.push(probability);
        } else {
            classes.push((key, branch, vec![probability]));
        }
    }
    classes.sort_by(|left, right| left.0.cmp(&right.0));
    classes
        .into_iter()
        .map(|(_, branch, weights)| {
            let total: Fraction = weights.iter().sum();
            (branch, total)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gi_genius_invokation_engine::Nat;

    #[test]
    fn card_instance_pairs_entity_and_card() {
        let instance = CardInstance::new(Nat::new(7), CardId::new(321004));
        assert_eq!(instance.entity, Nat::new(7));
        assert_eq!(instance.card, CardId::new(321004));
        assert_eq!(instance, CardInstance::new(Nat::new(7), CardId::new(321004)));
        assert_ne!(instance, CardInstance::new(Nat::new(8), CardId::new(321004)));
        let one = std::mem::size_of::<Nat>();
        assert_eq!(std::mem::size_of::<CardInstance>(), one.saturating_add(one));
    }
}
