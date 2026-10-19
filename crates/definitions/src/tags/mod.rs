//! **标签层（L3）**：与 `families` 同级的**顶层目录**。
//!
//! 结构（用户已拍板，最新）：
//! - **families/**：只放**类型枚举**（`ActionCardType` / `StatusType`；其余牌族没有类型）；
//! - **tags/**（本目录）：**标签**，内部**按 牌族 / 类型 分组**；
//!   实现按具体的 families / tags 去选，「没实现对应标签的牌族不允许调用」。
//!
//! **D54：每个标签 = 一个自定义的零大小类型**（不是 `str`、不是整数，无字段、编译后消失），
//! **一个标签一个文件**；**没有"总标签枚举"**，也不做数字编号。
//! **只允许导入最终层级**：卡牌只能 `use` 到具体那个标签，例如
//! `use crate::tags::action_card::support::location::Location;`，导不到"某类型的标签集合"。
//!
//! **运行期身份（D67）**：标签数据只有一份 —— build.rs 从 generated/data/terms.json 生成
//! `generated::TAGS`，**一个标签一个字段**（例如 `TAGS.location`）。
//! - 访问一律**字段访问**：`TAGS.location.name.get(Lang::Zh)`；编译后 = 基址 + 固定偏移，**没有查表**；
//! - **不建**数组 / 索引表 / 名字→下标映射 / HashMap 之类「为了访问而额外存的数据」；
//! - 运行期身份 = **字段地址**（`&TAGS.location`）；标签文件只留 `pub fn data() -> &'static TagText`
//!   返回这个引用，**不写字面量**（数据唯一一份在 TAGS）；
//! - 查询按指针比较 / 筛选，**不用数字编号、不用 TypeId**。
//!
//! **唯一定义**（D55）：每个标签只有一份定义（就在本树里那一个文件），使用处只导入那一份，
//! 不许第二份。跨牌族共用的标签（武器子类、元素、国家/势力等）由使用处导入定义处那一份：
//! - 武器子类（单手剑/弓/双手剑/法器/长柄武器）：唯一定义在 `action_card/equipment/`，角色导入；
//! - 元素 / 国家·势力 / 其他武器：唯一定义在 `character/`，敌人导入；
//! - 减伤 / 不可发现 / 希穆兰卡：召唤物导入 `action_card/` 下对应那一份。
//!
//! 分组：
//! - `tags/action_card/{support,equipment,event}`：三种行动牌各自的标签；
//! - `tags/action_card/common`：行动牌跨类型公共标签（家庭层级公共标签，需要的人直接导入这里）；
//! - `tags/character/{element,nation,arkhe}` + `other_weapons.rs`：角色（与敌人共用）的标签；
//! - `tags/summon/`：召唤物自己的标签（草元素产物），其余导入别处；
//! - `tags/enemy/`：敌人自己的标签（无元素类型），其余导入别处；
//! - `tags/status/`：角色状态 / 出战状态的标签（游戏内文本描述里状态没有任何标签 → 本轮先空）。
//!
//! 标签全集来自游戏内文本描述（`tagstext` 字段），
//! 按 D55「全建」写入。卡牌的**标签集合形态**（元组 / Vec / 位图 …）本轮**先不定**。

pub mod action_card;
pub mod character;
pub mod enemy;
pub mod status;
pub mod summon;

#[cfg(test)]
mod tests {
    use super::*;
    use std::any::type_name;

    /// D54/D55：每个标签都是**零大小类型**，且**标签类型名互不相同**（一标签一类型）。
    ///
    /// 如果哪天又退回"一个总枚举 + 常量"，类型名会重复，这里立刻穿帮；
    /// 新增/删除标签时同步改下面的清单（这是有意的：让标签变动可见）。
    #[test]
    fn every_tag_is_a_distinct_zero_sized_type() {
        let mut names: Vec<&'static str> = Vec::new();

        macro_rules! check {
            ($($ty:path),* $(,)?) => {
                $(
                    assert_eq!(
                        std::mem::size_of::<$ty>(),
                        0,
                        "标签必须是零大小类型：{}",
                        type_name::<$ty>()
                    );
                    names.push(type_name::<$ty>());
                )*
            };
        }

        check!(
            action_card::support::location::Location,
            action_card::support::companion::Companion,
            action_card::support::item::Item,
            action_card::support::elemental_transmutation::ElementalTransmutation,
            action_card::support::adventure_spot::AdventureSpot,
            action_card::support::untransformed::Untransformed,
            action_card::equipment::weapon::Weapon,
            action_card::equipment::artifact::Artifact,
            action_card::equipment::talent::Talent,
            action_card::equipment::technique::Technique,
            action_card::equipment::sword::Sword,
            action_card::equipment::bow::Bow,
            action_card::equipment::claymore::Claymore,
            action_card::equipment::catalyst::Catalyst,
            action_card::equipment::polearm::Polearm,
            action_card::equipment::hexerei::Hexerei,
            action_card::event::food::Food,
            action_card::event::arcane_legend::ArcaneLegend,
            action_card::event::resonance::Resonance,
            action_card::event::simulanka::Simulanka,
            action_card::common::undiscoverable::Undiscoverable,
            action_card::common::scrapped::Scrapped,
            action_card::common::automaton_card::AutomatonCard,
            action_card::common::combat_action::CombatAction,
            action_card::common::shield::Shield,
            action_card::common::damage_reduction::DamageReduction,
            character::element::pyro::Pyro,
            character::element::hydro::Hydro,
            character::element::electro::Electro,
            character::element::dendro::Dendro,
            character::element::cryo::Cryo,
            character::element::anemo::Anemo,
            character::element::geo::Geo,
            character::nation::mondstadt::Mondstadt,
            character::nation::liyue::Liyue,
            character::nation::inazuma::Inazuma,
            character::nation::sumeru::Sumeru,
            character::nation::fontaine::Fontaine,
            character::nation::natlan::Natlan,
            character::nation::nodkrai::NodKrai,
            character::nation::fatui::Fatui,
            character::nation::eremites::Eremites,
            character::nation::monster::Monster,
            character::nation::hilichurl::Hilichurl,
            character::nation::consecrated_beast::ConsecratedBeast,
            character::nation::cosmic_calamity::CosmicCalamity,
            character::nation::kairagi::Kairagi,
            character::arkhe::ousia::Ousia,
            character::arkhe::pneuma::Pneuma,
            character::other_weapons::OtherWeapons,
            summon::dendro_construct::DendroConstruct,
            enemy::no_elemental_type::NoElementalType,
        );

        let total = names.len();
        assert_eq!(total, 52, "标签清单数量变了：新增/删除标签要同步本测试");
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "标签类型名出现重复：不是「一标签一类型」");
    }
}
