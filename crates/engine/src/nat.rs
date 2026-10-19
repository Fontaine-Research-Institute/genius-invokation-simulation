//! 数值原语：`Nat` 族（饱和）与 `ExactNat` 族（精确，D153）。
//!
//! - **业务数值默认一律用 `Nat`（= `Nat32`，32 位）**；只有"特意设计"的场合才用别的宽度，
//!   且必须在 git 之外的 `不入库/nat-宽度统计.md` 登记。
//! - 两族各五个宽度：
//!   - 饱和族（[`nat_type!`]）：`Nat8` / `Nat16` / `Nat32` / `Nat64` / `Nat128`，
//!     默认 `Nat = Nat32`；原生类型 `u8` / `u16` / `u32` / `u64` / `u128`；
//!   - 精确族（[`exact_nat_type!`]，D153）：`ExactNat8` / `ExactNat16` / `ExactNat32` /
//!     `ExactNat64` / `ExactNat128`，默认 `ExactNat = ExactNat128`（**分数用它**，D152 / D153）。
//! - **错误处理按族区分，别混为一谈**：
//!   - 饱和族：加 / 减 / 乘**饱和**（溢出停在 `MAX`、减法到 `ZERO` 停）——
//!     **溢出是正常业务**，不 panic；
//!   - 精确族：加 / 减 / 乘**上溢、下溢都 panic**（D153：概率不允许静默失真）；
//!   - 两族除法：**除零是 bug**，`Div` 运算符与 `ndiv!` 一律 panic（debug / release 一致）。
//! - 宏 `nadd!` / `nsub!` / `nmul!` / `ndiv!` **两族共用**（D155），通过 [`NatArith`] 按类型分派：
//!   传 `Nat` 就是饱和、传 `ExactNat` 就是精确。
//! - 除法**只有这一种口径**：不提供 `div_or_zero` / `saturating_div` 这类"除零不崩"的变体，
//!   避免两套语义并存。

use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

/// 生成一个饱和数值类型（一族五个宽度；**默认用 `Nat`，不要直接手写这个宏的实例**）。
///
/// 生成的类型带：`ZERO` / `MAX` / `const fn new` / `const fn get` /
/// `saturating_add` / `saturating_sub` / `saturating_mul` /
/// 四个运算符（加/减/乘饱和；除零 panic）/ `Display` / `Debug` / 与原生类型的双向 `From`。
macro_rules! nat_type {
    (
        $(#[$attr:meta])*
        $name:ident, $native:ty
    ) => {
        $(#[$attr])*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name($native);

        impl $name {
            /// 加法/乘法的单位元、下界。
            pub const ZERO: $name = $name(0);
            /// 饱和上界。
            pub const MAX: $name = $name(<$native>::MAX);

            /// **const 构造**（写常量用）。
            pub const fn new(v: $native) -> $name {
                $name(v)
            }

            /// 取回原生类型（只读；不参与数值运算）。
            pub const fn get(self) -> $native {
                self.0
            }

            /// 饱和加：溢出停在 `MAX`（**溢出是正常业务**，不 panic）。
            pub const fn saturating_add(self, rhs: $name) -> $name {
                $name(self.0.saturating_add(rhs.0))
            }

            /// 饱和减：到 0 就停，不出负数（**下溢是正常业务**，不 panic）。
            pub const fn saturating_sub(self, rhs: $name) -> $name {
                $name(self.0.saturating_sub(rhs.0))
            }

            /// 饱和乘：溢出停在 `MAX`（**溢出是正常业务**，不 panic）。
            pub const fn saturating_mul(self, rhs: $name) -> $name {
                $name(self.0.saturating_mul(rhs.0))
            }
        }

        impl From<$native> for $name {
            #[inline]
            fn from(v: $native) -> $name {
                $name(v)
            }
        }

        impl From<$name> for $native {
            #[inline]
            fn from(n: $name) -> $native {
                n.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }

        impl Add for $name {
            type Output = $name;

            #[inline]
            fn add(self, rhs: $name) -> $name {
                self.saturating_add(rhs)
            }
        }

        impl Sub for $name {
            type Output = $name;

            #[inline]
            fn sub(self, rhs: $name) -> $name {
                self.saturating_sub(rhs)
            }
        }

        impl Mul for $name {
            type Output = $name;

            #[inline]
            fn mul(self, rhs: $name) -> $name {
                self.saturating_mul(rhs)
            }
        }

        impl Div for $name {
            type Output = $name;

            /// 除法：**除零 panic**（除零是逻辑错误；加/减/乘才是饱和）。
            #[inline]
            fn div(self, rhs: $name) -> $name {
                assert!(
                    rhs.0 != 0,
                    "除零：这是逻辑错误（溢出是正常业务，除零是 bug）"
                );
                $name(self.0 / rhs.0)
            }
        }
    };
}

nat_type!(
    /// 饱和 8 位无符号数值（**非特意设计不要用**；默认用 `Nat`）。
    Nat8, u8
);

nat_type!(
    /// 饱和 16 位无符号数值（**非特意设计不要用**；默认用 `Nat`）。
    Nat16, u16
);

nat_type!(
    /// 饱和 32 位无符号数值（**默认就用它**：`Nat` 是它的别名）。
    Nat32, u32
);

nat_type!(
    /// 饱和 64 位无符号数值（**非特意设计不要用**；默认用 `Nat`）。
    Nat64, u64
);

nat_type!(
    /// 饱和 128 位无符号数值（**非特意设计不要用**；默认用 `Nat`）。
    Nat128, u128
);

/// **默认数值类型**：就是 32 位的 `Nat32`（别名，同一类型）。
///
/// 所以现有代码（`EntityId = Nat` 等）继续写 `Nat` 即可。
pub type Nat = Nat32;

// 编译期断言：`Nat` 与 `Nat32` 是**同一个类型**（别名），双向直接赋值、无需任何转换。
// 若二者不是同一类型，这两行会编译失败。
const _: () = {
    let _: Nat = Nat32::ZERO;
    let _: Nat32 = Nat::ZERO;
};

/// 生成一个**精确**数值类型（决策日志 D153）：加 / 减 / 乘**上溢与下溢都 panic**，除零也 panic。
///
/// 与 [`nat_type!`] 生成的饱和族**并列存在、互不替换**：
/// - 业务数值仍用饱和的 `Nat` 族（溢出是正常业务，不 panic）；
/// - 概率 / 分数用精确的 `ExactNat` 族（D153：**饱和 = 概率静默失真**，必须 panic）。
///
/// 生成的类型带：`ZERO` / `MAX` / `const fn new` / `const fn get` /
/// `exact_add` / `exact_sub` / `exact_mul`（溢出 panic）/
/// 四个运算符（加/减/乘精确、除零 panic）/ `Display` / `Debug` / 与原生类型的双向 `From`。
macro_rules! exact_nat_type {
    (
        $(#[$attr:meta])*
        $name:ident, $native:ty
    ) => {
        $(#[$attr])*
        #[repr(transparent)]
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name($native);

        impl $name {
            /// 下界（也是减法下溢的边界）。
            pub const ZERO: $name = $name(0);
            /// 上界（也是加法/乘法上溢的边界）。
            pub const MAX: $name = $name(<$native>::MAX);

            /// **const 构造**（写常量用）。
            pub const fn new(v: $native) -> $name {
                $name(v)
            }

            /// 取回原生类型（只读；不参与数值运算）。
            pub const fn get(self) -> $native {
                self.0
            }

            /// 精确加：**上溢 panic**（精确族不允许饱和）。
            pub const fn exact_add(self, rhs: $name) -> $name {
                match self.0.checked_add(rhs.0) {
                    Some(value) => $name(value),
                    None => panic!("溢出：ExactNat 加法上溢（精确族不允许饱和）"),
                }
            }

            /// 精确减：**下溢 panic**（精确族不允许饱和）。
            pub const fn exact_sub(self, rhs: $name) -> $name {
                match self.0.checked_sub(rhs.0) {
                    Some(value) => $name(value),
                    None => panic!("溢出：ExactNat 减法下溢（精确族不允许饱和）"),
                }
            }

            /// 精确乘：**上溢 panic**（精确族不允许饱和）。
            pub const fn exact_mul(self, rhs: $name) -> $name {
                match self.0.checked_mul(rhs.0) {
                    Some(value) => $name(value),
                    None => panic!("溢出：ExactNat 乘法上溢（精确族不允许饱和）"),
                }
            }
        }

        impl From<$native> for $name {
            #[inline]
            fn from(v: $native) -> $name {
                $name(v)
            }
        }

        impl From<$name> for $native {
            #[inline]
            fn from(n: $name) -> $native {
                n.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name)).field(&self.0).finish()
            }
        }

        impl Add for $name {
            type Output = $name;

            /// 精确加：溢出 panic（**不是**饱和）。
            #[inline]
            fn add(self, rhs: $name) -> $name {
                self.exact_add(rhs)
            }
        }

        impl Sub for $name {
            type Output = $name;

            /// 精确减：下溢 panic（**不是**饱和）。
            #[inline]
            fn sub(self, rhs: $name) -> $name {
                self.exact_sub(rhs)
            }
        }

        impl Mul for $name {
            type Output = $name;

            /// 精确乘：溢出 panic（**不是**饱和）。
            #[inline]
            fn mul(self, rhs: $name) -> $name {
                self.exact_mul(rhs)
            }
        }

        impl Div for $name {
            type Output = $name;

            /// 除法：**除零 panic**（与饱和族同一口径）；无符号除法本身不会溢出。
            #[inline]
            fn div(self, rhs: $name) -> $name {
                assert!(
                    rhs.0 != 0,
                    "除零：这是逻辑错误（溢出是正常业务，除零是 bug）"
                );
                $name(self.0 / rhs.0)
            }
        }
    };
}

exact_nat_type!(
    /// 精确 8 位无符号数值（溢出 panic；**非特意设计不要用**）。
    ExactNat8, u8
);

exact_nat_type!(
    /// 精确 16 位无符号数值（溢出 panic；**非特意设计不要用**）。
    ExactNat16, u16
);

exact_nat_type!(
    /// 精确 32 位无符号数值（溢出 panic；**非特意设计不要用**）。
    ExactNat32, u32
);

exact_nat_type!(
    /// 精确 64 位无符号数值（溢出 panic；**非特意设计不要用**）。
    ExactNat64, u64
);

exact_nat_type!(
    /// 精确 128 位无符号数值（**分数默认用它**：`ExactNat` 是它的别名）。
    ExactNat128, u128
);

/// **精确数值默认类型**：就是 128 位的 `ExactNat128`（别名，同一类型）。
///
/// 分数（[`crate::prob::Fraction`]）的分子 / 分母用它（D152 / D153）。
pub type ExactNat = ExactNat128;

// 编译期断言：`ExactNat` 与 `ExactNat128` 是**同一个类型**（别名）。
const _: () = {
    let _: ExactNat = ExactNat128::ZERO;
    let _: ExactNat128 = ExactNat::ZERO;
};

/// 两族共用的算术分派（D155）：宏 `nadd!` / `nsub!` / `nmul!` / `ndiv!` 统一展开到这里。
///
/// - `Nat8` / `Nat16` / `Nat32` / `Nat64` / `Nat128`（**饱和族**）：加 / 减 / 乘**饱和**
///   （溢出停在 `MAX`、减法到 `ZERO` 停，不 panic）；
/// - `ExactNat8` / `ExactNat16` / `ExactNat32` / `ExactNat64` / `ExactNat128`（**精确族**）：
///   加 / 减 / 乘**上溢或下溢一律 panic**（D153）；
/// - 两族除法都**除零 panic**（与 `Div` 运算符一致）。
///
/// 语义差异由**类型**承载：调用点只写 `nadd!`，传 `Nat` 就是饱和、传 `ExactNat` 就是精确。
pub trait NatArith: Copy {
    /// 加法（按类型的语义：饱和或精确）。
    fn nat_add(self, rhs: Self) -> Self;
    /// 减法（按类型的语义：饱和或精确）。
    fn nat_sub(self, rhs: Self) -> Self;
    /// 乘法（按类型的语义：饱和或精确）。
    fn nat_mul(self, rhs: Self) -> Self;
    /// 除法（两族：除零 panic）。
    fn nat_div(self, rhs: Self) -> Self;
}

/// 给饱和族实现 [`NatArith`]：加 / 减 / 乘走饱和方法。
macro_rules! impl_nat_arith_saturating {
    ($($ty:ident),* $(,)?) => {
        $(
            impl NatArith for $ty {
                #[inline]
                fn nat_add(self, rhs: $ty) -> $ty {
                    self.saturating_add(rhs)
                }

                #[inline]
                fn nat_sub(self, rhs: $ty) -> $ty {
                    self.saturating_sub(rhs)
                }

                #[inline]
                fn nat_mul(self, rhs: $ty) -> $ty {
                    self.saturating_mul(rhs)
                }

                #[inline]
                fn nat_div(self, rhs: $ty) -> $ty {
                    self / rhs
                }
            }
        )*
    };
}

impl_nat_arith_saturating!(Nat8, Nat16, Nat32, Nat64, Nat128);

/// 给精确族实现 [`NatArith`]：加 / 减 / 乘走精确方法（溢出 panic）。
macro_rules! impl_nat_arith_exact {
    ($($ty:ident),* $(,)?) => {
        $(
            impl NatArith for $ty {
                #[inline]
                fn nat_add(self, rhs: $ty) -> $ty {
                    self.exact_add(rhs)
                }

                #[inline]
                fn nat_sub(self, rhs: $ty) -> $ty {
                    self.exact_sub(rhs)
                }

                #[inline]
                fn nat_mul(self, rhs: $ty) -> $ty {
                    self.exact_mul(rhs)
                }

                #[inline]
                fn nat_div(self, rhs: $ty) -> $ty {
                    self / rhs
                }
            }
        )*
    };
}

impl_nat_arith_exact!(ExactNat8, ExactNat16, ExactNat32, ExactNat64, ExactNat128);

/// 加法宏（**两族共用**，语义由类型决定，见 [`NatArith`]；D155）。
///
/// - `nadd!(Nat::MAX, Nat::new(1))` -> `Nat::MAX`（饱和，不 panic）；
/// - `nadd!(ExactNat::MAX, ExactNat::new(1))` -> panic（精确族不允许饱和）。
#[macro_export]
macro_rules! nadd {
    ($a:expr, $b:expr) => {
        $crate::nat::NatArith::nat_add($a, $b)
    };
}

/// 减法宏（**两族共用**）：饱和族到 0 就停；精确族下溢 panic。
#[macro_export]
macro_rules! nsub {
    ($a:expr, $b:expr) => {
        $crate::nat::NatArith::nat_sub($a, $b)
    };
}

/// 乘法宏（**两族共用**）：饱和族溢出停在 `MAX`；精确族上溢 panic。
#[macro_export]
macro_rules! nmul {
    ($a:expr, $b:expr) => {
        $crate::nat::NatArith::nat_mul($a, $b)
    };
}

/// 除法宏（**两族共用**：**除零 panic**，与 `Div` 运算符一致；加/减/乘的语义见 [`NatArith`]）。
#[macro_export]
macro_rules! ndiv {
    ($a:expr, $b:expr) => {
        $crate::nat::NatArith::nat_div($a, $b)
    };
}


#[cfg(test)]
mod tests {
    use super::*;

    fn n(v: u32) -> Nat {
        Nat::new(v)
    }

    /// 五个宽度各测一遍：
    /// 加溢出 / 减下溢 / 乘溢出（饱和，不 panic）；
    /// 正常除法结果正确；除 0 在 `Div` 与 `ndiv!` 下都 panic。
    macro_rules! width_tests {
        ($mod_name:ident, $ty:ident, $native:ty, $max:expr) => {
            mod $mod_name {
                use crate::nat::$ty;

                fn n(v: $native) -> $ty {
                    $ty::new(v)
                }

                #[test]
                fn add_saturates_on_overflow() {
                    assert_eq!(n(2).saturating_add(n(3)), n(5));
                    assert_eq!(n($max).saturating_add(n(1)), $ty::MAX);
                    assert_eq!($ty::MAX.saturating_add($ty::MAX), $ty::MAX);
                    assert_eq!(n($max) + n(1), $ty::MAX);
                }

                #[test]
                fn sub_underflow_returns_zero() {
                    assert_eq!(n(5).saturating_sub(n(3)), n(2));
                    assert_eq!(n(3).saturating_sub(n(5)), $ty::ZERO);
                    assert_eq!($ty::ZERO.saturating_sub(n(1)), $ty::ZERO);
                    assert_eq!(n(3) - n(5), $ty::ZERO);
                }

                #[test]
                fn mul_saturates_on_overflow() {
                    assert_eq!(n(6).saturating_mul(n(7)), n(42));
                    assert_eq!(n($max).saturating_mul(n(2)), $ty::MAX);
                    assert_eq!($ty::MAX.saturating_mul($ty::MAX), $ty::MAX);
                    assert_eq!(n(2) * $ty::MAX, $ty::MAX);
                }

                #[test]
                fn div_normal_results() {
                    assert_eq!(n(7) / n(2), n(3));
                    assert_eq!(n(6) / n(2), n(3));
                    assert_eq!($ty::ZERO / n(5), $ty::ZERO);
                    assert_eq!(crate::ndiv!(n(7), n(2)), n(3));
                }

                #[test]
                #[should_panic(expected = "除零")]
                fn div_by_zero_panics() {
                    let _ = n(7) / $ty::ZERO;
                }

                #[test]
                #[should_panic(expected = "除零")]
                fn ndiv_by_zero_panics() {
                    let _ = crate::ndiv!(n(7), $ty::ZERO);
                }
            }
        };
    }

    width_tests!(width_nat8, Nat8, u8, u8::MAX);
    width_tests!(width_nat16, Nat16, u16, u16::MAX);
    width_tests!(width_nat32, Nat32, u32, u32::MAX);
    width_tests!(width_nat64, Nat64, u64, u64::MAX);
    width_tests!(width_nat128, Nat128, u128, u128::MAX);

    #[test]
    fn nat_is_nat32_same_type() {
        // 编译期已由文件里的 const 断言；这里再确认运行期数值一致。
        assert_eq!(Nat::MAX, Nat32::MAX);
        assert_eq!(Nat::ZERO, Nat32::ZERO);
        assert_eq!(std::mem::size_of::<Nat>(), std::mem::size_of::<Nat32>());
        assert_eq!(std::mem::size_of::<Nat>(), std::mem::size_of::<u32>());
    }

    #[test]
    fn consts_and_accessors() {
        assert_eq!(Nat::ZERO, n(0));
        assert_eq!(Nat::MAX, n(u32::MAX));
        assert_eq!(n(1418).get(), 1418);
        assert_eq!(Nat::default(), Nat::ZERO);
    }

    #[test]
    fn from_conversions_roundtrip() {
        assert_eq!(Nat::from(7u32), n(7));
        assert_eq!(u32::from(n(7)), 7);
        // 其它宽度各取一个原生类型验证双向 From
        assert_eq!(Nat8::from(7u8), Nat8::new(7));
        assert_eq!(u8::from(Nat8::new(7)), 7);
        assert_eq!(Nat64::from(7u64), Nat64::new(7));
        assert_eq!(u64::from(Nat64::new(7)), 7);
        assert_eq!(Nat128::from(7u128), Nat128::new(7));
        assert_eq!(u128::from(Nat128::new(7)), 7);
    }

    #[test]
    fn add_saturates_on_overflow() {
        assert_eq!(n(2).saturating_add(n(3)), n(5));
        assert_eq!(n(u32::MAX).saturating_add(n(1)), Nat::MAX);
        assert_eq!(Nat::MAX.saturating_add(Nat::MAX), Nat::MAX);
        // 运算符同样饱和
        assert_eq!(n(u32::MAX) + n(1), Nat::MAX);
    }

    #[test]
    fn sub_underflow_returns_zero() {
        assert_eq!(n(5).saturating_sub(n(3)), n(2));
        assert_eq!(n(3).saturating_sub(n(5)), Nat::ZERO);
        assert_eq!(Nat::ZERO.saturating_sub(n(1)), Nat::ZERO);
        // 运算符同样饱和
        assert_eq!(n(3) - n(5), Nat::ZERO);
    }

    #[test]
    fn mul_saturates_on_overflow() {
        assert_eq!(n(6).saturating_mul(n(7)), n(42));
        assert_eq!(n(u32::MAX).saturating_mul(n(2)), Nat::MAX);
        assert_eq!(Nat::MAX.saturating_mul(Nat::MAX), Nat::MAX);
        // 运算符同样饱和
        assert_eq!(n(2) * n(u32::MAX), Nat::MAX);
    }

    #[test]
    fn div_normal_results() {
        assert_eq!(n(7) / n(2), n(3));
        assert_eq!(n(6) / n(2), n(3));
        assert_eq!(Nat::ZERO / n(5), Nat::ZERO);
        assert_eq!(ndiv!(n(7), n(2)), n(3));
    }

    #[test]
    #[should_panic(expected = "除零")]
    fn div_by_zero_panics() {
        // 除零是 bug：Div 运算符 panic（不是饱和、不是得 0）
        let _ = n(7) / Nat::ZERO;
    }

    #[test]
    #[should_panic(expected = "除零")]
    fn ndiv_by_zero_panics() {
        // 除零是 bug：ndiv! 同样 panic
        let _ = ndiv!(n(7), Nat::ZERO);
    }

    #[test]
    fn macros_are_usable() {
        let a = n(5);
        let b = n(3);
        assert_eq!(nadd!(a, b), n(8));
        assert_eq!(nsub!(b, a), Nat::ZERO);
        assert_eq!(nmul!(a, b), n(15));
        assert_eq!(ndiv!(a, b), n(1));
        // 宏在表达式里可嵌套
        assert_eq!(nadd!(nmul!(a, b), nsub!(a, b)), n(17));
        // 加/减/乘溢出仍然饱和（不 panic）
        assert_eq!(nadd!(Nat::MAX, n(1)), Nat::MAX);
        // 宏也能直接吃临时值
        assert_eq!(nadd!(n(1), n(2)), n(3));
        // 对其它宽度自动通用（调方法，不认具体宽度）
        assert_eq!(nadd!(Nat8::new(200), Nat8::new(100)), Nat8::MAX);
        assert_eq!(ndiv!(Nat64::new(9), Nat64::new(3)), Nat64::new(3));
    }

    #[test]
    fn display_and_ordering() {
        assert_eq!(n(42).to_string(), "42");
        assert_eq!(format!("{}", Nat::MAX), u32::MAX.to_string());
        assert_eq!(format!("{:?}", n(42)), "Nat32(42)");
        assert_eq!(format!("{:?}", Nat8::new(7)), "Nat8(7)");
        assert!(n(1) < n(2));
        assert_eq!(n(3).cmp(&n(3)), std::cmp::Ordering::Equal);
    }
}


#[cfg(test)]
mod exact_nat_tests {
    use super::*;

    /// 五个精确宽度各测一遍：正常路径不变；加/乘上溢、减下溢（方法与运算符两条路）、
    /// 除零都 panic；宏与运算符同语义。
    macro_rules! exact_width_tests {
        ($mod_name:ident, $ty:ident, $native:ty) => {
            mod $mod_name {
                use crate::nat::$ty;

                fn n(v: $native) -> $ty {
                    $ty::new(v)
                }

                #[test]
                fn normal_paths_are_exact() {
                    assert_eq!(n(2).exact_add(n(3)), n(5));
                    assert_eq!(n(5).exact_sub(n(3)), n(2));
                    assert_eq!(n(6).exact_mul(n(7)), n(42));
                    assert_eq!(n(7) / n(2), n(3));
                    assert_eq!(n(2) + n(3), n(5));
                    assert_eq!(n(5) - n(3), n(2));
                    assert_eq!(n(6) * n(7), n(42));
                    assert_eq!($ty::ZERO.exact_add($ty::ZERO), $ty::ZERO);
                    assert_eq!($ty::MAX.exact_sub($ty::MAX), $ty::ZERO);
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn add_overflow_by_method_panics() {
                    let _ = $ty::MAX.exact_add(n(1));
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn add_overflow_by_operator_panics() {
                    // 运算符同样 panic（不是 debug-only、不是 wrap）
                    let _ = $ty::MAX + n(1);
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn sub_underflow_by_method_panics() {
                    let _ = $ty::ZERO.exact_sub(n(1));
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn sub_underflow_by_operator_panics() {
                    let _ = $ty::ZERO - n(1);
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn mul_overflow_by_method_panics() {
                    let _ = $ty::MAX.exact_mul(n(2));
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn mul_overflow_by_operator_panics() {
                    let _ = $ty::MAX * n(2);
                }

                #[test]
                #[should_panic(expected = "除零")]
                fn div_by_zero_panics() {
                    let _ = n(7) / $ty::ZERO;
                }

                #[test]
                #[should_panic(expected = "除零")]
                fn ndiv_by_zero_panics() {
                    let _ = crate::ndiv!(n(7), $ty::ZERO);
                }

                #[test]
                fn macros_dispatch_to_exact() {
                    // 同一套宏（nadd! 等），传精确族就按精确语义算（D155）
                    assert_eq!(crate::nadd!(n(2), n(3)), n(5));
                    assert_eq!(crate::nsub!(n(5), n(3)), n(2));
                    assert_eq!(crate::nmul!(n(6), n(7)), n(42));
                    assert_eq!(crate::ndiv!(n(7), n(2)), n(3));
                    // 宏也能直接吃临时值
                    assert_eq!(crate::nadd!(n(1), n(2)), n(3));
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn nadd_overflow_panics() {
                    let _ = crate::nadd!($ty::MAX, n(1));
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn nsub_underflow_panics() {
                    let _ = crate::nsub!($ty::ZERO, n(1));
                }

                #[test]
                #[should_panic(expected = "溢出")]
                fn nmul_overflow_panics() {
                    let _ = crate::nmul!($ty::MAX, n(2));
                }
            }
        };
    }

    exact_width_tests!(exact_width_nat8, ExactNat8, u8);
    exact_width_tests!(exact_width_nat16, ExactNat16, u16);
    exact_width_tests!(exact_width_nat32, ExactNat32, u32);
    exact_width_tests!(exact_width_nat64, ExactNat64, u64);
    exact_width_tests!(exact_width_nat128, ExactNat128, u128);

    #[test]
    fn exact_nat_is_exact_nat128_same_type() {
        // 编译期已由文件里的 const 断言；这里再确认运行期数值与尺寸一致。
        let _: ExactNat = ExactNat128::ZERO;
        let _: ExactNat128 = ExactNat::ZERO;
        assert_eq!(ExactNat::MAX, ExactNat128::MAX);
        assert_eq!(std::mem::size_of::<ExactNat>(), std::mem::size_of::<u128>());
    }

    #[test]
    fn from_conversions_roundtrip() {
        assert_eq!(ExactNat::from(7u128), ExactNat::new(7));
        assert_eq!(u128::from(ExactNat::new(7)), 7);
        assert_eq!(ExactNat8::from(7u8), ExactNat8::new(7));
        assert_eq!(u8::from(ExactNat8::new(7)), 7);
        assert_eq!(ExactNat64::from(7u64), ExactNat64::new(7));
        assert_eq!(u64::from(ExactNat64::new(7)), 7);
    }

    #[test]
    fn display_and_ordering() {
        assert_eq!(ExactNat::new(42).to_string(), "42");
        assert_eq!(format!("{:?}", ExactNat::new(42)), "ExactNat128(42)");
        assert!(ExactNat::new(1) < ExactNat::new(2));
    }
}

