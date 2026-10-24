//! The tooltip info tables the client's tooltip scripts read (`baselib/tooltip/tooltip.lua`): `HUD.item_tip`,
//! `HUD.skill_tip`, `HUD.buff_text`. The shapes are the captured ones of `aa-ui/examples/fake/fake_bag.lua` /
//! `fake_ingame.lua` (`reference/ingame/tooltip_{item,skill,buff}`), filled from the game database. They
//! are built when a slot asks for its content or a tooltip shows, not every frame.
//!
//! What the numbers are: [`tipcalc`](super::tipcalc) works out the placeholders of skill and buff descriptions
//! over the player's attack numbers ([`TipState::attrs`], kept current by [`TipState::refresh`]);
//! [`tipitem`](super::tipitem) the stats of weapons and armor (attack, defence, stat bonuses, durability).

use aa_net::model::Equipment;
use mlua::{Lua, Table};

use super::data::GameData;
use super::tipcalc::Attrs;
use super::tipitem::{slot_type_name, Class};

pub(super) mod extra;
pub use extra::ItemExtra;

/// Native39570b90 ItemImplEnum names. Crafting compares these classes when inheriting grades.
pub(super) fn item_impl(id: i64) -> &'static str {
    const NAMES: [&str; 30] = [
        "misc",
        "weapon",
        "armor",
        "body",
        "bag",
        "housing",
        "housing_decoration",
        "tool",
        "summon_slave",
        "spawn_doodad",
        "accept_quest",
        "summon_mate",
        "recipe",
        "crafting",
        "portal",
        "enchanting_gem",
        "report_crime",
        "logic_doodad",
        "has_ucc",
        "open_emblem_ui",
        "shipyard",
        "socket",
        "backpack",
        "open_paper",
        "accessory",
        "treasure",
        "music_sheet",
        "dyeing",
        "slave_equipment",
        "grade_enchanting_support",
    ];
    usize::try_from(id).ok().and_then(|id| NAMES.get(id)).copied().unwrap_or("invalid item_impl")
}

/// X2Item's argument/returned itemGrade is a raw native grade ID. Internal bag snapshots
/// and item_tip use display order; keep that conversion at this public lookup boundary.
pub(super) fn install_native_lookup(lua: &Lua, bridge: &super::api::Shared) -> mlua::Result<()> {
    let item: Table = lua.globals().get("X2Item")?;
    let prior: mlua::Function = item.get("GetItemInfoByType")?;
    let state = bridge.clone();
    item.set(
        "GetItemInfoByType",
        lua.create_function(
            move |lua, (owner, id, grade, flags): (mlua::Value, u32, Option<u8>, mlua::Variadic<mlua::Value>)| {
                let data = state.borrow();
                if let (Some(game), Some(grade)) = (data.game.get(), grade) {
                    let result = item_tip(
                        lua,
                        game,
                        &data.tips,
                        id,
                        Some(i64::from(super::store::ui_grade(grade))),
                        None,
                        Instance::default(),
                    )?;
                    if let Some(info) = &result {
                        info.set("itemGrade", grade)?;
                        info.set("stack", mlua::Value::Nil)?;
                        //394224c6: impl15 always includes its modifier table;
                        // impl21 requires IIK_SOCKET_MODIFIER (default flags=-1).
                        //3947f318/39058d10: missing flags=-1; an explicitly supplied
                        // wrong type (including nil) leaves initialized output zero.
                        let flags = native_info_flags(flags.first());
                        if game.items.get(&id).is_some_and(|i| i.impl_id == 21) && flags & 1024 == 0 {
                            info.raw_set("modifier", mlua::Value::Nil)?;
                        }
                    }
                    Ok(result)
                } else {
                    drop(data);
                    prior.call::<Option<Table>>((owner, id, grade, flags))
                }
            },
        )?,
    )
}

/// Native omitted flags are -1; present nonnumeric values leave the zero initializer.
pub(super) fn native_info_flags(value: Option<&mlua::Value>) -> i32 {
    let value = match value {
        None => return -1,
        Some(mlua::Value::Integer(n)) => *n as f32,
        Some(mlua::Value::Number(n)) => *n as f32,
        Some(_) => 0.0,
    };
    if value.is_finite() && (-2147483648.0..2147483648.0).contains(&value) { value as i32 } else { i32::MIN }
}

/// `skills.ability_id` -> the key of the tooltip's ability colour and name (`locale.common.abilityCategory`).
const ABILITIES: [&str; 11] = ["general", "fight", "illusion", "adamant", "will", "death", "wild", "magic", "vocation", "romance", "love"];

/// What the tooltips need of the player: its attack numbers and what it wears.
#[derive(Default)]
pub struct TipState {
    /// The inputs `attrs` were worked out from.
    key: Option<(u8, Equipment, Vec<(u32, i16)>, [i32; 11])>,
    pub attrs: Attrs,
    /// The Character Info numbers (`UnitInfo` / `UnitModifierInfo` of the player), and how often they changed.
    pub sheet: super::tipcalc::PlayerSheet,
    pub sheet_gen: u32,
    /// The template ids of the worn items (the set bonuses count them).
    worn: Vec<u32>,
}

impl TipState {
    /// The inputs the tooltips need that do not come with the gear: the damage breakdown option and the
    /// recall point (zone and point name) the character is bound to.
    pub fn set_context(&mut self, details: bool, return_point: Option<(String, String)>) {
        self.attrs.details = details;
        self.attrs.return_point = return_point;
    }

    pub fn lung_capacity_ms(&self) -> u32 {
        self.key.as_ref().map_or(60_000, |_| self.attrs.lung_capacity_ms)
    }

    /// Bring the numbers up to date with the player's level, gear, buffs (id, ability level) and ability
    /// experience; nothing is worked out unless one of them changed.
    pub fn refresh(&mut self, game: &GameData, level: u8, equipment: &Equipment, buffs: Vec<(u32, i16)>, ability_exp: &[i32; 11]) {
        let abilities = ability_exp.map(|total| if total > 0 { i32::from(aa_net::model::exp::level_for_total(i64::from(total))) } else { 0 });
        if self.key.as_ref().is_some_and(|k| k.0 == level && k.1 == *equipment && k.2 == buffs && k.3 == abilities) {
            return;
        }
        let (details, return_point) = (self.attrs.details, self.attrs.return_point.take());
        self.attrs = game.tips.attrs(level, equipment, &buffs, &abilities);
        let sheet = game.tips.player_sheet(level, equipment, &buffs);
        if sheet != self.sheet {
            self.sheet = sheet;
            self.sheet_gen = self.sheet_gen.wrapping_add(1);
        }
        self.attrs.details = details;
        self.attrs.return_point = return_point;
        self.worn = equipment.slots.iter().enumerate().filter(|(i, _)| !aa_net::model::slot::is_body_part(*i)).filter_map(|(_, it)| it.as_ref().map(|it| it.template_id)).collect();
        self.key = Some((level, equipment.clone(), buffs, abilities));
    }
}

/// What the tooltip knows of one item beyond its template: the slot table the scripts built the item from
/// (`HUD.bag[i]`): `dur` (durability), `tp` / `tm` (temper values), `fl` (item flags), the creation/lifespan
/// timestamp, the look item and socketed gems. Nothing for an item that is not in the bag (a shop's, a loot
/// window's).
#[derive(Clone, Debug, Default)]
pub struct Instance {
    pub durability: Option<u8>,
    pub temper: (u16, u16),
    pub flags: Option<u8>,
    /// A pending unlock's deadline (Unix seconds, 0 = none).
    pub unsecure_time: i64,
    /// Creation time (Unix seconds) and total lifespan (minutes): `FUN_397d0400` / `FUN_397cd400`.
    pub create_time: i64,
    pub lifespan_mins: i64,
    /// The appearance item when it differs from the template (`lookType`/`lookChanged`).
    pub look_type: u32,
    pub gems: [u32; 7],
    /// The crafter's name, when the unit is known (`FUN_39018560`).
    pub crafter: Option<String>,
}

impl Instance {
    /// Everything a tooltip reads from a server item instance (`FUN_39423560` / `FUN_39421e20`).
    pub fn from_item(it: &aa_net::model::Item) -> Instance {
        let (durability, temper, gems, look_type) = match &it.detail {
            aa_net::model::ItemDetail::Equip(d) => (Some(d.durability), (d.temper_physical, d.temper_magical), d.gem_ids, d.image_item_template_id),
            _ => (None, (0, 0), [0; 7], 0),
        };
        Instance {
            durability,
            temper,
            flags: Some(it.flags),
            unsecure_time: it.unsecure_time,
            create_time: it.create_time,
            lifespan_mins: i64::from(it.lifespan_mins),
            look_type,
            gems,
            crafter: None,
        }
    }

    pub fn of(table: Option<&Table>) -> Instance {
        let Some(t) = table else { return Instance::default() };
        let get = |k: &str| t.raw_get::<Option<i64>>(k).ok().flatten();
        let gems = t
            .raw_get::<Option<Table>>("gm")
            .ok()
            .flatten()
            .map(|t| {
                let mut gems = [0u32; 7];
                for (i, gem) in gems.iter_mut().enumerate() {
                    *gem = t.raw_get::<Option<i64>>(i as i64 + 1).ok().flatten().unwrap_or(0).clamp(0, i64::from(u32::MAX)) as u32;
                }
                gems
            })
            .unwrap_or([0; 7]);
        Instance {
            durability: get("dur").filter(|d| *d >= 0).map(|d| d.min(255) as u8),
            temper: (get("tp").unwrap_or(0).clamp(0, 65535) as u16, get("tm").unwrap_or(0).clamp(0, 65535) as u16),
            flags: get("fl").map(|f| f as u8),
            unsecure_time: get("un").unwrap_or(0),
            create_time: get("ct").unwrap_or(0),
            lifespan_mins: get("ls").unwrap_or(0),
            look_type: get("look").unwrap_or(0).clamp(0, i64::from(u32::MAX)) as u32,
            gems,
            crafter: t.raw_get::<Option<String>>("cr").ok().flatten().filter(|c| !c.is_empty()),
        }
    }
}

/// `item_grades.id` of a grade the HUD numbers by `grade_order` (the order swaps Basic and Crude).
fn grade_id(order: i64) -> i32 {
    match order {
        1 => 0,
        0 => 1,
        g => g as i32,
    }
}

/// The info of an item as `X2Bag:GetBagItemInfo` returns it: `grade` is the number the server and the icons
/// use for display (0 crude, 1 basic, 2 grand ...); absent grade uses the item's fixed grade or basic.
/// Without the instance's gems, enchant, dye and look ([`item_tip_with`]).
pub fn item_tip(lua: &Lua, game: &GameData, state: &TipState, id: u32, grade: Option<i64>, count: Option<i64>, instance: Instance) -> mlua::Result<Option<Table>> {
    item_tip_at(lua, game, state, id, grade, count, instance, aa_net::secure::unix_now())
}

/// The tooltip of an item slot table (`HUD.bag[i]`, the worn items): the instance's durability, tempers, flags, gems,
/// enchant, dye and look come from the table.
pub fn item_tip_of_slot(lua: &Lua, game: &GameData, state: &TipState, id: u32, grade: Option<i64>, count: Option<i64>, slot: Option<&Table>) -> mlua::Result<Option<Table>> {
    item_tip_with(lua, game, state, id, grade, count, Instance::of(slot), &ItemExtra::of(slot))
}

/// `item_tip` with what the item instance carries besides its template.
#[allow(clippy::too_many_arguments)]
pub fn item_tip_with(lua: &Lua, game: &GameData, state: &TipState, id: u32, grade: Option<i64>, count: Option<i64>, instance: Instance, extra: &ItemExtra) -> mlua::Result<Option<Table>> {
    item_tip_at_with(lua, game, state, id, grade, count, instance, extra, aa_net::secure::unix_now())
}

#[allow(clippy::too_many_arguments)]
fn item_tip_at(
    lua: &Lua,
    game: &GameData,
    state: &TipState,
    id: u32,
    grade: Option<i64>,
    count: Option<i64>,
    instance: Instance,
    now: i64,
) -> mlua::Result<Option<Table>> {
    item_tip_at_with(lua, game, state, id, grade, count, instance, &ItemExtra::default(), now)
}

#[allow(clippy::too_many_arguments)]
fn item_tip_at_with(
    lua: &Lua,
    game: &GameData,
    state: &TipState,
    id: u32,
    grade: Option<i64>,
    count: Option<i64>,
    instance: Instance,
    extra: &ItemExtra,
    now: i64,
) -> mlua::Result<Option<Table>> {
    let Some(info) = game.items.get(&id) else { return Ok(None) };
    let grade = grade.unwrap_or_else(|| i64::from(super::store::ui_grade(info.fixed_grade.max(0) as u8)));
    let t = lua.create_table()?;
    t.raw_set("itemType", id)?;
    t.raw_set("isPetOnly", info.pet_only)?;
    // FUN_397cd490: the appearance item, and the ORANGE "%s%s [%s]" look-changed title when it is not
    // the template (tooltip.alb [1439]-[1456]).
    let look_type = if instance.look_type != 0 { instance.look_type } else { id };
    t.raw_set("lookType", look_type)?;
    if look_type != id {
        t.raw_set("lookChanged", true)?;
    }
    t.raw_set("itemGrade", grade)?;
    if let Some((name, color)) = game.grades.get(&grade) {
        t.raw_set("grade", name.as_str())?;
        t.raw_set("gradeColor", color.as_str())?;
    }
    t.raw_set("name", info.named.name.as_str())?;
    t.raw_set("category", info.category.as_str())?;
    t.raw_set("item_impl", item_impl(info.impl_id))?;
    match instance.flags {
        Some(flags) if flags & 1 != 0 => t.raw_set("soul_bound", 1)?,
        Some(_) => {}
        None => {
            if let Some(bind) = game.tips.items.soul_bind(id) {
                t.raw_set("soul_bind", bind)?;
            }
        }
    }
    if let Some(flags) = instance.flags {
        // The padlock overlay and the SKYBLUE line read these (item_icon_button.alb
        // [130]-[146], tooltip.alb [1466]-[1473]); flag 0x04 is the item lock.
        let security = aa_net::secure::state(flags, instance.unsecure_time, now);
        t.raw_set("securityState", i64::from(security.code()))?;
        t.raw_set("locked", flags & aa_net::secure::FLAG_SECURE != 0)?;
        if let Some(remain) = aa_net::secure::remaining(instance.unsecure_time, now) {
            let date = lua.create_table()?;
            for (key, value) in [
                ("year", remain.year),
                ("month", remain.month),
                ("day", remain.day),
                ("hour", remain.hour),
                ("minute", remain.minute),
                ("second", remain.second),
            ] {
                date.raw_set(key, value)?;
            }
            t.raw_set("securityRemainDate", date)?;
        }
    }
    // FUN_39423560 / FUN_39421e20: lifespan and expiry are seconds; `lifeSpan` is the period shown
    // while the item is still packed, `expireTime` the remaining lifetime (tooltip.alb [1532]-[1672]).
    let period = if instance.lifespan_mins > 0 {
        instance.lifespan_mins * 60
    } else if info.abs_lifetime > 0 {
        info.abs_lifetime * 60
    } else if info.online_lifetime > 0 {
        info.online_lifetime * 60
    } else {
        game.tips.items.extra.life_span(id).map_or(0, |span| span as i64)
    };
    if period > 0 {
        t.raw_set("lifeSpan", period)?;
        let remaining = if instance.create_time > 0 {
            (instance.create_time + period - now).max(0)
        } else {
            period
        };
        t.raw_set("expireTime", remaining)?;
    }
    if matches!(info.category_id, 4 | 5) && instance.flags.is_some_and(|f| f & 0x10 == 0) {
        t.raw_set("needsUnpack", true)?;
    }
    // FUN_395aca00 / FUN_397cc290: socketInfo.socketItem carries one entry per socket (0 = empty),
    // maxSocket its count. The tooltip's itemSocketInfos section reads it (tooltip.alb [1910]-[1967]).
    let socket_rule = game
        .tips
        .items
        .extra
        .max_sockets(game.tips.items.slot_type(id), grade_id(grade).clamp(0, 255) as u8, game.tips.items.item_level(id).unwrap_or(0)) as usize;
    let instance_gems = instance.gems.iter().any(|g| *g != 0);
    let gems = if instance_gems { instance.gems } else { extra.gems };
    let max_socket = (info.sockets as usize).max(gems.iter().rposition(|g| *g != 0).map_or(0, |i| i + 1)).max(socket_rule).min(7);
    if max_socket > 0 {
        let socket = lua.create_table()?;
        socket.raw_set("maxSocket", max_socket as i64)?;
        if instance_gems || extra.equip {
            socket.raw_set("socketItem", lua.create_sequence_from(gems[..max_socket].iter().map(|g| i64::from(*g)))?)?;
        }
        t.raw_set("socketInfo", socket.clone())?;
        t.raw_set("itemSocketInfos", socket)?;
    }
    match info.gender {
        1 => {
            t.raw_set("gender", "male")?;
            t.raw_set("onlyMale", true)?;
        }
        2 => {
            t.raw_set("gender", "female")?;
            t.raw_set("onlyFemale", true)?;
        }
        _ => {}
    }
    if info.grade_enchantable {
        t.raw_set("gradeEnchantable", true)?;
    }
    if let Some(crafter) = instance.crafter.as_deref() {
        t.raw_set("crafter", crafter)?;
    }
    if info.use_skill != 0 {
        t.raw_set("skillType", info.use_skill)?;
    }
    t.raw_set("description", info.desc.as_str())?;
    t.raw_set("stack", count.unwrap_or(1))?;
    t.raw_set("isStackable", info.max_stack > 1)?;
    t.raw_set("level_requirement", info.level)?;
    t.raw_set("level_limit", info.level_limit)?;
    t.raw_set("price", info.price)?;
    t.raw_set("refund", game.stores.refund(info.refund, grade_id(grade).clamp(0,255) as u8))?;
    t.raw_set("sellable", info.sellable)?;
    equipment_stats(lua, game, state, &t, id, grade_id(grade), instance.clone(), extra)?;
    if matches!(info.impl_id, 15 | 21) {
        if let Some(modifiers) = game.enchant.modifier_table(lua, id)? {
            t.raw_set("modifier", modifiers)?;
        }
    }
    template_parts(lua, game, &t, id, grade_id(grade), info.impl_id, instance, extra)?;
    super::user_music::fill_tip(lua, game, &t, id, extra)?;
    Ok(Some(t))
}

/// The keys the native builder (`39421e20`, `39423560`) adds to every item: regrade, gender, lifespan, procs, dye colour,
/// gem requirements, and for an equipment the sockets, the enchant gem, the dye and the look.
#[allow(clippy::too_many_arguments)]
fn template_parts(lua: &Lua, game: &GameData, t: &Table, id: u32, raw_grade: i32, impl_id: i64, instance: Instance, extra: &ItemExtra) -> mlua::Result<()> {
    let items = &game.tips.items;
    let tables = &items.extra;
    t.raw_set("useAsSkin", tables.use_as_skin(id, instance.flags, if extra.equip { 1 } else { extra.detail_kind }))?;
    t.raw_set("gradeEnchantable", tables.grade_enchantable(id, raw_grade))?;
    t.raw_set("gender", tables.gender(id))?;
    let procs = lua.create_table()?;
    for (i, text) in tables.procs(id).iter().enumerate() {
        let p = lua.create_table()?;
        p.raw_set("desc", text.as_str())?;
        procs.raw_set(i, p)?;
    }
    if !tables.procs(id).is_empty() {
        t.raw_set("procs", procs)?;
    }
    // a dye shows its colour (`dyeingRed` .. `Blue`, gamma 1 / 2.2)
    if impl_id == 27 {
        if let Some([r, g, b]) = tables.dye_color(id) {
            t.raw_set("dyeingRed", r)?;
            t.raw_set("dyeingGreen", g)?;
            t.raw_set("dyeingBlue", b)?;
        }
    }
    if matches!(impl_id, 15 | 21) {
        if let Some(need) = tables.requirements(lua, impl_id, id, |g| game.grades.get(&i64::from(super::store::ui_grade(g))).map(|(name, _)| name.clone()))? {
            t.raw_set("requiredCondition", need)?;
        }
    }
    // an item that binds when it is unpacked and has not been
    if matches!(items.bind_id(id), Some(4 | 5)) && instance.flags.is_none_or(|f| f & 0x10 == 0) {
        t.raw_set("needsUnpack", true)?;
    }
    if !matches!(impl_id, 1 | 2 | 24) {
        return Ok(());
    }
    // the magical enchant: its bonuses and procs
    if extra.equip && extra.rune != 0 {
        if let Some(bonus) = game.enchant.modifier_table(lua, extra.rune)?.filter(|m| m.raw_len() > 0) {
            t.raw_set("gemModifireTable", bonus)?;
        }
        if !tables.procs(extra.rune).is_empty() {
            let gem_procs = lua.create_table()?;
            for (i, text) in tables.procs(extra.rune).iter().enumerate() {
                let p = lua.create_table()?;
                p.raw_set("desc", text.as_str())?;
                gem_procs.raw_set(i, p)?;
            }
            t.raw_set("gem_procs", gem_procs)?;
        }
    }
    if impl_id == 2 {
        // armour: the dye on it, and whether its look can be dyed (the look it wears, when it wears another)
        if extra.equip && extra.dye != 0 {
            t.raw_set("dyeingType", extra.dye)?;
        }
        let dyed = if extra.equip && extra.image != 0 { extra.image } else { id };
        t.raw_set("isDyeable", tables.is_dyeable(dyed))?;
    }
    if instance.flags.is_some() {
        // whether it can still be tempered
        if impl_id != 24 {
            t.raw_set("scalable", tables.scalable(id, instance.temper))?;
        }
    }
    Ok(())
}

/// The weapon / armor sections of an item's table: attack or defence, speed, stat bonuses, durability, set.
#[allow(clippy::too_many_arguments)]
fn equipment_stats(lua: &Lua, game: &GameData, state: &TipState, t: &Table, id: u32, grade: i32, instance: Instance, _extra: &ItemExtra) -> mlua::Result<()> {
    let items = &game.tips.items;
    let Some(s) = items.stats(id, grade, instance.temper.0, instance.temper.1) else { return Ok(()) };
    let Some(class) = s.class else { return Ok(()) };
    t.raw_set("itemUsage", "equip")?;
    if let Some(name) = slot_type_name(s.slot_type) {
        t.raw_set("slotType", name)?;
    }
    t.raw_set("slotTypeNum", s.slot_type)?;
    if class == Class::Weapon {
        t.raw_set("DPS", s.dps)?;
        t.raw_set("minDamage", s.min_damage)?;
        t.raw_set("maxDamage", s.max_damage)?;
        if s.attack_delay > 0.0 {
            t.raw_set("attackDelay", s.attack_delay)?;
        }
        if s.magic_dps.round() != 0.0 {
            t.raw_set("magicDps", s.magic_dps.round())?;
        }
        if s.heal_dps.round() != 0.0 {
            t.raw_set("healDps", s.heal_dps.round())?;
        }
        // how the weapon hits: piercing, slashing, blunt
        if s.extra_damage.iter().any(|v| *v != 0) {
            for (i, v) in s.extra_damage.iter().enumerate() {
                t.raw_set(format!("extraDamage{}", i + 1), *v)?;
            }
        }
    }
    // 1.2 fills `extraDPS` and `extraArmor` with 0 (gems and enchants show in `socketInfo` / `gemModifireTable`); a shield, armour or
    // accessory always has armour (and armour also magic resistance), the template's own armor modifiers added.
    // MakeItemDiffInfo (tooltip.alb [641]-[656]) compares armour with
    // CalcDiffValue(source.armor, target.armor) and then multiplies the result by -1
    // without a nil guard, so every armour/accessory item must carry `armor`, 0
    // included (a costume's slot_type 31 coverage is 0). `magicResistance` is read
    // for the same comparison at [678]-[682]. The armour line builders hide the
    // zero values themselves ([1005]-[1011], [1032]-[1034]).
    if class == Class::Weapon {
        t.raw_set("extraDPS", 0.0)?;
    } else {
        t.raw_set("extraArmor", 0.0)?;
        t.raw_set("magicResistance", s.magic_resistance + s.magic_resistance_modifier)?;
    }
    if class != Class::Weapon || s.slot_type == 20 {
        t.raw_set("armor", s.armor + s.armor_modifier)?;
        if class == Class::Weapon {
            t.raw_set("extraArmor", 0.0)?;
        }
    } else if s.armor != 0 {
        t.raw_set("armor", s.armor)?;
    }
    if class == Class::Weapon && s.magic_resistance + s.magic_resistance_modifier != 0 {
        t.raw_set("magicResistance", s.magic_resistance + s.magic_resistance_modifier)?;
    }
    if s.max_durability > 0 {
        let durability = i32::from(instance.durability.unwrap_or(u8::MAX));
        t.raw_set("durability", if instance.durability.is_some() { durability } else { s.max_durability })?;
        t.raw_set("maxDurability", s.max_durability)?;
        t.raw_set("repairable", s.repairable)?;
        if s.repairable && durability < s.max_durability && instance.durability.is_some() {
            if let Some(cost) = super::repair::item_cost(game, id, grade.clamp(0, 255) as u8, durability as u8, s.max_durability as u8) {
                t.raw_set("repairCost", cost)?;
            }
        }
    }
    if !s.use_as_stat {
        t.raw_set("useAsStat", false)?;
    }
    // the bonuses (`3941dc60`): each primary stat is the generated value plus the template's first own modifier of it, then
    // the template's other modifiers in table order; the speed attributes 54 / 55 / 74 / 119 only once. {name, type, value}
    let modifiers = lua.create_table()?;
    let add = |attr: u8, kind: i32, value: i32| -> mlua::Result<()> {
        let Some(name) = super::tipcalc::attribute_name(attr) else { return Ok(()) };
        let m = lua.create_table()?;
        m.raw_set("name", name)?;
        m.raw_set("type", f64::from(kind as f32))?;
        m.raw_set("value", f64::from(value as f32))?;
        modifiers.raw_push(m)
    };
    for (attr, generated) in s.stats.into_iter().enumerate() {
        let own = s.mods.iter().find(|m| usize::from(m.attr) == attr).map_or(0, |m| m.value);
        if generated + own != 0 {
            add(attr as u8, 0, generated + own)?;
        }
    }
    let mut shared = false;
    for m in s.mods.iter().filter(|m| m.attr > 4) {
        if matches!(m.attr, 54 | 55 | 74 | 119) {
            if shared {
                continue;
            }
            shared = true;
        }
        add(m.attr, m.kind, m.value)?;
    }
    if modifiers.raw_len() > 0 {
        t.raw_set("modifier", modifiers)?;
    }
    if let Some(set) = set_info(lua, game, state, s.set, game.items.get(&id).map_or(0, |i| i.level) as i32)? {
        t.raw_set("equipSetInfo", set)?;
    }
    Ok(())
}

/// `equipSetInfo`: the set's name, its pieces (worn ones lit) and bonuses (the ones the worn pieces earn lit).
fn set_info(lua: &Lua, game: &GameData, state: &TipState, set: u32, level: i32) -> mlua::Result<Option<Table>> {
    let Some(set) = (set != 0).then(|| game.tips.items.set(set)).flatten() else { return Ok(None) };
    let worn = set.items.iter().filter(|i| state.worn.contains(i)).count();
    let t = lua.create_table()?;
    t.raw_set("equipSetItemInfoDesc", set.name.as_str())?;
    t.raw_set("equippedSetItemCount", worn)?;
    t.raw_set("maxSetItemCount", set.items.len())?;
    let pieces = lua.create_table()?;
    for item in &set.items {
        let p = lua.create_table()?;
        p.raw_set("item_name", game.items.get(item).map_or("", |i| i.named.name.as_str()))?;
        p.raw_set("equipped", state.worn.contains(item))?;
        pieces.raw_push(p)?;
    }
    t.raw_set("setItems", pieces)?;
    let bonuses = lua.create_table()?;
    for (needed, buff) in &set.bonuses {
        let b = lua.create_table()?;
        b.raw_set("numPieces", *needed)?;
        b.raw_set("satisfied", worn as i32 >= *needed)?;
        let text = game.tips.buff_text(*buff, level, &state.attrs).or_else(|| game.buffs.get(buff).map(|b| b.desc.clone())).unwrap_or_default();
        b.raw_set("bufDesc", text)?;
        bonuses.raw_push(b)?;
    }
    t.raw_set("bonuses", bonuses)?;
    Ok(Some(t))
}

/// The info of a skill as `slot:GetTooltip` / `X2Skill:GetSkillTooltip` return it; `level` is the rank the
/// player has learned (1 when unknown).
pub fn skill_tip(lua: &Lua, game: &GameData, state: &TipState, id: u32, level: u8) -> mlua::Result<Option<Table>> {
    let Some(info) = game.skills.get(&id) else { return Ok(None) };
    let t = lua.create_table()?;
    t.raw_set("tipType", "skill")?;
    t.raw_set("type", id)?;
    t.raw_set("name", info.named.name.as_str())?;
    t.raw_set("path", info.named.icon.as_str())?;
    t.raw_set("ability", ABILITIES[usize::from(info.ability).min(ABILITIES.len() - 1)])?;
    t.raw_set("skillLevel", level)?;
    t.raw_set("mana", game.tips.mana(id, &state.attrs).unwrap_or(info.mana))?;
    t.raw_set("casting", info.casting_ms)?;
    t.raw_set("channeling", info.channeling_ms)?;
    // a skill without a range is used on oneself (the script prints "Self" when there is no `minRange`)
    if info.max_range > 0 {
        t.raw_set("minRange", info.min_range)?;
        t.raw_set("maxRange", info.max_range)?;
    }
    t.raw_set("cooldown", info.cooldown_ms)?;
    let radius = game.tips.skill_radius(id);
    if radius > 0 {
        t.raw_set("targetAreaRadius", radius)?;
    }
    let desc = game.tips.skill_text(id, level, &state.attrs);
    t.raw_set("description", desc.as_deref().unwrap_or(info.desc.as_str()))?;
    Ok(Some(t))
}

/// A buff's description with its numbers filled in (the buff tooltips); `None` when it has no placeholders.
pub fn buff_text(game: &GameData, state: &TipState, id: u32, ab_level: i32) -> Option<String> {
    game.tips.buff_text(id, ab_level, &state.attrs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    #[test]
    fn native_item_lookup_keeps_raw_grade_and_distinct_crafting_classes() -> Result<(), Box<dyn std::error::Error>> {
        let mut game = GameData::default();
        game.items.insert(
            42,
            super::super::data::ItemInfo {
                impl_id: 11,
                ..Default::default()
            },
        );
        game.grades.insert(0, ("Crude".into(), "FF888888".into()));
        game.grades.insert(1, ("Basic".into(), "FFFFFFFF".into()));
        let data = super::super::api::HudData::default();
        assert!(data.game.0.set(game).is_ok());
        let bridge = Rc::new(RefCell::new(data));
        let lua = Lua::new();
        let api = lua.create_table()?;
        api.set("GetItemInfoByType", lua.create_function(|_, _: mlua::MultiValue| Ok(None::<Table>))?)?;
        lua.globals().set("X2Item", api)?;
        install_native_lookup(&lua, &bridge)?;
        lua.load(
            r#"
          local basic=X2Item:GetItemInfoByType(42,0)
          local crude=X2Item:GetItemInfoByType(42,1)
          assert(basic.itemGrade==0 and basic.grade=='Basic' and basic.gradeColor=='FFFFFFFF')
          assert(crude.itemGrade==1 and crude.grade=='Crude' and crude.gradeColor=='FF888888')
          assert(basic.item_impl=='summon_mate' and basic.stack==nil)
          assert(X2Item:GetItemInfoByType(43,0)==nil)
        "#,
        )
        .exec()?;
        let data = bridge.borrow();
        let game = data.game.get().unwrap();
        let bag = item_tip(&lua, game, &data.tips, 42, Some(1), Some(7), Instance::default())?.unwrap();
        assert_eq!(bag.get::<String>("grade")?, "Basic");
        assert_eq!(bag.get::<u8>("itemGrade")?, 1);
        assert_eq!(bag.get::<i32>("stack")?, 7);
        assert_ne!(item_impl(3), item_impl(5));
        assert_ne!(item_impl(11), item_impl(22));
        assert_eq!(item_impl(21), "socket");
        assert_eq!(item_impl(30), "invalid item_impl");
        Ok(())
    }

    #[test]
    fn costume_and_armour_keep_the_defence_keys_the_diff_reads() -> Result<(), Box<dyn std::error::Error>> {
        let Ok(game) = GameData::load(aa_anim::db::DEFAULT_DB) else { return Ok(()) };
        let lua = Lua::new();
        let state = TipState::default();
        let costume = item_tip(&lua, &game, &state, 28439, Some(1), Some(1), Instance::default())?.unwrap();
        assert_eq!(costume.get::<i32>("armor")?, 0);
        assert_eq!(costume.get::<i32>("magicResistance")?, 0);
        let armour = item_tip(&lua, &game, &state, 23387, Some(1), Some(1), Instance::default())?.unwrap();
        assert!(armour.get::<i32>("armor")? > 0);
        assert!(armour.get::<i32>("magicResistance")? > 0);
        Ok(())
    }

    #[test]
    fn item_security_state_exposes_the_lock_overlay_and_tooltip_line() -> Result<(), Box<dyn std::error::Error>> {
        let mut game = GameData::default();
        game.items.insert(42, super::super::data::ItemInfo::default());
        let lua = Lua::new();
        let state = TipState::default();
        let now = 1_700_000_000;
        let info = |flags, unsecure_time| {
            item_tip_at(&lua, &game, &state, 42, Some(1), Some(1), Instance { flags: Some(flags), unsecure_time, ..Default::default() }, now)
                .unwrap()
                .unwrap()
        };
        let unlocked = info(0, 0);
        assert_eq!(unlocked.get::<i64>("securityState")?, 2);
        assert!(unlocked.get::<Option<Table>>("securityRemainDate")?.is_none());
        let locked = info(aa_net::secure::FLAG_SECURE, 0);
        assert_eq!(locked.get::<i64>("securityState")?, 1);
        let unlocking = info(aa_net::secure::FLAG_SECURE, now + aa_net::secure::UNLOCK_DELAY_SECONDS);
        assert_eq!(unlocking.get::<i64>("securityState")?, 3);
        let remain: Table = unlocking.get("securityRemainDate")?;
        assert_eq!(remain.get::<i64>("minute")?, 10);
        assert_eq!(remain.get::<i64>("second")?, 0);
        let expired = info(aa_net::secure::FLAG_SECURE, now - 1);
        assert_eq!(expired.get::<i64>("securityState")?, 2);
        // Template-only items (a shop's, a loot window's) have no instance state.
        let plain = item_tip_at(&lua, &game, &state, 42, Some(1), Some(1), Instance::default(), now)?.unwrap();
        assert!(plain.get::<Option<i64>>("securityState")?.is_none());
        Ok(())
    }

    #[test]
    fn item_time_lock_look_socket_and_crafter_fields_fill_the_tooltip_sections() -> Result<(), Box<dyn std::error::Error>> {
        let mut game = GameData::default();
        game.items.insert(42, super::super::data::ItemInfo {
            category_id: 4,
            sockets: 2,
            ..Default::default()
        });
        let lua = Lua::new();
        let state = TipState::default();
        let now = 1_700_000_000;
        let info = item_tip_at(
            &lua,
            &game,
            &state,
            42,
            Some(1),
            Some(1),
            Instance {
                flags: Some(aa_net::secure::FLAG_SECURE),
                create_time: now - 3600,
                lifespan_mins: 120,
                look_type: 43,
                gems: [7, 8, 0, 0, 0, 0, 0],
                crafter: Some("Smith".into()),
                ..Default::default()
            },
            now,
        )?
        .unwrap();
        assert_eq!(info.get::<i64>("lifeSpan")?, 7200);
        assert_eq!(info.get::<i64>("expireTime")?, 3600);
        assert!(info.get::<bool>("needsUnpack")?);
        assert!(info.get::<bool>("locked")?);
        assert_eq!(info.get::<i64>("lookType")?, 43);
        assert!(info.get::<bool>("lookChanged")?);
        assert_eq!(info.get::<String>("crafter")?, "Smith");
        let socket: Table = info.get("socketInfo")?;
        assert_eq!(socket.get::<i64>("maxSocket")?, 2);
        let items: Table = socket.get("socketItem")?;
        assert_eq!((items.get::<i64>(1)?, items.get::<i64>(2)?), (7, 8));
        // An expired period still carries the packed/expiry fields (the script paints it ORANGE).
        let expired = item_tip_at(
            &lua,
            &game,
            &state,
            42,
            Some(1),
            Some(1),
            Instance { flags: Some(0), create_time: now - 9000, lifespan_mins: 120, ..Default::default() },
            now,
        )?
        .unwrap();
        assert_eq!(expired.get::<i64>("expireTime")?, 0);
        assert!(!expired.get::<bool>("locked")?);
        Ok(())
    }

    #[test]
    #[ignore = "loads the original world-stage tooltip scripts and read-only game catalog"]
    fn original_tooltip_builds_the_time_look_socket_and_crafter_sections() -> Result<(), Box<dyn std::error::Error>> {
        let (rt, _bridge, path) = super::super::breath::fixture::runtime(1280.0, 720.0)?;
        rt.lua
            .load(
                r#"
        local now = os.time()
        HUD.bag[1] = { id = 16385, grade = 1, count = 1, ct = now - 60, ls = 10, fl = 4,
                       look = 16386, gm = { 7, 8, 0, 0, 0, 0, 0 }, cr = 'Smith' }
        local info = X2Bag:GetBagItemInfo(0, 1)
        assert(info.lifeSpan == 600 and info.expireTime == 540, tostring(info.lifeSpan) .. '/' .. tostring(info.expireTime))
        assert(info.locked == true and info.lookType == 16386 and info.lookChanged == true)
        assert(info.socketInfo.maxSocket == 2 and info.socketInfo.socketItem[1] == 7)
        assert(info.crafter == 'Smith')
        tooltip.tooltipWindows[1].stickMeV = 'TOP'
        tooltip.tooltipWindows[1].stickMeH = 'LEFT'
        ShowTooltip(info)
        assert(tooltip.tooltipWindows[1]:IsVisible())
        "#,
            )
            .exec()?;
        assert!(rt.errors.is_empty(), "{:?}", rt.errors);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    #[test]
    #[ignore = "loads the original world-stage tooltip scripts and read-only game catalog"]
    fn original_tooltips_diff_costumes_against_worn_slots() -> Result<(), Box<dyn std::error::Error>> {
        let (rt, _bridge, path) = super::super::breath::fixture::runtime(1280.0, 720.0)?;
        rt.lua
            .load(
                r#"
        HUD.bag[1] = { id = 28439, grade = 1, count = 1 } -- sk_maid_butler001, slot_type 31
        HUD.bag[2] = { id = 23387, grade = 1, count = 1 } -- chest armour
        local costume = X2Bag:GetBagItemInfo(0, 1)
        local armour = X2Bag:GetBagItemInfo(0, 2)
        local failures = {}
        local function show(label, info)
          local ok, err = pcall(ShowTooltip, info)
          if not ok then failures[#failures + 1] = label .. ': ' .. tostring(err) end
        end
        -- The second compare window anchors to the first (tooltip_view.alb [488] reads
        -- its stickMeH); a window that has been shown before already carries those.
        tooltip.tooltipWindows[1].stickMeV = 'TOP'
        tooltip.tooltipWindows[1].stickMeH = 'LEFT'
        -- ShowEtcNeedCompareTooltip calls GetEquippedItemTooltipInfo for the compatible
        -- worn slots and skips nil (tooltip.alb [3422]-[3424]); an empty ES_COSPLAY is
        -- therefore not compared and no second window opens.
        HUD.equipment[ES_COSPLAY] = nil
        show('costume, empty worn slot', costume)
        -- Worn costume: MakeItemDiffInfo compares the two armour-item tables, whose
        -- armour/magicResistance must both be present (tooltip.alb [641]-[656], [678]-[682]).
        HUD.equipment[ES_COSPLAY] = { id = 28439, grade = 1, count = 1 }
        show('costume, worn costume', costume)
        -- Chest armour against a worn chest piece.
        HUD.equipment[ES_CHEST] = { id = 23387, grade = 1, count = 1 }
        show('chest armour, worn chest', armour)
        assert(#failures == 0, table.concat(failures, '\n'))
        assert(tooltip.tooltipWindows[1]:IsVisible())
        assert(tooltip.tooltipWindows[2]:IsVisible())
        assert(costume.armor == 0 and costume.magicResistance == 0)
        assert(type(armour.armor) == 'number' and armour.armor > 0)
        -- GetItemIconSet(0, 0) is read by SetInfo (tooltip.alb [1333]-[1334], [1372])
        -- when an item table has itemType 0; it must return a table, not nil.
        assert(type(X2Item:GetItemIconSet(0, 0)) == 'table')
    "#,
            )
            .exec()?;
        assert!(rt.errors.is_empty(), "{:?}", rt.errors);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// A weapon with sockets and a magical enchant shows them the way the native builder does: `socketInfo`
    /// (`maxSocket`, `socketItem`), `gemModifireTable` (the rune's bonuses), `gradeEnchantable`, `gender`, and
    /// `extraDPS` / `extraArmor` staying 0 as in 1.2.
    #[test]
    fn a_socketed_weapon_lists_its_gems_and_enchant() -> Result<(), Box<dyn std::error::Error>> {
        let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
        if !std::path::Path::new(&path).exists() {
            return Ok(());
        }
        let game = GameData::load(&path)?;
        let conn = rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let lua = Lua::new();
        let runes = conn.prepare("SELECT item_id FROM item_enchanting_gems ORDER BY item_id")?.query_map([], |r| r.get::<_, i64>(0))?.collect::<Result<Vec<_>, _>>()?;
        let rune = runes.into_iter().find(|id| game.enchant.modifier_table(&lua, *id as u32).ok().flatten().is_some_and(|m| m.raw_len() > 0)).expect("a rune with bonuses");
        let tables = &game.tips.items.extra;
        let weapon = game
            .items
            .iter()
            .filter(|(_, i)| i.impl_id == 1)
            .map(|(id, _)| *id)
            .filter(|id| tables.max_sockets(game.tips.items.slot_type(*id), 4, game.tips.items.item_level(*id).unwrap_or(0)) > 0)
            .min()
            .expect("a weapon that takes sockets");
        let state = TipState::default();
        let extra = ItemExtra { equip: true, gems: [30907, 0, 0, 0, 0, 0, 0], rune: rune as u32, ..Default::default() };
        let tip = item_tip_with(&lua, &game, &state, weapon, Some(4), None, Instance::default(), &extra)?.expect("tooltip");
        let sockets = tip.get::<Option<Table>>("socketInfo")?.unwrap_or_else(|| panic!("socketInfo of {weapon}"));
        assert!(sockets.get::<f64>("maxSocket")? >= 1.0);
        let gems = sockets.get::<Option<Table>>("socketItem")?.expect("socketItem");
        assert_eq!(gems.get::<f64>(1)? as u32, 30907);
        let bonus = tip.get::<Option<Table>>("gemModifireTable")?.unwrap_or_else(|| panic!("gemModifireTable of rune {rune}"));
        assert!(bonus.raw_len() > 0, "the rune's bonuses");
        assert_eq!(tip.get::<f64>("extraDPS")?, 0.0);
        assert!(tip.get::<String>("gender").is_ok());
        // the template alone (no instance) lists the room for gems but no gems
        let bare = item_tip_with(&lua, &game, &state, weapon, Some(4), None, Instance::default(), &ItemExtra::default())?.expect("tooltip");
        let sockets = bare.get::<Option<Table>>("socketInfo")?.expect("bare socketInfo");
        assert!(sockets.get::<Option<Table>>("socketItem")?.is_none());
        assert!(bare.get::<Option<Table>>("gemModifireTable")?.is_none());
        Ok(())
    }
}
