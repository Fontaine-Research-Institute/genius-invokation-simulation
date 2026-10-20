# Card Definition: Aino

> [[卡牌定义-爱诺|中文]] · English

*Scope: the character card "Aino".*

---

## Aino

![Aino](https://raw.githubusercontent.com/Fontaine-Research-Institute/genius-invokation-simulation/main/assets/character/1216.jpg)
> <p><sub>图片来源：米哈游《原神》官方素材，版权归 miHoYo / HoYoverse 所有。<br>Image source: official Genshin Impact artwork by HoYoverse. All rights belong to miHoYo / HoYoverse.</sub></p>

> Hydro, Claymore, Nod-Krai
>
> Max HP 10
>
> Max Energy 2

```rust
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

### Bish-Bash-Bosh Repair

> Normal Attack
>
> Cost 1 Hydro Elemental Dice, 2 Any Elemental Dice

```rust
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

> Deals 2 [[Keyword-Physical-DMG|Physical DMG]].

```rust
fn normal_attack_effect() {
    // 效果：造成 2 点物理伤害。
    // 缺：伤害结算入口 —— `DamageType::Physical` 只有类型（families/character/types.rs），
    //     `families/character/damage.rs` 的「造成伤害」入口尚未建立。
    todo!("缺：伤害结算入口（damage.rs 未建）")
}
```

### Musecatcher

> Elemental Skill
>
> Cost 3 Hydro Elemental Dice

```rust
pub static SKILL_12162: SkillDef = SkillDef {
    id: SKILL_ID_ELEMENTAL_SKILL,
    name: zh_only("妙思捕手"),
    description: zh_only(
        "造成2点水元素伤害，生成1层敏捷切换。如果手牌中有卡牌具有赋能，则改为造成3点水元素伤害。",
    ),
    kind: SkillKind::ElementalSkill,
    cost: SkillCost::from_dice(&[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))]),
    effect: elemental_skill_effect,
};
```

> Deal 2 [[Keyword-Hydro-DMG|Hydro DMG]] with 1 stack of [[Keyword-Agile-Switch|Agile Switch]] created. If you have a card with [[Keyword-Empowerment|Empowerment]] in your Hand, deal 3 [[Keyword-Hydro-DMG|Hydro DMG]] instead.

*注: 代码注释中的关键字编号已替换为名称.*
```rust
fn elemental_skill_effect() {
    // 效果：造成 2 点水元素伤害，生成 1 层「敏捷切换」；
    //       如果手牌中有卡牌具有「赋能」，则改为造成 3 点水元素伤害。
    // 缺：伤害结算入口（damage.rs 未建）。
    // 缺：状态定义与增删接口（「敏捷切换」状态定义未建）。
    // 缺：手牌查询 + 关键词「赋能」（赋能）判定（手牌区 / 关键词查询未建）。
    todo!("缺：伤害结算入口 + 状态生成 + 手牌赋能判定")
}
```

### Precision Hydronic Cooler

> Elemental Burst
>
> Cost 3 Hydro Elemental Dice
>
> 2 Energy

```rust
pub static SKILL_12163: SkillDef = SkillDef {
    id: SKILL_ID_ELEMENTAL_BURST,
    name: zh_only("精密水冷仪"),
    description: zh_only("造成2点水元素伤害，召唤冷静一下鸭。"),
    kind: SkillKind::ElementalBurst,
    cost: SkillCost::from_dice_and_energy(
        &[CostComponent::SpecificElement(Element::Hydro, Nat::new(3))],
        Nat::new(2),
    ),
    effect: elemental_burst_effect,
};
```

> Deals 2 [[Keyword-Hydro-DMG|Hydro DMG]] and summons Cool Your Jets Ducky.

```rust
fn elemental_burst_effect() {
    // 效果：造成 2 点水元素伤害，召唤「冷静一下鸭」。
    // 缺：伤害结算入口（damage.rs 未建）。
    // 缺：召唤物定义与召唤物区（冷静一下鸭 112161：SummonDef / zones/summon.rs 未建）。
    todo!("缺：伤害结算入口 + 召唤物区")
}
```

### Modular Efficiency Protocol

> Passive Skill

```rust
pub static SKILL_12164: SkillDef = SkillDef {
    id: SKILL_ID_PASSIVE,
    name: zh_only("模块式高效运作"),
    description: zh_only(
        "【被动】我方卡牌被赋予赋能时：如果我方场上存在冷静一下鸭，则使其可用次数+1，否则自身获得1点充能。（每回合1次）",
    ),
    kind: SkillKind::Passive,
    cost: SkillCost::FREE,
    effect: passive_effect,
};
```

> (Passive) When [[Keyword-Empowerment|Empowerment]] is attached to your card: If there is Cool Your Jets Ducky on your side, it gains 1 [[Keyword-Usage|Usage]], otherwise, gains 1 [[Keyword-Energy|Energy]]. (Once per Round)

```rust
fn passive_effect() {
    // 效果：【被动】我方卡牌被赋予「赋能」时：如果我方场上存在「冷静一下鸭」，
    //       则使其可用次数 +1，否则自身获得 1 点充能。（每回合 1 次）
    // 缺：被动 / 触发注册机制（"被赋予赋能时"的钩子未建）。
    // 缺：召唤物区查询（我方场上是否存在冷静一下鸭）。
    // 缺：回合时间线 + 每回合 1 次计数。
    // 注：自身获得 1 点充能这一步**底层已有**
    //     `CharacterState::energy_add`（families/character/character.rs），只差接线。
    todo!("缺：被动触发注册 + 召唤物区查询 + 每回合 1 次计数")
}
```


## Missing Images

> [!NOTE]
> Image to be added: the following card art is not in the repository yet: Aino.
