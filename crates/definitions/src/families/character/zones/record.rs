//! **角色区 / 出战状态区 / 装备栏的记录内容类型 + 记录入口**（D221 / D223 / D224 / D225）。
//!
//! 记录的唯一入口是 [crate::records::Recorder]（极薄容器：属于哪一步 + 任意内容）。
//! 内容类型由本文件定义（D225：单独一个文件），显示由 [ShowRecord] 决定。
//!
//! ## 口径
//!
//! - **旧入口不变**：已有 add_with_triggers / remove_with_triggers / ... 原样保留、行为不变；
//!   这里新增 **带记录的入口**（原名 + _recorded，多一个 &mut Recorder + step 参数）；
//! - 记录的是**真实动作**（进入 / 离场 / 切换 / 补位 / 附着 / 转移 / 移除 / 持续回合递减 /
//!   装备 / 替换旧装备弃置 / 卸下），不是前后局面 diff；
//! - feature records 关闭时：Recorder 是零大小类型、push 为空操作，记录内容**不构造**
//!   （cfg 分支包住 push），结算结果完全不变（D223）。
//!
//! 依据：D221（插入式结算回放）、D223 / D224（记录 API 极薄）、D225（内容类型单独文件）。

use crate::families::character::character::CharacterState;
use crate::records::{Recorder, ShowRecord};
use gi_genius_invokation_engine::{EntityId, Nat};

use super::combat_status::{CombatStatusResolution, CombatStatusTriggers, CombatStatusZone};
use super::equipment::{EquipmentEntry, EquipmentResolution, EquipmentTriggers, EquipmentZone};
use super::field::{CharacterZone, CharacterZoneResolution, CharacterZoneTriggers};

/// 本模块的记录器类型（内容类型由本模块定，容器用公用 Recorder）。
pub type CharacterZoneRecorder = Recorder<CharacterZoneRecord>;

/// 角色区 / 出战状态区 / 装备栏的记录内容（纯数据；显示见 ShowRecord）。
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CharacterZoneRecord {
    /// 角色进入角色区。
    Entered {
        /// 进入的角色实体。
        character: EntityId,
        /// 是否因此成为出战角色（第一个进入的会自动成为出战）。
        becomes_active: bool,
    },
    /// 角色离开角色区（被击倒 / 移出比赛）。
    Left {
        /// 离开的角色实体。
        character: EntityId,
        /// 离开的是不是当前出战角色。
        was_active: bool,
    },
    /// 静默设置出战角色（游戏准备用）。
    ActiveSet {
        /// 新的出战角色。
        character: EntityId,
        /// 之前的出战角色（可能没有）。
        previous: Option<EntityId>,
    },
    /// 切换角色（战斗行动）。
    ActiveSwitched {
        /// 原出战角色。
        from: EntityId,
        /// 新出战角色。
        to: EntityId,
    },
    /// 被击倒后的强制补位。
    AfterDefeatChosen {
        /// 新的出战角色。
        to: EntityId,
    },
    /// 出战状态附着。
    StatusAttached {
        /// 挂在哪个出战角色上。
        active: EntityId,
        /// 状态名（状态模块给的显示名）。
        status: &'static str,
        /// 层数（布尔类 None）。
        layers: Option<Nat>,
        /// 持续回合（可选）。
        duration: Option<Nat>,
    },
    /// 出战状态移除。
    StatusRemoved {
        /// 状态名。
        status: &'static str,
    },
    /// 切换角色时出战状态转移。
    StatusTransferred {
        /// 原挂载角色。
        from: EntityId,
        /// 新挂载角色。
        to: EntityId,
        /// 转移的状态条数。
        count: Nat,
    },
    /// 结束阶段持续回合 -1（未耗尽）。
    StatusTicked {
        /// 状态名。
        status: &'static str,
        /// 剩余持续回合。
        remaining: Nat,
    },
    /// 结束阶段持续回合耗尽、状态被弃置。
    StatusDepleted {
        /// 状态名。
        status: &'static str,
    },
    /// 装备挂上（空槽）。
    Equipped {
        /// 装备栏属于谁。
        owner: EntityId,
        /// 新装备实体。
        equip: EntityId,
    },
    /// 同槽 / 同种替换：旧装备弃置、新装备挂上。
    EquipmentReplaced {
        /// 装备栏属于谁。
        owner: EntityId,
        /// 新装备实体。
        equip: EntityId,
        /// 被弃置的旧装备实体。
        discarded: EntityId,
    },
    /// 装备被卸下 / 弃置。
    EquipmentDiscarded {
        /// 装备栏属于谁。
        owner: EntityId,
        /// 被弃置的装备实体。
        equip: EntityId,
    },
}

impl ShowRecord for CharacterZoneRecord {
    fn show(&self, out: &mut String) {
        match self {
            CharacterZoneRecord::Entered {
                character,
                becomes_active,
            } => {
                out.push_str("角色 ");
                push_nat(out, character);
                out.push_str(" 进入角色区");
                if *becomes_active {
                    out.push_str("（成为出战角色）");
                }
            }
            CharacterZoneRecord::Left {
                character,
                was_active,
            } => {
                out.push_str("角色 ");
                push_nat(out, character);
                out.push_str(" 离开角色区");
                if *was_active {
                    out.push_str("（原为出战角色）");
                }
            }
            CharacterZoneRecord::ActiveSet {
                character,
                previous,
            } => {
                out.push_str("设置出战角色为 ");
                push_nat(out, character);
                if let Some(previous) = previous {
                    out.push_str("（原出战 ");
                    push_nat(out, previous);
                    out.push('）');
                }
            }
            CharacterZoneRecord::ActiveSwitched { from, to } => {
                out.push_str("切换角色：");
                push_nat(out, from);
                out.push_str(" -> ");
                push_nat(out, to);
            }
            CharacterZoneRecord::AfterDefeatChosen { to } => {
                out.push_str("被击倒后补位：出战角色变为 ");
                push_nat(out, to);
            }
            CharacterZoneRecord::StatusAttached {
                active,
                status,
                layers,
                duration,
            } => {
                out.push_str("出战状态「");
                out.push_str(status);
                out.push_str("」附着到角色 ");
                push_nat(out, active);
                if let Some(layers) = layers {
                    out.push_str("，层数 ");
                    push_nat(out, layers);
                }
                if let Some(duration) = duration {
                    out.push_str("，持续 ");
                    push_nat(out, duration);
                    out.push_str(" 回合");
                }
            }
            CharacterZoneRecord::StatusRemoved { status } => {
                out.push_str("出战状态「");
                out.push_str(status);
                out.push_str("」移除");
            }
            CharacterZoneRecord::StatusTransferred { from, to, count } => {
                out.push_str("出战状态转移：");
                push_nat(out, from);
                out.push_str(" -> ");
                push_nat(out, to);
                out.push_str("，共 ");
                push_nat(out, count);
                out.push_str(" 条");
            }
            CharacterZoneRecord::StatusTicked { status, remaining } => {
                out.push_str("出战状态「");
                out.push_str(status);
                out.push_str("」持续回合递减，剩余 ");
                push_nat(out, remaining);
            }
            CharacterZoneRecord::StatusDepleted { status } => {
                out.push_str("出战状态「");
                out.push_str(status);
                out.push_str("」持续回合耗尽，弃置");
            }
            CharacterZoneRecord::Equipped { owner, equip } => {
                out.push_str("角色 ");
                push_nat(out, owner);
                out.push_str(" 装备 ");
                push_nat(out, equip);
            }
            CharacterZoneRecord::EquipmentReplaced {
                owner,
                equip,
                discarded,
            } => {
                out.push_str("角色 ");
                push_nat(out, owner);
                out.push_str(" 更换装备：");
                push_nat(out, discarded);
                out.push_str(" 弃置，");
                push_nat(out, equip);
                out.push_str(" 挂上");
            }
            CharacterZoneRecord::EquipmentDiscarded { owner, equip } => {
                out.push_str("角色 ");
                push_nat(out, owner);
                out.push_str(" 的装备 ");
                push_nat(out, equip);
                out.push_str(" 弃置");
            }
        }
    }
}

/// 追加一个 Nat（Nat 自带十进制 Display；零依赖、不出现裸整数）。
fn push_nat(out: &mut String, value: &Nat) {
    out.push_str(&value.to_string());
}

impl CharacterZone {
    /// 进入角色区（带记录；旧入口 add_with_triggers 不变）。
    pub fn add_recorded(
        &self,
        character: EntityId,
        state: CharacterState,
        triggers: &CharacterZoneTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<CharacterZoneResolution> {
        let becomes_active = self.active().is_none();
        let resolution = self.add_with_triggers(character, state, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(
            step,
            CharacterZoneRecord::Entered {
                character,
                becomes_active,
            },
        );
        #[cfg(not(feature = "records"))]
        {
            let _ = (recorder, step, becomes_active);
        }
        Some(resolution)
    }

    /// 离开角色区（带记录）。
    pub fn remove_recorded(
        &self,
        character: EntityId,
        triggers: &CharacterZoneTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<CharacterZoneResolution> {
        let was_active = self.active_entity() == Some(character);
        let resolution = self.remove_with_triggers(character, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(
            step,
            CharacterZoneRecord::Left {
                character,
                was_active,
            },
        );
        #[cfg(not(feature = "records"))]
        {
            let _ = (recorder, step, was_active);
        }
        Some(resolution)
    }

    /// 静默设置出战角色（带记录）。
    pub fn set_active_recorded(
        &self,
        character: EntityId,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<CharacterZone> {
        let previous = self.active_entity();
        let zone = self.set_active(character)?;
        #[cfg(feature = "records")]
        recorder.push(
            step,
            CharacterZoneRecord::ActiveSet {
                character,
                previous,
            },
        );
        #[cfg(not(feature = "records"))]
        {
            let _ = (recorder, step, previous);
        }
        Some(zone)
    }

    /// 切换角色（带记录）。
    pub fn switch_active_recorded(
        &self,
        to: EntityId,
        triggers: &CharacterZoneTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<CharacterZoneResolution> {
        let from = self.active_entity()?;
        let resolution = self.switch_active_with_triggers(to, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(step, CharacterZoneRecord::ActiveSwitched { from, to });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, from, to);
        Some(resolution)
    }

    /// 被击倒后的强制补位（带记录）。
    pub fn choose_active_after_defeat_recorded(
        &self,
        to: EntityId,
        triggers: &CharacterZoneTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<CharacterZoneResolution> {
        let resolution = self.choose_active_after_defeat_with_triggers(to, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(step, CharacterZoneRecord::AfterDefeatChosen { to });
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, to);
        Some(resolution)
    }
}

impl CombatStatusZone {
    /// 附着出战状态（带记录）。
    #[allow(clippy::too_many_arguments)] // 记录入口 = 旧签名 + recorder + step；保持旧入口可用的代价
    pub fn attach_recorded(
        &self,
        active: EntityId,
        status: &'static dyn crate::families::action_card::status::StateIdentity,
        layers: Option<Nat>,
        duration: Option<Nat>,
        triggers: &CombatStatusTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> CombatStatusResolution {
        let resolution = self.attach_with_triggers(active, status, layers, duration, triggers);
        #[cfg(feature = "records")]
        recorder.push(
            step,
            CharacterZoneRecord::StatusAttached {
                active,
                status: status.name(),
                layers,
                duration,
            },
        );
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, active, status, layers, duration);
        resolution
    }

    /// 移除出战状态（带记录）。
    pub fn remove_recorded(
        &self,
        status: &'static dyn crate::families::action_card::status::StateIdentity,
        triggers: &CombatStatusTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<CombatStatusResolution> {
        let resolution = self.remove_with_triggers(status, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(
            step,
            CharacterZoneRecord::StatusRemoved {
                status: status.name(),
            },
        );
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, status);
        Some(resolution)
    }

    /// 切换角色时转移出战状态（带记录）。
    pub fn transfer_recorded(
        &self,
        new_active: EntityId,
        triggers: &CombatStatusTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<CombatStatusResolution> {
        let from = self.attached_to()?;
        let count = self.len();
        let resolution = self.transfer_with_triggers(new_active, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(
            step,
            CharacterZoneRecord::StatusTransferred {
                from,
                to: new_active,
                count,
            },
        );
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, from, new_active, count);
        Some(resolution)
    }

    /// 结束阶段：持续回合递减 / 耗尽弃置（带记录）。
    pub fn tick_end_phase_recorded(
        &self,
        triggers: &CombatStatusTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> CombatStatusResolution {
        let before: Vec<(&'static str, &'static dyn crate::families::action_card::status::StateIdentity)> =
            self.entries()
                .iter()
                .map(|entry| (entry.status.name(), entry.status))
                .collect();
        let resolution = self.tick_end_phase_with_triggers(triggers);
        let after = resolution.zone.entries();
        #[cfg(not(feature = "records"))]
        let _ = &recorder;
        for (name, status) in before {
            let remaining = after
                .iter()
                .find(|entry| std::ptr::eq(entry.status, status))
                .and_then(|entry| entry.duration);
            #[cfg(feature = "records")]
            match remaining {
                Some(remaining) => recorder.push(
                    step,
                    CharacterZoneRecord::StatusTicked {
                        status: name,
                        remaining,
                    },
                ),
                None => recorder.push(step, CharacterZoneRecord::StatusDepleted { status: name }),
            }
            #[cfg(not(feature = "records"))]
            {
                let _ = (step, name, status, remaining);
            }
        }
        resolution
    }
}

impl EquipmentZone {
    /// 挂装备（带记录：空槽 / 替换旧装备弃置 / 被拒绝）。
    pub fn equip_recorded(
        &self,
        entry: EquipmentEntry,
        required_weapon: Option<crate::rules::ops::equip::WeaponType>,
        character: &crate::rules::ops::equip::CharacterEquip,
        triggers: &EquipmentTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> EquipmentResolution {
        let resolution = self.equip_with_triggers(entry, required_weapon, character, triggers);
        #[cfg(feature = "records")]
        match resolution.outcome {
            super::equipment::EquipmentOutcome::Attached => recorder.push(
                step,
                CharacterZoneRecord::Equipped {
                    owner: self.owner(),
                    equip: entry.equip,
                },
            ),
            super::equipment::EquipmentOutcome::Replaced { discarded, .. }
            | super::equipment::EquipmentOutcome::TalentOverwritten { discarded } => recorder.push(
                step,
                CharacterZoneRecord::EquipmentReplaced {
                    owner: self.owner(),
                    equip: entry.equip,
                    discarded,
                },
            ),
            super::equipment::EquipmentOutcome::Rejected(_) => {}
            super::equipment::EquipmentOutcome::Discarded { equip } => recorder.push(
                step,
                CharacterZoneRecord::EquipmentDiscarded {
                    owner: self.owner(),
                    equip,
                },
            ),
        }
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, entry, required_weapon, character, triggers);
        resolution
    }

    /// 卸下装备（带记录）。
    pub fn detach_recorded(
        &self,
        equip: EntityId,
        triggers: &EquipmentTriggers,
        recorder: &mut CharacterZoneRecorder,
        step: Nat,
    ) -> Option<EquipmentResolution> {
        let resolution = self.detach_with_triggers(equip, triggers)?;
        #[cfg(feature = "records")]
        recorder.push(
            step,
            CharacterZoneRecord::EquipmentDiscarded {
                owner: self.owner(),
                equip,
            },
        );
        #[cfg(not(feature = "records"))]
        let _ = (recorder, step, equip);
        Some(resolution)
    }
}

#[cfg(all(test, feature = "records"))]
mod tests {
    use super::*;
    use crate::base::CardId;
    use crate::families::action_card::status::{StateForm, StateIdentity, StateText};
    use crate::generated::Localized;
    use crate::rules::ops::equip::{CharacterEquip, SlotKind};

    static TEST_TEXT: StateText = StateText {
        name: Localized::from_all("测试状态", "Test", "テスト", "테스트"),
        description: Localized::EMPTY,
    };

    #[derive(Debug)]
    struct TestState(&'static StateText);

    impl StateIdentity for TestState {
        fn text(&self) -> &'static StateText {
            self.0
        }

        fn form(&self) -> StateForm {
            StateForm::Boolean
        }
    }

    static TEST_STATE: TestState = TestState(&TEST_TEXT);

    fn shown(record: &CharacterZoneRecord) -> String {
        let mut out = String::new();
        record.show(&mut out);
        out
    }

    #[test]
    fn field_actions_are_recorded_with_steps() {
        let triggers = CharacterZoneTriggers::new();
        let mut recorder = CharacterZoneRecorder::new();
        let zone = CharacterZone::new();
        let first = zone
            .add_recorded(
                Nat::ZERO,
                CharacterState::default(),
                &triggers,
                &mut recorder,
                Nat::new(1),
            )
            .expect("可进入");
        let second = first
            .zone
            .add_recorded(
                Nat::new(1),
                CharacterState::default(),
                &triggers,
                &mut recorder,
                Nat::new(2),
            )
            .expect("可进入");
        let resolution = second
            .zone
            .switch_active_recorded(Nat::new(1), &triggers, &mut recorder, Nat::new(3))
            .expect("可切换");
        assert_eq!(recorder.entries().len(), 3);
        assert_eq!(recorder.entries().first().expect("有记录").step, Nat::new(1));
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            CharacterZoneRecord::Entered {
                character: Nat::ZERO,
                becomes_active: true
            }
        );
        let last = recorder.entries().last().expect("有记录");
        assert_eq!(last.step, Nat::new(3));
        assert_eq!(
            last.content,
            CharacterZoneRecord::ActiveSwitched {
                from: Nat::ZERO,
                to: Nat::new(1)
            }
        );
        assert!(shown(&last.content).contains("切换角色"));
        assert!(!resolution.pending.is_empty() || resolution.pending.is_empty());
        // 旧入口仍可用、不被记录影响（不传 recorder）。
        assert!(zone
            .add_with_triggers(
                Nat::ZERO,
                CharacterState::default(),
                &triggers
            )
            .is_some());
    }

    #[test]
    fn combat_status_attach_and_equipment_replace_are_recorded() {
        let triggers = CombatStatusTriggers::new();
        let mut recorder = CharacterZoneRecorder::new();
        let zone = CombatStatusZone::new();
        zone.attach_recorded(
            Nat::ZERO,
            &TEST_STATE,
            Some(Nat::new(2)),
            Some(Nat::new(3)),
            &triggers,
            &mut recorder,
            Nat::new(1),
        );
        assert_eq!(
            recorder.entries().first().expect("有记录").content,
            CharacterZoneRecord::StatusAttached {
                active: Nat::ZERO,
                status: "测试状态",
                layers: Some(Nat::new(2)),
                duration: Some(Nat::new(3))
            }
        );
        assert!(shown(&recorder.entries().first().expect("有记录").content).contains("附着"));

        let triggers = EquipmentTriggers::new();
        let zone = EquipmentZone::new(Nat::ZERO);
        let first = zone.equip_recorded(
            EquipmentEntry {
                equip: Nat::new(10),
                card: CardId::new(1),
                slot: SlotKind::Artifact,
            },
            None,
            &CharacterEquip::OTHER_WEAPON,
            &triggers,
            &mut recorder,
            Nat::new(2),
        );
        assert_eq!(
            recorder.entries().last().expect("有记录").content,
            CharacterZoneRecord::Equipped {
                owner: Nat::ZERO,
                equip: Nat::new(10)
            }
        );
        let second = first.zone.equip_recorded(
            EquipmentEntry {
                equip: Nat::new(11),
                card: CardId::new(2),
                slot: SlotKind::Artifact,
            },
            None,
            &CharacterEquip::OTHER_WEAPON,
            &triggers,
            &mut recorder,
            Nat::new(3),
        );
        assert_eq!(
            recorder.entries().last().expect("有记录").content,
            CharacterZoneRecord::EquipmentReplaced {
                owner: Nat::ZERO,
                equip: Nat::new(11),
                discarded: Nat::new(10)
            }
        );
        assert!(shown(&recorder.entries().last().expect("有记录").content).contains("更换装备"));
        let _ = second.zone;
    }
}
