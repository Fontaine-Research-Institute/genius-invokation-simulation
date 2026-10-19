//! 手牌这块自己的**选择列表 + 去重入口**（D26 / D91 / D92 / D97）。
//!
//! - D91：每个随机点自己写自己的去重函数、自己传；手牌选择的折叠入口就在本文件，
//!   框架不提供「通用去重」。
//! - D97/D98：手牌与牌库共用同一套**组合折叠**（顺序一律不区分）；
//!   判据实现在 action_card zones 共享层；**入口仍放在各自块**（这里是手牌的入口）。
//! - D104/D105：状态是一个公共列表（families/status/state.rs）；手牌状态以后接上时，
//!   这里把 CardInstance 的状态一并转成 FoldCard.mark；当前手牌元素还没有状态，
//!   所以一律按「空状态键」参与判据。
//! - 概率是精确分数 [`Fraction`]（D158 / D159），不经过浮点。
//!
//! 手牌枚举本身（比如「从手牌选 2 张弃掉」）依赖引擎选择原语；本文件只提供折叠入口，
//! 供手牌那边的选择列表在拿到有序分支后调用。

use super::CardInstance;
use crate::families::action_card::zones::{fold_equivalent, FoldCard, FoldMark};
use gi_genius_invokation_engine::prob::Fraction;

/// 手牌选择分支的折叠入口（D97）。
///
/// 输入：全部**有序**分支（概率来自引擎枚举，均匀）；picked 取出该分支里被选中的手牌。
/// 输出：按等价类分组后的代表元 + 组内权重和（概率仍是「组内原始分支数 / 总数」，非均匀）。
///
/// 判据：一对牌顺序无关 ⟺ 同卡牌 id / 都无标记 / 标记相同（OR）；
/// 等价 ⟺ (卡牌 id, 标记) 多重集相同且所有顺序有关牌对先后一致。
pub fn fold_hand_branches<B>(
    branches: Vec<(B, Fraction)>,
    picked: impl Fn(&B) -> Vec<CardInstance>,
) -> Vec<(B, Fraction)> {
    fold_equivalent(branches, |branch| {
        picked(branch)
            .iter()
            .map(|instance| FoldCard {
                card: instance.card,
                // TODO(D104/D105)：手牌状态接上后，这里从公共状态列表按位置取出状态键。
                mark: FoldMark::empty(),
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::CardId;
    use gi_genius_invokation_engine::nat::ExactNat128;
    use gi_genius_invokation_engine::Nat;

    fn frac(num: u128, den: u128) -> Fraction {
        Fraction::new(ExactNat128::new(num), ExactNat128::new(den))
    }

    fn instance(entity: Nat, card: Nat) -> CardInstance {
        CardInstance::new(entity, CardId::new(card.get()))
    }

    fn total(branches: &[(Vec<CardInstance>, Fraction)]) -> Fraction {
        branches.iter().map(|(_, probability)| *probability).sum()
    }

    #[test]
    fn hand_same_card_id_orders_fold() {
        // 两张不同实体、同一张卡：先后看不出差别 -> 1 支
        let a = instance(Nat::ZERO, Nat::new(5));
        let b = instance(Nat::new(1), Nat::new(5));
        let branches = vec![
            (vec![a, b], frac(1, 2)),
            (vec![b, a], frac(1, 2)),
        ];
        let folded = fold_hand_branches(branches, |picked| picked.clone());
        assert_eq!(folded.len(), 1);
        assert_eq!(folded.first().expect("非空").1, Fraction::ONE);
    }

    #[test]
    fn hand_unmarked_orders_fold_to_combinations() {
        // 三张都无标记：任意两张的先后不计 -> C(3,2) = 3 支，每支 2/6 = 1/3
        let a = instance(Nat::ZERO, Nat::ZERO);
        let b = instance(Nat::new(1), Nat::new(1));
        let c = instance(Nat::new(2), Nat::new(2));
        let mut branches: Vec<(Vec<CardInstance>, Fraction)> = Vec::new();
        for first in [a, b, c] {
            for second in [a, b, c] {
                if first.entity == second.entity {
                    continue;
                }
                branches.push((vec![first, second], frac(1, 6)));
            }
        }
        let folded = fold_hand_branches(branches, |picked| picked.clone());
        assert_eq!(folded.len(), 3);
        assert!(total(&folded).is_one());
        for (_branch, probability) in &folded {
            assert_eq!(*probability, frac(1, 3));
        }
    }
}
