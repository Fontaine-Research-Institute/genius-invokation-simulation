//! 冒险 / 冒险地点（tags=冒险地点）。
//!
//! **依据**：
//! - 权威关键字 #22「冒险」：「从冒险地点中挑选 1 个生成。如果我方已存在冒险地点，
//!   则改为使该冒险地点的冒险经历 +1。」
//! - 4 张冒险地点：`321032` 沉玉谷（冒险经历达到 2/4/8 触发）、`321040` 层岩巨渊、
//!   `321033` 自体自身之塔、`321034` 天蛇船——阈值效果属**具体卡**，不在规则层实现。
//!
//! Lead 裁定 7（待用户复核）：只做「冒险动作」接口 + 「己方已有该地点 -> 冒险经历 +1」；
//! **不额外限制**每方数量（上限自然由支援区 4 约束）。

use gi_genius_invokation_engine::Nat;

/// 已有冒险地点时，一次冒险使「冒险经历」+1。
pub const ADVENTURE_EXPERIENCE_GAIN: Nat = Nat::new(1);

/// 一次「冒险」动作的结果。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AdventureOutcome {
    /// 己方还没有冒险地点：从冒险地点中挑选 1 个生成。
    GenerateSpot,
    /// 己方已有冒险地点：该地点冒险经历 +1（增量见 `ADVENTURE_EXPERIENCE_GAIN`）。
    GainExperience(Nat),
}

/// 冒险动作（纯函数）：`existing_spot` = 己方是否已有冒险地点。
pub fn resolve(existing_spot: bool) -> AdventureOutcome {
    if existing_spot {
        AdventureOutcome::GainExperience(ADVENTURE_EXPERIENCE_GAIN)
    } else {
        AdventureOutcome::GenerateSpot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adventure_generates_or_gains_experience() {
        assert_eq!(resolve(false), AdventureOutcome::GenerateSpot);
        assert_eq!(
            resolve(true),
            AdventureOutcome::GainExperience(Nat::new(1))
        );
        assert_eq!(ADVENTURE_EXPERIENCE_GAIN, Nat::new(1));
    }
}
