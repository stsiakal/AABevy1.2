use super::*;

#[test]
fn pet_experience_keeps_threshold_equality_level100_and_float_rounding() {
    let tables = Tables {
        levels: vec![(1, 0), (2, 50), (3, 250), (100, 16_417_500), (101, 16_917_500)],
        ..default()
    };
    assert_eq!(tables.progress(-1).unwrap(), [0.0, -1.0, 0.0, 50.0]);
    assert_eq!(tables.progress(50).unwrap(), [0.0, 50.0, 50.0, 200.0]);
    assert_eq!(tables.progress(100).unwrap(), [25.0, 100.0, 50.0, 200.0]);
    assert_eq!(tables.progress(16_417_500).unwrap(), [0.0, 16_417_500.0, 16_417_500.0, 500_000.0]);
    assert_eq!(tables.progress(16_917_500).unwrap()[0], 100.0);
    assert_eq!(
        tables.progress(16_917_501).unwrap()[1],
        16_917_500.0,
        "each returned integer converts through f32"
    );
}

#[test]
fn pet_identity_rejects_metadata_without_unit_owner_and_timeline() {
    let (player, mut units, mut mates) = super::super::active_fixture::received();
    let owner = player.spawn.as_ref().unwrap().character().id;
    assert_eq!(current(&mates, &units, owner).unwrap().0.obj_id, 201);
    assert!(current(&mates, &units, owner + 1).is_none());
    units.by_id.get_mut(&201).unwrap().kind = UnitKind::Mate {
        tl_id: 13,
        template_id: 504,
        owner_id: owner,
    };
    assert!(current(&mates, &units, owner).is_none());
    units.by_id.remove(&201);
    assert!(current(&mates, &units, owner).is_none());
    mates.own.as_mut().unwrap().obj_id = None;
    assert!(current(&mates, &units, owner).is_none());
}

#[test]
fn displayed_skills_preserve_received_order_and_first_seat_only() {
    let mut tables = Tables::default();
    let mut add = |id, seat| {
        tables.skills.insert(
            id,
            Skill {
                combat: id + 100,
                first_seat: seat,
                level: 1,
                target_type: 0,
                target_selection: 0,
            },
        );
    };
    add(10, None);
    add(20, Some(1));
    add(30, Some(2));
    add(40, Some(0));
    let pet = Pet {
        spawn: MateSpawn {
            skills: [0, 30, 20, 99, 10, 40, 20, 0, 0, 0],
            ..default()
        },
        ..default()
    };
    assert_eq!((1..=4).map(|i| displayed(&tables, &pet, i).unwrap().0).collect::<Vec<_>>(), [2, 4, 5, 6]);
    assert!(displayed(&tables, &pet, 0).is_none() && displayed(&tables, &pet, 5).is_none());
    let mounted = Pet { driving: true, ..pet };
    assert_eq!((1..=3).map(|i| displayed(&tables, &mounted, i).unwrap().0).collect::<Vec<_>>(), [2, 4, 6]);
    assert!(displayed(&tables, &mounted, 4).is_none());
}

#[test]
fn local_modes_use_attackable_and_active_command_duplicate_guards() {
    let mut data = HudData::default();
    assert!(!request_mode(&mut data, 0));
    let mut game = super::super::super::data::GameData::default();
    game.pets.owned.skills.insert(
        10,
        Skill {
            combat: 110,
            first_seat: None,
            level: 1,
            target_type: 0,
            target_selection: 0,
        },
    );
    assert!(data.game.0.set(game).is_ok());
    data.pets.owned.current = Some(Pet {
        spawn: MateSpawn {
            tl_id: 12,
            user_state: 1,
            skills: [10, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            ..default()
        },
        object: 201,
        ..default()
    });
    assert!(request_mode(&mut data, 2));
    assert!(!request_mode(&mut data, 2));
    data.pets.owned.commanded_skill = 10;
    assert!(request_mode(&mut data, 2), "same mode interrupts an active local command");
    assert_eq!(data.pets.owned.commanded_skill, 0);
    assert!(
        request_mode(&mut data, 1) && request_mode(&mut data, 1),
        "native passive has no duplicate-state guard"
    );
    data.pets.owned.current.as_mut().unwrap().spawn.skills = [0; 10];
    assert!(!request_mode(&mut data, 3));
    assert!(request_mode(&mut data, 0));
    assert!(!request_mode(&mut data, 0));
}

#[test]
fn mount_reason_follows_native_approach_and_seat_rules() {
    // The player stands at the origin; the mate faces +y (yaw 0) unless a test turns it.
    let mut data = HudData::default();
    data.pos = Vec3::ZERO;
    data.yaw = 2.0; // the player's own facing does not matter
    let at = |x: f32, y: f32| Vec3::new(x, y, 0.0);
    // the mate stands ahead of the player and faces away from it: the player is behind the mate
    assert_eq!(mount_reason(&data, at(0.0, 3.0), 0.0, false, false), 3, "behind the mate");
    assert_eq!(mount_reason(&data, at(0.0, 3.0), 0.0, false, true), 1, "behind + occupied passenger seat");
    assert_eq!(mount_reason(&data, at(0.0, 3.0), 0.0, true, false), 3, "behind ignores the driver seat");
    // the mate faces the player: the player is in front of the mate
    let toward = std::f32::consts::PI;
    assert_eq!(mount_reason(&data, at(0.0, 3.0), toward, false, false), 4, "in front of the mate");
    assert_eq!(mount_reason(&data, at(0.0, 3.0), toward, true, false), 1, "front + occupied driver seat");
    assert_eq!(mount_reason(&data, at(0.0, 3.0), toward, false, true), 4, "front ignores the passenger seat");
    // the mate at +x of the player and facing +y: the player is on the mate's left
    assert_eq!(mount_reason(&data, at(3.0, 0.0), 0.0, false, false), 1, "left side");
    assert_eq!(mount_reason(&data, at(-3.0, 0.0), 0.0, false, false), 2, "right side");
    assert_eq!(mount_reason(&data, at(3.0, 0.0), 0.0, true, true), 1, "left side, seats ignored");
    assert_eq!(mount_reason(&data, at(-3.0, 0.0), 0.0, true, true), 2, "right side ignores the driver seat");
    assert_eq!(mount_reason(&data, Vec3::ZERO, 0.0, false, false), 2, "under the seat is the side split");
    // the band edge: 0.96 is cos(16.3 deg)
    let near = 16.0_f32.to_radians();
    assert_eq!(mount_reason(&data, at(0.0, 3.0), near, false, false), 3, "inside the back cone");
    assert_eq!(mount_reason(&data, at(0.0, 3.0), 17.0_f32.to_radians(), false, false), 1, "outside it: the side by the cross sign");
}

#[test]
fn mount_skill_slot_uses_mate_self_and_player_hostile_targets() -> Result<(), Box<dyn std::error::Error>> {
    let (_rt, bridge, path, _player, _units, _mates) = super::super::active_fixture::runtime(1280., 720.)?;
    let mut data = bridge.borrow_mut();
    {
        let pet = data.pets.owned.current.as_mut().expect("fixture pet");
        pet.level = 30;
        pet.spawn.skills[0] = 5; // mount_skills 5: combat 11028, target_type 0 (self)
    }
    let command = use_pet_action(&data, 1, 0, None).expect("self skill");
    assert!(matches!(
        command,
        aa_net::Command::UseMateSkill { skill_id: 11028, mate_obj_id: 201, mount_skill_id: 5, target_obj_id: 201 }
    ));
    data.pets.owned.current.as_mut().expect("fixture pet").spawn.skills[0] = 2; // mount_skills 2: combat 11017, target_type 4
    assert!(use_pet_action(&data, 1, 0, None).is_none(), "a hostile mount skill needs a target");
    let command = use_pet_action(&data, 1, 987, None).expect("hostile skill");
    assert!(matches!(
        command,
        aa_net::Command::UseMateSkill { skill_id: 11017, mate_obj_id: 201, mount_skill_id: 2, target_obj_id: 987 }
    ));
    data.pets.owned.current.as_mut().expect("fixture pet").spawn.skills[0] = 12; // target_type 6, target_selection 2
    assert!(use_pet_action(&data, 1, 987, None).is_none(), "a position mount skill needs the target's spot");
    let command = use_pet_action(&data, 1, 987, Some(Vec3::new(10.0, 20.0, 30.0))).expect("position skill");
    assert!(matches!(
        command,
        aa_net::Command::UseMateSkillAt { skill_id: 11328, mate_obj_id: 201, mount_skill_id: 12, pos }
            if (pos.x, pos.y, pos.z) == (10.0, 20.0, 30.0)
    ));
    {
        let pet = data.pets.owned.current.as_mut().expect("fixture pet");
        pet.level = 1; // skill 5 needs ability level 20
        pet.spawn.skills[0] = 5;
    }
    assert!(use_pet_action(&data, 1, 0, None).is_none(), "unlearned mount skills do not start");
    drop(data);
    drop(bridge);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[test]
fn original_pet_mount_dismiss_and_action_bar_controls() -> Result<(), Box<dyn std::error::Error>> {
    use aa_net::model::Attachment;
    let (rt, bridge, path, player, units, mates) = super::super::active_fixture::runtime(1920.0, 1080.0)?;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_non_send(rt)
        .insert_non_send(HudBridge(bridge.clone()))
        .insert_resource(player)
        .insert_resource(units)
        .insert_resource(mates)
        .add_message::<NetEvent>()
        .add_message::<NetCommand>()
        .add_systems(Update, super::super::update);
    let drain = |app: &mut App| app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect::<Vec<_>>();
    app.update();
    assert!(drain(&mut app).is_empty());
    // The speed getter is the last received movement speed (3,4 m/s -> 5 m/s).
    app.world_mut()
        .resource_mut::<Units>()
        .by_id
        .get_mut(&201)
        .expect("fixture pet")
        .movement = Some({
            let mut m = aa_net::model::Movement::unit(aa_net::Pos::default(), 0.0, 1);
            m.velocity = [3 * 1024, 4 * 1024, 0];
            m
        });
    {
        let mut data = bridge.borrow_mut();
        super::super::refresh(app.world(), &mut data);
    }
    {
        let runtime = app.world().non_send::<UiRuntime>();
        runtime
            .lua
            .load("assert(X2Mate:CanMount()==true and X2Mate:CanDismiss()==true); assert(type(X2Mate:GetSpeedInfo())=='number' and X2Mate:GetSpeedInfo()==5)")
            .exec()?;
    }
    app.world_mut().resource_mut::<Units>().by_id.get_mut(&201).unwrap().movement = None;
    {
        let mut data = bridge.borrow_mut();
        super::super::refresh(app.world(), &mut data);
    }
    let runtime = app.world().non_send::<UiRuntime>();
    super::super::super::expansion::fixture::click(runtime, "petBar.command.buttons[PET_CMD_MOUNT]")?;
    app.update();
    assert!(matches!(
        drain(&mut app).as_slice(),
        [Command::Mate(MateCommand::Mount { tl_id: 12, point: 1, .. })]
    ));
    // The server's receipt seats the local player on the pet's driver point.
    app.world_mut()
        .resource_mut::<Units>()
        .by_id
        .get_mut(&100)
        .expect("own unit")
        .attached_to = Some(Attachment { obj_id: 201, point: 1 });
    {
        let mut data = bridge.borrow_mut();
        super::super::refresh(app.world(), &mut data);
        let now = data.start.elapsed().as_millis() as u64; data.pets.owned.last_action_ms = Some(now.saturating_sub(3_000));
    }
    app.update();
    assert!(drain(&mut app).is_empty());
    let runtime = app.world().non_send::<UiRuntime>();
    runtime.lua.load("assert(X2Mate:IsPlayerMounting()==true and X2Mate:CanMount()==false and X2Mate:CanDismiss()==true)").exec()?;
    super::super::super::expansion::fixture::click(runtime, "petBar.command.buttons[PET_CMD_MOUNT]")?;
    app.update();
    assert!(matches!(
        drain(&mut app).as_slice(),
        [Command::Mate(MateCommand::Unmount { tl_id: 12, point: 1, reason: 9 })]
    ));
    // Once the native two-second cooldown has passed, the CYA button first unmounts (reason 10),
    // then its model-action continuation removes the mate after the server's detach.
    {
        let mut data = bridge.borrow_mut();
        let now = data.start.elapsed().as_millis() as u64; data.pets.owned.last_action_ms = Some(now.saturating_sub(3_000));
    }
    app.update();
    let runtime = app.world().non_send::<UiRuntime>();
    super::super::super::expansion::fixture::click(runtime, "petBar.command.buttons[PET_CMD_CYA]")?;
    app.update();
    assert!(matches!(
        drain(&mut app).as_slice(),
        [Command::Mate(MateCommand::Unmount { tl_id: 12, point: 1, reason: 10 })]
    ));
    app.world_mut()
        .resource_mut::<Units>()
        .by_id
        .get_mut(&100)
        .expect("own unit")
        .attached_to = None;
    app.world_mut().write_message(NetEvent(Event::UnitDetached { obj_id: 100, reason: 10 }));
    app.update();
    assert!(matches!(
        drain(&mut app).as_slice(),
        [Command::Mate(MateCommand::Dismiss { tl_id: 12 })]
    ));
    // The original pet bar's action slots feed `ISLOT_PET_ACTION` into the app's UseSlot hook.
    app.world()
        .non_send::<UiRuntime>()
        .lua
        .load("AAUI.UseSlot(ISLOT_PET_ACTION,2,'LeftButton')")
        .exec()?;
    assert!(matches!(
        bridge.borrow().requests.as_slice(),
        [super::super::super::api::Request::UsePetAction { slot: 2 }]
    ));
    assert!(app.world().non_send::<UiRuntime>().errors.is_empty());
    drop(app);
    drop(bridge);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[test]
#[ignore = "isolated original pet frame/bar, native getter types and physical controls"]
fn original_pet_frame_passive_rename_and_stale_request_identity() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, bridge, path, player, units, mates) = super::super::active_fixture::runtime(1920.0, 1080.0)?;
    bridge.borrow_mut().pets.owned.current.as_mut().unwrap().level = 15;
    super::push(&rt, &bridge);
    rt.lua
        .load("local s=X2Mate:GetMountSkill(1);assert(s.learned==false and s.learnLevel==20);SetPetBarSkillTooltip(1,PetActionButton1)")
        .exec()?;
    bridge.borrow_mut().pets.owned.current.as_mut().unwrap().level = 30;
    super::push(&rt, &bridge);
    rt.lua
        .load(
            r#"
        local p,total,start,need=X2:GetPetExpToNextLevel()
        assert(type(p)=='number' and total==426750 and start==385700 and need==42050)
        local s=X2Mate:GetMountSkill(1)
        assert(s and type(s.learned)=='boolean' and s.learned==true)
        assert(type(s.skillUsable)=='number' and s.skillUsable==0 and s.needdiver==true)
        assert(X2Mate:GetMountSkill(0)==nil and X2Mate:GetMountSkill(99)==nil)
        local request=X2DialogManager.RequestDefaultDialog
        X2DialogManager.RequestDefaultDialog=function(self,handler,owner)
          local window=request(self,handler,owner); PET_RENAME_DIALOG=window;return window
        end
        ShowChangeMateName('playerpet')
        assert(PET_RENAME_DIALOG and PET_RENAME_DIALOG.editbox:GetText()=='Stormmane')
        PET_RENAME_DIALOG.editbox:SetText('a_1B')
        assert(PET_RENAME_DIALOG.editbox:GetText()=='A_1b' and PET_RENAME_DIALOG.btnOk:IsEnabled())
        PET_RENAME_DIALOG.editbox:SetText('a'..string.rep('B',30))
        assert(PET_RENAME_DIALOG.editbox:MaxTextLength()==26)
        assert(PET_RENAME_DIALOG.editbox:GetText()=='A'..string.rep('b',30) and not PET_RENAME_DIALOG.btnOk:IsEnabled())
        PET_RENAME_DIALOG.editbox:SetText('a B')
        assert(PET_RENAME_DIALOG.editbox:GetText()=='A b' and not PET_RENAME_DIALOG.btnOk:IsEnabled())
        PET_RENAME_DIALOG.editbox:SetText('Thunder')
        assert(PET_RENAME_DIALOG.btnOk:IsEnabled())
        X2Unit:TargetUnit('playerpet')
    "#,
        )
        .exec()?;
    assert!(matches!(
        bridge.borrow().requests.last(),
        Some(super::super::super::api::Request::Target { obj_id: 201 })
    ));
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_non_send(rt)
        .insert_non_send(HudBridge(bridge.clone()))
        .insert_resource(player)
        .insert_resource(units)
        .insert_resource(mates)
        .add_message::<NetEvent>()
        .add_message::<NetCommand>()
        .add_systems(Update, super::super::update);
    let drain = |app: &mut App| app.world_mut().resource_mut::<Messages<NetCommand>>().drain().map(|v| v.0).collect::<Vec<_>>();
    super::super::super::expansion::fixture::click(app.world().non_send::<UiRuntime>(), "PET_RENAME_DIALOG.btnCancel")?;
    app.update();
    assert!(drain(&mut app).is_empty());
    app.world()
        .non_send::<UiRuntime>()
        .lua
        .load("ShowChangeMateName('playerpet');PET_RENAME_DIALOG.editbox:SetText('Thunder')")
        .exec()?;
    super::super::super::expansion::fixture::click(app.world().non_send::<UiRuntime>(), "PET_RENAME_DIALOG.btnOk")?;
    app.update();
    assert!(matches!(drain(&mut app).as_slice(),[Command::Mate(MateCommand::Rename {tl_id:12,name})] if name=="Thunder"));
    assert_eq!(app.world().resource::<Units>().get(201).unwrap().name, "Stormmane", "name waits for receipt");
    // Complete the original dialog's close animation before the next outside interaction.
    aa_ui::lua::tick(&app.world().non_send::<UiRuntime>().lua, 300.0);
    super::super::super::expansion::fixture::click(app.world().non_send::<UiRuntime>(), "petBar.stateBar.buttons[PET_STATE_PASSIVE]")?;
    app.update();
    assert!(matches!(
        drain(&mut app).as_slice(),
        [Command::Mate(MateCommand::UserState { tl_id: 12, state: 1 })]
    ));
    app.world()
        .non_send::<UiRuntime>()
        .lua
        .load("assert(X2Mate:PetState()=='passive'); X2Mate:SetMateName('Stale')")
        .exec()?;
    app.world_mut().resource_mut::<Units>().by_id.get_mut(&201).unwrap().kind = UnitKind::Npc { template_id: 504, owner_id: 0 };
    app.update();
    assert!(drain(&mut app).is_empty(), "queued name cannot retarget a reused runtime ID");
    {
        let mut data = bridge.borrow_mut();
        super::refresh(app.world(), &mut data);
    }
    super::push(app.world().non_send::<UiRuntime>(), &bridge);
    aa_ui::lua::tick(&app.world().non_send::<UiRuntime>().lua, 0.0);
    app.world().non_send::<UiRuntime>().lua.load("assert(not X2Mate:IsPlayerPetExists() and X2Mate:PetState()==nil); assert(select('#',X2:GetPetExpToNextLevel())==0); assert(petFrame==nil and petBar==nil)").exec()?;
    assert!(app.world().non_send::<UiRuntime>().errors.is_empty());
    drop(app);
    drop(bridge);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[test]
fn original_chat_and_portal_name_dialogs_keep_case_spaces_and_distinct_limits() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, bridge, path, _, _, _) = super::super::active_fixture::runtime(1280., 720.)?;
    rt.lua.load(r#"
        local request=X2DialogManager.RequestDefaultDialog
        X2DialogManager.RequestDefaultDialog=function(self,handler,owner)
            local wnd=request(self,handler,owner);NAME_DIALOG=wnd;return wnd
        end
        ShowCreateChatTabName(1)
        NAME_DIALOG.editbox:SetText('Raid Room!')
        assert(NAME_DIALOG.editbox:GetText()=='Raid Room!' and NAME_DIALOG.btnOk:IsEnabled())
        NAME_DIALOG.editbox:SetText('aBc DEFGHIJKL')
        assert(NAME_DIALOG.editbox:MaxTextLength()==10)
        assert(NAME_DIALOG.editbox:GetText()=='aBc DEFGHIJKL' and not NAME_DIALOG.btnOk:IsEnabled())
        NAME_DIALOG.editbox:SetText('x');assert(not NAME_DIALOG.btnOk:IsEnabled())
        NAME_DIALOG.editbox:SetText('');NAME_DIALOG.editbox:SetFocus()
    "#).exec()?;
    rt.type_text("aBc DEFGHIJKL");
    rt.lua.load("assert(NAME_DIALOG.editbox:GetText()=='aBc DEFGHI' and NAME_DIALOG.btnOk:IsEnabled())").exec()?;
    super::super::super::expansion::fixture::click(&rt, "NAME_DIALOG.btnCancel")?;
    aa_ui::lua::tick(&rt.lua, 300.);
    rt.lua.load(r#"
        ShowRegisterPotal()
        NAME_DIALOG.editbox:SetText('AB');assert(not NAME_DIALOG.btnOk:IsEnabled())
        NAME_DIALOG.editbox:SetText('Western Coast')
        assert(NAME_DIALOG.editbox:GetText()=='Western Coast' and NAME_DIALOG.btnOk:IsEnabled())
        NAME_DIALOG.editbox:SetText('A'..string.rep('B',40))
        assert(NAME_DIALOG.editbox:MaxTextLength()==32)
        assert(NAME_DIALOG.editbox:GetText()=='A'..string.rep('B',40) and not NAME_DIALOG.btnOk:IsEnabled())
        NAME_DIALOG.editbox:SetText('');NAME_DIALOG.editbox:SetFocus()
    "#).exec()?;
    rt.type_text(&format!("A{}", "B".repeat(40)));
    rt.lua.load("assert(NAME_DIALOG.editbox:GetText()=='A'..string.rep('B',31) and NAME_DIALOG.btnOk:IsEnabled())").exec()?;
    assert!(rt.errors.is_empty(), "{:?}", rt.errors);
    assert!(bridge.borrow().pets.owned.requests.is_empty());
    drop(rt);
    drop(bridge);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}
