//! The original Summon Info window over a synthetic owned ship, and the part moves it asks for.
use super::*;
use crate::hud::api;
use crate::hud::vehicles::{self, Current};
use aa_net::model::ItemDetail;
use aa_ui::UiRuntime;
use bevy::prelude::*;

const CANNON: u32 = 28852;
/// `slaves.id` 22: small sailing ship with the 27 slots, customizing type 4.
const SHIP: u32 = 22;

fn item(template: u32, id: u64) -> Item {
    Item { template_id: template, id, count: 1, detail: ItemDetail::Other(10), detail_payload: Some(vec![0; 12]), ..default() }
}

fn ship(equipment: &[(usize, Item)]) -> Current {
    let mut worn = vec![None; 27];
    for (slot, part) in equipment {
        worn[*slot] = Some(part.clone());
    }
    Current {
        obj_id: 500,
        tl_id: 12,
        db_id: 77,
        template: SHIP,
        equipment: worn,
        point: Some(1),
        name: "Test Ship".into(),
        level: 1,
        hp: 100,
        max_hp: 100,
        pos: Vec3::ZERO,
        speed: 12.6,
        turn: 1.0,
        land: false,
        ship: true,
        cannon: false,
    }
}

fn runtime() -> (UiRuntime, api::Shared) {
    let (rt, bridge, path) = crate::hud::breath::fixture::runtime(1280.0, 720.0).expect("fixture");
    let _ = std::fs::remove_file(path);
    (rt, bridge)
}

fn bag_with(slot: usize, template: u32) -> Inventory {
    let mut inventory = Inventory::default();
    inventory.bag.resize(50, None);
    inventory.bag[slot] = Some(item(template, 900 + slot as u64));
    inventory
}

#[test]
fn database_rules_follow_the_servers_slot_and_hull_packs() {
    let conn = Connection::open_with_flags(aa_anim::db::DEFAULT_DB, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let tables = Tables::load(&conn).unwrap();
    // the ship declares 27 slots; the cannon (slot pack 1: slots 3..8, 25, 26) fits only those
    assert_eq!(tables.slots[&SHIP].len(), 25);
    for slot in 0..27u8 {
        let expect = matches!(slot, 3..=8 | 11 | 12 | 25 | 26); // item slot pack 11: cannons and masts
        assert_eq!(tables.can_equip(CANNON, SHIP, slot), expect, "slot {slot}");
    }
    assert!(!tables.can_equip(1, SHIP, 3), "not a ship part");
    assert!(!tables.can_equip(CANNON, 21, 3), "hull 21 declares only slots 0 and 1");
    assert_eq!(tables.customizing.get(&SHIP), Some(&4));
    assert_eq!(tables.customizing.get(&139), None, "a hull without slots has no equipment grid");
    assert_eq!(tables.kinds[&SHIP], "small_sailing_ship");
    assert!(!tables.names[&SHIP].is_empty());
}

#[test]
fn requests_echo_the_received_items_and_pick_the_bag_slot() {
    let mut tables = Tables::default();
    tables.slots.insert(SHIP, (0..27).collect());
    tables.parts.insert(CANNON, (0, 0));
    let current = ship(&[(3, item(CANNON, 40))]);
    let bag = bag_with(9, CANNON);
    // mount: the bag item first (bag container, its slot), the part it replaces second (equipment container 0xf2)
    let request = build(&tables, &current, 47, &bag, Request::Equip { bag: 9, slot: 3 }).unwrap();
    assert_eq!((request.character_id, request.slave_tl_id, request.db_slave_id), (47, 12, 77));
    assert_eq!(request.first_item.as_ref().map(|i| i.id), Some(909));
    assert_eq!(request.second_item.as_ref().map(|i| i.id), Some(40));
    assert_eq!((request.first_location.container, request.first_location.slot), (2, 9));
    assert_eq!((request.second_location.container, request.second_location.slot), (0xf2, 3));
    // a part that does not fit, or an empty bag slot, never leaves the client
    tables.parts.insert(CANNON, (0, 5));
    tables.slot_packs.insert(5, [3u8].into());
    assert!(build(&tables, &current, 47, &bag, Request::Equip { bag: 9, slot: 4 }).is_err());
    assert!(build(&tables, &current, 47, &bag, Request::Equip { bag: 10, slot: 3 }).is_err());
    // unequip: first free bag slot (or the dropped-on cell when empty), the part echoed second
    let off = build(&tables, &current, 47, &bag, Request::Unequip { slot: 3, bag: None }).unwrap();
    assert!(off.first_item.is_none());
    assert_eq!((off.first_location.container, off.first_location.slot), (2, 0));
    assert_eq!(off.second_item.as_ref().map(|i| i.id), Some(40));
    let off = build(&tables, &current, 47, &bag, Request::Unequip { slot: 3, bag: Some(9) }).unwrap();
    assert_eq!(off.first_location.slot, 0, "an occupied cell falls back to the first free one");
    let off = build(&tables, &current, 47, &bag, Request::Unequip { slot: 3, bag: Some(20) }).unwrap();
    assert_eq!(off.first_location.slot, 20);
    assert!(build(&tables, &current, 47, &bag, Request::Unequip { slot: 4, bag: None }).is_err(), "nothing mounted");
}

#[test]
fn changed_lists_the_slots_whose_part_differs() {
    let a = ship(&[(3, item(CANNON, 1))]).equipment;
    let b = ship(&[(3, item(CANNON, 2)), (4, item(CANNON, 3))]).equipment;
    assert_eq!(changed(&a, &b), [3, 4]);
    assert!(changed(&a, &a).is_empty());
}

#[test]
fn original_summon_info_window_runs_and_its_slots_follow_the_worn_parts() {
    let (rt, bridge) = runtime();
    {
        let mut data = bridge.borrow_mut();
        data.vehicles.current = Some(ship(&[(3, item(CANNON, 40)), (4, item(CANNON, 41))]));
    }
    // boarding: the mode bar gets skills; the information window does NOT open (only the Open Pet Details key does)
    vehicles::push(&rt, &bridge);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    let closed: bool = rt.lua.load("return not slaveInfoFrame:IsVisible()").eval().unwrap();
    assert!(closed, "boarding leaves the Summon Info window closed");
    vehicles::info_key(&rt, &bridge);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    let ok: bool = rt
        .lua
        .load(
            r#"
            local f = slaveInfoFrame
      assert(f:IsVisible(), 'summon info window opens with the key')
      assert(f.customizingType == 4, 'customizing type ' .. tostring(f.customizingType))
      local info = X2SiegeWeapon:GetMountedSiegeWeaponInfo()
      assert(info.kind == 'small_sailing_ship', tostring(info.kind)); assert(info.desc_name ~= '', 'desc'); assert(info.name == 'Test Ship', 'name')
      f:OnUpdate(0.1)
      assert(f.moveSpeed:GetText():find('12.6'), f.moveSpeed:GetText())
      local slots = f.equipmentWnd.equipWnd.slots
      local shown, byType = 0, {}
      for _, s in pairs(slots) do
        if s.slotType ~= nil and s:IsVisible() then shown = shown + 1; byType[s.slotType] = s end
      end
      assert(shown == #SLAVE_CUSTOMIZING_STYLE[4], 'shown ' .. shown)
      local cannon = X2SiegeWeapon:GetSlaveEquipSlotInfo(3)
      assert(cannon.has, 'has'); assert(cannon.itemInfo and cannon.itemInfo.lookType == 28852, 'item'); assert(cannon.itemInfo.itemGrade ~= nil, 'grade')
      local empty = X2SiegeWeapon:GetSlaveEquipSlotInfo(5)
      assert(empty.has, 'empty has'); assert(empty.itemInfo == nil, 'empty item')
      assert(X2SiegeWeapon:GetSlaveEquipSlotInfo(14).has == false, 'slot 14 is not declared')
      return true
    "#,
        )
        .eval()
        .expect("summon info scripts");
    assert!(ok);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    // a part changing reaches the slot through SLAVE_UNIT_EQUIPMENT_CHANGED
    {
        let mut data = bridge.borrow_mut();
        let current = data.vehicles.current.as_mut().unwrap();
        current.equipment[5] = Some(item(CANNON, 42));
    }
    vehicles::push(&rt, &bridge);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    let found: bool = rt
        .lua
        .load(
            r#"
      local info = X2SiegeWeapon:GetSlaveEquipSlotInfo(5)
      assert(info.itemInfo ~= nil and info.itemInfo.lookType == 28852)
      for _, s in pairs(slaveInfoFrame.equipmentWnd.equipWnd.slots) do
        if s.slotType == 5 then return s.procOnEnter == nil end
      end
      return false
    "#,
        )
        .eval()
        .unwrap();
    assert!(found, "the slot shows the new part (no empty-slot tooltip hook)");
    // leaving the seat hides the window again (SIEGEWEAPON_UNBOUND)
    bridge.borrow_mut().vehicles.current.as_mut().unwrap().point = None;
    vehicles::push(&rt, &bridge);
    let hidden: bool = rt.lua.load("return not slaveInfoFrame:IsVisible()").eval().unwrap();
    assert!(hidden);
}

#[test]
fn the_info_key_needs_a_slave_and_the_bar_without_skills_hides_the_window_again() {
    let (rt, bridge) = runtime();
    let visible = |rt: &UiRuntime| -> bool { rt.lua.load("return slaveInfoFrame:IsVisible()").eval().unwrap() };
    // no slave: the key does nothing
    vehicles::info_key(&rt, &bridge);
    assert!(!visible(&rt));
    bridge.borrow_mut().vehicles.current = Some(ship(&[]));
    vehicles::push(&rt, &bridge);
    vehicles::info_key(&rt, &bridge);
    assert!(visible(&rt), "the key opens it on a slave");
    // another slave whose mode bar has no skills: MODE_ACTIONS_UPDATE hides the window (`mode_action.alb`, count 0)
    {
        let mut data = bridge.borrow_mut();
        let current = data.vehicles.current.as_mut().unwrap();
        current.obj_id = 501;
        current.template = 0;
    }
    vehicles::push(&rt, &bridge);
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    assert!(!visible(&rt), "no skills on the bar: the window hides");
}

#[test]
fn picking_dropping_and_uninstalling_queue_the_moves_the_grid_asks_for() {
    let (rt, bridge) = runtime();
    bridge.borrow_mut().vehicles.current = Some(ship(&[(3, item(CANNON, 40))]));
    vehicles::push(&rt, &bridge);
    let lua = &rt.lua;
    // nothing held: every slot is "no preference" (nil), nothing is queued by the check
    assert!(lua.load("return X2SiegeWeapon:CanSlaveEquipPickedItem(3)").eval::<Option<bool>>().unwrap().is_none());
    // a cannon from bag cell 10 is held: it fits the cannon slots, not the figurehead
    lua.load("X2Cursor:SetHeld({kind='bag', slot=10, id=28852, count=1, total=1, partial=false})").exec().unwrap();
    assert_eq!(lua.load("return X2SiegeWeapon:CanSlaveEquipPickedItem(4)").eval::<Option<bool>>().unwrap(), Some(true));
    assert_eq!(lua.load("return X2SiegeWeapon:CanSlaveEquipPickedItem(0)").eval::<Option<bool>>().unwrap(), Some(false));
    // dropping on a slot it does not fit asks for nothing; on one it fits it asks to mount
    lua.load("X2SiegeWeapon:PickEquipSlotOfSlave(0)").exec().unwrap();
    assert!(bridge.borrow().vehicles.equip_requests.is_empty());
    lua.load("X2Cursor:SetHeld({kind='bag', slot=10, id=28852, count=1, total=1, partial=false}); X2SiegeWeapon:PickEquipSlotOfSlave(4)")
        .exec()
        .unwrap();
    assert_eq!(std::mem::take(&mut bridge.borrow_mut().vehicles.equip_requests), [Request::Equip { bag: 9, slot: 4 }]);
    assert!(!rt.holding());
    // a click on a worn part picks it up; the bag cell it is dropped on takes it off there
    lua.load("X2SiegeWeapon:PickEquipSlotOfSlave(3)").exec().unwrap();
    assert!(rt.holding());
    assert_eq!(lua.load("return X2Cursor:GetHeld().kind").eval::<String>().unwrap(), "slaveequip");
    lua.load("X2Bag:HandleClick(12)").exec().unwrap();
    assert!(!rt.holding());
    assert_eq!(
        std::mem::take(&mut bridge.borrow_mut().vehicles.equip_requests),
        [Request::Unequip { slot: 3, bag: Some(11) }] // cell 12 of the window, bag slot 11 (0 based)
    );
    // right click takes it off into the first free cell
    lua.load("X2SiegeWeapon:UninstallEquipSlotOfSlave(3)").exec().unwrap();
    assert_eq!(
        std::mem::take(&mut bridge.borrow_mut().vehicles.equip_requests),
        [Request::Unequip { slot: 3, bag: None }]
    );
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
}

#[test]
fn queued_moves_leave_as_change_slave_equipment_for_the_shown_slave() {
    use aa_net::{Command, NetCommand, slave_control::SlaveCommand};
    let (_rt, bridge) = runtime();
    {
        let mut data = bridge.borrow_mut();
        data.vehicles.current = Some(ship(&[(3, item(CANNON, 40))]));
        data.vehicles.equip_requests = vec![Request::Equip { bag: 9, slot: 4 }, Request::Equip { bag: 20, slot: 4 }, Request::Unequip { slot: 3, bag: None }];
    }
    let inventory = bag_with(9, CANNON);
    let mut app = App::new();
    app.add_message::<NetCommand>();
    app.insert_non_send(api::HudBridge(bridge.clone()));
    app.add_systems(Update, move |bridge: NonSend<api::HudBridge>, mut net: MessageWriter<NetCommand>| {
        super::super::vehicles::send_equipment_requests(&bridge.0, 47, Some(&inventory), &mut net);
    });
    app.update();
    let sent: Vec<_> = app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|m| m.0).collect();
    // the empty bag cell 20 is refused on the client; the mount and the unequip are sent
    assert_eq!(sent.len(), 2, "{sent:?}");
    let Command::Slave(SlaveCommand::ChangeEquipment(mount)) = &sent[0] else { panic!("{sent:?}") };
    assert_eq!((mount.slave_tl_id, mount.db_slave_id, mount.second_location.slot), (12, 77, 4));
    let Command::Slave(SlaveCommand::ChangeEquipment(off)) = &sent[1] else { panic!("{sent:?}") };
    assert!(off.first_item.is_none() && off.second_item.is_some());
    assert!(bridge.borrow().vehicles.equip_requests.is_empty());
}
