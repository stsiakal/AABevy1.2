//! Transfers (carriages, ferries, airships, lifts): boarding at the first free passenger
//! attach point 2..=7 (`CSBoardingTransfer`), the server's type-5 pose and the client's route
//! files.
//!
//! The routes are client data: `game/worlds/<world>/level_design/zone/<zone>/client/transfer_path.xml`,
//! loaded by the original `TransferManager::LoadPaths` (`FUN_393539e0`). Each `<Transfer Name=".."
//! Type="..">` names a `transfer_paths.path_name`; `Type` is the `transfers.id`. The points are game
//! metres. The server moves the transfer and sends the pose; the client does not integrate it
//! (`FUN_393540a0` only refreshes the path data around the player).

use std::collections::BTreeMap;

use aa_net::model::UnitKind;
use aa_net::state::{Player, Target, Units};
use aa_net::transfer::{FIRST_PASSENGER_POINT, LAST_PASSENGER_POINT, TransferCommand};
use aa_net::{Command, NetCommand};
use bevy::prelude::*;

use super::data::Db;
use super::world_switch::CurrentWorld;

/// One named route of one transfer: the points and the per-leg wait times.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferRoute {
    /// `transfers.id` from the XML `Type`.
    pub transfer_id: u32,
    /// `transfer_paths.path_name` from the XML `Name`.
    pub name: String,
    /// Game metres (x east, y north, z up).
    pub points: Vec<Vec3>,
    /// `transfer_paths.wait_time_start` (s).
    pub wait_start: f32,
    /// `transfer_paths.wait_time_end` (s).
    pub wait_end: f32,
}

impl TransferRoute {
    /// The length of the route in metres.
    pub fn length(&self) -> f32 {
        self.points.windows(2).map(|p| p[0].distance(p[1])).sum()
    }
}

/// Every transfer route of the loaded world, by `transfers.id`.
#[derive(Resource, Default)]
pub struct TransferRoutes {
    by_transfer: BTreeMap<u32, Vec<TransferRoute>>,
}

impl TransferRoutes {
    /// Loads every zone's `transfer_path.xml` of `world`. `waits` gives the per-leg wait times of a
    /// `(transfers.id, path_name)` pair (the XML does not carry them).
    pub fn load(world: &str, waits: impl Fn(u32, &str) -> (f32, f32)) -> Self {
        let pak = aa_pak::Pak::global();
        let prefix = format!("worlds/{world}/level_design/zone/");
        let mut by_transfer: BTreeMap<u32, Vec<TransferRoute>> = BTreeMap::new();
        for file in pak.list(&prefix) {
            if !file.to_ascii_lowercase().ends_with("/client/transfer_path.xml") {
                continue;
            }
            let Ok(xml) = pak.read_string(file) else { continue };
            for route in parse_transfer_paths(&xml, &waits) {
                by_transfer.entry(route.transfer_id).or_default().push(route);
            }
        }
        Self { by_transfer }
    }

    /// The routes of every transfer, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &[TransferRoute])> {
        self.by_transfer.iter().map(|(id, routes)| (*id, routes.as_slice()))
    }

    pub fn transfer_count(&self) -> usize {
        self.by_transfer.len()
    }

    pub fn len(&self) -> usize {
        self.by_transfer.values().map(Vec::len).sum()
    }
}

/// Parses one `transfer_path.xml`: every `<Transfer>` with a `Type` and at least one `<Point>`.
pub fn parse_transfer_paths(xml: &str, waits: &impl Fn(u32, &str) -> (f32, f32)) -> Vec<TransferRoute> {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return Vec::new() };
    doc.descendants()
        .filter(|n| n.has_tag_name("Transfer"))
        .filter_map(|t| {
            let name = t.attribute("Name")?.to_string();
            let transfer_id: u32 = t.attribute("Type")?.trim().parse().ok()?;
            let points: Vec<Vec3> = t
                .descendants()
                .filter(|n| n.has_tag_name("Point"))
                .filter_map(|p| {
                    let v: Vec<f32> = p.attribute("Pos")?.split(',').filter_map(|c| c.trim().parse().ok()).collect();
                    let [x, y, z] = v[..] else { return None };
                    Some(Vec3::new(x, y, z))
                })
                .collect();
            if points.is_empty() {
                return None;
            }
            let (wait_start, wait_end) = waits(transfer_id, &name);
            Some(TransferRoute { transfer_id, name, points, wait_start, wait_end })
        })
        .collect()
}

/// The first free passenger point of a transfer (`FUN_393546b0`): 2..=7 not in `used`.
pub fn first_free_point(used: impl IntoIterator<Item = u8>) -> Option<u8> {
    let mut taken = [false; 8];
    for point in used {
        if (FIRST_PASSENGER_POINT..=LAST_PASSENGER_POINT).contains(&point) {
            taken[usize::from(point)] = true;
        }
    }
    (FIRST_PASSENGER_POINT..=LAST_PASSENGER_POINT).find(|p| !taken[usize::from(*p)])
}

/// Boards the targeted transfer at its first free passenger point with the original's interaction
/// key (`FUN_393565c0` after `FUN_39362f50`): the packet names the transfer's timeline id and the
/// attach point. A player who is already seated sends nothing; the server answers with the attach.
pub fn transfer_seat(
    time: Res<Time>,
    actions: Res<crate::settings::Actions>,
    player: Res<Player>,
    units: Res<Units>,
    target: Option<Res<Target>>,
    mut out: MessageWriter<NetCommand>,
    mut last: Local<f32>,
) {
    let Some(spawn) = player.spawn.as_ref() else { return };
    if spawn.unit.attached_to.is_some() {
        return;
    }
    let now = time.elapsed_secs();
    if !actions.just_pressed("do_interaction_1", 0) || now - *last <= 0.5 {
        return;
    }
    let Some(transfer) = target
        .as_deref()
        .and_then(|t| t.unit(&units))
        .filter(|u| matches!(u.kind, UnitKind::Transfer { .. }) && !u.dead)
    else {
        return;
    };
    let UnitKind::Transfer { tl_id, .. } = transfer.kind else { return };
    let used = units.by_id.values().filter_map(|u| u.attached_to).filter(|a| a.obj_id == transfer.obj_id).map(|a| a.point);
    let Some(ap) = first_free_point(used) else { return };
    out.write(NetCommand(Command::Transfer(TransferCommand::Board { tl_id, ap })));
    *last = now;
}

/// Loads the routes of the world once (again after a world switch).
fn load_routes(
    mut routes: ResMut<TransferRoutes>,
    current: Res<CurrentWorld>,
    db: Option<Res<Db>>,
    mut loaded: Local<Option<String>>,
) {
    if !aa_cry::pak_available() || loaded.as_deref() == Some(current.0.as_str()) {
        return;
    }
    let Some(db) = db.as_deref() else { return };
    let waits = |id, name: &str| {
        db.transfer_waits(id, name).unwrap_or_else(|| {
            let wait = db.transfer_wait(id).unwrap_or(0.0);
            (wait, wait)
        })
    };
    *routes = TransferRoutes::load(&current.0, waits);
    info!("transfer routes: {} paths for {} transfers in {}", routes.len(), routes.transfer_count(), current.0);
    for (id, paths) in routes.iter() {
        let named: Vec<_> = paths
            .iter()
            .map(|p| format!("{} (Type {}) {:.0} m, waits {}/{} s", p.name, p.transfer_id, p.length(), p.wait_start, p.wait_end))
            .collect();
        debug!("transfer {id}: {}", named.join(", "));
    }
    *loaded = Some(current.0.clone());
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TransferRoutes>()
        .add_systems(Update, load_routes.run_if(super::world_screens));
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa_net::model::{Equipment, ModelParams, Unit};
    use aa_net::PlayerSpawn;
    use aa_net::Pos;
    use bevy::ecs::message::Messages;

    const XML: &str = r#"<Objects>
 <Transfer Name="mari_tram_01" Type="3" cellX="0" cellY="1">
  <Points>
   <Point Pos="10,20,1"/>
   <Point Pos="10,30,2"/>
  </Points>
 </Transfer>
 <Transfer Name="unresolved" Type="0" cellX="0" cellY="0">
  <Points><Point Pos="1,2,3"/></Points>
 </Transfer>
 <Transfer Name="empty" Type="9"><Points/></Transfer>
</Objects>"#;

    #[test]
    fn the_xml_type_is_the_transfer_id_and_the_points_are_game_metres() {
        let routes = parse_transfer_paths(XML, &|id, name| {
            if (id, name) == (3, "mari_tram_01") { (15.0, 20.0) } else { (0.0, 0.0) }
        });
        assert_eq!(routes.len(), 2, "the empty block is skipped, Type 0 is kept");
        assert_eq!((routes[0].transfer_id, routes[0].name.as_str()), (3, "mari_tram_01"));
        assert_eq!(routes[0].points, [Vec3::new(10.0, 20.0, 1.0), Vec3::new(10.0, 30.0, 2.0)]);
        assert!((routes[0].length() - (1.0f32 + 100.0).sqrt()).abs() < 1e-4);
        assert_eq!((routes[0].wait_start, routes[0].wait_end), (15.0, 20.0));
        assert_eq!(routes[1].transfer_id, 0);
        assert_eq!(parse_transfer_paths("<Objects/>", &|_, _| (0.0, 0.0)), []);
        assert_eq!(parse_transfer_paths("not xml", &|_, _| (0.0, 0.0)), []);
    }

    #[test]
    fn the_first_free_passenger_point_is_two_to_seven() {
        assert_eq!(first_free_point([]), Some(2));
        assert_eq!(first_free_point([2]), Some(3));
        assert_eq!(first_free_point([2, 4, 6]), Some(3));
        assert_eq!(first_free_point([2, 3, 4, 5, 6]), Some(7));
        assert_eq!(first_free_point(2..=7), None);
        // points outside the passenger range (a driver at 1, a child binding at 30) do not take one
        assert_eq!(first_free_point([0, 1, 8, 30]), Some(2));
    }

    fn unit(obj_id: u32, kind: UnitKind) -> Unit {
        Unit {
            obj_id,
            name: format!("u{obj_id}"),
            kind,
            owner_name: String::new(),
            pos: Pos::new(0.0, 0.0, 0.0),
            rotation: [0.0; 3],
            scale: 1.0,
            level: 1,
            model_id: 654,
            equipment: Equipment::default(),
            model_params: ModelParams::default(),
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
            skills: Vec::new(),
            passive_buffs: Vec::new(),
            ability_exp: Vec::new(),
            active_abilities: Vec::new(),
            buffs: Vec::new(),
            anim_action: 0,
            house_posture: None,
            state_attachments: None,
            doodad_bond: None,
            attached_to: None,
        }
    }

    fn player() -> PlayerSpawn {
        let mut state = aa_net::model::CharacterState::default();
        state.character.id = 47;
        PlayerSpawn { obj_id: 100, state, unit: unit(100, UnitKind::Character { char_id: 47 }) }
    }

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Player { spawn: Some(player()) })
            .insert_resource(crate::settings::Actions::default())
            .add_message::<NetCommand>()
            .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs(1)))
            .add_systems(Update, transfer_seat);
        let mut units = Units::default();
        units.by_id.insert(500, unit(500, UnitKind::Transfer { tl_id: 7, template_id: 6 }));
        app.insert_resource(units);
        let mut target = Target::default();
        target.obj_id = Some(500);
        app.insert_resource(target);
        // the interaction debounce counts from the start of the app
        for _ in 0..3 {
            app.update();
        }
        app
    }

    fn press(app: &mut App) -> Vec<Command> {
        // let the interaction debounce (0.5 s) expire before every press
        for _ in 0..2 {
            app.update();
        }
        app.world_mut().resource_mut::<crate::settings::Actions>().tap_for_test("do_interaction_1");
        app.update();
        app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect()
    }

    #[test]
    fn the_interaction_key_boards_the_targeted_transfer_at_the_first_free_point() {
        let mut app = app();
        assert!(matches!(
            press(&mut app).as_slice(),
            [Command::Transfer(TransferCommand::Board { tl_id: 7, ap: 2 })]
        ));
        // somebody at point 2: the next free point
        let mut rider = unit(700, UnitKind::Character { char_id: 5 });
        rider.attached_to = Some(aa_net::model::Attachment { obj_id: 500, point: 2 });
        app.world_mut().resource_mut::<Units>().by_id.insert(700, rider);
        app.world_mut().resource_mut::<Player>().spawn.as_mut().unwrap().unit.attached_to = None;
        assert!(matches!(
            press(&mut app).as_slice(),
            [Command::Transfer(TransferCommand::Board { tl_id: 7, ap: 3 })]
        ));
        // every seat taken: no packet
        for point in 3..=7 {
            let mut rider = unit(700 + u32::from(point), UnitKind::Character { char_id: 5 });
            rider.attached_to = Some(aa_net::model::Attachment { obj_id: 500, point });
            app.world_mut().resource_mut::<Units>().by_id.insert(rider.obj_id, rider);
        }
        assert!(press(&mut app).is_empty());
    }

    #[test]
    fn a_seated_player_or_a_target_that_is_not_a_transfer_sends_nothing() {
        let mut app = app();
        app.world_mut().resource_mut::<Player>().spawn.as_mut().unwrap().unit.attached_to =
            Some(aa_net::model::Attachment { obj_id: 500, point: 2 });
        assert!(press(&mut app).is_empty());
        app.world_mut().resource_mut::<Player>().spawn.as_mut().unwrap().unit.attached_to = None;
        app.world_mut().resource_mut::<Units>().by_id.get_mut(&500).unwrap().kind =
            UnitKind::Slave { id: 9, tl_id: 7, template_id: 60, summoner_id: 47 };
        assert!(press(&mut app).is_empty());
    }

    /// The 1.2 routes: `mari_tram_01` is transfer 3 and `lily_solzreed_in1` transfer 6.
    #[test]
    fn the_client_route_files_resolve_the_transfer_ids_of_the_database() {
        if !aa_cry::pak_available() {
            return;
        }
        let Some(db) = Db::open() else { return };
        let waits = |id, name: &str| db.transfer_waits(id, name).unwrap_or((0.0, 0.0));
        let routes = TransferRoutes::load(super::super::WORLD, waits);
        assert!(routes.transfer_count() > 5, "{} transfers", routes.transfer_count());
        let tram = routes.by_transfer.get(&3).expect("the Marianople tram");
        let route = tram.iter().find(|r| r.name == "mari_tram_01").expect("the Marianople tram route");
        assert_eq!((route.transfer_id, route.wait_end), (3, 15.0));
        assert!(route.points.len() > 20 && (route.length() - 344.0).abs() < 2.0, "{} points, {} m", route.points.len(), route.length());
        let solzreed = routes.by_transfer.get(&6).expect("the Solzreed carriage");
        assert!(solzreed.iter().any(|r| r.name == "lily_solzreed_in1"), "{:?}", solzreed.iter().map(|r| &r.name).collect::<Vec<_>>());
    }
}
