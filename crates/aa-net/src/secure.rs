//! Item security locks: `CSItemSecure` / `CSItemUnsecure` and the equipment-wide
//! `CSEquipmentsSecure` / `CSEquipmentsUnsecure` (`docs/research/re-inventory.md` 3.7).
//!
//! The lock state is item flag `0x04`. Locking sets it; unlocking keeps it and sets
//! `unsecure_time` to the end of the 600 s delay, which the client renders as the
//! "unlocking" state until that time passes.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{items::container, state::Inventory, wire::Writer};

/// `ItemFlag.Secure` (`models/item.rs:653`).
pub const FLAG_SECURE: u8 = 0x04;
/// `X2Item:GetSecurityUnlockDelayTime()`; the unlock dialog prints it divided by 60.
pub const UNLOCK_DELAY_SECONDS: i64 = 600;

/// The three states the original padlock overlay and tooltip line render
/// (`ITEM_SECURITY_LOCKED` / `_UNLOCKED` / `_UNLOCKING`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemSecurityState {
    Unlocked,
    Locked,
    Unlocking,
}

impl ItemSecurityState {
    /// The native constant: 1 locked, 2 unlocked, 3 unlocking.
    pub fn code(self) -> u8 {
        match self {
            Self::Locked => 1,
            Self::Unlocked => 2,
            Self::Unlocking => 3,
        }
    }
}

/// The state of an item instance. A pending unlock whose deadline has passed reads as
/// unlocked even if the stale `Secure` bit is still on the item.
pub fn state(flags: u8, unsecure_time: i64, now: i64) -> ItemSecurityState {
    if flags & FLAG_SECURE == 0 {
        return ItemSecurityState::Unlocked;
    }
    match unsecure_time {
        t if t <= 0 => ItemSecurityState::Locked,
        t if t > now => ItemSecurityState::Unlocking,
        _ => ItemSecurityState::Unlocked,
    }
}

/// Remaining unlock delay as the period table `locale.time.GetRemainDateToDateFormat`
/// formats (`tooltip.alb:1470-1473`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SecurityPeriod {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
}

/// `unsecure_time - now` split into the tooltip's period fields; `None` once the delay
/// has elapsed. The original client only ever shows the 600 s default, so the calendar
/// parts exist for completeness and use the usual 365-day year / 30-day month.
pub fn remaining(unsecure_time: i64, now: i64) -> Option<SecurityPeriod> {
    let mut secs = unsecure_time.checked_sub(now)?;
    if secs <= 0 {
        return None;
    }
    let mut period = SecurityPeriod::default();
    for (field, unit) in [
        (&mut period.year, 365 * 86_400),
        (&mut period.month, 30 * 86_400),
        (&mut period.day, 86_400),
        (&mut period.hour, 3_600),
        (&mut period.minute, 60),
    ] {
        *field = secs / unit;
        secs %= unit;
    }
    period.second = secs;
    Some(period)
}

/// The server's timestamp clock (the item times are Unix seconds).
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// An item identity that `CSItemSecure` / `CSItemUnsecure` name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemSecurityRequest {
    container: u8,
    slot: u8,
    item_id: u64,
}

impl ItemSecurityRequest {
    pub fn new(container: u8, slot: u8, item_id: u64) -> Result<Self, &'static str> {
        if item_id == 0 {
            return Err("item security requires an existing item database identity");
        }
        Ok(Self { container, slot, item_id })
    }

    pub fn container(&self) -> u8 {
        self.container
    }

    pub fn slot(&self) -> u8 {
        self.slot
    }

    pub fn item_id(&self) -> u64 {
        self.item_id
    }

    /// The item is still the one the mode was entered for, in the same slot.
    pub fn recheck(&self, inventory: &Inventory) -> Result<(), &'static str> {
        let index = usize::from(self.slot);
        let current = match self.container {
            container::INVENTORY if u32::from(self.slot) < inventory.bag_slots => inventory.bag.get(index),
            container::BANK if u32::from(self.slot) < inventory.bank_slots => inventory.bank.get(index),
            container::EQUIPMENT => inventory.equipment.slots.get(index),
            _ => return Err("item security location is not present in the received owned inventory"),
        };
        if current
            .and_then(Option::as_ref)
            .is_some_and(|item| item.id == self.item_id && item.template_id != 0)
        {
            Ok(())
        } else {
            Err("item security item identity or location changed")
        }
    }

    /// `item_handlers.rs:487-499` reads `u8, slotType, u8, slot, itemId`; the two
    /// skipped bytes are the zero padding of the original writer.
    pub(crate) fn write(&self, w: &mut Writer) {
        w.u8(0).u8(self.container).u8(0).u8(self.slot).u64(self.item_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{game::ClientMsg, model::Item};

    fn inventory() -> Inventory {
        let mut inv = Inventory::default();
        inv.bag_slots = 1;
        inv.bag = vec![Some(Item {
            id: 0x0102_0304_0506_0708,
            template_id: 99,
            ..Default::default()
        })];
        inv
    }

    #[test]
    fn item_security_packets_match_the_server_reader_and_the_empty_equipment_forms() {
        let request = ItemSecurityRequest::new(2, 5, 0x0102_0304_0506_0708).unwrap();
        let body = vec![0, 2, 0, 5, 8, 7, 6, 5, 4, 3, 2, 1];
        for (msg, opcode) in [
            (ClientMsg::ItemSecure(request), 0x04a),
            (ClientMsg::ItemUnsecure(request), 0x04b),
        ] {
            assert_eq!(msg.opcode(), opcode);
            assert_eq!(msg.encode_body(), body);
        }
        for (msg, opcode) in [
            (ClientMsg::EquipmentsSecure, 0x04c),
            (ClientMsg::EquipmentsUnsecure, 0x04d),
        ] {
            assert_eq!(msg.opcode(), opcode);
            assert!(msg.encode_body().is_empty());
        }
        assert!(ItemSecurityRequest::new(2, 0, 0).is_err());
    }

    #[test]
    fn lock_state_follows_the_flag_and_the_unlock_deadline() {
        let now = 1_700_000_000;
        assert_eq!(state(0, 0, now), ItemSecurityState::Unlocked);
        assert_eq!(state(FLAG_SECURE, 0, now), ItemSecurityState::Locked);
        assert_eq!(state(FLAG_SECURE, now + UNLOCK_DELAY_SECONDS, now), ItemSecurityState::Unlocking);
        assert_eq!(state(FLAG_SECURE, now, now), ItemSecurityState::Unlocked);
        assert_eq!(ItemSecurityState::Locked.code(), 1);
        assert_eq!(ItemSecurityState::Unlocked.code(), 2);
        assert_eq!(ItemSecurityState::Unlocking.code(), 3);
        assert_eq!(remaining(now + UNLOCK_DELAY_SECONDS, now), Some(SecurityPeriod { minute: 10, ..Default::default() }));
        assert_eq!(remaining(now + 90_061, now), Some(SecurityPeriod { day: 1, hour: 1, minute: 1, second: 1, ..Default::default() }));
        assert_eq!(remaining(now, now), None);
    }

    #[test]
    fn request_rechecks_the_live_item_location_without_mutating_it() {
        let request = ItemSecurityRequest::new(2, 0, 0x0102_0304_0506_0708).unwrap();
        let mut inv = inventory();
        assert_eq!((request.container(), request.slot(), request.item_id()), (2, 0, 0x0102_0304_0506_0708));
        assert!(request.recheck(&inv).is_ok());
        inv.bag[0].as_mut().unwrap().id = 77;
        assert!(request.recheck(&inv).is_err());
        inv.bag[0].as_mut().unwrap().id = 0x0102_0304_0506_0708;
        inv.bag[0].as_mut().unwrap().template_id = 0;
        assert!(request.recheck(&inv).is_err());
        inv.bag_slots = 0;
        assert!(request.recheck(&inv).is_err());
        assert!(ItemSecurityRequest::new(252, 0, 1).unwrap().recheck(&inv).is_err());
    }
}
