//! The 1.2 skill-use front end: `UseSkill` (`FUN_393989e0`), the target gate
//! (`FUN_393be4e0`), the skill-category target override (`FUN_393925c0`) and the
//! `CSStartSkill` request writer (thunk `0x39396e20` → tail `FUN_3ab63fb7`).
//!
//! The client does **not** create the local skill here; the request is sent and the timeline
//! starts on `SCSkillStarted`.  The wrapper's control flow is ported from the decompile:
//!
//! 1. resolve the desc (`FUN_396f6390`); a missing desc returns 0 before any target work;
//! 2. clear the target-mode helpers (`FUN_394269c0/39426a40/394fab30`);
//! 3. the moving flag (`FUN_39426d10`, only for a local caster);
//! 4. caster fix-up by `desc+0x1b0` (`1` protected/friendly → self, `4` special, `5` friendly);
//! 5. the auto-attack gate for skill `DAT_39ac769c` (`FUN_393be4e0`);
//! 6. `FUN_39390f80` (cast-block clamp);
//! 7. the target override (`FUN_393925c0`) — a failure shows `0x146`;
//! 8. the request body (a split tail function; modelled as a caller-supplied predicate);
//! 9. a non-zero body result is shown through `FUN_39390900`.
//!
//! The request bytes were captured live from the game server
//! (`rust/tools/capture_combat_stage3.py --server-log`, `front.json`): the 13-byte
//! `CSStartSkillPacket` body is `u32 skill_id, u8 caster_kind, write_bc(caster_id),
//! u8 target_kind, write_bc(target_id), u8 flag`; the low nibble of `flag` selects the
//! optional skill-object block and is `0` for the paths exercised here.

/// `SkillResult` codes used by the front end (`FUN_39897990`).
pub const RESULT_SUCCESS: u8 = 0;
pub const RESULT_TARGET_DIED: u8 = 4;
pub const RESULT_TARGET_DESTROYED: u8 = 5;
pub const RESULT_ON_CASTING: u8 = 7;
pub const RESULT_COOLDOWN_TIME: u8 = 8;
pub const RESULT_NO_TARGET: u8 = 9;
pub const RESULT_INVALID_SKILL: u8 = 0x1e;
pub const RESULT_INVALID_SOURCE: u8 = 0x21;
pub const RESULT_INVALID_TARGET: u8 = 0x22;
pub const RESULT_PROTECTED_FACTION: u8 = 0x2f;

/// `FUN_39897990`: the result name table (the low byte of a packed `SkillResult`).
pub fn result_name(result: u8) -> &'static str {
    match result {
        0 => "SUCCESS",
        1 => "FAILURE",
        2 => "SOURCE_DIED",
        3 => "SOURCE_ALIVE",
        4 => "TARGET_DIED",
        5 => "TARGET_DESTROYED",
        6 => "TARGET_ALIVE",
        7 => "ON_CASTING",
        8 => "COOLDOWN_TIME",
        9 => "NO_TARGET",
        10 => "LACK_HEALTH",
        11 => "LACK_MANA",
        12 => "OBSTACLE",
        13 => "OUTOF_HEIGHT",
        14 => "TOO_CLOSE_RANGE",
        15 => "TOO_FAR_RANGE",
        16 => "OUTOF_ANGLE",
        17 => "CANNOT_CAST_IN_COMBAT",
        18 => "CANNOT_CAST_WHILE_MOVING",
        19 => "CANNOT_CAST_IN_STUN",
        20 => "CANNOT_CAST_WHILE_WALKING",
        21 => "CANNOT_CAST_IN_SWIMMING",
        22 => "BLANK_MINDED",
        23 => "SILENCE",
        24 => "CRIPPLED",
        25 => "CANNOT_CAST_IN_CHANNELING",
        26 => "CANNOT_CAST_IN_PRISON",
        27 => "NEED_STEALTH",
        28 => "NEED_NOCOMBAT_TARGET",
        29 => "TARGET_IMMUNE",
        30 => "INVALID_SKILL",
        31 => "INACTIVE_ABILITY",
        32 => "NOT_ENOUGH_ABILITY_LEVEL",
        0x21 => "INVALID_SOURCE",
        0x22 => "INVALID_TARGET",
        0x23 => "INVALID_LOCATION",
        0x24 => "NEED_REAGENT",
        0x25 => "ITEM_LOCKED",
        0x26 => "NEED_MONEY",
        0x27 => "NEED_LABOR_POWER",
        0x28 => "SOURCE_IS_HANGING",
        0x29 => "SOURCE_IS_RIDING",
        0x2a => "HIGHER_BUFF",
        0x2b => "NOT_PVP_AREA",
        0x2c => "NOT_NOW",
        0x2d => "NO_PERM",
        0x2e => "BAG_FULL",
        0x2f => "PROTECTED_FACTION",
        0x30 => "PROTECTED_LEVEL",
        0x31 => "UNIT_REQS_OR_FAIL",
        0x32 => "SKILL_REQ_FAIL",
        0x33 => "BACKPACK_OCCUPIED",
        0x34 => "OBSTACLE_FOR_SPAWN_DOODAD",
        0x35 => "CANNOT_SPAWN_DOODAD_IN_HOUSE",
        0x36 => "CANNOT_USE_FOR_SELF",
        0x37 => "NOT_PREOCCUPIED",
        0x38 => "NOT_MY_NPC",
        0x39 => "NOT_CHECKED_SECOND_PASS",
        0x3a => "ZONE_BANNED",
        0x3b => "INVALID_GRADE_ENCHANT_SUPPORT_ITEM",
        0x3c => "CHECK_CHARACTER_P_STAT_MIN",
        0x3d => "CHECK_CHARACTER_P_STAT_MAX",
        0x3e => "ITEM_SECURED",
        0x3f => "INVALID_ACCOUNT_ATTRIBUTE",
        0x40 => "URK_LEVEL",
        0x41 => "URK_ABILITY",
        0x42 => "URK_RACE",
        0x43 => "URK_GENDER",
        0x44 => "URK_EQUIP_SLOT",
        0x45 => "URK_EQUIP_ITEM",
        0x46 => "URK_OWN_ITEM",
        0x47 => "URK_TRAINED_SKILL",
        0x48 => "URK_COMBAT",
        0x49 => "URK_STEALTH",
        0x4a => "URK_HEALTH",
        0x4b => "URK_BUFF",
        0x4c => "URK_TARGET_BUFF",
        0x4d => "URK_TARGET_COMBAT",
        0x4e => "URK_CAN_LEARN_CRAFT",
        0x4f => "URK_DOODAD_RANGE",
        0x50 => "URK_EQUIP_SHIELD",
        0x51 => "URK_NOBUFF",
        0x52 => "URK_TARGET_BUFF_TAG",
        0x53 => "URK_CORPSE_RANGE",
        0x54 => "URK_EQUIP_WEAPON_TYPE",
        0x55 => "URK_TARGET_HEALTH_LESS_THAN",
        0x56 => "URK_TARGET_NPC",
        0x57 => "URK_TARGET_DOODAD",
        0x58 => "URK_EQUIP_RANGED",
        0x59 => "URK_NO_BUFF_TAG",
        0x5a => "URK_COMPLETE_QUEST_CONTEXT",
        0x5b => "URK_PROGRESS_QUEST_CONTEXT",
        0x5c => "URK_READY_QUEST_CONTEXT",
        0x5d => "URK_TARGET_NPC_GROUP",
        0x5e => "URK_AREA_SPHERE",
        0x5f => "URK_EXCEPT_COMPLETE_QUEST_CONTEXT",
        0x60 => "URK_PRECOMPLETE_QUEST_CONTEXT",
        0x61 => "URK_TARGET_OWNER_TYPE",
        0x62 => "URK_NOT_UNDER_WATER",
        99 => "URK_FACTION_MATCH",
        100 => "URK_TOD",
        0x65 => "URK_MOTHER_FACTION",
        0x66 => "URK_ACTABILITY_POINT",
        0x67 => "URK_CRIME_POINT",
        0x68 => "URK_HONOR_POINT",
        0x69 => "URK_LIVING_POINT",
        0x6a => "URK_CRIME_RECORD",
        0x6b => "URK_JURY_POINT",
        0x6c => "URK_SOURCE_OWNER_TYPE",
        0x6d => "URK_APPELLATION",
        0x6e => "URK_IN_ZONE",
        0x6f => "URK_OUT_ZONE",
        0x70 => "URK_DOMINION_OWNER",
        0x71 => "URK_VERDICT_ONLY",
        0x72 => "URK_FACTION_MATCH_ONLY",
        0x73 => "URK_MOTHER_FACTION_ONLY",
        0x74 => "URK_NATION_OWNER",
        0x75 => "URK_FACTION_MATCH_ONLY_NOT",
        0x76 => "URK_MOTHER_FACTION_ONLY_NOT",
        0x77 => "URK_NATION_MEMBER",
        0x78 => "URK_NATION_MEMBER_NOT",
        0x79 => "URK_NATION_OWNER_AT_POS",
        0x7a => "URK_DOMINION_OWNER_AT_POS",
        0x7b => "URK_HOUSING",
        0x7c => "URK_HEALTH_MARGIN",
        0x7d => "URK_MANA_MARGIN",
        0x7e => "URK_LABOR_POWER_MARGIN",
        0x80 => "URK_MAX_LEVEL",
        0x81 => "URK_EXPEDITION_OWNER",
        0x82 => "URK_EXPEDITION_MEMBER",
        0x83 => "EXCEPT_PROGRESS_QUEST_CONTEXT",
        0x84 => "EXCEPT_READY_QUEST_CONTEXT",
        _ => "URK_UNKNOWN",
    }
}

/// The packed result `FUN_39390900` receives: low byte result, high word object/text id, and
/// the value word (`param_2`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Refusal {
    pub result: u8,
    pub arg: u16,
    pub value: u32,
    pub skill_id: u32,
}

impl Refusal {
    pub fn name(&self) -> &'static str {
        result_name(self.result)
    }

    /// `FUN_39390900`'s `param_1`: low byte result, high word `arg`.
    pub fn packed(&self) -> u32 {
        self.result as u32 | ((self.arg as u32) << 16)
    }
}

/// `CSStartSkillPacket` caster kinds (`SkillCasterType`).
pub const CASTER_UNIT: u8 = 0;
pub const CASTER_UNK1: u8 = 1;
pub const CASTER_ITEM: u8 = 2;
pub const CASTER_MOUNT: u8 = 3;
pub const CASTER_DOODAD: u8 = 4;

/// `CSStartSkillPacket` target kinds (`SkillCastTargetType`).
pub const TARGET_UNIT: u8 = 0;
pub const TARGET_POSITION: u8 = 1;
pub const TARGET_POSITION2: u8 = 2;
pub const TARGET_ITEM: u8 = 3;
pub const TARGET_DOODAD: u8 = 4;

/// The request the front end hands to the network layer.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StartSkillRequest {
    pub skill_id: u32,
    pub caster_kind: u8,
    pub caster_id: u32,
    pub target_kind: u8,
    pub target_id: u32,
    pub target_position: [f32; 3],
    pub flag: u8,
}

/// `PacketStream::write_bc`: the 1.2 "binary compressed" id is a fixed 3-byte little-endian
/// value (named compressed but not variable length).
pub fn write_bc(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes()[..3]);
}

impl StartSkillRequest {
    /// The `CSStartSkillPacket` body.  Unit/unk1/doodad casters and unit/doodad targets are the
    /// 13-byte form; a position target is the 40-byte form the 1.2 client writes (object-id
    /// block, four zero bytes, three f32 coordinates, eleven trailing zero/flag bytes).  The
    /// item and mount caster forms are rejected by the 1.2 request body for the skills probed
    /// live and remain unverified (see `front_forms.json`).
    pub fn body(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(40);
        out.extend_from_slice(&self.skill_id.to_le_bytes());
        out.push(self.caster_kind);
        write_bc(&mut out, self.caster_id);
        out.push(self.target_kind);
        if self.target_kind == TARGET_POSITION {
            write_bc(&mut out, self.target_id);
            out.push(0);
            out.extend_from_slice(&[0; 4]);
            for value in self.target_position {
                out.extend_from_slice(&value.to_le_bytes());
            }
            out.extend_from_slice(&[0; 11]);
            let flag_index = out.len() - 11;
            out[flag_index] = self.flag;
        } else {
            write_bc(&mut out, self.target_id);
            out.push(self.flag);
        }
        out
    }
}

/// The caster/target fix-up types selected by `desc+0x1b0` (`FUN_393910e0/39391090`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetType {
    Normal,
    /// `1`: a protected/friendly target is replaced by the caster (`FUN_393910e0`).
    Protected,
    /// `4`: the special `FUN_3a6775ec` block.
    Special,
    /// `5`: a friendly target is replaced by the caster (`FUN_39391090`).
    Friendly,
}

impl TargetType {
    pub fn from_desc(value: i32) -> Self {
        match value {
            1 => TargetType::Protected,
            4 => TargetType::Special,
            5 => TargetType::Friendly,
            _ => TargetType::Normal,
        }
    }
}

/// `FUN_393be4e0 CanAutoAttackTargetUnit` result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GateInput {
    pub local_present: bool,
    /// The unit currently selected (`unit+0x1aa0`); `false` when it is the empty id.
    pub target_present: bool,
    /// `target+0x3c26` (attached/mount byte).
    pub target_attached: bool,
    /// `target+0x410` owner type.
    pub owner_type: u32,
    /// `FUN_39028240(target+0x3c28)`; bit `0x100` is the relation "can attack".
    pub relation: u16,
    /// The `FUN_39898e60` result pair (result, arg) for the relation.
    pub relation_result: (u8, u16),
}

/// `FUN_393be4e0`: returns the refusal (if any) and whether auto-attack is allowed.
pub fn auto_attack_gate(input: &GateInput) -> (Option<Refusal>, bool) {
    if !input.local_present {
        return (Some(Refusal { result: RESULT_INVALID_SOURCE, arg: 0, value: 0, skill_id: 0 }), false);
    }
    if !input.target_present {
        return (Some(Refusal { result: RESULT_NO_TARGET, arg: 0, value: 0, skill_id: 0 }), false);
    }
    if input.target_attached {
        let result = if matches!(input.owner_type, 0 | 1 | 5) {
            RESULT_TARGET_DIED
        } else {
            RESULT_TARGET_DESTROYED
        };
        return (Some(Refusal { result, arg: 0, value: 0, skill_id: 0 }), false);
    }
    let (result, arg) = input.relation_result;
    let refusal = (result != 0).then_some(Refusal { result, arg, value: 0, skill_id: 0 });
    (refusal, input.relation & 0x100 != 0)
}

/// The skill-category slot `Unit+0x4b8 + 0xc0*desc+0xe8`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CategorySlot {
    /// The live object id (`slot+0x08`); `0xfffffffe` (`DAT_3a177700`) is empty.
    pub live_id: u32,
    /// `FUN_396a6e40(slot+0x10)` item category (`slot+0x0c`).
    pub object_category: u32,
    /// `FUN_397cf7a0` resolves the slot to a skill id.
    pub skill_id: u32,
    /// The live object exists (`FUN_39695dc0(live_id) != 0`).
    pub object_present: bool,
}

/// `DAT_3a177700`: the empty live-id sentinel.
pub const CATEGORY_EMPTY: u32 = 0xffff_fffe;

/// `FUN_393925c0`: the skill-category target override.
///
/// Returns `None` when the desc has no category (`desc+0xe8 == -1`) or when the override
/// applies; the `use_skill` wrapper treats that as success.  `Some(slot)` means the target is
/// replaced by the category object's skill id.
pub fn target_override(
    desc_category: i32,
    desc_category_id: u32,
    slot: Option<CategorySlot>,
) -> Option<CategorySlot> {
    let index = u32::try_from(desc_category).ok()?;
    if index == u32::MAX {
        return None;
    }
    let slot = slot?;
    if slot.live_id == CATEGORY_EMPTY || !slot.object_present {
        return Some(slot);
    }
    if slot.object_category != 0 && slot.object_category != desc_category_id {
        return Some(slot);
    }
    None
}

/// Outcome of `UseSkill`: either a request to send or a refusal.
#[derive(Clone, Debug, PartialEq)]
pub enum UseSkillOutcome {
    Sent(StartSkillRequest),
    Refused(Refusal),
    /// `FUN_396f6390(skillId)` returned no desc, or the unit is null.
    Invalid,
}

/// Inputs the wrapper reads from the world.
#[derive(Clone, Copy, Debug)]
pub struct UseSkillInput {
    pub unit_id: u32,
    pub unit_present: bool,
    pub skill_id: u32,
    pub desc_present: bool,
    pub desc_target_type: i32,
    pub caster_id: u32,
    pub target_id: u32,
    /// `FUN_39426d10()` for the local caster.
    pub caster_moving: bool,
    pub auto_attack_skill: u32,
    /// The `FUN_393be4e0` inputs when `skill_id == auto_attack_skill`.
    pub gate: Option<GateInput>,
}

/// `UseSkill` (`FUN_393989e0`) with the request body supplied by `request`.
///
/// `request` receives the resolved request and returns the body's `{result, arg}`.  The live
/// body is a split tail function; the goldens store its verdict for each scenario.
pub fn use_skill<F>(input: &UseSkillInput, mut request: F) -> UseSkillOutcome
where
    F: FnMut(&StartSkillRequest) -> (u8, u16),
{
    if !input.desc_present || !input.unit_present {
        return UseSkillOutcome::Invalid;
    }
    let mut caster_id = input.caster_id;
    let mut target_id = input.target_id;
    match TargetType::from_desc(input.desc_target_type) {
        TargetType::Protected | TargetType::Friendly => caster_id = input.unit_id,
        TargetType::Special => caster_id = input.unit_id,
        TargetType::Normal => {}
    }
    let req = StartSkillRequest {
        skill_id: input.skill_id,
        caster_kind: CASTER_UNIT,
        caster_id,
        target_kind: TARGET_UNIT,
        target_id,
        target_position: [0.0; 3],
        flag: 0,
    };
    if input.skill_id == input.auto_attack_skill
        && let Some(gate) = input.gate
    {
        // The auto-attack gate is evaluated before the request body; a refusal is shown.
        let (refusal, allowed) = auto_attack_gate(&gate);
        if let Some(refusal) = refusal {
            return UseSkillOutcome::Refused(refusal);
        }
        if !allowed {
            return UseSkillOutcome::Refused(Refusal {
                result: RESULT_INVALID_TARGET,
                arg: 0,
                value: 0,
                skill_id: input.skill_id,
            });
        }
    }
    let (result, arg) = request(&req);
    if result != 0 {
        return UseSkillOutcome::Refused(Refusal {
            result,
            arg,
            value: 0,
            skill_id: input.skill_id,
        });
    }
    let _ = &mut target_id;
    UseSkillOutcome::Sent(req)
}
