//! **通用状态**（D104 / D105）：状态身份抽象 + **一个公共状态列表**的容器与操作。
//!
//! 依据：
//! - D95/D96：状态身份 = **类型化静态实例指针**，不用 Nat 编号、不建状态编号表；
//! - D104：状态先做一个通用的（同时支持「必须带层数」与「布尔值」两种形态），
//!   减费 / 赋能等具体状态在通用之上实现，各自模块定义身份类型，通用层不塞具体语义；
//! - D105：**所有状态共用一个列表**（不分位置、不分种类）：
//!   StateEntry { state, pos, layers }；层数类 layers = Some(n)，布尔类 layers = None，
//!   **在列表里即代表「有」、不额外存 bool**。
//!
//! 归属（D112/D113）：本模块在 `families/action_card/status/` —— 减费 / 赋能等
//! 「挂在行动牌上的状态」属于行动牌；`families/status/` 只放状态牌族的 L2 类型枚举。
//!
//! 位置（pos）由上层解释：牌库里 = 从顶往下数的序号；上层在牌库插入 / 删除时要调用
//! on_insert / on_remove 让列表里的 pos 跟着牌走（D82/D86/D89 的位置口径不变）。
//!
//! 层数只存与合并、**不钳制**（D100：减费层数钳到 0 在费用结算侧）。

use crate::generated::Localized;
use gi_genius_invokation_engine::{nadd, nsub, Nat};

/// 状态的形态（D104）：必须带层数 / 布尔值。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum StateForm {
    /// 布尔值：只有「有 / 没有」，不记录层数。
    Boolean,
    /// 必须带层数：每条状态有层数，同状态同位置合并时相加。
    Layered,
}

/// 状态的静态定义数据（D110）：**与 TagText 同构** —— 4 语名字 + 4 语描述。
///
/// 状态「相当于 tag」：定义模块给一份静态 StateText，状态身份 = 指向它所属静态实例的指针；
/// 列表 / 手牌里只存身份指针 + pos + layers，**不拷贝名字描述**（只有一份，D64 精神）。
#[derive(Clone, Copy, Debug)]
pub struct StateText {
    /// 4 语名字。
    pub name: Localized,
    /// 4 语描述。
    pub description: Localized,
}

/// 状态身份（D95/D96/D110）：具体状态模块定义自己的零大小类型 + 一份静态实例；
/// 运行期身份 = 指向那份实例的指针；静态定义数据 = 它给出的 StateText（名字 + 描述）。
///
/// 谁定义状态谁给类型；使用处 import 那个模块、拿到静态实例后交给 StateList 的操作。
/// 本层**只解释身份 + 形态**，不解释减费 / 赋能等具体语义（D108）。
pub trait StateIdentity: 'static + std::fmt::Debug {
    /// 状态的静态定义数据（4 语名字 + 描述；只有一份，不拷贝）。
    fn text(&self) -> &'static StateText;

    /// 这个状态是布尔值还是必须带层数。
    fn form(&self) -> StateForm;

    /// **身份地址** = 指向该身份**定义数据**的指针（D57 / D58 口径）。
    ///
    /// 为什么不用 `&dyn StateIdentity` 的地址：零大小（ZST）静态实例**没有独立地址**
    /// （本仓库实测 rustc 1.98.1 下不同 ZST 静态实例地址相同），直接比 `&dyn` 会把
    /// 不同身份判成同一身份。所以身份一律取「它指向的那份数据」的地址。
    ///
    /// 默认实现 = [`StateIdentity::text`] 返回的那份 `StateText` 的地址：本仓库每个身份
    /// 都有唯一一份文本数据；将来有独立定义数据（如 `&'static StatusDef`）的实现可以覆盖，
    /// 返回定义本身的地址（同样唯一）。
    fn identity_address(&self) -> *const () {
        self.text() as *const StateText as *const ()
    }

    /// 状态的可读名字（调试 / 日志用；**不参与身份判断**）。
    ///
    /// 默认从 StateText 取中文原文；状态模块想换语言自己覆盖。
    fn name(&self) -> &'static str {
        self.text()
            .name
            .get(crate::generated::Lang::ZhCn)
            .text()
            .unwrap_or("")
    }
}

/// 状态身份的**身份地址**（= 指向其定义数据的指针；折叠键 / 比较都用它）。
pub fn state_address(state: &'static dyn StateIdentity) -> *const () {
    state.identity_address()
}

/// 两个状态身份是否是**同一份身份**（比较 `identity_address()`；D57 / D58）。
///
/// **不要**直接比 `&dyn StateIdentity`（`std::ptr::addr_eq`）：ZST 静态实例的地址可能相同。
pub fn same_state_identity(
    left: &'static dyn StateIdentity,
    right: &'static dyn StateIdentity,
) -> bool {
    std::ptr::eq(left.identity_address(), right.identity_address())
}

/// 一条**不挂位置**的状态（D188：待展开的随机状态 / 抽到的牌携带的状态用）。
///
/// 与 StateEntry 的区别：StateEntry 带 pos（公共状态列表里按位置存），
/// Mark 只是「状态身份 + 层数」，用在还没落到具体位置上的场合。
#[derive(Clone, Copy, Debug)]
pub struct Mark {
    /// 状态身份（类型化静态实例指针，D95/D96）。
    pub state: &'static dyn StateIdentity,
    /// 层数（布尔类为 None；层数类为 Some）。
    pub layers: Option<Nat>,
}

impl PartialEq for Mark {
    fn eq(&self, other: &Mark) -> bool {
        same_state_identity(self.state, other.state) && self.layers == other.layers
    }
}

impl Eq for Mark {}

/// 公共状态列表里的一条：**列表里有它 = 这个位置有这个状态**。
///
/// - layers = None：布尔类（不额外存 bool）；
/// - layers = Some(n)：层数类。
#[derive(Clone, Copy)]
pub struct StateEntry {
    /// 状态身份（类型化静态实例指针，D95/D96）。
    pub state: &'static dyn StateIdentity,
    /// 位置（上层解释：牌库里 = 从顶往下数的序号）。
    pub pos: Nat,
    /// 层数（布尔类为 None；层数类为 Some）。
    pub layers: Option<Nat>,
}

impl PartialEq for StateEntry {
    fn eq(&self, other: &StateEntry) -> bool {
        same_state_identity(self.state, other.state)
            && self.pos == other.pos
            && self.layers == other.layers
    }
}

impl Eq for StateEntry {}

impl std::fmt::Debug for StateEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StateEntry")
            .field("state", &self.state.name())
            .field("pos", &self.pos)
            .field("layers", &self.layers)
            .finish()
    }
}

/// 折叠用的位置状态键（D105）：该位置的全部状态条目压成两个排序集合。
///
/// 布尔条目进 booleans；层数条目进 layered。身份都是指针（D95），不建编号表。
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct StateKey {
    /// 该位置的布尔状态（状态静态实例地址，已排序）。
    pub booleans: Vec<*const ()>,
    /// 该位置的层数状态（状态静态实例地址, 层数；已排序）。
    pub layered: Vec<(*const (), Nat)>,
}

impl StateKey {
    /// 空键（没有状态）。
    pub fn empty() -> StateKey {
        StateKey {
            booleans: Vec::new(),
            layered: Vec::new(),
        }
    }

    /// 由布尔集合 / 层数条目构造（排序后保证跨分支可比较）。
    pub fn new(mut booleans: Vec<*const ()>, mut layered: Vec<(*const (), Nat)>) -> StateKey {
        booleans.sort();
        layered.sort();
        StateKey { booleans, layered }
    }
}

/// **公共状态列表**（D105）：一个列表装所有状态，entries 可跨位置。
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct StateList {
    entries: Vec<StateEntry>,
}

impl StateList {
    /// 空列表。
    pub const fn new() -> StateList {
        StateList {
            entries: Vec::new(),
        }
    }

    /// 全部条目（读）。
    pub fn entries(&self) -> &[StateEntry] {
        &self.entries
    }

    /// 条目数。
    pub fn len(&self) -> Nat {
        Nat::new(self.entries.len() as _)
    }

    /// 列表是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 某个位置是否有这个状态（布尔 / 层数都算「有」）。
    pub fn contains(&self, pos: Nat, state: &'static dyn StateIdentity) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.pos == pos && same_state_identity(entry.state, state))
    }

    /// 某个位置这个状态的层数：不存在 -> None；存在 -> Some(layers)（布尔类为 Some(None)）。
    pub fn layers(&self, pos: Nat, state: &'static dyn StateIdentity) -> Option<Option<Nat>> {
        self.entries
            .iter()
            .find(|entry| entry.pos == pos && same_state_identity(entry.state, state))
            .map(|entry| entry.layers)
    }

    /// 某个位置的全部条目的折叠键（D105）。
    pub fn key_at(&self, pos: Nat) -> StateKey {
        let mut booleans: Vec<*const ()> = Vec::new();
        let mut layered: Vec<(*const (), Nat)> = Vec::new();
        for entry in &self.entries {
            if entry.pos != pos {
                continue;
            }
            match entry.layers {
                None => booleans.push(state_address(entry.state)),
                Some(layers) => layered.push((state_address(entry.state), layers)),
            }
        }
        StateKey::new(booleans, layered)
    }

    /// 某个位置的全部条目（拷贝；按列表顺序）。
    pub fn entries_at(&self, pos: Nat) -> Vec<StateEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.pos == pos)
            .copied()
            .collect()
    }

    /// 加一条状态（D104 通用操作：存在性 / 合并）。
    ///
    /// 形态由状态自己的 form 决定：
    /// - Boolean：layers 一律按 None 存（**不额外存 bool**，在列表里即代表有），重复添加幂等；
    /// - Layered：layers 为 None 时按 1 层处理，同状态同位置**层数相加**；
    /// - 不同状态 -> 各是一条（列表允许并存）。
    pub fn add(&mut self, pos: Nat, state: &'static dyn StateIdentity, layers: Option<Nat>) {
        let normalized = normalize_layers(state, layers);
        if let Some(existing) = self
            .entries
            .iter_mut()
            .find(|entry| entry.pos == pos && same_state_identity(entry.state, state))
        {
            existing.layers = merge_layers(existing.layers, normalized);
            return;
        }
        self.entries.push(StateEntry {
            state,
            pos,
            layers: normalized,
        });
    }

    /// 加一条状态，并先与**相反状态**抵消（opposite 由调用方给，D94 规则）。
    ///
    /// 1. 同状态 -> 合并（同 add）；
    /// 2. 同位置存在 opposite -> 层数相减（None 按 1 层）：净 0 删条；原来的多则保留剩余；
    ///    新来的多则删掉 opposite、按新状态的剩余层数入表；
    /// 3. 其它情况 -> 直接加。
    ///
    /// 只存与合并、不钳制（D100）。
    pub fn add_with_opposite(
        &mut self,
        pos: Nat,
        state: &'static dyn StateIdentity,
        layers: Option<Nat>,
        opposite: &'static dyn StateIdentity,
    ) {
        if self.contains(pos, state) {
            self.add(pos, state, layers);
            return;
        }
        let normalized = normalize_layers(state, layers);
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.pos == pos && same_state_identity(entry.state, opposite))
        {
            let existing = self.entries.get(index).copied();
            let existing_layers = existing
                .map(|entry| effective_layers(entry.layers))
                .unwrap_or(Nat::ZERO);
            let incoming_layers = effective_layers(normalized);
            if existing_layers >= incoming_layers {
                let left = nsub!(existing_layers, incoming_layers);
                if left.get() == 0 {
                    self.entries.remove(index);
                } else if let Some(entry) = self.entries.get_mut(index) {
                    entry.layers = kept_layers(entry.state, left);
                }
            } else {
                self.entries.remove(index);
                self.entries.push(StateEntry {
                    state,
                    pos,
                    layers: kept_layers(state, nsub!(incoming_layers, existing_layers)),
                });
            }
            return;
        }
        self.entries.push(StateEntry {
            state,
            pos,
            layers: normalized,
        });
    }

    /// 移除某个位置的某个状态（不存在也算成功）。
    pub fn remove(&mut self, pos: Nat, state: &'static dyn StateIdentity) {
        self.entries
            .retain(|entry| !(entry.pos == pos && same_state_identity(entry.state, state)));
    }

    /// 清掉某个位置的全部状态。
    pub fn clear_pos(&mut self, pos: Nat) {
        self.entries.retain(|entry| entry.pos != pos);
    }

    /// 上层在 pos 处**移除**一张牌后调用（D82/D86/D89）：
    /// 该位置的条目随牌消失，后面位置的序号整体前移。
    pub fn on_remove(&mut self, pos: Nat) {
        self.entries.retain(|entry| entry.pos != pos);
        for entry in &mut self.entries {
            if entry.pos > pos {
                entry.pos = nsub!(entry.pos, Nat::new(1));
            }
        }
    }

    /// 上层在 pos 处**插入**一张牌后调用：从该位置起序号整体后移。
    pub fn on_insert(&mut self, pos: Nat) {
        for entry in &mut self.entries {
            if entry.pos >= pos {
                entry.pos = nadd!(entry.pos, Nat::new(1));
            }
        }
    }
}

/// 按状态形态把层数归一：布尔类恒 None；层数类缺省按 1 层。
fn normalize_layers(state: &'static dyn StateIdentity, layers: Option<Nat>) -> Option<Nat> {
    match state.form() {
        StateForm::Boolean => None,
        StateForm::Layered => Some(layers.unwrap_or(Nat::new(1))),
    }
}

/// 合并层数：None + None = None；None + Some(n) = Some(n)；Some + Some 相加（D88/D94，不钳制 D100）。
fn merge_layers(left: Option<Nat>, right: Option<Nat>) -> Option<Nat> {
    match (left, right) {
        (None, None) => None,
        (None, Some(layers)) | (Some(layers), None) => Some(layers),
        (Some(left), Some(right)) => Some(nadd!(left, right)),
    }
}

/// 抵消时的有效层数：None 按 1 层算。
fn effective_layers(layers: Option<Nat>) -> Nat {
    layers.unwrap_or(Nat::new(1))
}

/// 抵消后剩余层数怎么存：布尔类仍为 None；层数类存 Some(remaining)。
fn kept_layers(state: &'static dyn StateIdentity, remaining: Nat) -> Option<Nat> {
    match state.form() {
        StateForm::Boolean => None,
        StateForm::Layered => Some(remaining),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用静态定义数据（D110：与 TagText 同构）。
    static EMPOWERED_TEXT: StateText = StateText {
        name: Localized::from_all("赋能", "Empowered", "強化", "강화"),
        description: Localized::from_all("布尔状态示例", "boolean example", "例", "예"),
    };
    static CHEAPER_TEXT: StateText = StateText {
        name: Localized::from_all("减费", "Cost Reduction", "コスト減少", "비용 감소"),
        description: Localized::EMPTY,
    };
    static EXPENSIVE_TEXT: StateText = StateText {
        name: Localized::from_all("加费", "Cost Increase", "コスト増加", "비용 증가"),
        description: Localized::EMPTY,
    };

    /// 测试用布尔状态定义。
    #[derive(Debug)]
    struct BooleanState(&'static StateText);

    impl StateIdentity for BooleanState {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Boolean
        }
    }

    /// 测试用层数状态定义。
    #[derive(Debug)]
    struct LayeredState(&'static StateText);

    impl StateIdentity for LayeredState {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Layered
        }
    }

    static EMPOWERED: BooleanState = BooleanState(&EMPOWERED_TEXT);
    static CHEAPER: LayeredState = LayeredState(&CHEAPER_TEXT);
    static EXPENSIVE: LayeredState = LayeredState(&EXPENSIVE_TEXT);

    /// 声明一个**零大小**测试身份：自身没有独立地址，身份只能靠 `text()` 的数据地址区分。
    macro_rules! zst_status {
        ($ty:ident, $text:ident, $name:literal) => {
            #[derive(Debug)]
            struct $ty;

            static $text: StateText = StateText {
                name: Localized::from_all($name, $name, $name, $name),
                description: Localized::EMPTY,
            };

            impl StateIdentity for $ty {
                fn text(&self) -> &'static StateText {
                    &$text
                }

                fn form(&self) -> StateForm {
                    StateForm::Boolean
                }
            }
        };
    }

    zst_status!(ZstA, ZST_TEXT_A, "零A");
    zst_status!(ZstB, ZST_TEXT_B, "零B");
    zst_status!(ZstC, ZST_TEXT_C, "零C");

    static ZST_A: ZstA = ZstA;
    static ZST_B: ZstB = ZstB;
    static ZST_C: ZstC = ZstC;

    #[test]
    fn different_zst_state_identities_are_distinct_by_data_address() {
        // 三个不同的 ZST 身份（size_of == 0）：不能拿 `&dyn` 的地址当身份，
        // 必须靠 `identity_address()`（指向各自文本数据的地址）区分。
        assert_eq!(std::mem::size_of::<ZstA>(), 0);
        let a: &'static dyn StateIdentity = &ZST_A;
        let b: &'static dyn StateIdentity = &ZST_B;
        let c: &'static dyn StateIdentity = &ZST_C;
        assert!(!same_state_identity(a, b));
        assert!(!same_state_identity(a, c));
        assert!(!same_state_identity(b, c));
        assert_ne!(a.identity_address(), b.identity_address());
        // 同一身份经两条路径 -> 相等。
        let again: &'static dyn StateIdentity = &ZST_A;
        assert!(same_state_identity(a, again));
        // 不同 ZST 不能被 StateList 合并：同位置两条、折叠键也不同。
        let mut list = StateList::new();
        list.add(Nat::ZERO, a, None);
        list.add(Nat::ZERO, b, None);
        assert_eq!(list.len(), Nat::new(2));
        let mut only_a = StateList::new();
        only_a.add(Nat::ZERO, a, None);
        let mut same_a = StateList::new();
        same_a.add(Nat::ZERO, again, None);
        assert_eq!(only_a.key_at(Nat::ZERO), same_a.key_at(Nat::ZERO));
        assert_ne!(only_a.key_at(Nat::ZERO), list.key_at(Nat::ZERO));
    }

    #[test]
    fn state_identity_exposes_text_without_copying_it() {
        // D110：身份能拿到 &'static StateText（名字 + 描述），且只有一份（指针相同）。
        let text = EMPOWERED.text();
        assert!(std::ptr::addr_eq(text, &EMPOWERED_TEXT));
        assert_eq!(EMPOWERED.name(), "赋能");
        assert_eq!(
            text.name.get(crate::generated::Lang::EnUs).text(),
            Some("Empowered")
        );
        assert_eq!(
            text.description.get(crate::generated::Lang::ZhCn).text(),
            Some("布尔状态示例")
        );
        // 列表条目只存状态身份 + pos + layers，不拷贝名字 / 描述。
        let mut list = StateList::new();
        list.add(Nat::ZERO, &EMPOWERED, None);
        let entry = list.entries().first().copied().expect("有一条");
        assert!(std::ptr::addr_eq(entry.state.text(), &EMPOWERED_TEXT));
    }

    #[test]
    fn boolean_entry_has_no_extra_bool_field() {
        // D104/D105：布尔条目**不额外存 bool**——条目结构只有 state / pos / layers 三个字段，
        // 在列表里即代表「有」；布尔类 layers == None。
        let mut list = StateList::new();
        list.add(Nat::new(2), &EMPOWERED, None);
        let entry = list.entries().first().copied().expect("有一条");
        let StateEntry {
            state: _,
            pos,
            layers,
        } = entry;
        assert_eq!(pos, Nat::new(2));
        assert_eq!(layers, None);
        assert!(list.contains(Nat::new(2), &EMPOWERED));
        // 重复添加幂等：还是只有一条
        list.add(Nat::new(2), &EMPOWERED, None);
        assert_eq!(list.len(), Nat::new(1));
        assert_eq!(list.layers(Nat::new(2), &EMPOWERED), Some(None));
        // 不同位置各一条
        list.add(Nat::new(5), &EMPOWERED, None);
        assert_eq!(list.len(), Nat::new(2));
    }

    #[test]
    fn layered_states_merge_layers() {
        let mut list = StateList::new();
        list.add(Nat::new(1), &CHEAPER, Some(Nat::new(2)));
        assert_eq!(list.layers(Nat::new(1), &CHEAPER), Some(Some(Nat::new(2))));
        // 同状态同位置合并：2 + 3 = 5（不钳制，D100）
        list.add(Nat::new(1), &CHEAPER, Some(Nat::new(3)));
        assert_eq!(list.layers(Nat::new(1), &CHEAPER), Some(Some(Nat::new(5))));
        // 不同状态并存
        list.add(Nat::new(1), &EXPENSIVE, Some(Nat::new(1)));
        assert_eq!(list.len(), Nat::new(2));
        assert_eq!(list.layers(Nat::new(1), &EXPENSIVE), Some(Some(Nat::new(1))));
        // 移除
        list.remove(Nat::new(1), &CHEAPER);
        assert!(!list.contains(Nat::new(1), &CHEAPER));
        assert!(list.contains(Nat::new(1), &EXPENSIVE));
    }

    #[test]
    fn same_state_identity_from_two_paths_merges() {
        // 同一 static 经两条路径（直接取址 / 经函数返回 dyn 引用）传入：必须合并成一条。
        fn direct() -> &'static dyn StateIdentity {
            &CHEAPER
        }
        fn via_alias() -> &'static dyn StateIdentity {
            &CHEAPER
        }
        let first = direct();
        let second = via_alias();
        assert!(
            same_state_identity(first, second),
            "同一身份经两条路径必须相等"
        );
        let mut list = StateList::new();
        list.add(Nat::ZERO, first, Some(Nat::new(2)));
        list.add(Nat::ZERO, second, Some(Nat::new(3)));
        assert_eq!(list.len(), Nat::new(1));
        assert_eq!(list.layers(Nat::ZERO, first), Some(Some(Nat::new(5))));
    }

    #[test]
    fn opposite_states_cancel_by_layers() {
        // CHEAPER 与 EXPENSIVE 互为相反（由调用方指定 opposite）
        let mut list = StateList::new();
        list.add(Nat::ZERO, &CHEAPER, Some(Nat::new(3)));
        // 反向 1 层：剩 CHEAPER 2 层
        list.add_with_opposite(Nat::ZERO, &EXPENSIVE, Some(Nat::new(1)), &CHEAPER);
        assert_eq!(list.layers(Nat::ZERO, &CHEAPER), Some(Some(Nat::new(2))));
        assert!(!list.contains(Nat::ZERO, &EXPENSIVE));
        // 反向 5 层：CHEAPER 2 层抵消完，剩 EXPENSIVE 3 层
        list.add_with_opposite(Nat::ZERO, &EXPENSIVE, Some(Nat::new(5)), &CHEAPER);
        assert!(!list.contains(Nat::ZERO, &CHEAPER));
        assert_eq!(list.layers(Nat::ZERO, &EXPENSIVE), Some(Some(Nat::new(3))));
    }

    #[test]
    fn positions_follow_insert_and_remove() {
        let mut list = StateList::new();
        list.add(Nat::new(1), &EMPOWERED, None);
        list.add(Nat::new(2), &CHEAPER, Some(Nat::new(2)));
        // 在最前面插入一张：所有位置 +1
        list.on_insert(Nat::ZERO);
        assert!(list.contains(Nat::new(2), &EMPOWERED));
        assert_eq!(list.layers(Nat::new(3), &CHEAPER), Some(Some(Nat::new(2))));
        // 移除位置 0：所有位置 -1
        list.on_remove(Nat::ZERO);
        assert!(list.contains(Nat::new(1), &EMPOWERED));
        assert_eq!(list.layers(Nat::new(2), &CHEAPER), Some(Some(Nat::new(2))));
        // 移除带状态的位置本身：该位置的条目消失
        list.on_remove(Nat::new(1));
        assert!(!list.contains(Nat::new(1), &EMPOWERED));
    }

    #[test]
    fn key_at_contains_boolean_bits_and_layers() {
        let mut list = StateList::new();
        list.add(Nat::ZERO, &EMPOWERED, None);
        list.add(Nat::ZERO, &CHEAPER, Some(Nat::new(2)));
        let key = list.key_at(Nat::ZERO);
        assert_eq!(key.booleans.len(), 1);
        assert_eq!(key.layered.len(), 1);
        assert_eq!(key.layered.first().expect("一条").1, Nat::new(2));
        // 不同位置 -> 键不同（空键）
        assert_ne!(key, list.key_at(Nat::new(1)));
        assert_eq!(list.key_at(Nat::new(1)), StateKey::empty());
    }
}
