use super::*;
use aa_net::{
    items::{ItemTask, ItemTasks},
    model::{Item, UnitKind, slot},
};
use aa_ui::UiRuntime;
use mlua::FromLua;

fn npc(template: u32) -> Event {
    let mut unit = crate::hud::expansion::fixture::spawn().unit;
    unit.obj_id = 500;
    unit.kind = UnitKind::Npc {
        template_id: template,
        owner_id: 0,
    };
    Event::UnitSpawned(Box::new(unit))
}
fn dispatch(state: &Shared, event: Event) {
    crate::hud::enchant::dispatch(None, state, &event, None);
}
fn grade(grade: u8) -> Event {
    Event::ItemsChanged(ItemTasks {
        tasks: vec![ItemTask::Grade {
            container: 1,
            slot: slot::BACKPACK as u8,
            item_id: 999,
            grade,
        }],
        ..Default::default()
    })
}
fn runtime() -> Result<(UiRuntime, Shared), Box<dyn std::error::Error>> {
    let (rt, state) = super::super::tests::runtime()?;
    assert!(state.borrow().game.0.set(GameData::load(aa_anim::db::DEFAULT_DB)?).is_ok());
    let mut spawn = crate::hud::expansion::fixture::spawn();
    let item = Item {
        id: 123,
        template_id: 20090,
        grade: 0,
        count: 1,
        ..Default::default()
    };
    spawn.unit.equipment.slots[slot::BACKPACK] = Some(item.clone());
    spawn.state.character.equipment.slots[slot::BACKPACK] = Some(item);
    dispatch(&state, Event::CharacterSelected(Box::new(spawn.state.clone())));
    // NPC rows in the selected character's epoch may precede its own spawn.
    dispatch(&state, npc(4473));
    dispatch(&state, Event::PlayerSpawn(Box::new(spawn)));
    dispatch(
        &state,
        Event::TargetChanged {
            obj_id: crate::hud::expansion::fixture::spawn().obj_id,
            target_obj_id: 500,
        },
    );
    // Interaction context is independent and deliberately wrong. Refund reads
    // the received current target and resolves its actual NPC template instead.
    state.borrow_mut().dialog.npc = Some((999, u32::MAX));
    Ok((rt, state))
}

#[test]
fn actual_twelve_native_quotes_use_received_backpack_grade_npc_and_raw_ratio() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, state) = runtime()?;
    let before = crate::hud::enchant::equipment(&state.borrow(), slot::BACKPACK as u8)
        .cloned()
        .unwrap();
    for (template, coin, expected) in [
        (4473, 0, [112280, 102118, 308464, -1123]),
        (14766, 0, [75927, 59585, 261205, -759]),
        (4482, 32103, [13, 12, 33, 0]),
    ] {
        dispatch(&state, Event::UnitsRemoved(vec![500]));
        dispatch(&state, npc(template));
        dispatch(
            &state,
            Event::TargetChanged {
                obj_id: crate::hud::expansion::fixture::spawn().obj_id,
                target_obj_id: 500,
            },
        );
        for ((grade_value, ratio), amount) in [(0, 100), (1, 117), (6, 130), (0, -1)].into_iter().zip(expected) {
            dispatch(&state, grade(grade_value));
            dispatch(&state, Event::SpecialtyRatio(ratio));
            let values: MultiValue = rt.lua.load("return X2Store:GetSpecialtyRatioRefund(-999999)").eval()?;
            assert_eq!(values.len(), if coin == 0 { 2 } else { 3 });
            assert!(matches!(values.get(0), Some(Value::Boolean(true))));
            assert_eq!(
                f64::from_lua(values.get(1).unwrap().clone(), &rt.lua)?,
                f64::from(amount as f32),
                "NPC {template}, grade {grade_value}, ratio {ratio}"
            );
            if coin != 0 {
                assert_eq!(f64::from_lua(values.get(2).unwrap().clone(), &rt.lua)?, f64::from(coin as f32));
            }
        }
    }
    let after = crate::hud::enchant::equipment(&state.borrow(), slot::BACKPACK as u8)
        .cloned()
        .unwrap();
    assert_eq!(before, after, "passive quotes do not alter equipment, flags, counts or details");
    Ok(())
}

#[test]
fn strict_numeric_refund_admission_ignores_value_and_preserves_failure_arity() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, state) = runtime()?;
    rt.lua
        .load(
            r#"
      assert(select('#',X2Store:GetSpecialtyRatioRefund())==0)
      for _,v in ipairs({false,'1',{}}) do assert(select('#',X2Store:GetSpecialtyRatioRefund(v))==0) end
      assert(select('#',X2Store:GetSpecialtyRatioRefund(nil))==0)
      for _,v in ipairs({0,1,-1,1.25,1/0,0/0}) do
        local ok,amount,coin=X2Store:GetSpecialtyRatioRefund(v)
        assert(ok==true and amount==112280 and coin==nil)
      end
    "#,
        )
        .exec()?;
    dispatch(&state, Event::UnitsRemoved(vec![500]));
    rt.lua
        .load("assert(select('#',X2Store:GetSpecialtyRatioRefund(0))==1 and X2Store:GetSpecialtyRatioRefund(0)==false)")
        .exec()?;
    dispatch(&state, npc(4473));
    dispatch(&state, Event::LeftWorld { target: 0 });
    rt.lua
        .load("assert(X2Store:GetSpecialtyRatioRefund(0)==false and X2Store:GetSpecialtyRatio()==100)")
        .exec()?;
    Ok(())
}

#[test]
fn receipt_target_is_independent_from_dialog_and_future_target_rows() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, state) = runtime()?;
    rt.lua
        .load(
            r#"
        QUOTED={};local w=UIParent:CreateWidget('emptywidget','specialtyTargetObserver','UIParent')
        w:RegisterEvent('NPC_INTERACTION_START')
        w:SetHandler('OnEvent',function()local ok,n=X2Store:GetSpecialtyRatioRefund(0);QUOTED[#QUOTED+1]={ok,n}end)
    "#,
        )
        .exec()?;
    let mut other = match npc(14766) {
        Event::UnitSpawned(unit) => unit,
        _ => unreachable!(),
    };
    other.obj_id = 600;
    dispatch(&state, Event::UnitSpawned(other));
    for event in [
        Event::TargetChanged {
            obj_id: 999,
            target_obj_id: 600,
        },
        Event::SpecialtyRatio(100),
        Event::TargetChanged {
            obj_id: 100,
            target_obj_id: 600,
        },
        Event::SpecialtyRatio(100),
        Event::TargetChanged {
            obj_id: 100,
            target_obj_id: 0,
        },
        Event::SpecialtyRatio(100),
    ] {
        crate::hud::enchant::dispatch(Some(&rt), &state, &event, None);
    }
    rt.lua
        .load(
            "assert(#QUOTED==3 and QUOTED[1][1] and QUOTED[1][2]==112280 and QUOTED[2][1] and QUOTED[2][2]==75927 and QUOTED[3][1]==false)",
        )
        .exec()?;
    assert_eq!(
        state.borrow().dialog.npc,
        Some((999, u32::MAX)),
        "target changes never substitute the interaction manager"
    );
    Ok(())
}

#[test]
fn original_specialty_confirmation_uses_real_quote_and_physical_cancel() -> Result<(), Box<dyn std::error::Error>> {
    for (npc, grade, ratio, amount, coin) in [(4473, 6, 130, 308464, 0), (4482, 6, 130, 33, 32103), (4482, 0, -1, 0, 32103)] {
        let (rt, state, path) = crate::hud::breath::fixture::runtime(1280., 720.)?;
        super::super::fixture::prepare_quote(&rt, &state, npc, grade)?;
        crate::hud::enchant::dispatch(Some(&rt), &state, &Event::SpecialtyRatio(ratio), None);
        aa_ui::lua::tick(&rt.lua, 16.);
        aa_ui::lua::flush(&rt.lua);
        rt.lua
            .load(format!(
                r#"
            local ok,n,c=X2Store:GetSpecialtyRatioRefund(0)
            assert(ok and n=={amount} and (c or 0)=={coin}, 'refund')
            assert(GetSpecialtyStoreMessageBoxHandleWindow():IsVisible(), 'message box')
            assert(SPECIALTY_DIALOG and SPECIALTY_DIALOG:IsVisible(), 'dialog')
            assert(SPECIALTY_DIALOG.btnOk:IsEnabled()=={enabled}, 'ok enabled')
        "#,
                enabled = if coin == 0 || amount > 0 { "true" } else { "false" }
            ))
            .exec()?;
        crate::hud::expansion::fixture::click(&rt, "SPECIALTY_DIALOG.btnCancel")?;
        rt.lua
            .load("assert(not GetSpecialtyStoreMessageBoxHandleWindow():IsVisible() and not SPECIALTY_DIALOG:IsVisible())")
            .exec()?;
        assert!(
            state.borrow().requests.is_empty(),
            "passive quote/cancel cannot invent a server command"
        );
        assert!(rt.errors.is_empty(), "{:?}", rt.errors);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

#[test]
fn received_ratio_callback_quotes_before_later_grade_and_npc_removal() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, state) = runtime()?;
    rt.lua
        .load(
            r#"
      QUOTES={};local w=UIParent:CreateWidget('emptywidget','actualSpecialtyQuoteObserver','UIParent')
      w:RegisterEvent('NPC_INTERACTION_START')
      w:SetHandler('OnEvent',function(_,event,...)
        assert(event=='NPC_INTERACTION_START' and ...=='specialty')
        local ok,amount=X2Store:GetSpecialtyRatioRefund(1)
        assert(ok);QUOTES[#QUOTES+1]=amount
      end)
    "#,
        )
        .exec()?;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<NetEvent>()
        .insert_non_send(rt)
        .insert_non_send(HudBridge(state.clone()))
        .insert_resource(crate::hud::HudBuilt { up: true, fresh: false });
    crate::hud::enchant::plugin(&mut app);
    for event in [grade(6), Event::SpecialtyRatio(130), grade(1), Event::UnitsRemoved(vec![500])] {
        app.world_mut().write_message(NetEvent(event));
    }
    app.update();
    app.world()
        .non_send::<UiRuntime>()
        .lua
        .load("assert(#QUOTES==1 and QUOTES[1]==308464 and X2Store:GetSpecialtyRatioRefund(1)==false)")
        .exec()?;
    assert_eq!(
        crate::hud::enchant::equipment(&state.borrow(), slot::BACKPACK as u8).unwrap().grade,
        1
    );
    Ok(())
}

#[test]
fn backpack_goods_info_is_the_type3_tooltip_and_sale_queues_the_current_npc() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, state) = runtime()?;
    {
        let data = state.borrow();
        let item = crate::hud::enchant::equipment(&data, slot::BACKPACK as u8).expect("equipped pack");
        let game = data.game.get().expect("game data");
        assert!(game.specialties.is_pack(item.template_id), "template {}", item.template_id);
        assert_ne!(game.items[&item.template_id].price, 0);
    }
    rt.lua
        .load(
            r#"
        local goods = X2Equipment:GetBackPackGoodsInfo('player')
        assert(goods ~= nil and goods.itemType == 20090 and goods.lookType == 20090)
        assert(goods.itemGrade == 0 and goods.refund == 25000)
    "#,
        )
        .exec()?;
    dispatch(&state, Event::SpecialtyRatio(117));
    state.borrow_mut().dialog.npc = Some((500, 4473));
    rt.lua
        .load("assert(select('#',X2Store:SellBackPackGoods('ignored',1))==0)")
        .exec()?;
    {
        let data = state.borrow();
        assert_eq!(data.specialty.ratio, 100, "394fbadd resets the ratio after the sale sender");
        assert!(matches!(
            data.requests.as_slice(),
            [crate::hud::api::Request::SellBackpackGoods { npc_obj_id: 500 }]
        ));
    }
    // The target and the talked-to NPC disagree: the original interaction context is not inferred.
    state.borrow_mut().dialog.npc = Some((999, u32::MAX));
    rt.lua.load("X2Store:SellBackPackGoods()").exec()?;
    assert_eq!(state.borrow().requests.len(), 1);
    // Without a type-3 pack the getter is nil and the sale sends nothing.
    let mut spawn = crate::hud::expansion::fixture::spawn();
    spawn.unit.equipment.slots[slot::BACKPACK] = None;
    spawn.state.character.equipment.slots[slot::BACKPACK] = None;
    dispatch(&state, Event::PlayerSpawn(Box::new(spawn)));
    rt.lua
        .load("assert(X2Equipment:GetBackPackGoodsInfo('player')==nil); X2Store:SellBackPackGoods()")
        .exec()?;
    assert_eq!(state.borrow().requests.len(), 1);
    Ok(())
}

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE item_backpacks(item_id INTEGER,backpack_type_id INTEGER); \
      CREATE TABLE npcs(id INTEGER,specialty_coin_id INTEGER); \
      CREATE TABLE specialty_npcs(npc_id INTEGER,specialty_bundle_id INTEGER); \
      CREATE TABLE specialty_bundle_items(id INTEGER,item_id INTEGER,profit INTEGER,ratio INTEGER,specialty_bundle_id INTEGER); \
      CREATE TABLE item_grades(id INTEGER,refund_multiplier INTEGER); \
      CREATE TABLE content_configs(id INTEGER,kind_id INTEGER,value INTEGER); \
      CREATE TABLE specialties(id INTEGER,row_zone_group_id INTEGER,col_zone_group_id INTEGER,ratio INTEGER,profit INTEGER,vendor_exist TEXT); \
      INSERT INTO specialties VALUES(1,1,2,2000,4000,'t'),(2,1,3,2000,4000,'f'),(3,4,5,2000,4000,'t'),(4,4,6,2000,4000,'t'); \
      INSERT INTO item_backpacks VALUES(10,3);INSERT INTO npcs VALUES(1,0),(2,32103),(3,0); \
      INSERT INTO specialty_npcs VALUES(1,7),(2,7),(3,8),(99,7); \
      INSERT INTO specialty_bundle_items VALUES(1,10,1000,100,7),(2,10,99999,100,7); \
      INSERT INTO item_grades VALUES(0,100); \
      INSERT INTO content_configs VALUES(22,3,10000),(64,22,2);",
    )
    .unwrap();
    conn
}

#[test]
fn static_loader_keeps_first_bundle_match_id22_config_and_missing_descriptor_boundaries() {
    let conn = database();
    let tables = Tables::load(&conn).unwrap();
    let mut game = GameData::default();
    game.items.insert(
        10,
        crate::hud::data::ItemInfo {
            price: 100,
            refund: 100,
            ..Default::default()
        },
    );
    let item = Item {
        template_id: 10,
        ..Default::default()
    };
    assert_eq!(tables.quote(&game, 1, &item, 100), Ok(Some((200, 0))));
    assert_eq!(
        tables.quote(&game, 2, &item, 100),
        Ok(Some((0, 32103))),
        "known zero coin amount remains success"
    );
    assert_eq!(tables.quote(&game, 3, &item, 100), Err(225));
    assert_eq!(
        tables.quote(&game, 99, &item, 100),
        Ok(None),
        "orphan NPC map cannot fabricate its descriptor"
    );
    assert_eq!(
        tables.quote(
            &game,
            1,
            &Item {
                template_id: 11,
                ..item.clone()
            },
            100
        ),
        Ok(None)
    );
    assert_eq!(
        tables.quote(
            &game,
            1,
            &Item {
                grade: 255,
                ..item.clone()
            },
            100
        ),
        Ok(None)
    );
    game.items.get_mut(&10).unwrap().price = 0;
    assert_eq!(tables.quote(&game, 1, &item, 100), Ok(None));
    conn.execute_batch("DROP TABLE item_backpacks;").unwrap();
    assert!(Tables::load(&conn).is_err());
}

#[test]
fn production_and_sellable_zone_groups_follow_the_native_rows() {
    let tables = Tables::load(&database()).unwrap();
    assert_eq!(tables.production_zones(), vec![1, 4]);
    assert_eq!(tables.sellable_zones(1), vec![2], "vendor_exist filters row 1 col 3");
    assert_eq!(tables.sellable_zones(4), vec![5, 6]);
    assert!(tables.sellable_zones(99).is_empty());
}

#[test]
fn route_ratio_native_builds_the_zone_lists_and_requests_once_per_cooldown() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, state) = runtime()?;
    // First specialties row id 1: production 1, sellable 2,3,4,5,8,12,17,20,27,59.
    let list = rt
        .lua
        .load(
            r#"
      local zones=X2Store:GetProductionZoneGroups()
      assert(#zones>0 and zones[1].id==1 and #zones[1].name>0)
      for _,z in ipairs(zones) do assert(type(z.id)=='number' and type(z.name)=='string' and #z.name>0) end
      local sell=X2Store:GetSellableZoneGroups(1)
      local ids={}
      for _,z in ipairs(sell) do ids[z.id]=z.name end
      assert(#sell==10 and ids[2] and ids[3] and ids[4] and ids[5] and ids[8] and ids[12] and ids[17] and ids[20] and ids[27] and ids[59])
      assert(not ids[1] and not ids[6] and not ids[7] and not ids[9] and not ids[10])
      assert(#X2Store:GetSellableZoneGroups(99999)==0)
    "#,
        )
        .exec();
    list?;
    let first: f64 = rt.lua.load("return X2Store:GetSpecialtyRatioBetween(1,2)").eval()?;
    assert_eq!(first, 5000.0);
    {
        let data = state.borrow();
        assert!(matches!(
            data.requests.as_slice(),
            [crate::hud::api::Request::SpecialtyRatioBetween { from: 1, to: 2 }]
        ));
    }
    let second: f64 = rt.lua.load("return X2Store:GetSpecialtyRatioBetween(1,2)").eval()?;
    assert!(second > 0.0 && second <= 5000.0);
    assert_eq!(state.borrow().requests.len(), 1, "the cooldown suppresses a second request");
    assert_eq!(
        rt.lua.load("return X2Store:GetSpecialtyRatioBetween('1',2)").eval::<f64>()?,
        0.0,
        "the native adapter requires numeric arguments"
    );
    Ok(())
}

#[test]
fn specialty_current_receipt_raises_the_rate_rows() -> Result<(), Box<dyn std::error::Error>> {
    let (rt, state) = runtime()?;
    rt.lua
        .load(
            r#"
      BETWEEN=nil
      UIParent:SetEventHandler('SPECIALTY_RATIO_BETWEEN_INFO',function(rows) BETWEEN=rows end)
    "#,
        )
        .exec()?;
    super::super::receive(
        Some(&rt.lua),
        &state,
        &Event::SpecialtyCurrent {
            from: 1,
            to: 2,
            rates: vec![(20090, 117), (999999, 130)],
        },
    );
    rt.lua
        .load(
            r#"
      assert(#BETWEEN==2)
      assert(BETWEEN[1].ratio==117 and BETWEEN[1].itemInfo.itemType==20090 and #BETWEEN[1].itemInfo.name>0)
      assert(BETWEEN[2].ratio==130 and BETWEEN[2].itemInfo.itemType==999999 and BETWEEN[2].itemInfo.name=='')
    "#,
        )
        .exec()?;
    Ok(())
}
