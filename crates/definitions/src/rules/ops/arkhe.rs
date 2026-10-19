//! 始基力：荒性 / 芒性（tags=始基力：荒性 / 芒性）。
//!
//! **依据**：
//! - 权威关键字 #87「『失能形态』」与 #88「荒芒湮灭效应」：
//!   「具有『荒性』的角色在受到具有『始基力：芒性』的伤害后，或具有『芒性』的角色在受到
//!   具有『始基力：荒性』的伤害后，将会发生荒芒湮灭效应。」
//! - 数据里 9 张带「荒性」、6 张带「芒性」：如 `1313` 夏沃蕾（荒）、`1412` 克洛琳德（荒）、
//!   `1110` 夏洛蒂（芒）、`1212` 芙宁娜（芒）；行动牌 `423045` 机巧符术·爆发攻势·荒性 也带荒性。
//!
//! 标签本身只标「角色属于哪一极」；湮灭效应的伤害侧实现归**角色 / 伤害块**（D133 / D164），
//! 规则层只提供「哪一极是相反极」这一条判定依据。

/// 始基力两极。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Arkhe {
    /// 荒性。
    Ousia,
    /// 芒性。
    Pneuma,
}

/// 相反极（荒 <-> 芒；关键字 #87 / #88）。
pub fn opposite(arkhe: Arkhe) -> Arkhe {
    match arkhe {
        Arkhe::Ousia => Arkhe::Pneuma,
        Arkhe::Pneuma => Arkhe::Ousia,
    }
}

/// 受到 `incoming` 极的伤害后，会不会发生荒芒湮灭效应（自身极必须是 incoming 的相反极）。
pub fn collides_with(self_arkhe: Arkhe, incoming: Arkhe) -> bool {
    opposite(self_arkhe) == incoming
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ousia_and_pneuma_are_opposites() {
        assert_eq!(opposite(Arkhe::Ousia), Arkhe::Pneuma);
        assert_eq!(opposite(Arkhe::Pneuma), Arkhe::Ousia);
        // 互逆
        assert_eq!(opposite(opposite(Arkhe::Ousia)), Arkhe::Ousia);
    }

    #[test]
    fn collision_only_on_opposite_arkhe() {
        assert!(collides_with(Arkhe::Ousia, Arkhe::Pneuma));
        assert!(collides_with(Arkhe::Pneuma, Arkhe::Ousia));
        assert!(!collides_with(Arkhe::Ousia, Arkhe::Ousia));
    }
}
