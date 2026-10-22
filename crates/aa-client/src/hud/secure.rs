//! Item security locks: the four-button lock window's selection modes
//! (`inventory/sort_bag.alb:281-347`) and the equipment-wide requests. The current lock
//! state itself is item flag `0x04`; `tips.rs` puts it into the item info table, which the
//! original bag cell and tooltip scripts already render as the padlock overlay and line.

use mlua::{Lua, Table};

use super::api::{Request, Shared};

const SECURE_LUA: &str = include_str!("secure.lua");

pub fn install(lua: &Lua, hud: &Table, bridge: &Shared) -> mlua::Result<()> {
    let b = bridge.clone();
    hud.set(
        "security_state",
        lua.create_function(move |_, slot: usize| {
            let d = b.borrow();
            let Some(s) = d.panels.bag.get(slot.saturating_sub(1)).and_then(Option::as_ref) else {
                return Ok(None);
            };
            let Some(flags) = s.flags else { return Ok(None) };
            Ok(Some(i64::from(
                aa_net::secure::state(flags, s.unsecure_time, aa_net::secure::unix_now()).code(),
            )))
        })?,
    )?;
    let b = bridge.clone();
    hud.set(
        "secure_item",
        lua.create_function(move |_, (slot, lock): (usize, bool)| {
            let request = {
                let d = b.borrow();
                let Some(s) = d.panels.bag.get(slot.saturating_sub(1)).and_then(Option::as_ref) else {
                    return Ok(false);
                };
                let Ok(slot) = u8::try_from(slot.saturating_sub(1)) else { return Ok(false) };
                aa_net::secure::ItemSecurityRequest::new(2, slot, s.uid).ok()
            };
            let Some(request) = request else { return Ok(false) };
            b.borrow_mut().requests.push(Request::SecureItem { request, lock });
            Ok(true)
        })?,
    )?;
    let b = bridge.clone();
    hud.set(
        "secure_equipments",
        lua.create_function(move |_, lock: bool| {
            b.borrow_mut().requests.push(Request::SecureEquipments { lock });
            Ok(())
        })?,
    )?;
    lua.load(SECURE_LUA).set_name("@secure.lua").exec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::api::HudData;
    use super::super::panels::SlotSnap;

    #[test]
    fn lock_modes_gate_bag_clicks_and_send_the_original_requests() -> mlua::Result<()> {
        let lua = Lua::new();
        let now = aa_net::secure::unix_now();
        lua.load(
            r#"
          ITEM_SECURITY_LOCKED, ITEM_SECURITY_UNLOCKED, ITEM_SECURITY_UNLOCKING = 1, 2, 3
          X2Item, X2Equipment, HUD = {}, {}, {}
        "#,
        )
        .exec()?;
        let hud: Table = lua.globals().get("HUD")?;
        let bridge = std::rc::Rc::new(std::cell::RefCell::new(HudData::default()));
        bridge.borrow_mut().panels.bag = vec![
            Some(SlotSnap { uid: 111, flags: Some(0), ..Default::default() }),
            Some(SlotSnap { uid: 222, flags: Some(aa_net::secure::FLAG_SECURE), ..Default::default() }),
            Some(SlotSnap {
                uid: 333,
                flags: Some(aa_net::secure::FLAG_SECURE),
                unsecure_time: now + aa_net::secure::UNLOCK_DELAY_SECONDS,
                ..Default::default()
            }),
            None,
        ];
        install(&lua, &hud, &bridge)?;
        lua.load(
            r#"
          assert(X2Item:GetSecurityUnlockDelayTime() == 600)
          assert(not X2Item:IsInSecurityLockMode() and not X2Item:IsInSecurityUnlockMode())
          assert(X2Item:IsSecurityLockable(1) and not X2Item:IsSecurityLockable(2))
          assert(X2Item:IsSecurityUnlockable(2) and not X2Item:IsSecurityUnlockable(3))
          assert(HUD.security_click(1) == false)              -- no mode active
          X2Item:EnterSecurityLockMode()
          assert(X2Item:IsInSecurityLockMode() and not X2Item:IsInSecurityUnlockMode())
          assert(HUD.security_click(1) == true)               -- unlocked item: lock it
          assert(HUD.security_click(2) == true)               -- locked item: consumed, not sent
          X2Item:LeaveSecurityLockMode()
          X2Item:EnterSecurityUnlockMode()
          assert(HUD.security_click(2) == true)               -- locked item: start unlocking
          assert(HUD.security_click(3) == true)               -- already unlocking: not sent
          HUD.security_click(4)                                -- empty cell: consumed silently
          X2Item:LeaveSecurityUnlockMode()
          X2Equipment.SecurityLock()
          X2Equipment.SecurityUnlock()
        "#,
        )
        .exec()?;
        let requests = &bridge.borrow().requests;
        assert_eq!(requests.len(), 4, "{requests:?}");
        assert!(matches!(
            &requests[0],
            Request::SecureItem { request, lock: true } if request.item_id() == 111 && request.slot() == 0
        ));
        assert!(matches!(
            &requests[1],
            Request::SecureItem { request, lock: false } if request.item_id() == 222 && request.slot() == 1
        ));
        assert!(matches!(requests[2], Request::SecureEquipments { lock: true }));
        assert!(matches!(requests[3], Request::SecureEquipments { lock: false }));
        Ok(())
    }

    #[test]
    #[ignore = "loads the original world-stage scripts and read-only game catalog"]
    fn original_lock_button_opens_the_four_button_window_and_item_modes() -> Result<(), Box<dyn std::error::Error>> {
        let (rt, bridge, path, _) = super::super::expansion::fixture::runtime(1280.0, 720.0)?;
        // The server's SCItemTaskSuccess sets the Secure bit. Deliver it before any lock mode
        // is active, so the bag's normal update path draws the padlock.
        bridge.borrow_mut().panels.bag[4].as_mut().unwrap().flags = Some(aa_net::secure::FLAG_SECURE);
        super::super::api::push_changes(&rt, &bridge);
        rt.fire_event("BAG_UPDATE", (-1i64, -1i64));
        let state: Option<i64> = rt.lua.load("local i = X2Bag:GetBagItemInfo(0, 5); return i and i.securityState").eval()?;
        let fl: Option<i64> = rt.lua.load("return HUD.bag[5] and HUD.bag[5].fl").eval()?;
        let visible: bool = rt.lua.load("return newbag.slots.btns[5].lockIcon:IsVisible()").eval()?;
        assert_eq!((state, fl, visible), (Some(1), Some(4), true), "lock icon and tooltip state");

        super::super::expansion::fixture::click(&rt, "newbag.lockButton")?;
        rt.lua
            .load(
                r#"
        assert(newbag.itemLockWnd and newbag.itemLockWnd:IsVisible())
        assert(newbag.itemLockWnd.itemLockBtn and newbag.itemLockWnd.equipLockBtn)
        assert(newbag.itemLockWnd.itemUnlockBtn and newbag.itemLockWnd.equipUnlockBtn)
        assert(not X2Item:IsInSecurityLockMode() and not X2Item:IsInSecurityUnlockMode())
    "#,
            )
            .exec()?;
        // Unlock Item: a locked bag cell starts the server-side unlock delay.
        super::super::expansion::fixture::click(&rt, "newbag.itemLockWnd.itemUnlockBtn")?;
        assert!(rt.lua.load("return X2Item:IsInSecurityUnlockMode()").eval::<bool>()?);
        assert!(rt.lua.load("return X2Item:IsSecurityUnlockable(5)").eval::<bool>()?);
        super::super::expansion::fixture::click(&rt, "newbag.slots.btns[5]")?;
        rt.lua.load("newbag.itemLockWnd:Show(false)").exec()?;
        assert!(!rt.lua.load("return X2Item:IsInSecurityLockMode() or X2Item:IsInSecurityUnlockMode()").eval::<bool>()?);

        // The server cleared the flag: Lock Item now targets the same cell.
        {
            let mut data = bridge.borrow_mut();
            let slot = data.panels.bag[4].as_mut().unwrap();
            slot.flags = Some(0);
            slot.unsecure_time = 0;
        }
        super::super::api::push_changes(&rt, &bridge);
        rt.fire_event("BAG_UPDATE", (-1i64, -1i64));
        super::super::expansion::fixture::click(&rt, "newbag.lockButton")?;
        super::super::expansion::fixture::click(&rt, "newbag.itemLockWnd.itemLockBtn")?;
        assert!(rt.lua.load("return X2Item:IsInSecurityLockMode()").eval::<bool>()?);
        super::super::expansion::fixture::click(&rt, "newbag.slots.btns[5]")?;
        rt.lua.load("newbag.itemLockWnd:Show(false)").exec()?;
        assert!(!rt.lua.load("return X2Item:IsInSecurityLockMode() or X2Item:IsInSecurityUnlockMode()").eval::<bool>()?);
        let requests = &bridge.borrow().requests;
        assert!(matches!(
            &requests[..],
            [
                Request::SecureItem { request: first, lock: false },
                Request::SecureItem { request: second, lock: true }
            ] if first.item_id() == 9_007_199_254_740_993 && second.item_id() == 9_007_199_254_740_993
        ), "{requests:?}");
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}
