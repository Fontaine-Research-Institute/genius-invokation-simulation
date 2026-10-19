# Crate-Cards（具体卡牌）

> 中文 · [[Crate-Cards-EN|English]]

*本页范围：`cards` crate 的职责、卡牌组织与样板、公开入口、不变量、依据、测试与待接入项。*

---

## 1. 职责

具体卡牌层：只放「这张卡具体是什么」——每张卡自己的数据（id / 标签 / 生命 / 充能 / 技能 / 文案）与效果逻辑。卡牌的资源与形状定义留在 `definitions`（D240 / D241）；单向依赖 `definitions` / `engine`，不反向。

- 目录：`new/crates/cards/`
- package / lib：`gi-genius-invokation-cards` / `gi_genius_invokation_cards`

## 2. 关键概念与类型

| 概念 | 位置 | 说明 |
|---|---|---|
| crate 根 | `src/lib.rs` | `pub mod cards; pub mod source;` + `#![deny(clippy::arithmetic_side_effects)]` |
| 具体卡牌树 | `src/cards/` | 目录结构平移自原 `definitions/src/cards/`；目前只有 `character/aino/` |
| 牌族入口 | `src/cards/character/mod.rs` | 每张卡一个自建目录（D163）；没有集中注册表（注册机制待建） |
| 第一张样板卡 | `src/cards/character/aino/mod.rs` | 爱诺（角色牌 1216）：`ID` / `MAX_HP = 10` / `MAX_ENERGY = 2` / 标签元组 `(Hydro, Claymore, NodKrai)` / 4 个 `SkillDef` / `AINO: CharacterDef<Tags>` |
| 版本同步 | 同文件：`card_game_version!(6, 2);` | 生成 `GAME_VERSION: GameVersion`（D244–D246）；新版本没调整只改数字 |
| 源码自声明 | 同文件：`card_source!(path: ["character", "aino"]);` | 生成 `pub static SOURCE: SourceFile`（原文 + 编译期解析的 use 依赖；D214–D216） |
| 卡牌源码表 | `src/source/mod.rs` | `SOURCE_FILES`（手写清单）、`source_files()` / `source_of(path)` / `source_files_under(prefix)` |
| 源码机制 | `definitions/src/source/mod.rs` | `card_source!` / `module_source!`、`SourceFile` / `SourceOwner`、依赖解析器留在 definitions（D242） |
| 效果体 | 同 Aino 文件 | 4 个 `fn ..._effect()` 目前是 `todo!("缺：…")` 占位（文件里逐条列出缺哪些接口） |

标签集合形态本轮未定；样板用类型级零大小元组 `type Tags = (Hydro, Claymore, NodKrai)` + `const TAGS: Tags`（D54：标签就是类型本身）。

## 3. 公开入口

| 入口 | 说明 |
|---|---|
| `cards::cards::character::aino::{ID, MAX_HP, MAX_ENERGY, Tags, TAGS, SKILL_*, AINO, SOURCE}` | 样板卡的全部常量与定义 |
| `cards::source::{SOURCE_FILES, source_files, source_of, source_files_under}` | 卡牌源码表查询 |
| `cards::source::{SourceOwner, DependencyKind}` | 再导出 definitions 的源码机制类型 |
| `definitions::card_source!` / `card_game_version!` | 卡牌文件用的宏（机制在 definitions，D242） |

> [!NOTE]
> 卡牌定义用 `static`（需要稳定地址）；技能定义 `SkillDef` 含 id / 名字 / 描述 / `kind` / `cost` / `effect` 函数指针。

### 3.1 真实源码（逐字摘自当前仓库）

**卡牌 id `ID`**

`new/crates/cards/src/cards/character/aino/mod.rs:30-31`

```rust
/// 爱诺的卡牌 id（角色牌族）。
pub const ID: CardId = CardId::new(1216);
```

**版本声明 `card_game_version!`**

`new/crates/cards/src/cards/character/aino/mod.rs:42`

```rust
gi_genius_invokation_definitions::card_game_version!(6, 2);
```

**源码声明 `card_source!`**

`new/crates/cards/src/cards/character/aino/mod.rs:54`

```rust
gi_genius_invokation_definitions::card_source!(path: ["character", "aino"]);
```

**标签元组 `Tags`**

`new/crates/cards/src/cards/character/aino/mod.rs:62-65`

```rust
/// 标签集合（**类型级零大小元组**）：水元素 / 双手剑 / 挪德卡莱。
///
/// 标签就是类型本身（D54），所以这个元组零大小、无分配。
pub type Tags = (Hydro, Claymore, NodKrai);
```

**标签常量 `TAGS`**

`new/crates/cards/src/cards/character/aino/mod.rs:67-68`

```rust
/// 标签集合的常量值（三个零大小标签；运行期不存在）。
pub const TAGS: Tags = (Hydro, Claymore, NodKrai);
```

**技能定义 `SKILL_12161`**

`new/crates/cards/src/cards/character/aino/mod.rs:138-152`

```rust
/// 技能 12161 敲打修理法（普通攻击）。
///
/// - 文案：造成 2 点物理伤害。
/// - 消耗：1 水元素骰 + 2 无色骰。
pub static SKILL_12161: SkillDef = SkillDef {
    id: SKILL_ID_NORMAL_ATTACK,
    name: zh_only("敲打修理法"),
    description: zh_only("造成2点物理伤害。"),
    kind: SkillKind::normal_attack(),
    cost: SkillCost::from_dice(&[
        CostComponent::SpecificElement(Element::Hydro, Nat::new(1)),
        CostComponent::AnyColor(Nat::new(2)),
    ]),
    effect: normal_attack_effect,
};
```

**角色牌定义 `AINO`**

`new/crates/cards/src/cards/character/aino/mod.rs:206-217`

```rust
/// 爱诺的角色牌定义（第一张卡样板）。
///
/// 用 `static` 而不是 `const`：定义需要**稳定地址**（将来注册表 / 状态里存引用）。
pub static AINO: CharacterDef<Tags> = CharacterDef {
    id: ID,
    name: zh_only("爱诺"),
    tags: TAGS,
    max_hp: MAX_HP,
    max_energy: MAX_ENERGY,
    // 顺序固定：普攻 → 战技 → 爆发 → 被动（与游戏内文本描述的 skills 顺序一致）。
    skills: &[&SKILL_12161, &SKILL_12162, &SKILL_12163, &SKILL_12164],
};
```

**卡牌源码表 `SOURCE_FILES`**

`new/crates/cards/src/source/mod.rs:23-27`

```rust
/// 已接入的**卡牌**源码文件清单（手写登记，按 path 字典序升序）。
///
/// 新增卡：文件里写 `gi_genius_invokation_definitions::card_source!(path: [...]);` 后，
/// 在这里加一行 `&crate::cards::<牌族>::<卡>::SOURCE`。
pub static SOURCE_FILES: &[&SourceFile] = &[&crate::cards::character::aino::SOURCE];
```

**`source_files`**

`new/crates/cards/src/source/mod.rs:29-32`

```rust
/// 全部已登记的卡牌源码文件（只读；顺序 = [SOURCE_FILES]）。
pub fn source_files() -> &'static [&'static SourceFile] {
    SOURCE_FILES
}
```

**`source_of`**

`new/crates/cards/src/source/mod.rs:34-37`

```rust
/// 按 path 精确查找（path 口径 = 宏路径分段拼出的相对 crate 根路径）。
pub fn source_of(path: &str) -> Option<&'static SourceFile> {
    SOURCE_FILES.iter().copied().find(|file| file.path == path)
}
```

**`source_files_under`**

`new/crates/cards/src/source/mod.rs:39-45`

```rust
/// 按路径前缀筛出某目录下的文件（path 口径同上；只读迭代器）。
pub fn source_files_under(prefix: &str) -> impl Iterator<Item = &'static SourceFile> + '_ {
    SOURCE_FILES
        .iter()
        .copied()
        .filter(move |file| file.path.starts_with(prefix))
}
```


## 4. 不变量与禁止

> [!IMPORTANT]
> 单向依赖：`cards -> definitions / engine`；`definitions` 不得依赖 `cards`（守卫测试 `source::tests::dependency_direction_is_cards_to_definitions`）。

- 卡牌结构 = 原 definitions 结构：`src/cards/<牌族>/[<类型>/]<卡>`，一卡一目录（D163）；源码 `path` 仍以 `src/cards/` 开头。
- 资源 / 形状不搬：`CardId` / `CardFamily` / 类型 / 标签 / `CostComponent` / `CharacterDef` / `SkillDef` 等留在 `definitions`（D241）。
- 每张卡必须声明 `card_game_version!`；数值 = 该卡「已同步到的最新版本」；新版本没调整只改数字（D245 / D246）。
- 源码自声明后还要在 `SOURCE_FILES` 加一行（漏加不编译失败，但 `source_of` 找不到）；清单按 path 升序。
- 文案按 D120 只填中文；四语原文在 `definitions/generated/data/descriptions.json`（测试语料）。
- 数字口径：`Nat` / 常量；crate 级 `deny(clippy::arithmetic_side_effects)`。

> [!WARNING]
> 禁止从参考实现取具体卡牌信息（D122）；卡牌数据逐字取自「游戏内文本描述」。

## 5. 依据（规则书与 D 编号）

| 主题 | 依据 |
|---|---|
| 角色牌结构照参考项目；文案只填中文 | D120 |
| 参考实现只允许看实现方法，禁止取具体信息 | D122 |
| 第一张卡（爱诺）样板结构口径 | D162 |
| 具体卡牌路径一卡一目录 | D163 |
| 具体卡牌拆到独立 crate、目录不变 | D240 |
| 资源与形状留 definitions，只搬具体实例 | D241 |
| 源码嵌入宏与机制留 definitions，只有 `SOURCE_FILES` 跟卡牌走 | D242 |
| simulation 默认不含具体卡牌 | D243 |
| 版本同步宏口径（只写「已同步到」） | D244 / D245 / D246 |
| 卡牌多版本（未实现） | D248（留空 #24） |
| 卡牌数据依据 | 规则书 / 卡面文本（文件头注释） |

## 6. 常见误用

> [!WARNING]
> 不要把资源 / 形状类型放进 cards、不要让 definitions 反向依赖 cards、不要忘记 `SOURCE_FILES` 与 `card_game_version!`。

- 新版本同步时顺手改卡牌内容：没调整的卡只改版本数字（D246）。
- 在卡牌层重新实现规则 / 复制 definitions 的结算逻辑。
- 从参考实现抄卡牌具体数据（D122）。
- 现在就开始写「多版本卡牌」：D248 未实现（留空 #24）。
- 爱诺的 4 个效果体是 `todo!`：调用即 panic，只作数据样板。

## 7. 测试

- `cargo test --offline -p gi-genius-invokation-cards`：当前 11 条。
- 代表用例：

| 用例 | 覆盖点 |
|---|---|
| `cards/src/cards/character/aino/mod.rs::cards::character::aino::tests::aino_basic_data_matches_in_game_text` | 爱诺基础数据 |
| `...::aino_tags_are_three_zero_sized_tags` | 标签零大小元组 |
| `...::aino_skills_match_in_game_text` | 4 技能 / 消耗 / 文案 |
| `...::game_version_matches_declared_sync_version` | 版本宏 |
| `...::source_embeds_this_file_verbatim` | 源码嵌入原文 |
| `...::source_is_registered_in_source_registry` | 源码表登记 |
| `cards/src/source/mod.rs::source::tests::registry_lists_card_files_in_path_order` | 清单顺序 |
| `cards/src/source/mod.rs::source::tests::dependency_direction_is_cards_to_definitions` | 依赖方向守卫 |

## 8. 待接入与待实测

| 编号 | 事项 |
|---|---|
| #24 | 卡牌多版本（区间不重叠）+ 加载时选一个版本（D248）未实现；按卡独立选版本、默认最新（D249 / D251）待做 |
| #32 | 卡面「翻倍」类效果需要在 cards 层声明 `DamageArgs::with_modifiers`；卡牌尚未接入 |
| #92 | `has_combat_action_tag` 的数据来源在 cards 层（flow 只收 bool） |
| #81 / #109 | 料理（含复苏料理）的调度归 cards + flow；definitions 只给纯操作与 `flow::revive` |
| 无编号 | 注册机制缺：`cards/mod.rs` 明说「没有集中注册表，等注册机制（缺，P3）建好再接」；爱诺暂未注册 |
| 无编号 | 爱诺 4 个效果体是占位：伤害结算入口 / 状态生成 / 召唤物区 / 被动触发注册等接口在文件注释里列出 |

---

最后核对：2026-10-19（代码：`new/crates/cards/src/cards/character/aino/mod.rs:AINO`）
