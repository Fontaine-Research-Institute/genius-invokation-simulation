# Crate-Engine (机制原语) 

> 中文 · [[Crate-Engine-EN|English]]

*本页范围: `engine` crate 的职责, 模块与公开入口, 不变量, 依据, 测试定位与待接入项.*

---

## 1. 职责 (这一层解决什么问题)

机制原语 crate: 只提供数值, 标识, 实体, 区容器, 随机与选择, 精确分数, 统计这些"没有任何游戏语义"的底层零件.默认零依赖; 上层 (`definitions` / `cards` / `simulation`) 依赖它, 它不依赖任何业务 crate.

- 目录: `new/crates/engine/`
- package / lib: `gi-genius-invokation-engine` / `gi_genius_invokation_engine`

## 2. 关键概念与类型

- 饱和数值族
  - 位置: `src/nat.rs`: `Nat8/16/32/64/128`, 默认 `Nat = Nat32`
  - 说明: 加 / 减 / 乘饱和 (溢出停 `MAX`, 减法到 `ZERO` 停, 不 panic) ; 除零 panic (D44 / D36 / D37)
- 精确数值族
  - 位置: `src/nat.rs`: `ExactNat8/16/32/64/128`, 默认 `ExactNat = ExactNat128`
  - 说明: 加 / 减 / 乘上溢 / 下溢都 panic (D153) ; 概率 / 分数用它 (D152 / D156)
- 算术宏
  - 位置: `src/nat.rs`: `nadd!` / `nsub!` / `nmul!` / `ndiv!`
  - 说明: 两族共用一套名字, 语义由类型决定 (D155)
- 实体标识
  - 位置: `src/id.rs`: `EntityId = Nat`
  - 说明: 单调不复用流水号; 引擎不记实体在哪个区 (D42)
- 列表下标
  - 位置: `src/id.rs`: `ActionId = u32`
  - 说明: 只是"列表第几项", 不参与数值运算
- 实体表
  - 位置: `src/entity.rs`: `EntityTable`
  - 说明: 只发号: `alloc` / `len` / `is_empty`
- 区容器
  - 位置: `src/area.rs`: `Area` (多实体) , `SingleArea` (单实体 = 变量)
  - 说明: 没有身份, 编号, 登记; 是状态字段的类型 (D42)
- 选择原语
  - 位置: `src/select.rs`: `SelectionSource<S>` + `enumerate_select`
  - 说明: 只收"列表 + 数量", 没有 `who` (D25) ; 列表形状归各块 `parse.rs` (D26)
- 随机原语
  - 位置: `src/random.rs`: `DedupFn` / `enumerate_random` / `enumerate_random_weighted`
  - 说明: 纯枚举, 不消费 PRNG; 分支概率是 `Fraction` (D149 / D158)
- 精确分数
  - 位置: `src/prob.rs`: `Fraction` / `total` / `normalize` / `pick_index`
  - 说明: 分子 / 分母 `ExactNat128`; 自动约分; 分母 0 panic
- 随机源
  - 位置: `src/rng.rs`: `Rng`
  - 说明: SplitMix64; 对外只有 `next_u128` / `next_below_u128` (拒绝采样, D160)
- 统计原语
  - 位置: `src/stats.rs`: `Counter` / `Seq<T>`
  - 说明: 普通值字段, 区自持, 对外只读 (D181–D183)
- 求解接口
  - 位置: `src/rules.rs`: `GameRules<S>` / `GameResult<P>` / `ActionList`
  - 说明: 仅 feature `rules` 时编译 (D24) ; 含 `BothLose` (D267)

**Cargo feature**

- 默认 (空): 零外部依赖
- `rules`: 编译 `src/rules.rs` (求解接口)
- `dedup`: 启用概率去重 (只优化分支数, 不改分布)
- `opt-all`: 全部计算优化 (当前 `= ["dedup"]`)
- `dll`: 机制 dll 加载 (`dep:libloading`) ——`src/load.rs` 尚未实现, 打开会编译失败

## 3. 公开入口

`src/lib.rs` 导出 `Area` / `SingleArea` / `EntityTable` / `ActionId` / `EntityId` / `Nat` / `enumerate_random` / `DedupFn` / `enumerate_select` / `SelectionSource`; 其余模块按路径使用.

- `Nat` / `ExactNat`: `ZERO` / `MAX` / `const fn new` / `get`; 饱和或精确的 `add/sub/mul`; 除零 panic
- `EntityTable`: `new` / `alloc(&mut self) -> EntityId` / `len` / `is_empty`
- `Area` / `SingleArea`: `push` / `take_at` (swap_remove) / `remove` / `as_slice`; `get` / `set` / `clear`
- `SelectionSource<S>`: `len` / `is_empty` / `apply(&self, &S, usize) -> (S, Fraction)`
- `enumerate_select`: `(state, list, count: Nat) -> Vec<(S, Fraction)>`
- `enumerate_random`: `(state, list, repeatable, all_same, times, dedup) -> Vec<(S, Fraction)>`
- `enumerate_random_weighted`: 增加 `weights: Option<&[Fraction]>`
- `Fraction`: `new(num, den)` (分母 0 panic) / `exact_add` / `exact_mul` / `exact_div` / `cmp_value`
- `Rng`: `from_seed(u64)` / `next_u128()` / `next_below_u128(n)` (`n == 0` panic)
- `Counter` / `Seq<T>`: 计数增 / 减 / 清零; 序列 push / get / iter / as_slice
- `GameRules<S>`: `type Player` + `to_move` / `terminal` / `actions` / `evaluate` (feature `rules`)

**边界行为 (写死) **

- `times == 0` / `list.len() == 0`: 空 `Vec`
- `all_same == true`: 先选 1 项, 其余次数都取它; 结果条数 = `list.len()` (优先于下面判定)
- `repeatable == false` 且 `times > list.len()`: 空 `Vec` (不 panic)
- `count == 1` (选择): `list.len()` 支, 每支 `1/len` (精确分数)
- `count > 1` (选择): 空 `Vec` (D25 未拍板, 不 panic)
- 权重长度不等 / 权重之和为 0: 空 `Vec` (不 panic)
- 分数分母 0 / 除零 / `next_below_u128(0)`: panic (逻辑错误, D44)

### 3.1 真实源码

下面给出 Nat 族与生成宏 的实现:

文件: crates/engine/src/nat.rs:23-137

```rust
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
```

以上是 Nat 族与生成宏 的定义; 具体行为以该文件为准.


下面给出 默认数值类型 `Nat` 的实现:

文件: crates/engine/src/nat.rs:164-167

```rust
/// **默认数值类型**：就是 32 位的 `Nat32`（别名，同一类型）。
///
/// 所以现有代码（`EntityId = Nat` 等）继续写 `Nat` 即可。
pub type Nat = Nat32;
```

以上是 默认数值类型 `Nat` 的定义; 具体行为以该文件为准.


下面给出 两族共用的算术分派 的实现:

文件: crates/engine/src/nat.rs:344-362

```rust
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
```

以上是 两族共用的算术分派 的定义; 具体行为以该文件为准.


下面给出 加法宏 `nadd!` 的实现:

文件: crates/engine/src/nat.rs:426-435

```rust
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
```

以上是 加法宏 `nadd!` 的定义; 具体行为以该文件为准.


下面给出 除法宏 `ndiv!` (除零 panic) 的实现:

文件: crates/engine/src/nat.rs:453-459

```rust
/// 除法宏（**两族共用**：**除零 panic**，与 `Div` 运算符一致；加/减/乘的语义见 [`NatArith`]）。
#[macro_export]
macro_rules! ndiv {
    ($a:expr, $b:expr) => {
        $crate::nat::NatArith::nat_div($a, $b)
    };
}
```

以上是 除法宏 `ndiv!` (除零 panic) 的定义; 具体行为以该文件为准.


下面给出 实体标识 `EntityId` 的实现:

文件: crates/engine/src/id.rs:13-14

```rust
/// 实体 id：从 0 起、单调、不复用的流水号（由实体表发放）。
pub type EntityId = Nat;
```

以上是 实体标识 `EntityId` 的定义; 具体行为以该文件为准.


下面给出 列表下标 `ActionId` 的实现:

文件: crates/engine/src/id.rs:16-20

```rust
/// 列表下标（**不是数值**）。
///
/// 注意：本 crate 不参与上层"编译期裸整型扫描"，所以这里保留 `u32` 下标；
/// `ActionId` 只用于"列表第几项"，**不得参与数值运算**。
pub type ActionId = u32;
```

以上是 列表下标 `ActionId` 的定义; 具体行为以该文件为准.


下面给出 实体表 `EntityTable` 的实现:

文件: crates/engine/src/entity.rs:10-15

```rust
/// 实体表：**只发号**（从 0 起、单调、不复用）。
#[derive(Clone, Debug, Default)]
pub struct EntityTable {
    /// 已发放的实体数（= 下一个要发的号）。
    next: usize,
}
```

以上是 实体表 `EntityTable` 的定义; 具体行为以该文件为准.


下面给出 `EntityTable` 的方法 的实现:

文件: crates/engine/src/entity.rs:17-41

```rust
impl EntityTable {
    /// 一个号都还没发过。
    pub const fn new() -> Self {
        EntityTable { next: 0 }
    }

    /// 发号：从 0 起、单调、**不复用**。
    pub fn alloc(&mut self) -> EntityId {
        // id 是 u32 量级（40 亿+），实际对局不可能耗尽；
        // 真耗尽时宁可显式失败，也不发重复号（那会破坏"不复用"契约）。
        let raw = u32::try_from(self.next).expect("实体 id 空间耗尽");
        self.next += 1;
        Nat::new(raw)
    }

    /// 已发放的实体数。
    pub fn len(&self) -> usize {
        self.next
    }

    /// 一个号都还没发过。
    pub fn is_empty(&self) -> bool {
        self.next == 0
    }
}
```

以上是 `EntityTable` 的方法 的定义; 具体行为以该文件为准.


下面给出 多实体容器 `Area` 的实现:

文件: crates/engine/src/area.rs:12-18

```rust
/// 多实体容器：只保证"放/取不频繁搬动 + 不设容量上限"。
///
/// 不保证顺序：[`Area::take_at`] 走 `swap_remove` 语义（末尾元素填补空位）。
#[derive(Clone, Debug, Default)]
pub struct Area {
    items: Vec<EntityId>,
}
```

以上是 多实体容器 `Area` 的定义; 具体行为以该文件为准.


下面给出 `Area` 的操作 的实现:

文件: crates/engine/src/area.rs:20-84

```rust
impl Area {
    /// 空容器。
    pub const fn new() -> Self {
        Area { items: Vec::new() }
    }

    /// 放进来，返回它所在的下标（末尾）。
    pub fn push(&mut self, id: EntityId) -> usize {
        let index = self.items.len();
        self.items.push(id);
        index
    }

    /// 实体个数。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 里面一个实体都没有。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 只读切片（顺序 = 当前存放顺序，不保证稳定）。
    pub fn as_slice(&self) -> &[EntityId] {
        &self.items
    }

    /// 按当前顺序遍历。
    pub fn iter(&self) -> std::slice::Iter<'_, EntityId> {
        self.items.iter()
    }

    /// 是否装着这个实体。
    pub fn contains(&self, id: EntityId) -> bool {
        self.items.contains(&id)
    }

    /// 这个实体的下标（重复放入同一实体时给第一个）。
    pub fn index_of(&self, id: EntityId) -> Option<usize> {
        self.items.iter().position(|item| *item == id)
    }

    /// O(1) 取出：**swap_remove 语义**（不保证顺序，末尾元素填补空位）。
    ///
    /// 下标越界 -> `None`，容器不变。
    pub fn take_at(&mut self, index: usize) -> Option<EntityId> {
        if index < self.items.len() {
            Some(self.items.swap_remove(index))
        } else {
            None
        }
    }

    /// 按实体摘除（swap_remove 语义）；返回是否真的摘到了。
    pub fn remove(&mut self, id: EntityId) -> bool {
        match self.index_of(id) {
            Some(index) => {
                self.items.swap_remove(index);
                true
            }
            None => false,
        }
    }
}
```

以上是 `Area` 的操作 的定义; 具体行为以该文件为准.


下面给出 单实体容器 `SingleArea` 的实现:

文件: crates/engine/src/area.rs:86-92

```rust
/// 单实体容器 = 一个"变量"：要么没有，要么只有一个实体。
///
/// 用途举例（上层）：像"要么没有、要么只有一张"的那种位置。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SingleArea {
    item: Option<EntityId>,
}
```

以上是 单实体容器 `SingleArea` 的定义; 具体行为以该文件为准.


下面给出 `SingleArea` 的操作 的实现:

文件: crates/engine/src/area.rs:94-119

```rust
impl SingleArea {
    /// 空变量。
    pub const fn new() -> Self {
        SingleArea { item: None }
    }

    /// 当前装着的实体。
    pub const fn get(&self) -> Option<EntityId> {
        self.item
    }

    /// 放入（**替换**旧的），返回被替换掉的。
    pub fn set(&mut self, id: EntityId) -> Option<EntityId> {
        self.item.replace(id)
    }

    /// 清空，返回原来的。
    pub fn clear(&mut self) -> Option<EntityId> {
        self.item.take()
    }

    /// 里面没有实体。
    pub fn is_empty(&self) -> bool {
        self.item.is_none()
    }
}
```

以上是 `SingleArea` 的操作 的定义; 具体行为以该文件为准.


下面给出 选择列表 `SelectionSource` 的实现:

文件: crates/engine/src/select.rs:17-29

```rust
/// 定义层提供的「选择列表」：引擎只要求能数出多少项、以及「取第 i 项后状态变成什么」。
pub trait SelectionSource<S> {
    /// 列表有多少项。
    fn len(&self) -> usize;

    /// 列表是否为空（默认由 `len` 推出，实现方可以覆盖）。
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 返回：取第 `index` 项后的新状态 + 该项权重（精确分数，D158）。
    fn apply(&self, state: &S, index: usize) -> (S, Fraction);
}
```

以上是 选择列表 `SelectionSource` 的定义; 具体行为以该文件为准.


下面给出 选择原语 `enumerate_select` 的实现:

文件: crates/engine/src/select.rs:31-70

```rust
/// ③-b 选择：**只收列表与数量，没有 who**（D25）。
///
/// 语义：从列表的项里选 `count` 个。
/// 「谁在选」「每项数据怎么解析表达」由**列表类型自己**表达（D26）。
///
/// v1 规则（写死）：
/// - `count == 0` 或 `list.len() == 0` -> 返回空 `Vec`；
/// - `count == 1` -> `list.len()` 个分支，每支概率 `1 / len`（精确分数，Σ = 1）；
/// - `count > 1` -> **暂时返回空 `Vec`（不 panic）**。
///
/// ⚠️ **多选 / 连抽的实现语义尚未拍板，见决策日志 D25**。
/// 这里**不擅自实现成「看起来合理」的样子**（先选后选、是否放回、概率怎么乘都还没定），
/// 先把入口留空并把 TODO 标在代码里；拍板后只改这一个分支。
/// debug 构建下也**不 panic**：调用方只是传了个值，不该因此炸掉。
pub fn enumerate_select<S, L>(state: &S, list: &L, count: Nat) -> Vec<(S, Fraction)>
where
    S: Clone,
    L: SelectionSource<S>,
{
    let count = count.get() as usize;
    let len = list.len();

    if count == 0 || len == 0 {
        return Vec::new();
    }

    if count > 1 {
        // 多选/连抽的实现语义尚未拍板，见 D25：先返回空，**不 panic**。
        return Vec::new();
    }

    // count == 1：均匀取一项。概率完全由列表长度决定（§4.2 写死为 1/len）。
    let probability = Fraction::new(ExactNat128::new(1), ExactNat128::new(len as u128));
    let mut branches = Vec::with_capacity(len);
    for index in 0..len {
        let (next, _weight) = list.apply(state, index);
        branches.push((next, probability));
    }
    branches
}
```

以上是 选择原语 `enumerate_select` 的定义; 具体行为以该文件为准.


下面给出 去重函数指针 `DedupFn` 的实现:

文件: crates/engine/src/random.rs:28-40

```rust
/// 去重函数指针（D1：去重**在调用点声明**，引擎只收函数指针）。
///
/// 输入 `Vec<(原始分支序号, 权重)>`，输出 `Vec<(代表分支序号, 合并后的权重)>`。
/// 权重是**精确分数** [`Fraction`]（D158），不是浮点。
///
/// **原始分支序号的约定**（去重函数必须认识它所服务的这次枚举）：
/// - `all_same = true`：第 `i` 支 = 所有次数都取列表第 `i` 项（共 `len` 支）；
/// - 其他情况：按「依次取了哪些项」的字典序编号（末位变化最快）：
///   `repeatable = true` 时是**可重复**元组，共 `len^times` 支；
///   `repeatable = false` 时是**不重复有序**元组，共 `P(len, times)` 支。
///
/// 契约：只合并等价分支、权重相加，**Σ 不变**（分工计划 §4.2「边界行为」表）。
pub type DedupFn = fn(Vec<(usize, Fraction)>) -> Vec<(usize, Fraction)>;
```

以上是 去重函数指针 `DedupFn` 的定义; 具体行为以该文件为准.


下面给出 随机原语 `enumerate_random` 的实现:

文件: crates/engine/src/random.rs:42-69

```rust
/// ③-a 随机：是否可以重复选择某项 / 是否所有次数都选择同一项 / 选择次数。
///
/// 边界行为（分工计划 §4.2「边界行为（写死）」表，判定顺序即下面代码的顺序）：
/// - `times == 0` -> 空；
/// - `list.len() == 0` -> 空；
/// - `all_same == true` -> 先选 1 项、其余次数都取它；结果条数 = `list.len()`。
///   **优先于下面的「不重复且次数超长」判定**：全同本来就是重复取同一项，不该被它判空；
/// - `repeatable == false` 且 `times > list.len()` -> 空（不 panic）；
/// - 其余 -> 均匀枚举全部原始分支，每支概率 `1 / 分支数`（精确分数）；
/// - 每条分支 `prob` 相加 = 1（精确分数，可直接用 [`Fraction::is_one`] 判断）；
///   去重只合并权重，Σ 不变。
///
/// ⚠️ **不做分支数上限保护**：`repeatable == true` 时原始分支数是 `len^times`，会指数爆炸。
/// 枚举语义本来就是「全枚举」，`times` 要小由调用方（定义层）保证。
pub fn enumerate_random<S, L>(
    state: &S,
    list: &L,
    repeatable: bool,
    all_same: bool,
    times: Nat,
    dedup: Option<DedupFn>,
) -> Vec<(S, Fraction)>
where
    S: Clone,
    L: SelectionSource<S>,
{
    enumerate_random_weighted(state, list, repeatable, all_same, times, dedup, None)
}
```

以上是 随机原语 `enumerate_random` 的定义; 具体行为以该文件为准.


下面给出 带分数列表的随机 `enumerate_random_weighted` 的实现:

文件: crates/engine/src/random.rs:71-169

```rust
/// ③-a 随机（**可选分数列表**，D150）：在 [`enumerate_random`] 上多收一个权重列表。
///
/// `weights` 与候选列表**一一对应**（`weights.len() == list.len()`）：
/// - `None`：与 [`enumerate_random`] **完全一致**（等概率）；
/// - `Some(weights)`：单次抽取内各选项按 `weights[i]` 的占比抽；
///   分支概率 = **各次抽取的选项条件概率之积**（精确分数）；
/// - 权重长度不等 -> 返回空 `Vec`（不 panic，与 `times == 0` / `list.len() == 0` 同风格）；
/// - 权重之和为 0（所有分子都是 0）-> 归一化无定义，返回空 `Vec`（不 panic）。
///
/// **各模式的加权语义**（与等概率语义一致地推导）：
/// - `all_same == true`：只抽 **1 次**，第 `i` 支概率 = `w_i / Σw`
///   （不是 `(w_i / Σw)^times`；等权重时 = `1 / len`，与等概率一致）；
/// - `repeatable == true`：每次都在**全列表**里独立抽，概率 = 各次占比之积
///   （等权重时 = `1 / len^times`，与等概率一致）；
/// - `repeatable == false`：**不放回**抽样，第 k 次在「剩余项」里按占比抽，
///   概率 = 各次**条件**占比之积（等权重时 = `1 / P(len, times)`，与等概率一致）。
///   若某次「剩余项权重之和为 0」 -> **panic**（概率模型自相矛盾；不静默退化成均匀抽）。
///
/// 概率为 0 的分支**仍然列出**（不剪枝）；去重只合并等价分支、权重相加，**Σ 不变**。
pub fn enumerate_random_weighted<S, L>(
    state: &S,
    list: &L,
    repeatable: bool,
    all_same: bool,
    times: Nat,
    dedup: Option<DedupFn>,
    weights: Option<&[Fraction]>,
) -> Vec<(S, Fraction)>
where
    S: Clone,
    L: SelectionSource<S>,
{
    let times = times.get() as usize;
    let len = list.len();

    // —— 边界行为（§4.2 表，顺序即优先级）——
    if times == 0 || len == 0 {
        return Vec::new();
    }
    // 分数列表必须与候选列表一一对应；长度不等 -> 空（不 panic）。
    if let Some(weights) = weights {
        if weights.len() != len {
            return Vec::new();
        }
    }
    // all_same 优先：全同本来就是「重复取同一项」，不能用「不重复且次数超长」判空。
    if !all_same && !repeatable && times > len {
        return Vec::new();
    }

    // —— ① 先枚举原始分支（只算下标序列，不碰状态）——
    let raw_choices = raw_branches(len, times, repeatable, all_same);
    if raw_choices.is_empty() {
        return Vec::new();
    }

    // —— ①' 原始分支的权重：不传分数 -> 均匀；传了 -> 精确按分数占比 ——
    let raw: Vec<(usize, Fraction)> = match weights {
        None => {
            // 原始分支**均匀**：每支 1/分支数（精确分数）。
            let each = Fraction::new(
                ExactNat128::new(1),
                ExactNat128::new(raw_choices.len() as u128),
            );
            (0..raw_choices.len())
                .map(|ordinal| (ordinal, each))
                .collect()
        }
        Some(weights) => {
            let Some(probabilities) =
                weighted_probabilities(len, repeatable, all_same, &raw_choices, weights)
            else {
                // 权重之和为 0：归一化无定义 -> 空（不 panic）。
                return Vec::new();
            };
            probabilities.into_iter().enumerate().collect()
        }
    };

    // —— ② 去重（条件编译；关闭时参数被忽略，分布不变、只是分支更多）——
    let representatives = merge_branches(raw, dedup);

    // —— ③ 只对代表元调用 apply 建状态 ——
    let mut out = Vec::with_capacity(representatives.len());
    for (ordinal, probability) in representatives {
        let Some(choices) = raw_choices.get(ordinal) else {
            // 去重函数违约（返回了不存在的原始分支序号）：跳过，不让它把枚举带崩。
            debug_assert!(false, "去重函数返回了不存在的原始分支序号");
            continue;
        };
        let mut next = state.clone();
        for &index in choices {
            let (after, _weight) = list.apply(&next, index);
            next = after;
        }
        out.push((next, probability));
    }
    out
}
```

以上是 带分数列表的随机 `enumerate_random_weighted` 的定义; 具体行为以该文件为准.


下面给出 精确分数 `Fraction` 的实现:

文件: crates/engine/src/prob.rs:54-62

```rust
/// 分数：`num / den`（分子 / 分母，均 `ExactNat128`；D150 / D152 / D153 / D158）。
///
/// 用 [`Fraction::new`] 构造；分母为 0 会 panic。
/// 加 / 乘 / 除的结果**自动约分**（如 `2/4` 会变成 `1/2`），所以可直接用结构化相等比较值。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fraction {
    num: ExactNat128,
    den: ExactNat128,
}
```

以上是 精确分数 `Fraction` 的定义; 具体行为以该文件为准.


下面给出 `Fraction` 的构造与精确运算 的实现:

文件: crates/engine/src/prob.rs:64-144

```rust
impl Fraction {
    /// 零：`0 / 1`（加法单位元）。
    pub const ZERO: Fraction = Fraction {
        num: ExactNat128::ZERO,
        den: ExactNat128::new(1),
    };

    /// 一：`1 / 1`（乘法单位元）。
    pub const ONE: Fraction = Fraction {
        num: ExactNat128::new(1),
        den: ExactNat128::new(1),
    };

    /// 构造 `num / den`（**不约分**；需要规范化时用加 / 乘 / 除的结果）。
    ///
    /// **分母为 0 -> panic**（与引擎除法一致：除零是逻辑错误，不是饱和业务）。
    pub const fn new(num: ExactNat128, den: ExactNat128) -> Fraction {
        assert!(
            den.get() != 0,
            "除零：分数分母为 0（与引擎除法一致，这是逻辑错误）"
        );
        Fraction { num, den }
    }

    /// 分子。
    pub const fn num(self) -> ExactNat128 {
        self.num
    }

    /// 分母（保证 > 0）。
    pub const fn den(self) -> ExactNat128 {
        self.den
    }

    /// 数值是否为 0。
    pub const fn is_zero(self) -> bool {
        self.num.get() == 0
    }

    /// 数值是否为 1（分母恒 > 0，所以 `num == den` 就是值 1）。
    pub const fn is_one(self) -> bool {
        self.num.get() == self.den.get()
    }

    /// 精确加：`a/b + c/d = (a*d + c*b) / (b*d)`，结果**约分**；**上溢 panic**（D153）。
    pub const fn exact_add(self, rhs: Fraction) -> Fraction {
        let num = self
            .num
            .exact_mul(rhs.den)
            .exact_add(rhs.num.exact_mul(self.den));
        let den = self.den.exact_mul(rhs.den);
        reduce(num, den)
    }

    /// 精确乘：`(a/b) * (c/d) = (a*c) / (b*d)`，结果**约分**；**上溢 panic**（D153）。
    pub const fn exact_mul(self, rhs: Fraction) -> Fraction {
        reduce(
            self.num.exact_mul(rhs.num),
            self.den.exact_mul(rhs.den),
        )
    }

    /// 精确除：`(a/b) / (c/d) = (a*d) / (b*c)`，结果**约分**。
    ///
    /// **除数分子为 0 -> panic**（除零是逻辑错误）；上溢同样 panic（D153）。
    pub const fn exact_div(self, rhs: Fraction) -> Fraction {
        assert!(
            rhs.num.get() != 0,
            "除零：分数除数的分子为 0（与引擎除法一致，这是逻辑错误）"
        );
        reduce(
            self.num.exact_mul(rhs.den),
            self.den.exact_mul(rhs.num),
        )
    }

    /// 精确比较两个分数的**值**（交叉相乘；乘积用 128×128 -> 256 位，不会溢出）。
    pub fn cmp_value(self, other: Fraction) -> std::cmp::Ordering {
        compare_scaled(self.num.get(), self.den.get(), other.num.get(), other.den.get())
    }
}
```

以上是 `Fraction` 的构造与精确运算 的定义; 具体行为以该文件为准.


下面给出 分数求和 `total` 的实现:

文件: crates/engine/src/prob.rs:214-217

```rust
/// 一组分数的和（精确族累加，溢出一律 panic；空切片 -> [`Fraction::ZERO`]）。
pub fn total(fractions: &[Fraction]) -> Fraction {
    fractions.iter().sum()
}
```

以上是 分数求和 `total` 的定义; 具体行为以该文件为准.


下面给出 分数归一化 `normalize` 的实现:

文件: crates/engine/src/prob.rs:219-237

```rust
/// 一组分数的**归一化**（D150 / D156）：按各自占比变成**精确分数**（Σ = 1）。
///
/// 返回 `None` 的情况（**不 panic**，与引擎其它边界行为一致的「返回空」风格）：
/// - 空切片；
/// - 分数之和为 0（所有分子都是 0：归一化无定义）。
///
/// 内部用 [`Fraction::exact_div`] 精确相除并约分；**不经过浮点**（D158 / D159）。
pub fn normalize(fractions: &[Fraction]) -> Option<Vec<Fraction>> {
    let total = total(fractions);
    if total.is_zero() {
        return None;
    }
    Some(
        fractions
            .iter()
            .map(|fraction| fraction.exact_div(total))
            .collect(),
    )
}
```

以上是 分数归一化 `normalize` 的定义; 具体行为以该文件为准.


下面给出 按分数占比落一单 `pick_index` 的实现:

文件: crates/engine/src/prob.rs:239-265

```rust
/// 按分数占比选一个下标：`unit` 是 `[0, 2^128)` 的均匀随机整数（由 [`crate::rng::Rng`] 取）。
///
/// 语义：把归一化后的累计概率映射到 128 位整数区间（`unit / 2^128`），取第一个累计占比
/// 严格大于该点的小标。全程整数比较（把比例放大到 256 位做交叉相乘），不经过浮点。
/// 量化误差 < `2^-128`（128 位区间的必然精度；实际可忽略）。
///
/// 返回 `None` 的情况与 [`normalize`] 相同（空列表 / 分数之和为 0）。
/// 概率为 0 的项永远不会被选中（累计值不变）。
pub fn pick_index(fractions: &[Fraction], unit: u128) -> Option<usize> {
    let total = total(fractions);
    if total.is_zero() {
        return None;
    }
    let mut cumulative = Fraction::ZERO;
    for (index, fraction) in fractions.iter().enumerate() {
        cumulative = cumulative.exact_add(*fraction);
        // 比较「累计占比 p = cumulative / total」与「unit / 2^128」：
        //   p > unit / 2^128  <=>  p.num * 2^128 > unit * p.den。
        // 左边 = (高 128 位 p.num, 低 128 位 0)；右边 = unit * p.den 的 256 位乘积。
        // 高位比较即可（低位左边是 0，永远不会更大）。
        let probability = cumulative.exact_div(total);
        if probability.num().get() > mul_u128_hi(unit, probability.den().get()) {
            return Some(index);
        }
    }
    Some(fractions.len() - 1)
}
```

以上是 按分数占比落一单 `pick_index` 的定义; 具体行为以该文件为准.


下面给出 随机源 `Rng` 的实现:

文件: crates/engine/src/rng.rs:12-18

```rust
/// 引擎的随机源（D149 / D151 / D160）：内部状态用 SplitMix64，对外只给 128 位随机整数。
///
/// 枚举**不消费**它；只有采样才 `&mut` 进来。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}
```

以上是 随机源 `Rng` 的定义; 具体行为以该文件为准.


下面给出 `Rng` 的公开方法 的实现:

文件: crates/engine/src/rng.rs:20-56

```rust
impl Rng {
    /// 从种子建随机源：**同一 seed -> 同一序列**（可复现）。
    pub const fn from_seed(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// 取一个原始 64 位随机数（SplitMix64 一步；内部积木，**不是对外随机接口**）。
    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// 取 `[0, 2^128)` 的均匀随机整数（两个 64 位拼成；D160 的对外随机接口）。
    pub fn next_u128(&mut self) -> u128 {
        let high = self.next_u64() as u128;
        let low = self.next_u64() as u128;
        (high << 64) | low
    }

    /// 取 `[0, n)` 的均匀随机整数（**拒绝采样**，无取模偏差；D160）。
    ///
    /// `n == 0` 是逻辑错误（`[0,0)` 无定义）-> panic，与除零同一口径。
    pub fn next_below_u128(&mut self, n: u128) -> u128 {
        assert!(n != 0, "取 [0,0) 无定义：next_below_u128 的 n 为 0（逻辑错误）");
        // 2^128 % n = (-n) % n：落在 [0, threshold) 的样本会引入偏差，丢弃重取。
        let threshold = n.wrapping_neg() % n;
        loop {
            let value = self.next_u128();
            if value >= threshold {
                return value % n;
            }
        }
    }
}
```

以上是 `Rng` 的公开方法 的定义; 具体行为以该文件为准.


下面给出 计数器 `Counter` 的实现:

文件: crates/engine/src/stats.rs:25-40

```rust
/// 计数器：一个 [`Nat`] 的**饱和**增 / 减 / 清零 / 读。
///
/// 普通值类型变量（D182）：由实体区自己当字段持有，可以一个区多个、区之间互不共享；
/// `Copy`、无内部可变性、`const fn` 构造。
///
/// 只读边界（D183）：字段私有，外部只能拿 `&Counter` 读（[`Counter::get`] / [`Counter::is_zero`]）；
/// 修改（`add` / `sub` / `clear`）都要 `&mut self`，只有持有它的实体区做得到。
///
/// 口径（与 `Nat` 族一致：**溢出是正常业务，不 panic**）：
/// - 加：溢出停在 [`Nat::MAX`]；
/// - 减：到 [`Nat::ZERO`] 就停（不变负数、不 panic）；
/// - 清零：直接回到 [`Nat::ZERO`]。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Counter {
    value: Nat,
}
```

以上是 计数器 `Counter` 的定义; 具体行为以该文件为准.


下面给出 `Counter` 的操作 的实现:

文件: crates/engine/src/stats.rs:42-77

```rust
impl Counter {
    /// 计数 0。
    pub const ZERO: Counter = Counter { value: Nat::ZERO };

    /// 用初始值建计数器。
    pub const fn new(value: Nat) -> Counter {
        Counter { value }
    }

    /// 当前值（读）。
    pub const fn get(self) -> Nat {
        self.value
    }

    /// 是否已经是 0。
    pub const fn is_zero(&self) -> bool {
        self.value.get() == 0
    }

    /// 加：**饱和**（溢出停在 [`Nat::MAX`]，不 panic）；返回加完之后的值。
    pub fn add(&mut self, amount: Nat) -> Nat {
        self.value = self.value.saturating_add(amount);
        self.value
    }

    /// 减：**饱和**（到 0 就停，不 panic）；返回减完之后的值。
    pub fn sub(&mut self, amount: Nat) -> Nat {
        self.value = self.value.saturating_sub(amount);
        self.value
    }

    /// 清零（回到 [`Nat::ZERO`]）。
    pub fn clear(&mut self) {
        self.value = Nat::ZERO;
    }
}
```

以上是 `Counter` 的操作 的定义; 具体行为以该文件为准.


下面给出 有序可重复序列 `Seq<T>` 的实现:

文件: crates/engine/src/stats.rs:93-106

```rust
/// 有序**可重复**序列：允许重复、保持插入顺序。
///
/// 普通值类型变量（D182）：由实体区自己当字段持有，可以一个区多个、区之间互不共享；
/// `Clone`、无内部可变性、`const fn` 构造。
///
/// 只读边界（D183）：字段私有，外部只能拿 `&Seq<T>` 读（`len` / `is_empty` / `get` / `iter` /
/// `as_slice`，都是只读指针）；没有可变切片 / 可变字段入口；`push` / `clear` 需要 `&mut self`。
///
/// 「卡牌 ID 列表（有序、可出现多次）」用它承载，例如 `Seq<CardId>`（D181）；
/// 引擎不知道元素语义，只保证**顺序**与**重复**。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seq<T> {
    items: Vec<T>,
}
```

以上是 有序可重复序列 `Seq<T>` 的定义; 具体行为以该文件为准.


下面给出 `Seq<T>` 的操作 的实现:

文件: crates/engine/src/stats.rs:108-149

```rust
impl<T: Clone> Seq<T> {
    /// 空序列。
    pub const fn new() -> Seq<T> {
        Seq { items: Vec::new() }
    }

    /// 追加一项到末尾（**允许重复**）。
    pub fn push(&mut self, item: T) {
        self.items.push(item);
    }

    /// 元素个数（`Nat`，与引擎数值口径一致）。
    pub fn len(&self) -> Nat {
        Nat::new(self.items.len() as _)
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 取第 `index` 项（0 开始；**越界 -> `None`**）。
    pub fn get(&self, index: Nat) -> Option<&T> {
        // bare-int-ok: 容器下标，非业务数值
        self.items.get(index.get() as usize)
    }

    /// 按**插入顺序**迭代。
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// 清空（长度归 0）。
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// 底层切片（只读；顺序即插入顺序）。
    pub fn as_slice(&self) -> &[T] {
        &self.items
    }
}
```

以上是 `Seq<T>` 的操作 的定义; 具体行为以该文件为准.


下面给出 求解接口 `GameRules` 的实现:

文件: crates/engine/src/rules.rs:18-37

```rust
/// 对局规则接口：求解器只通过这四个函数看局面。
pub trait GameRules<S> {
    /// 「谁」的类型：由定义层给出，最小约束只要 `Copy + Eq`。
    /// 「我方 / 敌方」是它的两个**具名实例**，不是编号。
    type Player: Copy + Eq;

    /// 现在轮到谁行动。
    fn to_move(&self, st: &S) -> Self::Player;

    /// 局面是否已结束（`None` = 未结束）。
    fn terminal(&self, st: &S) -> Option<GameResult<Self::Player>>;

    /// `who` 在当前局面下的全部选择列表。
    fn actions(&self, st: &S, who: Self::Player) -> Vec<ActionList>;

    /// 局面估值（越大越偏向「我方」那个具名实例；谁是「我方」由调用方约定）。
    ///
    /// 用精确族 `ExactNat128`（D159）：求解器内部一律精确整数，不出现浮点。
    fn evaluate(&self, st: &S) -> ExactNat128;
}
```

以上是 求解接口 `GameRules` 的定义; 具体行为以该文件为准.


下面给出 终局结果 `GameResult` 的实现:

文件: crates/engine/src/rules.rs:39-54

```rust
/// 终局结果（`P` = 定义层给的「谁」）。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum GameResult<P> {
    /// 某一方获胜。
    Win(P),
    /// 平局。
    Draw,
    /// 达到回合上限。
    RoundLimit,
    /// **双方都输（双负）**：终局判定时双方角色区都为空。
    ///
    /// - 不是平局（`Draw`），也不是某一方获胜（`Win`）；
    /// - 依据 **D267**（用户 2026-10-19 明确）：终局判定在**步边界**（本步效果全部执行完才判，步内不提前终止）；
    ///   实例：芙宁娜（白芙）天赋牌 + 普通攻击 可造成双负。
    BothLose,
}
```

以上是 终局结果 `GameResult` 的定义; 具体行为以该文件为准.



## 4. 不变量与禁止

> [!IMPORTANT]
> 引擎只有机制, 没有语义: 不认识卡牌 / 召唤物 / 支援 / 伤害 / 反应 / 费用 / 阶段.
> 没有 `Side` (D29 / D34) , 没有区编号 / 区表 / 实体归属表 (D42) .

- 默认零外部依赖; 唯一例外是 `dll` feature 下的 `libloading` (当前 `load.rs` 未实现) .
- 概率一律精确: 分支概率与去重权重都是 `Fraction`; 每条分支概率之和 = 1; 去重只合并等价分支, 权重相加, Σ 不变.
- feature 开关只影响性能, 不影响分布: `dedup` 关闭时去重参数被忽略, 分支更多但分布一致.
- 枚举不消费 RNG: `enumerate_random` / `enumerate_select` 是纯函数; 只有采样才用 `Rng` (D149) .
- 不新增数值类型 / 运算宏: 统计模块明确不扩展 `Nat` 族口径 (D181) .
- 无全局可变状态, 无内部可变性 (统计容器也是) .

## 5. 依据 (规则书条款 / 决策编号)

引擎是机制层, 不读规则书; 行为依据全部来自决策日志.

- 去重函数指针 + feature 开关: D1 / D3
- 求解接口默认不编译: D24
- 选择原语没有 who; 列表归各自 `parse.rs`: D25 / D26
- 敌我不编号, 两个具名实例, 无 `Side`: D29 / D34
- `Nat` 族 / `Nat = Nat32` / 非默认宽度要登记: D36 / D37
- 区 = 状态变量, 无区号, 无归属表: D42
- 除零 panic: D44
- 随机分工 (枚举纯函数, 采样才用 RNG): D149 / D150 / D151
- 精确分数 / 精确族 / 宏按类型分派: D152 / D153 / D155 / D156
- 对外精确, 全仓零 f64, 随机源 `u128`: D158 / D159 / D160
- 统计原语 / 区自持 / 对外只读: D181 / D182 / D183
- `GameResult::BothLose` (双负): D267 / D268
- 边界行为表: `分工计划.md` §4.2'边界行为 (写死) '

## 6. 常见误用

> [!WARNING]
> 不要在 `engine` 里加游戏概念, `Side`, `AreaId`, 也不要以为 `enumerate_select` 支持一次选多个 (`count > 1` 返回空) .

- 以为去重会改变概率分布: 只合并等价分支, Σ 不变.
- 以为 `enumerate_random` 会消耗随机数: 它是纯函数.
- 用 `f64` / 64 位随机 / 饱和语义算概率: 必须 `Fraction` (`ExactNat128`) 与 `u128`.
- 想用 `EntityTable` 查实体在哪个区: 查不到, 也不该查 (D42) .
- 默认以为 `rules` 模块存在: 要 `--features rules`.
- 打开 `dll` feature: `load.rs` 未实现, 会编译失败.

## 7. 测试在哪

- 全部测试: `cargo test --offline -p gi-genius-invokation-engine`, 当前 184 条 (单元 + 3 个集成测试文件) .
- 代表用例 (`文件::模块::测试名`) : 

- `engine/src/nat.rs::nat::tests::add_saturates_on_overflow`: 饱和族五个宽度
- `engine/src/nat.rs::nat::exact_nat_tests::exact_width_nat128::add_overflow_by_method_panics`: 精确族溢出 panic
- `engine/src/prob.rs::prob::tests::normalize_gives_exact_proportions_summing_to_one`: 精确归一化
- `engine/src/random.rs::random::tests::none_weights_is_bit_identical_to_plain_enumerate_random`: 加权与等概率一致
- `engine/src/random.rs::random::tests::dedup_fn_is_ignored_without_feature`: dedup feature 关闭分布不变
- `engine/src/rng.rs::rng::tests::next_u64_matches_splitmix64_reference_vectors`: 锁死 PRNG 算法
- `engine/tests/entity_area.rs::alloc_keeps_growing_and_never_reuses`: 实体号单调不复用
- `engine/tests/select.rs::count_two_is_empty_and_does_not_panic`: 多选未拍板时返回空

> [!NOTE]
> `tools/check_*.py` 只扫 definitions 等上层 crate, 不扫 engine (§8.3) .

## 8. 待接入与待实测

- #5: 引擎 `DedupFn` 无上下文 (当前折叠是 definitions 侧后置合并, D87)
- #13: `GameRules::evaluate` 返回无符号 `ExactNat128`; 求解器若有负分局面需另立精确有符号类型
- #14: 浮点只允许出现在核心 crates 之外; `Fraction::to_f64` 已删除
- #43: `GameRules::to_move` / `actions` / `evaluate` 仍是占位
- #44: `definitions` 的 `rules` feature 是否进 CI——待确认 (`ci.yml` 已用 workspace features 覆盖)
- 无编号: `dll` feature 的 `src/load.rs` 不存在, 打开即编译失败 (D226 记录, CI 注释明确不开)

---

## 小结

引擎只提供机制原语, 不解释任何游戏语义. 概率与随机全部是精确整数, 上层按需组合这些原语.

---

最后核对: 2026-10-19 (代码: `new/crates/engine/src/lib.rs:pub mod`) 
