//! Skill requirement evaluators.
//!
//! `FUN_393a0510` walks a skill's `unit_reqs`/`or_unit_reqs` vector, filters each row with
//! `FUN_3936c500`, switches on the row implementation kind and calls one of the evaluators.
//! Every evaluator returns the 8-byte [`ReqResult`] (`u8 result, u16 text id, u32 value`); the
//! first non-zero result wins.  The row kind is the *implementation* kind at `impl+4`; `impl+8`
//! points at the value array used by the evaluator.
//!
//! Only the client-side decisions are ported: item/unit/buff lookups and every arithmetic and
//! control-flow branch are from the decompile.  Calls into UI-protection, quest-context and
//! engine-placement subsystems are inputs on [`ReqCtx`] (`helpers`), because those live in
//! other modules; the goldens record the same values.  A `ReqResult` of 0 is success.

use std::collections::BTreeMap;

/// The 8-byte evaluator result layout (`+0` result, `+2` text id, `+4` value).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReqResult {
    pub result: u8,
    pub text_id: u16,
    pub value: u32,
}

impl ReqResult {
    pub const SUCCESS: Self = Self { result: 0, text_id: 0, value: 0 };

    pub fn fail(result: u8) -> Self {
        Self { result, text_id: 0, value: 0 }
    }

    pub fn fail_text(result: u8, text_id: u16) -> Self {
        Self { result, text_id, value: 0 }
    }

    pub fn fail_value(result: u8, value: u32) -> Self {
        Self { result, text_id: 0, value }
    }

    pub fn is_success(&self) -> bool {
        self.result == 0
    }
}

/// `SkillResult` codes the evaluators return.
pub const INVALID_SKILL: u8 = 0x1e;
pub const INVALID_SOURCE: u8 = 0x21;
pub const INVALID_TARGET: u8 = 0x22;
pub const NOT_NOW: u8 = 0x2c;
pub const UNIT_REQS_OR_FAIL: u8 = 0x31;
pub const SKILL_REQ_FAIL: u8 = 0x32;
pub const CANNOT_USE_FOR_SELF: u8 = 0x36;
pub const NOT_PREOCCUPIED: u8 = 0x37;
pub const ZONE_BANNED: u8 = 0x3a;
pub const ITEM_SECURED: u8 = 0x3e;
pub const URK_LEVEL: u8 = 0x40;

/// One `SkillCaster` / `SkillCastTarget` block.  The raw words are exactly the eight dwords the
/// evaluators index; helpers name the roles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReqBlock {
    pub raw: [u32; 8],
}

impl ReqBlock {
    pub fn new(kind: u32, id: u32, x: u32, y: u32, z: u32) -> Self {
        let mut raw = [0; 8];
        raw[0] = kind;
        raw[2] = id;
        raw[4] = x;
        raw[5] = y;
        raw[6] = z;
        Self { raw }
    }

    pub fn kind(&self) -> u32 {
        self.raw[0]
    }

    pub fn id(&self) -> u32 {
        self.raw[2]
    }

    pub fn x(&self) -> u32 {
        self.raw[4]
    }

    pub fn y(&self) -> u32 {
        self.raw[5]
    }

    pub fn z(&self) -> u32 {
        self.raw[6]
    }

    pub fn word(&self, index: usize) -> u32 {
        self.raw.get(index).copied().unwrap_or(0)
    }
}

/// One unit as the evaluators read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReqUnit {
    pub id: u32,
    /// `+0x3c26`.
    pub attached: bool,
    /// `+0x3c27`.
    pub rider: bool,
    /// `+0x410`.
    pub kind: i32,
    /// `+0x420`.
    pub owner: u32,
    /// `+0x4ac`.
    pub class_id: i32,
    /// `+0x5e0` (the value `FUN_3989a7b0` returns for the level requirement).
    pub req_level: i32,
    /// `+0x3a7c` / `+0x3a80` (the two health/mana pools the stat requirement reads).
    pub stat_3a7c: i32,
    pub stat_3a80: i32,
    /// `+0x3bcc + 4*index` stat margins base.
    pub stats: [i32; 4],
    /// `+0x3c28` relation bits.
    pub relation: u32,
    /// The unit's buffs (`+0x20/+0x28` rows).
    pub buffs: Vec<ReqBuff>,
}

/// One buff row as an evaluator can inspect it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReqBuff {
    pub id: u32,
    pub tag: u32,
    pub kind: i32,
}

/// One item/object as the evaluators read it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReqItem {
    pub id: u32,
    /// `+0x48` item type/kind.
    pub kind: i32,
    /// `+0x20` category.
    pub category: i32,
    /// `+0x24` equip slot.
    pub slot: i32,
    /// `+0x1c` owner/controller id.
    pub owner: u32,
    /// `+0x74` sub-type.
    pub sub_type: i32,
    /// `+0x94` "secured" flag.
    pub secured: bool,
    /// `+0x84` item id for the caster equality path.
    pub item_id: u32,
}

/// The requirement row itself (the object at `desc+0x1e8` points to).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReqRow {
    /// `impl+4` — the evaluator kind.
    pub kind: i32,
    /// The values at `impl+8` (sub-kind at `[0]` for kind `0x18`).
    pub values: [i32; 8],
    /// `FUN_3936c500` result (owner/target row filter).
    pub applies: bool,
    /// The row's raw id used in messages.
    pub row_id: u32,
}

impl ReqRow {
    pub fn new(kind: i32, values: [i32; 8]) -> Self {
        Self { kind, values, applies: false, row_id: 0 }
    }

    pub fn sub_kind(&self) -> i32 {
        self.values[0]
    }

    fn v(&self, index: usize) -> i32 {
        self.values.get(index).copied().unwrap_or(0)
    }
}

/// The caller-described world for one evaluation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReqCtx {
    /// The local player id (`DAT_39b02294+0x10c+8`), 0 when not in world.
    pub local_id: u32,
    pub local: Option<ReqUnit>,
    /// The resolved source (caster) unit.
    pub source: Option<ReqUnit>,
    /// The resolved target unit.
    pub target: Option<ReqUnit>,
    pub source_block: ReqBlock,
    pub target_block: ReqBlock,
    /// The `param_7` flags word (bit 2 = position check, bit 9 = weapon-controller check,
    /// bit 12 = item/trade check).
    pub flags: u32,
    /// Units addressable by id (including the local player and the target).
    pub units: Vec<ReqUnit>,
    /// Items addressable by `(kind, id)`.
    pub items: Vec<ReqItem>,
    /// The skill desc fields the evaluators read.
    pub desc_target_type: i32,
    pub desc_flag_1a1: bool,
    pub desc_range: i32,
    /// Opaque helper results.  Named keys used by the port:
    /// `weapon_conflict` (0/1), `stealth_ok`, `target_alive`, `protected`, `item_check`,
    /// `slot_check`, `equip_check`, `name_ok`, `dist`, `can_learn`, `inv_free`, `cargo_ok`,
    /// `level_ok`, `buff_ok`, `quest`, `doodad`, `doodad_place`, `time_left`, `relation_ok`,
    /// `slot_a`, `slot_b`, `stat_low`, `stat_high`.
    pub helpers: BTreeMap<String, i32>,
    /// The current engine time in ms.
    pub now_ms: u32,
    pub filetime: u64,
}

impl ReqCtx {
    pub fn helper(&self, name: &str) -> i32 {
        self.helpers.get(name).copied().unwrap_or(0)
    }

    pub fn helper_bool(&self, name: &str) -> bool {
        self.helper(name) != 0
    }

    pub fn unit(&self, id: u32) -> Option<&ReqUnit> {
        if id == 0 {
            return None;
        }
        if let Some(unit) = &self.local
            && unit.id == id
        {
            return Some(unit);
        }
        if let Some(unit) = &self.source
            && unit.id == id
        {
            return Some(unit);
        }
        if let Some(unit) = &self.target
            && unit.id == id
        {
            return Some(unit);
        }
        self.units.iter().find(|unit| unit.id == id)
    }

    pub fn item(&self, kind: i32, id: u32) -> Option<&ReqItem> {
        if id == 0 {
            return None;
        }
        self.items
            .iter()
            .find(|item| item.id == id && (kind <= 0 || item.kind == kind))
    }

    pub fn item_by_id(&self, id: u32) -> Option<&ReqItem> {
        self.items.iter().find(|item| item.id == id || item.item_id == id)
    }

    pub fn local_unit(&self) -> Option<&ReqUnit> {
        self.local.as_ref()
    }

    pub fn source_id(&self) -> u32 {
        match self.source_block.kind() {
            0 | 3 => self.source_block.id(),
            _ => 0,
        }
    }

    pub fn target_unit(&self) -> Option<&ReqUnit> {
        self.target.as_ref()
    }
}

/// `FUN_393a0510`: evaluate a requirement vector, first non-zero result wins.
pub fn evaluate_requirements(rows: &[ReqRow], ctx: &ReqCtx) -> ReqResult {
    for row in rows {
        if !row.applies {
            continue;
        }
        let result = evaluate_row(row, ctx);
        if !result.is_success() {
            return result;
        }
    }
    // The tail pass over the source's buff/skill controllers (kind 0x18 sub 0x1b/0x4f).
    if ctx.flags & (1 << 9) != 0 && ctx.helper_bool("weapon_conflict") {
        return ReqResult::fail(0x2e);
    }
    ReqResult::SUCCESS
}

/// One row through the same switch as `FUN_393a0510`.
pub fn evaluate_row(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    match row.kind {
        0 => require_d3f0(row, ctx),
        6 => require_f0d0(row, ctx),
        9 => ReqResult::fail(INVALID_SKILL),
        0xc => require_d4c0(row, ctx),
        0xd => require_ea60(row, ctx),
        0x18 => require_nested(row, ctx),
        0x19 => require_d670(row, ctx),
        0x1a => require_f030(row, ctx),
        0x1b => require_f800(row, ctx),
        0x1c => require_d280(row, ctx),
        0x1d => require_e9c0(row, ctx),
        0x22 => require_e930(row, ctx),
        0x26 => require_dea0(row, ctx),
        _ => ReqResult::SUCCESS,
    }
}

fn require_nested(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    match row.sub_kind() {
        0xf => require_ffa0(row, ctx),
        0x19 => require_d670(row, ctx),
        0x1b | 0x4f => require_d8b0(row, ctx),
        0x1e => require_d7b0(row, ctx),
        0x31 => require_d700(row, ctx),
        0x38 => require_d0d0(row, ctx),
        0x39 => require_d150(row, ctx),
        0x40 => ReqResult::fail(INVALID_SKILL),
        0x59 => require_d8f0(row, ctx),
        0x5c => require_a0130(row, ctx),
        0x5f => require_ebb0(row, ctx),
        0x60 => require_d960(row, ctx),
        0x61 => require_ef50(row, ctx),
        0x62 => require_ee20(row, ctx),
        0x67 => require_d9d0(row, ctx),
        0x6a => require_ecf0(row, ctx),
        0x6b => require_f990(row, ctx),
        0x6c => require_dab0(row, ctx),
        0x6e => require_de20(row, ctx),
        0x6f => require_db60(row, ctx),
        0x70 => require_dc20(row, ctx),
        0x71 => require_dd10(row, ctx),
        0x72 => require_ff50(row, ctx),
        _ => ReqResult::SUCCESS,
    }
}

fn require_d0d0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(0) != 0 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let Some(unit) = ctx.unit(row.v(2) as u32) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if unit.kind == 5 && ctx.helper_bool("item_available") {
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(INVALID_TARGET)
    }
}

fn require_d150(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(0) != 2 || (row.v(2) == 0 && row.v(3) == 0) {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.target_block.kind() != 3
        || (ctx.target_block.x() == 0 && ctx.target_block.y() == 0)
    {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.local_unit().is_none() {
        return ReqResult::fail(1);
    }
    let first = ctx.helper("slot_a");
    if first == 0 || first != row.v(4) {
        return ReqResult::fail(INVALID_SOURCE);
    }
    let second = ctx.helper("slot_b");
    if second == 0 || second != ctx.target_block.z() as i32 {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.helper_bool("relation_ok") {
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(INVALID_TARGET)
    }
}

fn require_d280(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(0) != 2 || (row.v(2) == 0 && row.v(3) == 0) {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.target_block.kind() != 3
        || (ctx.target_block.x() == 0 && ctx.target_block.y() == 0)
    {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.local_unit().is_none() {
        return ReqResult::fail(1);
    }
    let first = ctx.helper("slot_a");
    if first == 0 || first != row.v(4) {
        return ReqResult::fail(INVALID_SOURCE);
    }
    let item_id = ctx.target_block.z();
    let second = ctx.helper("slot_b");
    if second == 0 || second != item_id as i32 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let Some(item) = ctx.item_by_id(item_id) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if item.kind != 8 {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.helper_bool("item_check") {
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(INVALID_TARGET)
    }
}

fn require_d3f0(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.flags & (1 << 9) == 0 {
        return ReqResult::SUCCESS;
    }
    if !ctx.helper_bool("buff_desc") {
        return ReqResult::fail(INVALID_SKILL);
    }
    if ctx.helper_bool("weapon_conflict") {
        ReqResult::fail(0x2e)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_d4c0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    let source = ctx.source.clone();
    let Some(source) = source else {
        return ReqResult::fail(INVALID_SOURCE);
    };
    if source.attached {
        return ReqResult::fail(INVALID_TARGET);
    }
    let (base, multiplier) = match row.v(0) {
        1 => (ctx.helper("stat_3a80") / 100, ctx.helper("attr_6")),
        2 => (
            (ctx.helper("stat_3a7c") + 99) / 100,
            ctx.helper("attr_7"),
        ),
        _ => return ReqResult::fail(INVALID_SKILL),
    };
    let value = match row.v(1) {
        1 => row.v(2),
        2 => row.v(2) * multiplier / 100,
        3 => row.v(2) * base / 100,
        _ => 0,
    };
    if value <= base {
        ReqResult::SUCCESS
    } else if row.v(0) == 1 {
        ReqResult::fail(0x0b)
    } else {
        ReqResult::fail(0x0a)
    }
}

fn require_d670(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    let Some(source) = &ctx.source else {
        return ReqResult::fail(INVALID_SKILL);
    };
    if source.id != ctx.local_id {
        return ReqResult::fail(INVALID_SKILL);
    }
    if row.v(1) == 0 && !ctx.helper_bool("target_alive") {
        return ReqResult::fail(INVALID_SKILL);
    }
    ReqResult::SUCCESS
}

fn require_d700(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.target_block.kind() != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let item_id = ctx.target_block.z();
    let Some(item) = ctx.item_by_id(item_id) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if !item.secured {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.local_unit().is_none() {
        return ReqResult::fail(1);
    }
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    ReqResult::SUCCESS
}

fn require_d7b0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    let Some(local) = ctx.local_unit() else {
        return ReqResult::fail(INVALID_SOURCE);
    };
    if row.v(2) != 0 {
        return ReqResult::SUCCESS;
    }
    let threshold = if local.class_id == 4 { 0x1e } else { 0x14 };
    if threshold <= ctx.helper("dist") {
        return ReqResult::fail(NOT_NOW);
    }
    if ctx.helper_bool("name_ok") {
        ReqResult::fail_text(1, 0x150)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_d8b0(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.flags & (1 << 9) != 0 && ctx.helper_bool("weapon_conflict") {
        ReqResult::fail(0x2e)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_d8f0(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.source.is_none() {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.local.is_none() {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.helper_bool("stealth_source") {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.helper_bool("stealth_target") {
        return ReqResult::fail(INVALID_TARGET);
    }
    ReqResult::SUCCESS
}

fn require_d960(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    let Some(local) = ctx.local_unit() else {
        return ReqResult::fail(INVALID_SOURCE);
    };
    if local.req_level < row.v(1) {
        ReqResult::fail_text(INVALID_SOURCE, 0x29e)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_d9d0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(0) != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let Some(item) = ctx.item(row.v(5), row.v(4) as u32) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if item.kind == 1 || item.kind == 2 {
        if item.owner == 0 {
            return ReqResult::fail(INVALID_TARGET);
        }
    } else if item.kind == 0x16 && (item.sub_type != 9 || item.owner == 0) {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    ReqResult::SUCCESS
}

fn require_dab0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(0) != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let Some(item) = ctx.item(row.v(5), row.v(4) as u32) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if ctx.helper_bool("slot_check") {
        if ctx.helper_bool("protected") {
            return ReqResult::fail_text(ITEM_SECURED, 0x298);
        }
        if item.category == 0 {
            return ReqResult::fail(INVALID_TARGET);
        }
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(INVALID_TARGET)
    }
}

fn require_db60(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(0) != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let Some(item) = ctx.item(row.v(5), row.v(4) as u32) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    if item.kind != row.v(1) {
        return ReqResult::fail(INVALID_TARGET);
    }
    if !ctx.helper_bool("equip_check") {
        return ReqResult::SUCCESS;
    }
    ReqResult::fail(INVALID_TARGET)
}

fn require_dc20(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(0) != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let Some(item) = ctx.item(row.v(5), row.v(4) as u32) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    if item.kind == 0 {
        return ReqResult::fail(INVALID_TARGET);
    }
    if !ctx.helper_bool("slot_check") {
        return ReqResult::fail(INVALID_TARGET);
    }
    if !ctx.helper_bool("equip_check") {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.helper_bool("usable") {
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(INVALID_TARGET)
    }
}

fn require_dd10(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    let index = row.v(1);
    if !(1..=5).contains(&index) {
        return ReqResult::fail(INVALID_SKILL);
    }
    let low = ctx.helper("stat_min");
    let high = ctx.helper("stat_max");
    if low == 0 && high == 0 {
        return ReqResult::fail(INVALID_SKILL);
    }
    let Some(unit) = ctx.unit(ctx.source_id()) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if unit.attached || unit.kind != 0 {
        return ReqResult::fail(INVALID_SKILL);
    }
    let value = ctx.helper("stat_value") + row.v(2);
    if value >= low && value <= high {
        ReqResult::SUCCESS
    } else if value > high {
        ReqResult::fail_value(0x3d, index as u32)
    } else {
        ReqResult::fail_value(0x3c, index as u32)
    }
}

fn require_de20(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if row.v(1) < 1 {
        return ReqResult::fail(INVALID_SKILL);
    }
    if !ctx.helper_bool("time_ready") {
        return ReqResult::fail(0x23);
    }
    let left = ctx.helper("time_left");
    if left != 0 {
        ReqResult::fail_text(NOT_NOW, left as u16)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_dea0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if !ctx.helper_bool("account_flag") {
        return ReqResult::SUCCESS;
    }
    if ctx.source.as_ref().map(|unit| unit.id) != Some(ctx.local_id) {
        return ReqResult::fail(INVALID_SKILL);
    }
    if ctx.helper("account_result") == row.v(3) {
        if row.v(4) == 1 {
            if row.v(3) == 0 {
                return ReqResult::fail_text(0x3f, 0x340);
            }
            return ReqResult::fail_text(0x3f, 0x33f);
        }
        ReqResult::fail(0x3f)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_e930(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if !ctx.helper_bool("impl") {
        return ReqResult::SUCCESS;
    }
    if !ctx.helper_bool("doodad") {
        return ReqResult::fail(INVALID_SKILL);
    }
    if ctx.helper_bool("obstacle") {
        return ReqResult::fail(0x0c);
    }
    ReqResult::SUCCESS
}

fn require_e9c0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if !ctx.helper_bool("target_flag") || row.v(0) != 2 {
        return ReqResult::SUCCESS;
    }
    if row.v(2) as u32 == ctx.local_id {
        return ReqResult::SUCCESS;
    }
    if !ctx.helper_bool("buff_target") {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    ReqResult::SUCCESS
}

fn require_ea60(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if !ctx.helper_bool("impl") {
        return ReqResult::fail(INVALID_SKILL);
    }
    if row.v(1) == 0x5c {
        if ctx.target_block.kind() != 2 || row.v(0) != 2 {
            return ReqResult::fail(INVALID_SKILL);
        }
        if ctx.local_id != row.v(2) as u32 {
            return ReqResult::fail(INVALID_SKILL);
        }
        if !ctx.helper_bool("item_check") {
            return ReqResult::fail(INVALID_SKILL);
        }
        if ctx.helper_bool("protected") {
            return ReqResult::fail_text(ITEM_SECURED, 0x298);
        }
    }
    ReqResult::SUCCESS
}

fn require_ebb0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.target_block.kind() != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.local_unit().is_none() {
        return ReqResult::fail(1);
    }
    let item_id = ctx.target_block.z();
    let Some(item) = ctx.item_by_id(item_id) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if item.owner != ctx.local_id {
        return ReqResult::fail(INVALID_TARGET);
    }
    let expected = row.v(1) as u32;
    if expected == 0 {
        return ReqResult::SUCCESS;
    }
    if item.id == expected {
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(INVALID_TARGET)
    }
}

fn require_ecf0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    let item = ctx.item(row.v(5), row.v(4) as u32);
    if row.v(1) != 0 && item.is_none() {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.target_block.kind() != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.local_unit().is_none() {
        return ReqResult::fail(1);
    }
    let Some(item) = item else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if item.id == 0 {
        return ReqResult::fail(INVALID_TARGET);
    }
    if item.owner != ctx.local_id {
        return ReqResult::fail(INVALID_TARGET);
    }
    if row.v(1) != 0 && !ctx.helper_bool("item_check") {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    ReqResult::SUCCESS
}

fn require_ee20(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.target_block.kind() != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let Some(local) = ctx.local_unit() else {
        return ReqResult::fail(1);
    };
    let item_id = ctx.target_block.z();
    let Some(item) = ctx.item_by_id(item_id) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if item.owner != local.id {
        return ReqResult::fail(INVALID_TARGET);
    }
    if !ctx.helper_bool("item_usable") {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    ReqResult::SUCCESS
}

fn require_ef50(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.local_id == 0 {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.helper_bool("weapon_conflict") {
        return ReqResult::fail(0x2e);
    }
    if ctx.helper("inventory_count") < ctx.helper("required_count") {
        return ReqResult::fail(0x20);
    }
    if ctx.helper_bool("slot_busy") {
        ReqResult::fail(INVALID_SOURCE)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_f030(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.target_block.kind() != 3 || row.v(0) != 3 {
        return ReqResult::fail(INVALID_SKILL);
    }
    if ctx.local_id != row.v(2) as u32 {
        return ReqResult::fail(INVALID_SKILL);
    }
    let Some(item) = ctx.item(row.v(5), row.v(4) as u32) else {
        return ReqResult::fail(INVALID_SKILL);
    };
    if item.owner != 0 {
        if ctx.helper_bool("protected") {
            return ReqResult::fail_text(ITEM_SECURED, 0x298);
        }
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(INVALID_SKILL)
    }
}

fn require_f0d0(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if (row.v(0) & 4) == 0 || ctx.helper("desc_1b0") == 0 {
        return ReqResult::SUCCESS;
    }
    if ctx.helper_bool("position_ok") && ctx.helper_bool("obstacle") {
        ReqResult::fail(0x0c)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_f800(row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.local_id != row.v(0) as u32 {
        return ReqResult::fail(INVALID_SKILL);
    }
    if ctx.helper("item_id") == 0 {
        return ReqResult::fail(INVALID_SKILL);
    }
    if ctx.helper_bool("slot_busy") {
        return ReqResult::fail(0x23);
    }
    if ctx.helper_bool("item_check") {
        ReqResult::SUCCESS
    } else {
        ReqResult::fail(0x24)
    }
}

fn require_f990(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.local_id == 0 {
        return ReqResult::fail(INVALID_SOURCE);
    }
    if ctx.helper("item_34") != 0 && ctx.helper("item_34_count") < ctx.helper("item_35") {
        return ReqResult::fail_text(1, 0x1b);
    }
    match ctx.helper("broken") {
        1 => ReqResult::fail_text(1, 0xb5),
        2 => ReqResult::fail_text(1, 0x2e8),
        _ => ReqResult::SUCCESS,
    }
}

fn require_ff50(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.helper("quest_context") != ctx.helper("required_context") {
        ReqResult::fail(INVALID_TARGET)
    } else {
        ReqResult::SUCCESS
    }
}

fn require_ffa0(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if !ctx.helper_bool("impl") {
        return ReqResult::SUCCESS;
    }
    if !ctx.helper_bool("item_object") {
        return ReqResult::fail(INVALID_SKILL);
    }
    if ctx.helper_bool("item_check") {
        return ReqResult::SUCCESS;
    }
    if !ctx.helper_bool("equip_check") {
        return ReqResult::SUCCESS;
    }
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(NOT_PREOCCUPIED, 0x298);
    }
    ReqResult::SUCCESS
}

fn require_a0130(_row: &ReqRow, ctx: &ReqCtx) -> ReqResult {
    if ctx.source_block.kind() != 2 || ctx.target_block.kind() != 3 {
        return ReqResult::fail(INVALID_TARGET);
    }
    let source_item = ctx.helper("src_item");
    let target_item = ctx.helper("dst_item");
    if source_item == ctx.helper("desc_item") {
        if target_item != ctx.helper("desc_target_item") {
            return ReqResult::fail(INVALID_TARGET);
        }
    } else if target_item == ctx.helper("desc_target_item") {
        return ReqResult::fail(INVALID_TARGET);
    }
    let item_id = ctx.target_block.z();
    let Some(item) = ctx.item_by_id(item_id) else {
        return ReqResult::fail(INVALID_TARGET);
    };
    if !ctx.helper_bool("item_owned") {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.helper("item_level") < ctx.helper("req_level") {
        return ReqResult::fail_text(INVALID_TARGET, 0x206);
    }
    if ctx.helper("item_grade") != -1 && ctx.helper("item_grade") != item.kind {
        return ReqResult::fail(INVALID_TARGET);
    }
    if ctx.local_unit().is_none() {
        return ReqResult::fail(1);
    }
    if ctx.helper_bool("protected") {
        return ReqResult::fail_text(ITEM_SECURED, 0x298);
    }
    if ctx.helper_bool("grade_mismatch") {
        return ReqResult::fail_text(INVALID_TARGET, 0x299);
    }
    if ctx.flags & (1 << 12) != 0 && ctx.helper_bool("trade_check") {
        return ReqResult::fail(0x26);
    }
    ReqResult::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_non_zero_result_wins() {
        let ctx = ReqCtx { local: Some(ReqUnit { id: 7, ..ReqUnit::default() }), ..ReqCtx::default() };
        let rows = [
            ReqRow { kind: 0, values: [0; 8], applies: false, row_id: 0 },
            ReqRow { kind: 9, values: [0; 8], applies: true, row_id: 1 },
            ReqRow { kind: 0, values: [0; 8], applies: true, row_id: 2 },
        ];
        assert_eq!(evaluate_requirements(&rows, &ctx).result, INVALID_SKILL);
    }

    #[test]
    fn empty_requirement_vector_success() {
        assert!(evaluate_requirements(&[], &ReqCtx::default()).is_success());
    }
}
