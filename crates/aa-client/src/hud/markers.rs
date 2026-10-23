//! The icons of the roadmap (top right) and of the big map (M): quest "!" and "?", the bank, the auction
//! house, stables, mailboxes, crafting stations, housing province signs, portals, the gravesite and the
//! dots of the units in view.
//!
//! The client's own data says where everything is. `worlds/main_world/map_data/npc_map/<zone>.dat` and
//! `doodad_map/<zone>.dat` (16 byte records: template id, x, y, z as f32 in game metres) list every NPC
//! and every map-worthy doodad of a zone group, so a bank or mailbox far outside the range the server
//! shows units in still has its icon. The quest marks are the other way round: they belong to the units
//! the client has, as the 3D marks over NPCs do ([`super::marks`]); `reference/ingame/spawn` shows the
//! two quest givers within 120 m of the player and `reference/ingame/worldmap_zone` none at Desireen,
//! 170 m away. What an NPC or doodad is drawn as comes from the game database: the NPC's service flags
//! (`banker`, `auctioneer`, `stabler` ...), the doodad's group (`doodad_groups`).
//!
//! What this module hands over is a list of icons in game coordinates, `HUD.map_icons` (rebuilt when
//! something changes: the zone, the player's quests, the dots twice a second), and `markers.lua` does
//! the rest: it keeps the filters of the world map window (`X2Map:SetNpcFilter` ...), places the icons
//! on the zone or city map as the roadmap and the big map show them and gives them to
//! `AAUI.RoadMap().markers` and `AAUI.WorldMap().markers` (aa-ui draws them). The static NPC/doodad
//! icons of every zone group are loaded, so a map the player is not in shows them too (`maps now
//! display icons in zones that your character is not located in`, the 1.2 patch notes).
//!
//! Quest objective areas are the `area` circles of the client's
//! `worlds/*/level_design/zone/<zone>/client/quest_sign_sphere.g` ([`super::data::QuestArea`]): the
//! quest notifier (`questcontext/quest_context_notifier.alb`) gives the map the decal index of a
//! tracked quest with `X2Map:AddNotifyQuestInfo(decalIndex, qType, show)` (line 23 of its
//! `AddMapQuestSign`), and `markers.lua` draws that quest's circles in the index's colour and its grey
//! number cell from `SetNotifyAreaCoords` / `SetNotifyAreaColors` (`map/common.alb` lines 411-440).
//!
//! Checked against `reference/ingame/worldmap_zone` (the bank, the mailbox and the stables of Desireen,
//! the pumpkin farm zone sign west of it): the weapon, armor and general merchants and the priestess
//! have no icon there, so they get none here. The memory tomes do (the blue book, mostly hidden behind
//! the bank chest).

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use aa_net::model::UnitKind;
use aa_net::state::{Buffs, Inventory, Player, PlayerStats, Quests, Skills, Units};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, Table};
use rusqlite::{Connection, OpenFlags};

use super::api::HudBridge;
use super::data::{GameData, any_flag, any_int};
use super::quests::{Mark, PlayerFacts, kind};
use crate::Screen;

/// Frames between two looks at what the icons are (the quests and the units in view change slowly).
const REFRESH_FRAMES: u32 = 30;
/// How far from the player the dots of the units in view reach, metres (the roadmap shows about 2400).
const DOT_RANGE: f32 = 250.0;
/// A unit this close to the NPC of the map file is that NPC (metres).
const SAME_SPOT: f32 = 20.0;
/// The map's "NPC" check (`roadmap:SetRoadMapNpc`) is on: the dots are worth the work (set by `HUD.map_dots`).
static DOTS: AtomicBool = AtomicBool::new(false);

/// One icon of the map, in game coordinates.
#[derive(Clone, Debug, PartialEq)]
struct Icon {
    x: f32,
    y: f32,
    /// A symbol of the client's map icon sheet (`quest_zone_gives`, `npc_store_bank` ...).
    kind: &'static str,
    /// What the tooltip says; empty: the symbol's own text.
    tip: String,
    /// The zone group whose map the icon is on.
    zone: u32,
    /// The city (sub zone with a map of its own) the icon is in, 0 for none.
    city: u32,
}

/// An objective circle of an active quest (`HUD.map_quest_areas`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct AreaIcon {
    quest: u32,
    x: f32,
    y: f32,
    radius: f32,
    /// The zone group whose map the area is on.
    zone: u32,
    /// The city map the area is also on, 0 for none.
    city: u32,
}

/// A place in a map file: template id and position.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Spot {
    id: u32,
    x: f32,
    y: f32,
    city: u32,
}

/// The records of an `npc_map` / `doodad_map` file.
fn parse_spots(bytes: &[u8]) -> Vec<Spot> {
    bytes
        .chunks_exact(16)
        .map(|r| {
            let f = |i: usize| f32::from_le_bytes([r[i], r[i + 1], r[i + 2], r[i + 3]]);
            Spot { id: u32::from_le_bytes([r[0], r[1], r[2], r[3]]), x: f(4), y: f(8), city: 0 }
        })
        .collect()
}

/// The icon of a doodad group (`doodad_almighties.group_id`); `None`: the map has none for it.
fn doodad_icon(group: u32) -> Option<&'static str> {
    Some(match group {
        6 => "doodad_mail",
        7 => "doodad_alchemy",
        9 => "doodad_cook",
        10 => "doodad_craft",
        16 => "doodad_machinery",
        17 => "doodad_metal",
        18 => "doodad_coatofarms",
        19 => "doodad_stone",
        20 => "doodad_fabric",
        21 => "doodad_leather",
        22 => "doodad_weapon",
        23 => "doodad_timber",
        // the portal of Mirage Isle (the teal swirl beside the bank of Lacton in the capture), the world gates, the dungeon doors
        44 => "doodad_portal_archemall",
        43 => "doodad_portal",
        45 | 47 => "doodad_portal_dungeon",
        46 => "doodad_special_product",
        48 => "doodad_archium",
        49 => "doodad_light_armor",
        50 => "doodad_leather_armor",
        51 => "doodad_woodwork",
        52 => "doodad_heavy_armor",
        // the memory tomes: the blue book half hidden behind the bank chest at Desireen and Lacton in the capture
        53 => "doodad_inn",
        60 => "house_normal_house",
        61 => "house_sea_farm_house",
        62 => "house_high_house",
        63 => "house_pumpkin_house",
        64 => "house_farm_house",
        66 => "doodad_paper",
        67 => "doodad_print",
        71 => "doodad_fish_stand",
        78 => "house_benny_house",
        79 => "house_bungalow_house",
        83 => "doodad_art_work",
        _ => return None,
    })
}

/// The icon of an NPC's service, from its flags in `npcs` (`trader`: the trade goods and relic sellers).
fn service_icon(banker: bool, auctioneer: bool, stabler: bool, expedition: bool, trader: bool, ability_changer: bool, priest: bool) -> Option<&'static str> {
    if banker {
        Some("npc_store_bank")
    } else if auctioneer {
        Some("npc_store_auction_house")
    } else if stabler {
        Some("npc_stabler")
    } else if expedition {
        Some("npc_store_expedition")
    } else if trader {
        Some("npc_store_trade")
    } else if ability_changer && !priest {
        Some("npc_ability_changer")
    } else {
        None
    }
}

/// What the map's icons come from in the game database, read once in the background.
#[derive(Default)]
struct MapTables {
    /// NPC template -> the icon of the service it gives.
    npcs: HashMap<u32, &'static str>,
    /// Doodad template -> its icon and English name.
    doodads: HashMap<u32, (&'static str, String)>,
}

impl MapTables {
    fn load(path: &str) -> rusqlite::Result<MapTables> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
        let mut t = MapTables::default();
        {
            let mut q = conn.prepare("SELECT id, banker, auctioneer, stabler, expedition, trader, ability_changer, priest, specialty FROM npcs")?;
            let mut rows = q.query([])?;
            while let Some(r) = rows.next()? {
                let flag = |i: usize| any_flag(r, i);
                if let Some(icon) = service_icon(flag(1)?, flag(2)?, flag(3)?, flag(4)?, flag(5)? || flag(8)?, flag(6)?, flag(7)?) {
                    t.npcs.insert(any_int(r, 0)? as u32, icon);
                }
            }
        }
        let mut names: HashMap<u32, String> = HashMap::new();
        {
            let mut q = conn.prepare("SELECT idx, en_us FROM localized_texts WHERE tbl_name = 'doodad_almighties' AND tbl_column_name = 'name' AND en_us IS NOT NULL AND en_us <> ''")?;
            let mut rows = q.query([])?;
            while let Some(r) = rows.next()? {
                names.insert(any_int(r, 0)? as u32, r.get(1)?);
            }
        }
        {
            let mut q = conn.prepare("SELECT id, group_id FROM doodad_almighties")?;
            let mut rows = q.query([])?;
            while let Some(r) = rows.next()? {
                let id = any_int(r, 0)? as u32;
                if let Some(icon) = doodad_icon(any_int(r, 1)? as u32) {
                    t.doodads.insert(id, (icon, names.remove(&id).unwrap_or_default()));
                }
            }
        }
        Ok(t)
    }
}

/// The map files of one zone group.
#[derive(Default)]
struct ZoneSpots {
    npcs: Vec<Spot>,
    doodads: Vec<Spot>,
}

/// The city (a sub zone with a map of its own: the road map of a city, a cave) around a place, 0 for none.
fn city_at(game: &GameData, x: f32, y: f32) -> u32 {
    game.sub_zone_root_at(x, y).filter(|id| game.sub_zone_rects.contains_key(id)).unwrap_or(0)
}

/// Every zone group's `npc_map` / `doodad_map` file (about 250 small files in all), so the map of a
/// zone the player is not in has its icons too.
fn load_all_zones(game: &GameData) -> BTreeMap<u32, ZoneSpots> {
    let pak = aa_pak::Pak::global();
    let mut out: BTreeMap<u32, ZoneSpots> = BTreeMap::new();
    for (dir, npcs) in [("npc_map", true), ("doodad_map", false)] {
        for file in pak.list(&format!("worlds/main_world/map_data/{dir}/")) {
            let Some(key) = file.rsplit('/').next().and_then(|f| f.strip_suffix(".dat")) else { continue };
            let Some((group, _)) = game.zone_groups.iter().find(|(_, g)| g.key == key) else { continue };
            let Ok(bytes) = pak.read(file) else { continue };
            let mut spots = parse_spots(&bytes);
            for s in &mut spots {
                s.city = city_at(game, s.x, s.y);
            }
            let z = out.entry(*group).or_default();
            if npcs { z.npcs = spots } else { z.doodads = spots }
        }
    }
    out
}

/// The objective circles of the active quests' progress components, from the client's
/// `quest_sign_sphere.g` tables.
fn build_areas(game: &GameData, quests: &Quests) -> Vec<AreaIcon> {
    let mut out = Vec::new();
    for q in &quests.active {
        if game.quests.def(q.template_id).is_none() {
            continue;
        }
        let mut components: Vec<u32> = game.quests.parts_of(q.template_id, kind::PROGRESS).map(|c| c.id).collect();
        if components.is_empty() && q.component_id != 0 {
            components.push(q.component_id);
        }
        if components.is_empty() {
            continue;
        }
        for (zone, areas) in &game.quest_areas {
            for a in areas {
                if a.quest == q.template_id && components.contains(&a.component) {
                    out.push(AreaIcon {
                        quest: a.quest,
                        x: a.x,
                        y: a.y,
                        radius: a.radius,
                        zone: *zone,
                        city: city_at(game, a.x, a.y),
                    });
                }
            }
        }
    }
    // A stable order: HashMap iteration must not make the list differ every 30 frames.
    out.sort_by(|a, b| {
        a.quest.cmp(&b.quest).then(a.zone.cmp(&b.zone)).then(a.x.total_cmp(&b.x)).then(a.y.total_cmp(&b.y))
    });
    out.dedup();
    out
}

#[derive(Resource, Default)]
pub struct MapIcons {
    tables: Arc<OnceLock<MapTables>>,
    loading: bool,
    /// Every zone group's static icons (loaded once, when the database tables are ready).
    zones: BTreeMap<u32, ZoneSpots>,
    zones_loaded: bool,
    /// Where the player died while exp can still be won back (the gravesite), and whether they were dead a frame ago.
    corpse: Option<Vec2>,
    was_dead: bool,
    refresh_in: u32,
    icons: Vec<Icon>,
    /// Bumped with every new list: what the Lua state has seen last.
    version: u32,
    areas: Vec<AreaIcon>,
    /// Bumped with every new objective area list.
    areas_version: u32,
}

/// `HUD.map_dots(on)`: the "NPC" check of the roadmap options (a result of the scripts' option data).
pub fn install(lua: &Lua, hud: &Table) -> mlua::Result<()> {
    hud.raw_set("map_dots", lua.create_function(|_, on: Option<bool>| Ok(DOTS.store(on.unwrap_or(false), Ordering::Relaxed)))?)
}

/// `markers.lua`: the filters, the places and the hooks of the toolkit (after `ingame.lua`).
pub fn load_script(lua: &Lua) -> mlua::Result<()> {
    lua.load(include_str!("markers.lua")).set_name("@markers.lua").exec()
}

pub fn plugin(app: &mut App) {
    app.init_resource::<MapIcons>()
        .add_systems(OnExit(Screen::InGame), |mut icons: ResMut<MapIcons>| {
            let tables = icons.tables.clone();
            let zones = std::mem::take(&mut icons.zones);
            *icons = MapIcons { tables, loading: icons.loading, zones, zones_loaded: icons.zones_loaded, ..default() };
        })
        .add_systems(Update, update_icons.after(super::sync_hud).run_if(super::hud_up).run_if(in_state(Screen::InGame)));
}

/// Quest icon of an NPC for the player: the kind of the quests it offers, takes in or waits for.
fn quest_icon(game: &GameData, facts: &PlayerFacts, quests: &Quests, npc: u32, mark: Mark) -> &'static str {
    let defs = |ids: &[u32]| ids.iter().filter_map(|q| game.quests.def(*q)).collect::<Vec<_>>();
    if mark.is_offer() {
        let offered = defs(&game.quests.offers(npc, facts));
        if offered.iter().any(|d| d.is_main()) {
            "quest_main_gives"
        } else if offered.iter().any(|d| !d.is_daily() && !d.is_livelihood()) {
            "quest_zone_gives"
        } else if offered.iter().any(|d| d.is_livelihood()) {
            "quest_livelihood_gives"
        } else {
            "quest_repeat"
        }
    } else if mark.is_ready() {
        let done = defs(&game.quests.completes(npc, facts));
        if done.iter().any(|d| d.is_main()) {
            "quest_main_completes"
        } else if done.iter().any(|d| d.is_livelihood()) {
            "quest_livelihood_complete"
        } else {
            "quest_zone_completes"
        }
    } else {
        debug_assert!(mark.is_progress());
        let waiting: Vec<u32> = quests.active.iter().map(|q| q.template_id).filter(|q| game.quests.report_npc(*q) == Some(npc)).collect();
        if defs(&waiting).iter().any(|d| d.is_main()) { "quest_main_progress" } else { "quest_zone_progress" }
    }
}

/// Everything the map shows now.
#[allow(clippy::too_many_arguments)]
fn build_icons(
    tables: &MapTables,
    zones: &BTreeMap<u32, ZoneSpots>,
    game: &GameData,
    facts: &PlayerFacts,
    quests: &Quests,
    units: &Units,
    me: Vec3,
    own_faction: u32,
    zone_group: u32,
    corpse: Option<Vec2>,
) -> Vec<Icon> {
    let mut out = Vec::new();
    let npc_name = |id: u32| game.npcs.get(&id).map(|n| n.name.clone()).unwrap_or_default();
    // The quest marks belong to the units the client has (as the 3D marks do): the real big map shows no "!" at
    // Desireen, 170 m from where the capture stands, while it shows its bank, mailbox and stables.
    let mut quest_icons = Vec::new();
    let mut quest_givers: Vec<(u32, u32, f32, f32)> = Vec::new();
    let mut by_template: HashMap<u32, Option<&'static str>> = HashMap::new();
    for u in units.iter() {
        let UnitKind::Npc { template_id, .. } = u.kind else { continue };
        let quest = *by_template.entry(template_id).or_insert_with(|| game.quests.mark(template_id, facts).map(|m| quest_icon(game, facts, quests, template_id, m)));
        let Some(kind) = quest else { continue };
        let tip = Some(npc_name(template_id)).filter(|n| !n.is_empty()).unwrap_or_else(|| u.name.clone());
        quest_givers.push((zone_group, template_id, u.pos.x, u.pos.y));
        quest_icons.push(Icon { x: u.pos.x, y: u.pos.y, kind, tip, zone: zone_group, city: city_at(game, u.pos.x, u.pos.y) });
    }
    // every zone group's doodads and NPCs, on their own maps
    for (group, zone) in zones {
        for s in &zone.doodads {
            if let Some((kind, name)) = tables.doodads.get(&s.id) {
                out.push(Icon { x: s.x, y: s.y, kind, tip: name.clone(), zone: *group, city: s.city });
            }
        }
        // the NPCs of the map file: their service icon, unless the NPC is in view with a quest icon over it
        for s in &zone.npcs {
            let Some(kind) = tables.npcs.get(&s.id) else { continue };
            if quest_givers.iter().any(|(g, id, x, y)| *g == *group && *id == s.id && (x - s.x).hypot(y - s.y) < SAME_SPOT) {
                continue;
            }
            out.push(Icon { x: s.x, y: s.y, kind, tip: npc_name(s.id), zone: *group, city: s.city });
        }
    }
    if DOTS.load(Ordering::Relaxed) {
        for u in units.iter() {
            let UnitKind::Npc { .. } = u.kind else { continue };
            let here = Vec3::new(u.pos.x, u.pos.y, u.pos.z);
            if u.dead || me.distance(here) > DOT_RANGE {
                continue;
            }
            let kind = match game.relation(own_faction, u.faction_id) {
                "hostile" => "hostile",
                "friendly" => "friendly",
                _ => continue,
            };
            out.push(Icon { x: here.x, y: here.y, kind, tip: u.name.clone(), zone: zone_group, city: city_at(game, here.x, here.y) });
        }
    }
    out.append(&mut quest_icons);
    if let Some(c) = corpse {
        out.push(Icon { x: c.x, y: c.y, kind: "corpse_pos", tip: String::new(), zone: zone_group, city: city_at(game, c.x, c.y) });
    }
    out
}

/// The list as the Lua state reads it: `HUD.map_icons` (`x`, `y`, `kind`, `tip`, `zone`, `city`), the
/// rectangles of every zone group and city (`HUD.map_zone_rects` / `HUD.map_city_rects`), the active
/// quest objective circles (`HUD.map_quest_areas`) and their versions.
fn push(
    lua: &Lua,
    game: &GameData,
    icons: &[Icon],
    areas: &[AreaIcon],
    version: u32,
    areas_version: u32,
) -> mlua::Result<()> {
    let hud: Table = lua.globals().raw_get("HUD")?;
    let list = lua.create_table_with_capacity(icons.len(), 0)?;
    for (i, ic) in icons.iter().enumerate() {
        let t = lua.create_table_with_capacity(0, 6)?;
        t.raw_set("x", f64::from(ic.x))?;
        t.raw_set("y", f64::from(ic.y))?;
        t.raw_set("kind", ic.kind)?;
        t.raw_set("tip", ic.tip.as_str())?;
        t.raw_set("zone", ic.zone)?;
        if ic.city != 0 {
            t.raw_set("city", ic.city)?;
        }
        list.raw_set(i + 1, t)?;
    }
    let zones = lua.create_table()?;
    for (id, z) in &game.zone_groups {
        zones.raw_set(*id, lua.create_sequence_from(z.rect.iter().map(|v| f64::from(*v)))?)?;
    }
    let cities = lua.create_table()?;
    for (id, r) in &game.sub_zone_rects {
        cities.raw_set(*id, lua.create_sequence_from(r.iter().map(|v| f64::from(*v)))?)?;
    }
    let area_list = lua.create_table_with_capacity(areas.len(), 0)?;
    for (i, a) in areas.iter().enumerate() {
        let t = lua.create_table_with_capacity(0, 6)?;
        t.raw_set("qtype", a.quest)?;
        t.raw_set("x", f64::from(a.x))?;
        t.raw_set("y", f64::from(a.y))?;
        t.raw_set("radius", f64::from(a.radius))?;
        t.raw_set("zone", a.zone)?;
        if a.city != 0 {
            t.raw_set("city", a.city)?;
        }
        area_list.raw_set(i + 1, t)?;
    }
    hud.raw_set("map_icons", list)?;
    hud.raw_set("map_zone_rects", zones)?;
    hud.raw_set("map_city_rects", cities)?;
    hud.raw_set("map_icons_version", version)?;
    hud.raw_set("map_quest_areas", area_list)?;
    hud.raw_set("map_quest_areas_version", areas_version)
}

#[allow(clippy::too_many_arguments)]
fn update_icons(
    mut state: ResMut<MapIcons>,
    bridge: NonSend<HudBridge>,
    rt: Option<NonSend<UiRuntime>>,
    units: Res<Units>,
    player: Res<Player>,
    quests: Res<Quests>,
    inventory: Res<Inventory>,
    skills: Res<Skills>,
    stats: Res<PlayerStats>,
    buffs: Res<Buffs>,
) {
    let (Some(rt), Some(spawn)) = (rt, player.spawn.as_ref()) else { return };
    let data = bridge.0.borrow();
    let Some(game) = data.game.get() else { return };
    if !state.loading {
        state.loading = true;
        let slot = state.tables.clone();
        std::thread::Builder::new()
            .name("hud-map-db".into())
            .spawn(move || {
                let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
                match MapTables::load(&path) {
                    Ok(t) => {
                        let _ = slot.set(t);
                    }
                    Err(e) => {
                        warn!("hud: map icons from {path}: {e}");
                        let _ = slot.set(MapTables::default());
                    }
                }
            })
            .ok();
    }
    let tables = state.tables.clone();
    let Some(tables) = tables.get() else { return };
    // where the player died stays a gravesite while the exp lost can still be won back
    let dead = stats.dead;
    if dead && !state.was_dead {
        state.corpse = Some(Vec2::new(data.pos.x, data.pos.y));
    } else if !dead && stats.recoverable_exp <= 0 && state.corpse.is_some() {
        state.corpse = None;
        state.refresh_in = 0;
    }
    state.was_dead = dead;
    if !state.zones_loaded {
        state.zones = load_all_zones(game);
        state.zones_loaded = true;
        let (npcs, doodads) = state.zones.values().fold((0, 0), |(n, d), z| (n + z.npcs.len(), d + z.doodads.len()));
        debug!("hud: map icons of {} zones: {npcs} npcs, {doodads} doodads", state.zones.len());
        state.refresh_in = 0;
    }
    // what the stage's Lua state has seen (a stage built anew starts without any list)
    let hud = rt.lua.globals().raw_get::<Table>("HUD").ok();
    let seen = hud.as_ref().and_then(|h| h.raw_get::<Option<u32>>("map_icons_version").ok().flatten());
    let seen_areas = hud.as_ref().and_then(|h| h.raw_get::<Option<u32>>("map_quest_areas_version").ok().flatten());
    if state.refresh_in > 0 && seen.is_some() {
        state.refresh_in -= 1;
        return;
    }
    state.refresh_in = REFRESH_FRAMES;
    let facts = PlayerFacts::of(spawn, &stats, &inventory, &skills, &buffs, &quests);
    let zone_group = data.snapshot.zone_group;
    let mut icons = build_icons(
        tables,
        &state.zones,
        game,
        &facts,
        &quests,
        &units,
        data.pos,
        spawn.character().faction_id,
        zone_group,
        state.corpse,
    );
    let own_instance = spawn.state.instance_id;
    for m in &data.party.members {
        if m.id == spawn.character().id || m.instance_id != own_instance { continue; }
        // Before the first remote status, only nearby units have a trustworthy position.
        if !m.has_status && units.get(m.obj_id).is_none() { continue; }
        if m.has_status && game.zones.get(&m.zone_id).map(|z| z.0) != Some(data.snapshot.zone_group) { continue; }
        let kind = if !m.online { "offline_party" } else if m.id == data.party.owner_id && !data.party.is_party { "raidteam_owner" }
            else if data.party.is_party { "party" } else { "raidteam" };
        icons.push(Icon { x: m.pos.x, y: m.pos.y, kind, tip: m.name.clone(), zone: zone_group, city: city_at(game, m.pos.x, m.pos.y) });
    }
    let areas = build_areas(game, &quests);
    let mut changed = false;
    if icons != state.icons {
        state.version += 1;
        state.icons = icons;
        changed = true;
    }
    if areas != state.areas {
        state.areas_version += 1;
        state.areas = areas;
        changed = true;
    }
    if !changed && seen == Some(state.version) && seen_areas == Some(state.areas_version) {
        return;
    }
    if let Err(e) = push(&rt.lua, game, &state.icons, &state.areas, state.version, state.areas_version) {
        warn!("hud: map icons: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_are_sixteen_byte_records() {
        let mut bytes = Vec::new();
        for (id, x, y, z) in [(3597u32, 15562.45f32, 15354.32f32, 127.96f32), (320, 100.0, 200.0, 3.0)] {
            bytes.extend_from_slice(&id.to_le_bytes());
            for v in [x, y, z] {
                bytes.extend_from_slice(&v.to_le_bytes());
            }
        }
        bytes.extend_from_slice(&[1, 2, 3]); // a torn record is dropped
        let spots = parse_spots(&bytes);
        assert_eq!(spots.len(), 2);
        assert_eq!((spots[0].id, spots[0].x, spots[0].y), (3597, 15562.45, 15354.32));
        assert_eq!(spots[1].id, 320);
    }

    #[test]
    fn what_the_map_draws() {
        // a bank, the stables, the mailbox and the farm zone sign of the Nuian spawn; not the priestess
        assert_eq!(service_icon(false, false, false, false, false, true, true), None);
        assert_eq!(service_icon(true, false, false, false, false, false, false), Some("npc_store_bank"));
        assert_eq!(service_icon(false, false, true, false, false, false, false), Some("npc_stabler"));
        assert_eq!(doodad_icon(6), Some("doodad_mail"));
        assert_eq!(doodad_icon(63), Some("house_pumpkin_house"));
        assert_eq!(doodad_icon(53), Some("doodad_inn"), "the memory tome");
        assert_eq!(doodad_icon(34), None, "a test doodad");
    }

    /// The real `markers.lua`: the notifier's count-first tables, the decal index and the circle size.
    #[test]
    fn quest_area_markers_use_the_notifier_colour_and_number() {
        let lua = Lua::new();
        lua.load(
            r#"
            HUD = {
              state = { zone_group = 5, zone_rect = { 11925, 13266, 4810.2446, 2882 }, zone_key_name = "w_solzreed" },
              map_icons = {}, map_icons_version = 1,
              map_quest_areas = { { qtype = 269, x = 14449.12, y = 14451.57, radius = 30, zone = 5 } },
              map_quest_areas_version = 1,
              map_zone_rects = { [5] = { 11925, 13266, 4810.2446, 2882 } }, map_city_rects = {},
              map_dots = function() end,
            }
            X2Map = {}
            X2 = {}
            X2Quest = {
              GetActiveQuestListCount = function() return 1 end,
              GetActiveQuestType = function(_, i) return i == 1 and 269 or nil end,
            }
            HUD_ROAD_FRAMES = {}
            locale = { map = { getSymbolText = function() return nil end } }
            AAUI = { WorldMap = function() return { level = 2, zone = 5 } end }
            "#,
        )
        .exec()
        .unwrap();
        lua.load(include_str!("markers.lua")).set_name("@markers.lua").exec().unwrap();
        let markers: Table = lua.load("return AAUI.WorldMap().markers").eval().unwrap();
        assert_eq!(markers.raw_len(), 2, "the ring and its grey number");
        let ring: Table = markers.raw_get(1).unwrap();
        assert_eq!(ring.raw_get::<String>("icon").unwrap(), "ui/map/territory_circle.dds");
        assert_eq!(ring.raw_get::<String>("layer").unwrap(), "background");
        assert!((ring.raw_get::<f32>("w").unwrap() - 60.0 / 4810.2446).abs() < 1e-6);
        let color: Table = ring.raw_get("color").unwrap();
        assert!((color.raw_get::<f32>(2).unwrap() - 96.0 / 255.0).abs() < 1e-6, "decal 1 is the red area colour");
        let number: Table = markers.raw_get(2).unwrap();
        let coords: Table = number.raw_get("coords").unwrap();
        assert_eq!((coords.raw_get::<f32>(1).unwrap(), coords.raw_get::<f32>(2).unwrap()), (240.0, 0.0), "the grey 1 cell");
    }

    /// A city map shows the icons of that city only: an icon of another city is skipped, not indexed as a boolean.
    #[test]
    fn road_map_icon_of_another_city_is_skipped() {
        let lua = Lua::new();
        lua.load(
            r#"
            HUD = {
              state = { zone_group = 5, zone_rect = { 11925, 13266, 4810.2446, 2882 }, zone_key_name = "w_solzreed" },
              map_icons = { { x = 14000, y = 14000, kind = "npc_store_bank", tip = "", zone = 5, city = 3 } },
              map_icons_version = 1, map_quest_areas = {}, map_quest_areas_version = 1,
              map_zone_rects = { [5] = { 11925, 13266, 4810.2446, 2882 } },
              map_city_rects = { [3] = { 13800, 13800, 1000, 1000 } },
              map_dots = function() end,
            }
            X2Map = {}
            X2 = {}
            X2Quest = { GetActiveQuestListCount = function() return 0 end }
            HUD_ROAD_FRAMES = {}
            locale = { map = { getSymbolText = function() return nil end } }
            AAUI = { RoadMap = function() return { level = 3, zone = 5 } end }
            "#,
        )
        .exec()
        .unwrap();
        lua.load(include_str!("markers.lua")).set_name("@markers.lua").exec().unwrap();
        let markers: Table = lua.load("return AAUI.RoadMap().markers").eval().unwrap();
        assert_eq!(markers.raw_len(), 0, "an icon of city 3 is not on the map of city 5");
    }

    /// The same for a quest area of another city.
    #[test]
    fn road_map_area_of_another_city_is_skipped() {
        let lua = Lua::new();
        lua.load(
            r#"
            HUD = {
              state = { zone_group = 5, zone_rect = { 11925, 13266, 4810.2446, 2882 }, zone_key_name = "w_solzreed" },
              map_icons = {}, map_icons_version = 1,
              map_quest_areas = { { qtype = 269, x = 14000, y = 14000, radius = 30, zone = 5, city = 3 } },
              map_quest_areas_version = 1,
              map_zone_rects = { [5] = { 11925, 13266, 4810.2446, 2882 } },
              map_city_rects = { [3] = { 13800, 13800, 1000, 1000 } },
              map_dots = function() end,
            }
            X2Map = {}
            X2 = {}
            X2Quest = {
              GetActiveQuestListCount = function() return 1 end,
              GetActiveQuestType = function(_, i) return i == 1 and 269 or nil end,
            }
            HUD_ROAD_FRAMES = {}
            locale = { map = { getSymbolText = function() return nil end } }
            AAUI = { RoadMap = function() return { level = 3, zone = 5 } end }
            "#,
        )
        .exec()
        .unwrap();
        lua.load(include_str!("markers.lua")).set_name("@markers.lua").exec().unwrap();
        let markers: Table = lua.load("return AAUI.RoadMap().markers").eval().unwrap();
        assert_eq!(markers.raw_len(), 0, "an area of city 3 is not on the map of city 5");
    }

    /// Only the active quest's progress components get their circles: not its report step, not another quest.
    #[test]
    fn areas_follow_the_active_progress_components() {
        use aa_net::quest::Quest;
        use super::super::data::QuestArea;
        let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
        let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return };
        let Ok(quests) = super::super::quests::QuestTables::load(&conn) else { return };
        let mut game = GameData::default();
        game.quests = quests;
        game.quest_areas.insert(
            5,
            vec![
                QuestArea { quest: 299, component: 1191, x: 15500.0, y: 15000.0, radius: 50.0 },
                QuestArea { quest: 299, component: 915, x: 15600.0, y: 15000.0, radius: 50.0 },
                QuestArea { quest: 270, component: 914, x: 15400.0, y: 15000.0, radius: 50.0 },
            ],
        );
        let active = Quests {
            active: vec![Quest { template_id: 299, component_id: 1191, ..default() }],
            ..default()
        };
        let areas = build_areas(&game, &active);
        assert_eq!(areas.len(), 1);
        assert_eq!((areas[0].quest, areas[0].zone, areas[0].radius), (299, 5, 50.0));
    }

    /// The real tables and the real map file of Solzreed Peninsula, when the game database is there.
    #[test]
    fn solzreed_has_a_bank_and_a_mailbox() {
        let path = std::env::var("AA_DB").unwrap_or_else(|_| aa_anim::db::DEFAULT_DB.to_string());
        let Ok(t) = MapTables::load(&path) else { return };
        let Ok(bytes) = aa_pak::Pak::global().read("worlds/main_world/map_data/npc_map/w_solzreed.dat") else { return };
        let spots = parse_spots(&bytes);
        assert!(spots.iter().any(|s| t.npcs.get(&s.id) == Some(&"npc_store_bank")));
        assert!(spots.iter().any(|s| t.npcs.get(&s.id) == Some(&"npc_stabler")));
        assert_eq!(t.doodads.get(&320).map(|d| d.0), Some("doodad_mail"));
        assert_eq!(t.doodads.get(&320).map(|d| d.1.as_str()), Some("Mailbox"));
    }
}
