//! Which of an NPC's functions the player wants, and the priest blessing service.
//!
//! The server answers `CSStartInteraction` with `SCNpcInteractionSkillList`: the NPC's
//! functions as skill ids. The original client shows the choices in the NPC interaction
//! window (`npc_interaction_view.alb`): the greeting (`locale.npcInteraction.baseText`) and
//! one button per function. The quest actions come from `npc_interaction.alb`'s
//! `NPC_INTERACTION_START("quest", "talk" | "complete" | "start", npcId)` and the services
//! from its `"store"`, `"bank"`, `"blacksmith"`, `"ability_change"`, `"priest"` and
//! `"stabler"` branches. A single choice with no quest is opened at once, like the original.

use aa_net::interaction::InteractEvent;
use aa_net::shop::ShopCommand;
use aa_net::state::{Player, Units};
use aa_net::{Command, Event, NetCommand, NetEvent};
use aa_ui::UiRuntime;
use bevy::prelude::*;
use mlua::{Lua, Table, Value};
use rusqlite::Connection;

use super::api::{HudBridge, HudData, Shared};
use super::data::{any_int, GameData};
use crate::Screen;

/// How far the player reaches an NPC (the original's interaction range is the talk range).
const SERVICE_RANGE: f32 = super::dialog::TALK_RANGE;

/// `priest_buffs`: the blessings a priest sells (`CSBuyPriestBuff`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PriestBuff {
    /// `priest_buffs.id` (the wire's `priest_buff_id`).
    pub id: u32,
    pub buff_id: u32,
    pub cost: i32,
}

#[derive(Default)]
pub(super) struct Tables {
    pub priest: Vec<PriestBuff>,
}

impl Tables {
    pub(super) fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut priest = Vec::new();
        let mut q = conn.prepare("SELECT id,buff_id,cost FROM priest_buffs ORDER BY id")?;
        let mut rows = q.query([])?;
        while let Some(r) = rows.next()? {
            priest.push(PriestBuff {
                id: any_int(r, 0)?.max(0) as u32,
                buff_id: any_int(r, 1)?.max(0) as u32,
                cost: any_int(r, 2)?.clamp(0, i64::from(i32::MAX)) as i32,
            });
        }
        Ok(Self { priest })
    }
}

/// A function the server advertised (the supported subset of the 1.2 skills).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Service {
    Store,
    Bank,
    Repair,
    Ability,
    Priest,
    Stabler,
    LookConverter,
    Auction,
}

impl Service {
    fn from_skill(skill: u32) -> Option<Self> {
        Some(match skill {
            12087 => Self::Store,
            13238 => Self::Bank,
            12086 => Self::Repair,
            12082 => Self::Ability,
            12084 => Self::Priest,
            12195 => Self::Stabler,
            23478 => Self::LookConverter,
            12083 => Self::Auction,
            _ => return None,
        })
    }

    fn skill(self) -> u32 {
        match self {
            Self::Store => 12087,
            Self::Bank => 13238,
            Self::Repair => 12086,
            Self::Ability => 12082,
            Self::Priest => 12084,
            Self::Stabler => 12195,
            Self::LookConverter => 23478,
            Self::Auction => 12083,
        }
    }

    /// The name `npc_interaction.alb` gives this service in `NPC_INTERACTION_START`
    /// (F9, lines 269-304: `"store"`, `"bank"`, `"blacksmith"`, `"ability_change"`, ...).
    fn key(self) -> &'static str {
        match self {
            Self::Store => "store",
            Self::Bank => "bank",
            // The original maps both repairman and blacksmith to the blacksmith repair window;
            // `repairman` is an undefined dead branch in the 1.2 script.
            Self::Repair => "blacksmith",
            Self::Ability => "ability_change",
            Self::Priest => "priest",
            Self::Stabler => "stabler",
            Self::LookConverter => "look_converter",
            Self::Auction => "auction",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum What {
    Service(Service),
    /// The second argument of `NPC_INTERACTION_START("quest", ...)`.
    Quest(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Choice {
    what: What,
    /// The skill's authored name for a service; empty for quests (Lua reads its locale).
    label: String,
}

#[derive(Default, Debug)]
pub(super) struct View {
    /// The NPC whose interaction list this is (object id).
    npc: Option<u32>,
    choices: Vec<Choice>,
    chosen: Option<usize>,
    cancel: bool,
    visible: bool,
    buys: Vec<(u32, u32)>,
}

impl View {
    /// Whether this frame's `SCNpcInteractionSkillList` belongs to the NPC service router, so
    /// the store/stablehand modules must not open their own window from the same packet.
    pub(super) fn claimed(&self, npc: u32) -> bool {
        self.npc == Some(npc)
    }
}

/// `npc_interaction.alb` F8 (lines 248-262) decides "talk", then "complete", then "start".
fn quest_choice(d: &HudData) -> Option<&'static str> {
    if !d.dialog.talk.is_empty() {
        Some("talk")
    } else if !d.dialog.complete.is_empty() {
        Some("complete")
    } else if !d.dialog.start.is_empty() {
        Some("start")
    } else {
        None
    }
}

fn service_label(game: Option<&GameData>, service: Service) -> String {
    game.and_then(|g| g.skills.get(&service.skill()))
        .map(|s| s.named.name.clone())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| service.key().to_string())
}

/// Opens the original windows of a service: the shop/bank context, the stablehand context, the
/// auction house state, or nothing (their windows are driven only by `NPC_INTERACTION_START`).
fn open(data: &mut HudData, service: Service, rt: &UiRuntime) -> Option<&'static str> {
    let npc = data.dialog.npc;
    match service {
        Service::Store => {
            let npc = npc?;
            super::store::open_service(data, npc, false);
        }
        Service::Bank => {
            let npc = npc?;
            super::store::open_service(data, npc, true);
        }
        Service::Stabler => {
            let npc = npc?;
            super::pets::open_service(data, npc);
        }
        Service::Auction => {
            if let Some(npc) = npc {
                super::auction::open_service(rt, npc);
            }
        }
        Service::Repair | Service::Ability | Service::Priest | Service::LookConverter => {}
    }
    Some(service.key())
}

pub(super) fn install(lua: &Lua, bridge: &Shared) -> mlua::Result<()> {
    let hud: Table = lua.globals().get("HUD")?;
    let b = bridge.clone();
    hud.set(
        "npc_choice",
        lua.create_function(move |lua, ()| {
            let d = b.borrow();
            let out = lua.create_table()?;
            for choice in &d.services.choices {
                let row = lua.create_table()?;
                match &choice.what {
                    What::Service(service) => {
                        row.set("kind", "service")?;
                        row.set("key", service.key())?;
                        row.set("label", choice.label.as_str())?;
                    }
                    What::Quest(what) => {
                        row.set("kind", "quest")?;
                        row.set("what", *what)?;
                    }
                }
                out.raw_push(row)?;
            }
            Ok(out)
        })?,
    )?;
    let b = bridge.clone();
    hud.set(
        "npc_choose",
        lua.create_function(move |_, index: usize| {
            let mut d = b.borrow_mut();
            if index > 0 && index <= d.services.choices.len() {
                d.services.chosen = Some(index - 1);
            }
            Ok(())
        })?,
    )?;
    let b = bridge.clone();
    hud.set(
        "npc_choice_close",
        lua.create_function(move |_, ()| {
            b.borrow_mut().services.cancel = true;
            Ok(())
        })?,
    )?;

    let x2priest: Table = lua.globals().get("X2Priest")?;
    let b = bridge.clone();
    x2priest.set(
        "GetPriestBuffCount",
        lua.create_function(move |_, _: Value| Ok(b.borrow().game.get().map_or(0, |g| g.services.priest.len())))?,
    )?;
    let b = bridge.clone();
    x2priest.set(
        "GetPriestBuff",
        lua.create_function(move |lua, (_self, index): (Value, i64)| {
            let d = b.borrow();
            let Some(game) = d.game.get() else { return Ok(None) };
            let Some(buff) = index.checked_sub(1).and_then(|i| usize::try_from(i).ok()).and_then(|i| game.services.priest.get(i)) else {
                return Ok(None);
            };
            let t = lua.create_table()?;
            t.set("id", buff.id)?;
            t.set("buffId", buff.buff_id)?;
            t.set("cost", f64::from(buff.cost))?;
            if let Some(info) = game.buffs.get(&buff.buff_id) {
                t.set("name", info.named.name.as_str())?;
                t.set("desc", info.desc.as_str())?;
            }
            Ok(Some(t))
        })?,
    )?;
    let b = bridge.clone();
    x2priest.set(
        "BuyPriestBuff",
        lua.create_function(move |_, (_self, index): (Value, i64)| {
            let mut d = b.borrow_mut();
            let Some(npc) = d.dialog.npc.map(|n| n.0) else { return Ok(()) };
            let Some(buff) = index
                .checked_sub(1)
                .and_then(|i| usize::try_from(i).ok())
                .and_then(|i| d.game.get().and_then(|g| g.services.priest.get(i)).cloned())
            else {
                return Ok(());
            };
            d.services.buys.push((buff.id, npc));
            Ok(())
        })?,
    )?;
    x2priest.set(
        "GetCostForRecoverExp",
        lua.create_function(move |_, _: Value| Ok(None::<f64>))?,
    )?;
    x2priest.set("RecoverExp", lua.create_function(move |_, _: Value| Ok(()))?)?;
    lua.load(include_str!("services.lua")).set_name("@services.lua").exec()
}

pub(super) fn update(
    mut events: MessageReader<NetEvent>,
    rt: Option<NonSend<UiRuntime>>,
    bridge: NonSend<HudBridge>,
    player: Res<Player>,
    units: Res<Units>,
    mut net: MessageWriter<NetCommand>,
) {
    let Some(rt) = rt else {
        events.clear();
        return;
    };
    let own = player.spawn.as_ref().map_or(0, |s| s.obj_id);
    let mut show = false;
    let mut hide = false;
    let mut end = false;
    let mut fire: Option<(&'static str, &'static str)> = None;
    let buys;
    {
        let mut d = bridge.0.borrow_mut();
        for NetEvent(event) in events.read() {
            let Event::Interaction(InteractEvent::SkillList {
                npc_obj_id,
                obj_id,
                skills,
                interactable,
                ..
            }) = event
            else {
                continue;
            };
            let Some((npc_obj, _)) = d.dialog.npc.filter(|n| n.0 == *npc_obj_id) else { continue };
            if *obj_id != own || !*interactable || own == 0 {
                continue;
            }
            // Every service module sees this packet; claim it so only this router opens a window.
            if d.services.visible {
                hide = true;
            }
            d.services.npc = Some(npc_obj);
            d.services.choices.clear();
            d.services.chosen = None;
            d.services.visible = false;

            let mut services = Vec::new();
            for skill in skills {
                if let Some(service) = Service::from_skill(*skill)
                    && !services.contains(&service)
                {
                    services.push(service);
                }
            }
            let quest = quest_choice(&d);
            if services.is_empty() {
                if let Some(what) = quest {
                    // `npc_interaction.alb` F8: the preferred quest action (talk, complete, start).
                    fire = Some(("quest", what));
                } else {
                    end = true;
                }
            } else if services.len() == 1 && quest.is_none() {
                // The original opens a lone service window without a menu.
                if let Some(service) = services.first()
                    && let Some(key) = open(&mut d, *service, &rt)
                {
                    fire = Some((key, ""));
                }
            } else {
                if let Some(what) = quest {
                    d.services.choices.push(Choice { what: What::Quest(what), label: String::new() });
                }
                for service in services {
                    let label = service_label(d.game.get(), service);
                    d.services.choices.push(Choice { what: What::Service(service), label });
                }
                show = true;
            }
        }

        if let Some(index) = d.services.chosen.take()
            && let Some(choice) = d.services.choices.get(index).cloned()
        {
            match choice.what {
                What::Service(service) => {
                    if let Some(key) = open(&mut d, service, &rt) {
                        fire = Some((key, ""));
                    }
                }
                What::Quest(what) => fire = Some(("quest", what)),
            }
            d.services.choices.clear();
            d.services.visible = false;
            hide = true;
        }
        if d.services.cancel {
            d.services.cancel = false;
            d.services.choices.clear();
            d.services.visible = false;
            hide = true;
            end = true;
        }

        // The greeting/choice window belongs to the current NPC talk.
        let stale = d.services.npc.is_some_and(|npc| {
            let talking = d.dialog.npc.is_some_and(|(id, _)| id == npc);
            let near = units.get(npc).is_some_and(|unit| d.pos.distance(Vec3::new(unit.pos.x, unit.pos.y, unit.pos.z)) <= SERVICE_RANGE);
            player.spawn.is_none() || !talking || !near
        });
        if stale {
            d.services.npc = None;
            d.services.choices.clear();
            if d.services.visible {
                d.services.visible = false;
                hide = true;
            }
            if d.dialog.npc.is_some() {
                end = true;
            }
        }
        if show {
            d.services.visible = true;
        }
        buys = std::mem::take(&mut d.services.buys);
        if end {
            super::dialog::end_talk(&mut d, &mut net);
        }
    }
    if hide {
        let _ = rt.exec("@npc_service_choice", "HideNpcServiceChoice()");
    }
    if show && let Err(error) = rt.exec("@npc_service_choice", "ShowNpcServiceChoice()") {
        warn!("hud: npc service choice: {error}");
    }
    if let Some((kind, what)) = fire {
        let npc = bridge.0.borrow().dialog.npc.map_or(0, |n| n.0);
        debug!("hud: NPC_INTERACTION_START({kind}, {what}, {npc})");
        rt.fire_event("NPC_INTERACTION_START", (kind, what, npc.to_string()));
    }
    let current = bridge.0.borrow().dialog.npc.map(|n| n.0);
    for (buff, npc) in buys {
        if current == Some(npc) {
            net.write(NetCommand(Command::Shop(ShopCommand::PriestBuff { npc_obj_id: npc, priest_buff_id: buff })));
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        update
            .after(super::sync_hud)
            .after(super::dialog::handle_dialog)
            .before(aa_ui::UiFrame)
            .run_if(super::hud_up)
            .run_if(in_state(Screen::InGame)),
    )
    .add_systems(OnExit(Screen::InGame), |bridge: NonSend<HudBridge>| {
        bridge.0.borrow_mut().services = default()
    });
}

/// Dev fixture: the original NPC greeting/choice window over a quest + two functions.
pub(crate) fn run() {
    let (width, height) = aa_dev::DevArgs::from_env().size;
    let (rt, bridge, path, inventory, units, _npc) = super::pets::fixture::runtime(width as f32, height as f32).unwrap();
    bridge.borrow_mut().dialog.start = vec![1];
    super::api::push_changes(&rt, &bridge);
    let mut app = App::new();
    app.add_plugins((aa_dev::default_plugins("aa-client: original NPC function menu"), aa_dev::DevPlugin))
        .add_plugins(aa_ui::UiPlugin { config: Some(crate::login::host::ui_config()) })
        .insert_non_send(rt)
        .insert_non_send(HudBridge(bridge))
        .insert_resource(inventory)
        .insert_resource(units)
        .insert_resource(Player { spawn: Some(super::expansion::fixture::spawn()) })
        .add_message::<NetEvent>()
        .add_message::<NetCommand>()
        .add_systems(Startup, physical_pixels)
        .add_systems(Update, update);
    app.world_mut().write_message(NetEvent(Event::Interaction(InteractEvent::SkillList {
        npc_obj_id: 200,
        obj_id: 100,
        extra_info: 0,
        pick_id: 0,
        mouse_button: 2,
        skills: vec![12087, 12086],
        interactable: true,
        modifier_keys: 0,
    })));
    app.run();
    if path.exists() {
        std::fs::remove_file(path).unwrap();
    }
}

fn physical_pixels(mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    let (width, height) = aa_dev::DevArgs::from_env().size;
    for mut window in &mut windows {
        window.resolution.set_scale_factor_override(Some(1.0));
        window.resolution.set_physical_resolution(width, height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aa_net::interaction::Interact;

    fn skill_list(npc_obj_id: u32, obj_id: u32, skills: Vec<u32>) -> Event {
        Event::Interaction(InteractEvent::SkillList {
            npc_obj_id,
            obj_id,
            extra_info: 0,
            pick_id: 0,
            mouse_button: 2,
            skills,
            interactable: true,
            modifier_keys: 0,
        })
    }

    fn setup(
        skills: &[u32],
        quests: usize,
    ) -> Result<(App, Shared), Box<dyn std::error::Error>> {
        let mut rt = UiRuntime::new("en_us", 1920., 1080.)?;
        let bridge = std::rc::Rc::new(std::cell::RefCell::new(HudData::default()));
        bridge.borrow_mut().snapshot.unit_id = "100".into();
        bridge.borrow_mut().dialog.npc = Some((200, 8000025));
        bridge.borrow_mut().dialog.start = (1..=quests as u32).collect();
        let mut game = GameData::default();
        game.services.priest.push(PriestBuff { id: 1, buff_id: 239, cost: 100 });
        assert!(bridge.borrow_mut().game.0.set(game).is_ok());
        super::super::api::install(&mut rt, &bridge)?;
        rt.lua
            .load("NPC_EVENTS={};UIParent:SetEventHandler('NPC_INTERACTION_START',function(kind,what,id) NPC_EVENTS[#NPC_EVENTS+1]={kind=tostring(kind),what=tostring(what),id=tostring(id)} end)")
            .exec()?;
        let mut unit = super::super::expansion::fixture::spawn().unit;
        unit.obj_id = 200;
        let mut units = Units::default();
        units.by_id.insert(200, unit);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_non_send(rt)
            .insert_non_send(HudBridge(bridge.clone()))
            .insert_resource(Player {
                spawn: Some(super::super::expansion::fixture::spawn()),
            })
            .insert_resource(units)
            .add_message::<NetEvent>()
            .add_message::<NetCommand>()
            .add_systems(Update, update);
        app.world_mut().write_message(NetEvent(skill_list(200, 100, skills.to_vec())));
        app.update();
        Ok((app, bridge))
    }

    fn drain(app: &mut App) -> Vec<Command> {
        app.world_mut()
            .resource_mut::<Messages<NetCommand>>()
            .drain()
            .map(|v| v.0)
            .collect()
    }

    #[test]
    fn server_skills_map_to_the_original_npc_interaction_windows() {
        for (skill, key) in [
            (12087, "store"),
            (13238, "bank"),
            (12086, "blacksmith"),
            (12082, "ability_change"),
            (12084, "priest"),
            (12195, "stabler"),
            (23478, "look_converter"),
            (12083, "auction"),
        ] {
            let service = Service::from_skill(skill).unwrap();
            assert_eq!(service.key(), key);
            assert_eq!(service.skill(), skill);
        }
        assert_eq!(Service::from_skill(0), None, "unknown skills are not services");
    }

    #[test]
    fn a_lone_function_opens_its_window_without_a_menu() -> Result<(), Box<dyn std::error::Error>> {
        let (app, bridge) = setup(&[12087], 0)?;
        assert!(bridge.borrow().services.choices.is_empty());
        assert_eq!(bridge.borrow().store.npc.map(|n| n.0), Some(200));
        let events = app.world().non_send::<UiRuntime>().lua.load("return #NPC_EVENTS==1 and NPC_EVENTS[1].kind=='store' and NPC_EVENTS[1].what==''").eval::<bool>()?;
        assert!(events);
        Ok(())
    }

    #[test]
    fn quest_and_functions_share_the_original_choice_menu() -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, bridge) = setup(&[12087, 12086], 1)?;
        {
            let data = bridge.borrow();
            assert_eq!(data.services.choices.len(), 3, "one quest action plus two functions");
            assert!(matches!(data.services.choices[0].what, What::Quest("start")));
            assert!(matches!(data.services.choices[1].what, What::Service(Service::Store)));
            assert!(matches!(data.services.choices[2].what, What::Service(Service::Repair)));
            assert!(data.store.npc.is_none(), "the menu itself opens no service");
        }
        app.world()
            .non_send::<UiRuntime>()
            .lua
            .load("assert(#HUD.npc_choice()==3); HUD.npc_choose(2)")
            .exec()?;
        app.update();
        assert_eq!(bridge.borrow().store.npc.map(|n| n.0), Some(200), "the store context follows the chosen row");
        assert!(!bridge.borrow().store.is_bank());
        assert!(bridge.borrow().services.choices.is_empty());
        let fired = app
            .world()
            .non_send::<UiRuntime>()
            .lua
            .load("return NPC_EVENTS[#NPC_EVENTS].kind=='store'")
            .eval::<bool>()?;
        assert!(fired);
        Ok(())
    }

    #[test]
    fn choice_routes_quest_bank_stable_and_ability_actions() -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, bridge) = setup(&[12082, 13238, 12195], 0)?;
        {
            let data = bridge.borrow();
            assert_eq!(data.services.choices.len(), 3);
        }
        app.world().non_send::<UiRuntime>().lua.load("HUD.npc_choose(2)").exec()?;
        app.update();
        assert!(bridge.borrow().store.is_bank() && bridge.borrow().store.npc.is_some());
        assert!(bridge.borrow().services.choices.is_empty());
        Ok(())
    }

    #[test]
    fn a_pure_quest_npc_opens_the_preferred_quest_action_without_a_menu() -> Result<(), Box<dyn std::error::Error>> {
        let (app, bridge) = setup(&[0], 1)?;
        assert!(bridge.borrow().services.choices.is_empty());
        let fired = app
            .world()
            .non_send::<UiRuntime>()
            .lua
            .load("return #NPC_EVENTS==1 and NPC_EVENTS[1].kind=='quest' and NPC_EVENTS[1].what=='start'")
            .eval::<bool>()?;
        assert!(fired);
        Ok(())
    }

    #[test]
    fn an_empty_list_with_no_quest_ends_the_talk() -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, bridge) = setup(&[0], 0)?;
        assert!(bridge.borrow().dialog.npc.is_none());
        assert!(matches!(drain(&mut app).as_slice(), [Command::Interact(Interact::NpcEnd { obj_id: 200 })]));
        Ok(())
    }

    #[test]
    fn priest_blessing_quote_and_purchase_follow_the_received_table() -> Result<(), Box<dyn std::error::Error>> {
        let (mut app, bridge) = setup(&[12084], 0)?;
        let rt = app.world().non_send::<UiRuntime>();
        let (count, cost) = rt
            .lua
            .load("return X2Priest:GetPriestBuffCount(), X2Priest:GetPriestBuff(1).cost")
            .eval::<(usize, f64)>()?;
        assert!(count >= 1);
        assert_eq!(cost, 100.0);
        rt.lua.load("X2Priest:BuyPriestBuff(1)").exec()?;
        app.update();
        let commands = drain(&mut app);
        assert!(matches!(
            commands.as_slice(),
            [Command::Shop(ShopCommand::PriestBuff { npc_obj_id: 200, priest_buff_id: 1 })]
        ));
        assert_eq!(bridge.borrow().services.buys.len(), 0, "the request is drained once");
        Ok(())
    }

    #[test]
    #[ignore = "loads original world-stage assets; run in an isolated fixture process"]
    fn original_windows_open_from_a_multi_service_interaction_list() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, bridge, path, inventory, units, _npc) =
        super::super::pets::fixture::runtime(1920.0, 1080.0)?;
    bridge.borrow_mut().snapshot.abilities = vec![1, 2, 3];
    bridge.borrow_mut().snapshot.ability_exp = [1_000; 11];
    super::super::api::push_changes(&rt, &bridge);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_non_send(rt)
        .insert_non_send(HudBridge(bridge.clone()))
        .insert_resource(inventory)
        .insert_resource(units)
        .insert_resource(Player {
            spawn: Some(super::super::expansion::fixture::spawn()),
        })
        .add_message::<NetEvent>()
        .add_message::<NetCommand>()
        .add_systems(Update, update);
    let send = |app: &mut App, skills: &[u32]| {
        app.world_mut().write_message(NetEvent(skill_list(200, 100, skills.to_vec())));
        app.update();
    };
    let lua = |app: &App, code: &str| app.world().non_send::<UiRuntime>().lua.load(code).exec();
    // A lone banker function opens the original warehouse window.
    send(&mut app, &[13238]);
    let bank = app.world().non_send::<UiRuntime>().lua.load("return IsActivatedBank()").eval::<bool>()?;
    assert!(bank, "bank window not active");
    assert!(bridge.borrow().store.is_bank());
    // A lone repairman function goes straight to the original blacksmith/repair window.
    send(&mut app, &[12086]);
    let (has, visible, choice) = app
        .world()
        .non_send::<UiRuntime>()
        .lua
        .load("return repairWindow~=nil, repairWindow~=nil and repairWindow:IsVisible(), npcInteraction.window:IsVisible()")
        .eval::<(bool, bool, bool)>()?;
    assert!(has && visible && !choice);
    // Ability change and the priest blessing use their authored windows.
    send(&mut app, &[12082]);
    let (has, visible) = app
        .world()
        .non_send::<UiRuntime>()
        .lua
        .load("return abilityChangeFrame~=nil, abilityChangeFrame~=nil and abilityChangeFrame:IsVisible()")
        .eval::<(bool, bool)>()?;
    assert!(has && visible);
    lua(
        &app,
        r#"
        local original=X2DialogManager.RequestDefaultDialog
        X2DialogManager.RequestDefaultDialog=function(self,handler,owner)
          local w=original(self,handler,owner)
          if owner==GetDivineBlessingMessageBoxHandleWindow():GetId() then BLESSING=w end
          return w
        end
    "#,
    )?;
    send(&mut app, &[12084]);
    lua(&app, "assert(BLESSING and BLESSING:IsVisible()); assert(X2Priest:GetPriestBuff(1).cost==100)")?;
    super::super::expansion::fixture::click(app.world().non_send::<UiRuntime>(), "BLESSING.btnOk")?;
    app.update();
    let commands = drain(&mut app);
    assert!(matches!(
        commands.as_slice(),
        [Command::Shop(ShopCommand::PriestBuff { npc_obj_id: 200, priest_buff_id: 1 })]
    ));
    // Quests and two services share the original greeting window; the chosen row opens the shop.
    bridge.borrow_mut().dialog.start = vec![1];
    send(&mut app, &[12087, 12086]);
    lua(
        &app,
        "assert(npcInteraction.window:IsVisible()); assert(#HUD.npc_choice()==3); assert(npcInteraction.interactionButtons[1]:IsVisible())",
    )?;
    super::super::expansion::fixture::click(app.world().non_send::<UiRuntime>(), "npcInteraction.interactionButtons[2]")?;
    app.update();
    lua(&app, "assert(IsActivatedStore())")?;
    assert!(!bridge.borrow().services.visible);
    drop(app);
    drop(bridge);
    std::fs::remove_file(path)?;
    Ok(())
    }

    /// The greeting window is wired once, but must still open on every talk (the second talk used
    /// to return from `ShowNpcServiceChoice` because `wire` reported the already-wired window as a
    /// failure).
    #[test]
    fn every_talk_shows_the_function_menu() -> Result<(), Box<dyn std::error::Error>> {
        let lua = Lua::new();
        lua.load(
            r#"
            HUD = { npc_choice = function()
              return { { kind = "service", key = "store", label = "Shop" } }
            end }
            locale = { npcInteraction = { baseText = "Greetings!", interactions = { "", "", "", "Shop" } } }
            npcInteraction = {
              window = { shown = false, Show = function(self, on) self.shown = on end, SetHandler = function() end },
              interactionButtons = {
                { shown = false, Show = function(self, on) self.shown = on end, SetText = function() end, SetHandler = function() end },
              },
              message = { SetText = function() end, Show = function() end },
              closeButton = { SetHandler = function() end },
            }
            "#,
        )
        .exec()?;
        lua.load(include_str!("services.lua")).exec()?;
        let visible = |lua: &Lua| lua.load("return npcInteraction.window.shown").eval::<bool>();
        lua.load("ShowNpcServiceChoice()").exec()?;
        assert!(visible(&lua)?, "the first talk shows the menu");
        lua.load("HideNpcServiceChoice()").exec()?;
        assert!(!visible(&lua)?);
        lua.load("ShowNpcServiceChoice()").exec()?;
        assert!(visible(&lua)?, "the next talk shows the menu again");
        Ok(())
    }
}
