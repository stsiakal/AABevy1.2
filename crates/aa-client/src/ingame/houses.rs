//! Houses and farms in the world: the client's housing-area polygons (`housing_area.xml`) for the placement
//! preview, and the unit model that follows `SCHouseBuildProgress`.
//!
//! The 1.2 server validates a plot against the polygons the level designers drew (`systems/npcs/subzone.rs`
//! `parse_housing_area_file`, `systems/housing/placement.rs::validate_house`). The client only knows the pak, so
//! [`HousingAreas`] reads the same `main_world/level_design/zone/<zone>/client/housing_area.xml` files and their
//! zone origin cells from `world.xml`. The server stays authoritative: a green preview is not a reservation.
//!
//! A house's unit model is the current construction step's model (`housing_build_steps.model_id`, sent back in
//! `SCHouseBuildProgress`). The server sends the new model on each build action; this module updates the received
//! unit and lets `units::sync_views` rebuild the entity, rather than keeping a second house view.

use std::collections::HashMap;

use aa_net::housing::HousingEvent;
use aa_net::model::UnitKind;
use aa_net::state::{Houses, Units};
use aa_net::{Event, NetEvent};
use bevy::prelude::*;

use super::placement::Placement;
use super::units::Views;

/// One housing polygon (`housing_area.xml`'s `Area value1`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HousingShape {
    /// `housing_areas.id` (the XML `value1`, not the engine trigger `Id`).
    pub id: u32,
    /// West, south, east, north of the polygon, game metres.
    pub bounds: [f32; 4],
    pub points: Vec<[f32; 2]>,
}

impl HousingShape {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let b = self.bounds;
        if x < b[0] || x > b[2] || y < b[1] || y > b[3] {
            return false;
        }
        let mut inside = false;
        let mut j = self.points.len().wrapping_sub(1);
        for i in 0..self.points.len() {
            let (pi, pj) = (self.points[i], self.points[j]);
            if (pi[1] > y) != (pj[1] > y) && x < (pj[0] - pi[0]) * (y - pi[1]) / (pj[1] - pi[1]) + pi[0] {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    fn area(&self) -> f32 {
        (self.bounds[2] - self.bounds[0]) * (self.bounds[3] - self.bounds[1])
    }
}

/// The housing polygons of `main_world`, read once from the pak.
#[derive(Resource, Default)]
pub struct HousingAreas {
    pub shapes: Vec<HousingShape>,
}

impl HousingAreas {
    pub fn load() -> HousingAreas {
        let mut areas = HousingAreas::default();
        let pak = aa_pak::Pak::global();
        let world = "worlds/main_world";
        let Ok(world_xml) = pak.read_string(&format!("{world}/world.xml")) else { return areas };
        let mut origins: HashMap<u32, (f32, f32)> = HashMap::new();
        for tag in world_xml.split("<Zone ").skip(1) {
            let tag = format!(" {}", tag.split('>').next().unwrap_or(""));
            let (Some(id), Some(x), Some(y)) = (attr(&tag, "id"), attr(&tag, "originX"), attr(&tag, "originY")) else {
                continue;
            };
            if let (Ok(id), Ok(x), Ok(y)) = (id.parse(), x.parse(), y.parse()) {
                origins.insert(id, (x, y));
            }
        }
        let prefix = format!("{world}/level_design/zone/");
        let files: Vec<String> = pak
            .list(&prefix)
            .filter(|p| p.ends_with("/client/housing_area.xml"))
            .map(str::to_string)
            .collect();
        for file in files {
            let zone: Option<u32> = file
                .split("level_design/zone/")
                .nth(1)
                .and_then(|r| r.split('/').next())
                .and_then(|z| z.parse().ok());
            let Some(origin) = zone.and_then(|z| origins.get(&z)).copied() else { continue };
            if let Ok(text) = pak.read_string(&file) {
                parse_housing_areas(&text, origin, &mut areas.shapes);
            }
        }
        areas
    }

    /// The `housing_areas.id` tags of every polygon containing the point, smallest first.
    pub fn area_at(&self, x: f32, y: f32) -> Vec<u32> {
        let mut found: Vec<&HousingShape> = self.shapes.iter().filter(|s| s.contains(x, y)).collect();
        found.sort_by(|a, b| a.area().total_cmp(&b.area()));
        found.into_iter().map(|s| s.id).collect()
    }

    /// The smallest housing polygon containing the point (its edge markers are the retail yellow posts).
    pub fn shape_at(&self, x: f32, y: f32) -> Option<&HousingShape> {
        self.shapes.iter().filter(|s| s.contains(x, y)).min_by(|a, b| a.area().total_cmp(&b.area()))
    }
}

fn attr<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let at = text.find(&key)? + key.len();
    let end = text[at..].find('"')?;
    Some(&text[at..at + end])
}

fn pair(text: &str) -> Option<(f32, f32)> {
    let mut it = text.split(',');
    Some((it.next()?.trim().parse().ok()?, it.next()?.trim().parse().ok()?))
}

/// Parses one `housing_area.xml`: entity `Pos` + cell offset (1024 m cells, plus the zone origin) and the area's
/// `value1` database id. Kept public for the geometry tests.
pub fn parse_housing_areas(text: &str, origin: (f32, f32), out: &mut Vec<HousingShape>) {
    for entity in text.split("<Entity ").skip(1) {
        let (Some(pos), Some(cx), Some(cy)) = (attr(entity, "Pos").and_then(pair), attr(entity, "cellX"), attr(entity, "cellY")) else {
            continue;
        };
        let (Ok(cx), Ok(cy)) = (cx.parse::<f32>(), cy.parse::<f32>()) else { continue };
        let Some(value1) = entity.find("value1=\"").and_then(|a| entity[a + 8..].split('"').next()) else {
            continue;
        };
        let Ok(id) = value1.parse::<u32>() else { continue };
        let gx = (origin.0 + cx) * 1024.0 + pos.0;
        let gy = (origin.1 + cy) * 1024.0 + pos.1;
        let points: Vec<[f32; 2]> = entity
            .split("<Point Pos=\"")
            .skip(1)
            .filter_map(|p| pair(p.split('"').next()?))
            .map(|(x, y)| [gx + x, gy + y])
            .collect();
        if points.len() < 3 {
            continue;
        }
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for p in &points {
            bounds = [bounds[0].min(p[0]), bounds[1].min(p[1]), bounds[2].max(p[0]), bounds[3].max(p[1])];
        }
        out.push(HousingShape { id, bounds, points });
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<HousingAreas>()
        .add_systems(Startup, load_areas)
        .add_systems(
            Update,
            (close_created_build, log_house_factions, sync_house_models.before(super::units::sync_views))
                .run_if(super::world_screens),
        );
}

/// The server's faction for a house is logged once per spawned unit: design ids collide with NPC template ids,
/// so the HUD must not read the NPC table for a house and we need to see whether the server sends 0.
fn log_house_factions(units: Res<Units>, mut seen: Local<std::collections::HashSet<u32>>) {
    for unit in units.by_id.values() {
        let UnitKind::Housing { tl_id, .. } = unit.kind else { continue };
        if seen.insert(unit.obj_id) {
            info!("housing: unit {} (tl {tl_id}) received faction_id {}", unit.obj_id, unit.faction_id);
        }
    }
}

/// `CSCreateHouse` accepted: the server sends `SCMyHouse` and then spawns the house. The original build window
/// stays open after a scripted `house_build` (only its Build button hides it itself), so raise the client's
/// `BUILDER_END` UI event that `housing_manager.alb` handles with `ShowBuildWindow()` (which hides it).
pub fn close_created_build(
    mut events: MessageReader<NetEvent>,
    mut placement: ResMut<Placement>,
    rt: Option<NonSend<aa_ui::UiRuntime>>,
) {
    for NetEvent(event) in events.read() {
        let Event::Housing(HousingEvent::MyHouse(house)) = event else { continue };
        let design = house.template_id;
        let answered_session = placement.session.as_ref().is_some_and(|s| s.is_house() && s.done && s.house_design() == Some(design));
        let answered_pending = placement.session.is_none() && placement.pending_house == Some(design);
        if !answered_session && !answered_pending {
            continue;
        }
        info!(
            "housing: server created house tl {} (design {design}); closing the Start Construction window",
            house.tl_id
        );
        placement.pending_house = None;
        if let Some(session) = placement.session.as_mut() {
            session.created = true;
            session.done = true;
        }
        if let Some(rt) = rt.as_ref() {
            rt.fire_event("BUILDER_END", ());
        }
    }
}

fn load_areas(mut areas: ResMut<HousingAreas>) {
    let loaded = HousingAreas::load();
    info!("housing: {} area polygons from housing_area.xml", loaded.shapes.len());
    *areas = loaded;
}

/// `SCHouseBuildProgress` changed a house's model: update the received unit and make `sync_views` rebuild its
/// entity. The server's model id is the only source; nothing is inferred from action counts.
pub fn sync_house_models(mut units: ResMut<Units>, houses: Res<Houses>, mut views: ResMut<Views>, mut commands: Commands) {
    let mut updates: Vec<(u32, u32)> = Vec::new();
    for house in houses.by_tl_id.values() {
        if house.demolished {
            continue;
        }
        let Some(unit) = units
            .by_id
            .values()
            .find(|u| matches!(u.kind, UnitKind::Housing { tl_id, .. } if tl_id == house.snapshot.tl_id))
        else {
            continue;
        };
        if unit.model_id != house.snapshot.model_id {
            updates.push((unit.obj_id, house.snapshot.model_id));
        }
    }
    for (obj_id, model_id) in updates {
        if let Some(unit) = units.by_id.get_mut(&obj_id) {
            unit.model_id = model_id;
        }
        if let Some(entity) = views.forget_unit(obj_id) {
            commands.entity(entity).despawn();
        }
        info!("housing: unit {obj_id} now draws construction model {model_id}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<Objects>
 <Entity Name="gwe_housing" Pos="100,200,10" cellX="1" cellY="0">
  <Area Id="0" value1="128"><Points>
   <Point Pos="0,0,0"/><Point Pos="10,0,0"/><Point Pos="10,10,0"/><Point Pos="0,10,0"/>
  </Points></Area>
 </Entity>
 <Entity Name="second" Pos="0,0,0" cellX="0" cellY="0">
  <Area Id="7" value1="5"><Points>
   <Point Pos="500,500,0"/><Point Pos="520,500,0"/><Point Pos="520,520,0"/><Point Pos="500,520,0"/>
  </Points></Area>
 </Entity>
</Objects>"#;

    #[test]
    fn the_area_polygons_use_value1_and_the_zone_origin() {
        let mut shapes = Vec::new();
        parse_housing_areas(XML, (2.0, 3.0), &mut shapes);
        assert_eq!(shapes.len(), 2);
        assert_eq!(shapes[0].id, 128);
        assert_eq!(shapes[0].points[0], [2.0 * 1024.0 + 1.0 * 1024.0 + 100.0, 3.0 * 1024.0 + 200.0]);
        assert!(shapes[0].contains(3172.0, 3272.0));
        assert!(!shapes[0].contains(500.0, 500.0));
        assert_eq!(shapes[1].id, 5, "the engine Id is not the housing area id");
    }

    fn house(tl_id: u16, model_id: u32) -> aa_net::housing::House {
        aa_net::housing::House {
            snapshot: aa_net::housing::HouseSnapshot {
                tl_id,
                db_id: 1,
                obj_id: 50,
                template_id: 267,
                model_id,
                co_owner_id: 0,
                owner_id: 1,
                owner_name: "Owner".into(),
                account_id: 1,
                permission: 0,
                all_actions: 1,
                current_actions: 0,
                tax: 0,
                x: aa_net::wire::coord_to_long(100.0),
                y: aa_net::wire::coord_to_long(200.0),
                z: 10.0,
                name: "Scarecrow Garden".into(),
                allow_recover: true,
                sell_price: 0,
                sell_to_player_id: 0,
                sell_to_name: String::new(),
            },
            demolished: false,
            build_pay: None,
            tax: None,
            sale: None,
        }
    }

    fn unit(tl_id: u16, model_id: u32) -> aa_net::Unit {
        aa_net::Unit {
            obj_id: 50,
            name: "Scarecrow Garden".into(),
            kind: UnitKind::Housing { tl_id, template_id: 267, build_step: 0 },
            owner_name: "Owner".into(),
            pos: aa_net::Pos::new(100.0, 200.0, 10.0),
            rotation: [0.0; 3],
            scale: 1.0,
            level: 1,
            model_id,
            equipment: default(),
            model_params: default(),
            hp: 100,
            mp: 0,
            max_hp: 100,
            max_mp: 0,
            dead: false,
            race_gender: 0,
            movement: None,
            faction_id: 0,
            expedition_id: 0,
            appellation: 0,
            flags: 0,
            gm_modes: [0; 4],
            bounty_money: None,
            pvp_honor: 0,
            pvp_kills: 0,
            skills: vec![],
            passive_buffs: vec![],
            ability_exp: vec![],
            active_abilities: vec![],
            buffs: vec![],
            anim_action: 0,
            attached_to: None,
            house_posture: None,
            state_attachments: None,
        }
    }

    #[test]
    fn a_build_progress_model_updates_the_received_unit() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Units>();
        app.init_resource::<Houses>();
        app.init_resource::<Views>();
        app.add_systems(Update, sync_house_models);
        app.world_mut().resource_mut::<Units>().by_id.insert(50, unit(7, 989));
        app.world_mut().resource_mut::<Houses>().by_tl_id.insert(7, house(7, 989));
        app.update();
        assert_eq!(app.world().resource::<Units>().by_id[&50].model_id, 989, "nothing changed yet");

        app.world_mut().resource_mut::<Houses>().by_tl_id.get_mut(&7).unwrap().snapshot.model_id = 987;
        app.update();
        assert_eq!(app.world().resource::<Units>().by_id[&50].model_id, 987, "the received model follows SCHouseBuildProgress");
        assert_eq!(app.world().resource::<Views>().unit_entities(), 0);
    }

    #[test]
    fn the_house_created_answer_clears_the_pending_build() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<NetEvent>()
            .insert_resource(Placement { session: None, pending_house: Some(267), ..Default::default() })
            .add_systems(Update, close_created_build);
        let answer = Event::Housing(HousingEvent::MyHouse(house(7, 989).snapshot));
        app.world_mut().write_message(NetEvent(answer));
        app.update();
        assert_eq!(app.world().resource::<Placement>().pending_house, None);
    }

    #[test]
    fn the_smallest_polygon_wins_and_marks_its_edges() {
        let areas = HousingAreas { shapes: vec![
            HousingShape { id: 1, bounds: [0.0, 0.0, 100.0, 100.0], points: vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]] },
            HousingShape { id: 2, bounds: [10.0, 10.0, 30.0, 30.0], points: vec![[10.0, 10.0], [30.0, 10.0], [30.0, 30.0], [10.0, 30.0]] },
        ] };
        assert_eq!(areas.area_at(20.0, 20.0), vec![2, 1]);
        assert_eq!(areas.shape_at(20.0, 20.0).map(|s| s.id), Some(2));
        assert_eq!(areas.area_at(200.0, 20.0), Vec::<u32>::new());
    }
}
