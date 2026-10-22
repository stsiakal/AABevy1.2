//! Explicit native slave controls. Requests never mutate authoritative owned/attachment state.

use crate::opcodes::cs;
use crate::model::Item;
use crate::wire::Writer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlaveItemLocation {
    pub container: u8,
    pub slot: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlaveSpawnRequest {
    pub template_id: u32,
    /// Native long fixed-point coordinates, retained without a client-side position planner.
    pub x: i64,
    pub y: i64,
    pub z: f32,
    pub yaw: f32,
    pub item_id: u64,
    /// Present exactly when item_id != 0. Native writes two bytes, not the lane's four.
    pub item_location: Option<SlaveItemLocation>,
    pub hide_spawn_effect: bool,
}

/// Container byte of a ship's / vehicle's equipment (`EQUIPMENT_SLAVE`).
pub const SLAVE_EQUIPMENT_CONTAINER: u8 = 0xf2;
/// Container byte of the character's bag.
pub const BAG_CONTAINER: u8 = 2;
/// Equip slots a slave can have (`slave_equip_slots.equip_slot_id`, 0..=26).
pub const SLAVE_EQUIPMENT_SLOTS: u8 = 27;

/// `CSChangeSlaveEquipment` (1.2 body): the header, one item pair echoed as the client sees it,
/// and the two locations. The first location is the bag slot (or another slave slot), the second
/// the slave equipment slot; an empty first item unequips the second.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlaveEquipmentRequest {
    /// Stored character id, not the runtime object id.
    pub character_id: u32,
    pub slave_tl_id: u16,
    /// The slave's database id; the server accepts 0 and resolves the slave by timeline id.
    pub db_slave_id: u32,
    pub bts: bool,
    pub first_item: Option<Item>,
    pub second_item: Option<Item>,
    pub first_location: SlaveItemLocation,
    pub second_location: SlaveItemLocation,
}

impl SlaveEquipmentRequest {
    pub fn validate(&self) -> std::result::Result<(), &'static str> {
        if self.character_id == 0 || self.slave_tl_id == 0 {
            return Err("slave equipment needs the stored character and the slave timeline id");
        }
        let first = self.first_location;
        let second = self.second_location;
        if second.container != SLAVE_EQUIPMENT_CONTAINER || second.slot >= SLAVE_EQUIPMENT_SLOTS {
            return Err("the second slave equipment location must be an equipment slot (0xf2, 0..26)");
        }
        let first_ok = match first.container {
            BAG_CONTAINER => true,
            SLAVE_EQUIPMENT_CONTAINER => first.slot < SLAVE_EQUIPMENT_SLOTS,
            _ => false,
        };
        if !first_ok {
            return Err("the first slave equipment location must be a bag or an equipment slot");
        }
        if self.first_item.is_none() && self.second_item.is_none() {
            return Err("slave equipment move has no item");
        }
        for item in [&self.first_item, &self.second_item].into_iter().flatten() {
            if item.template_id == 0 || item.id == 0 {
                return Err("slave equipment items must be real items");
            }
            if item.detail.kind() != 0 && item.detail_payload.is_none() {
                return Err("slave equipment item lacks its received detail bytes");
            }
        }
        Ok(())
    }

    fn write(&self, w: &mut Writer) {
        w.u32(self.character_id)
            .u16(self.slave_tl_id)
            .u32(self.db_slave_id)
            .bool(self.bts);
        write_item(w, self.first_item.as_ref());
        write_item(w, self.second_item.as_ref());
        w.u8(self.first_location.container)
            .u8(self.first_location.slot)
            .u8(self.second_location.container)
            .u8(self.second_location.slot);
    }
}

/// `Item.Write`, from the retained detail bytes of a received item.
fn write_item(w: &mut Writer, item: Option<&Item>) {
    let Some(i) = item else {
        w.u32(0);
        return;
    };
    w.u32(i.template_id)
        .u64(i.id)
        .u8(i.grade)
        .u8(i.flags)
        .i32(i.count)
        .u8(i.detail.kind())
        .raw(i.detail_payload.as_deref().unwrap_or(&[]))
        .i64(i.create_time)
        .i32(i.lifespan_mins)
        .u32(i.made_unit_id)
        .u8(i.world_id)
        .i64(i.unsecure_time)
        .i64(i.unpack_time);
}

#[derive(Clone, Debug, PartialEq)]
pub enum SlaveCommand {
    Bind {
        tl_id: u16,
    },
    Unbind {
        tl_id: u16,
    },
    Dismiss {
        tl_id: u16,
    },
    Despawn {
        obj_id: u32,
    },
    Rename {
        tl_id: u16,
        name: String,
    },
    /// Exact native packet only: lane handler is a stub and its reader mismatches item locations.
    Spawn(SlaveSpawnRequest),
    /// Equip, unequip or re-slot a ship / vehicle part.
    ChangeEquipment(Box<SlaveEquipmentRequest>),
}

impl SlaveCommand {
    pub fn validate(&self) -> std::result::Result<(), &'static str> {
        match self {
            Self::Bind { tl_id }
            | Self::Unbind { tl_id }
            | Self::Dismiss { tl_id }
            | Self::Rename { tl_id, .. }
                if *tl_id == 0 =>
            {
                return Err("slave timeline ID must be nonzero");
            }
            Self::Despawn { obj_id } if *obj_id == 0 || *obj_id > 0xffffff => {
                return Err("slave object ID must be nonzero and fit in 24 bits");
            }
            _ => {}
        }
        match self {
            Self::Rename { name, .. }
                if name.trim().is_empty() || name.len() > 128 || name.contains('\0') =>
            {
                Err(
                    "slave name must contain text, fit the native 128-byte buffer and contain no NUL",
                )
            }
            Self::ChangeEquipment(p) => p.validate(),
            Self::Spawn(p) if p.template_id == 0 => Err("slave template ID must be nonzero"),
            Self::Spawn(p) if !p.z.is_finite() || !p.yaw.is_finite() => {
                Err("slave spawn height and yaw must be finite")
            }
            Self::Spawn(p) if (p.item_id != 0) != p.item_location.is_some() => {
                Err("slave item location must be present exactly when the item ID is nonzero")
            }
            _ => Ok(()),
        }
    }

    pub fn opcode(&self) -> u16 {
        match self {
            Self::Bind { .. } => cs::CSBindSlavePacket,
            Self::Unbind { .. } => cs::CSDiscardSlavePacket,
            Self::Dismiss { .. } => cs::CSDestroySlavePacket,
            Self::Despawn { .. } => cs::CSDespawnSlavePacket,
            Self::Rename { .. } => cs::CSChangeSlaveNamePacket,
            Self::Spawn(_) => cs::CSSpawnSlavePacket,
            Self::ChangeEquipment(_) => cs::CSChangeSlaveEquipmentPacket,
        }
    }

    pub fn write(&self, w: &mut Writer) {
        match self {
            Self::Bind { tl_id } | Self::Unbind { tl_id } | Self::Dismiss { tl_id } => {
                w.u16(*tl_id);
            }
            Self::Despawn { obj_id } => {
                w.bc(*obj_id);
            }
            Self::Rename { tl_id, name } => {
                w.u16(*tl_id).string(name);
            }
            Self::ChangeEquipment(p) => p.write(w),
            Self::Spawn(p) => {
                w.u32(p.template_id)
                    .i64(p.x)
                    .i64(p.y)
                    .f32(p.z)
                    .f32(p.yaw)
                    .u64(p.item_id);
                if p.item_id != 0 {
                    if let Some(location) = p.item_location {
                        w.u8(location.container).u8(location.slot);
                    }
                }
                w.bool(p.hide_spawn_effect);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ClientMsg;

    pub(crate) fn spawn() -> SlaveSpawnRequest {
        SlaveSpawnRequest {
            template_id: 68,
            x: 0x0102030405060708,
            y: 0x1122334455667788,
            z: 3.5,
            yaw: 0.5,
            item_id: 0,
            item_location: None,
            hide_spawn_effect: true,
        }
    }

    #[test]
    fn native_controls_have_exact_timeline_object_and_string_widths() {
        let cases = [
            (SlaveCommand::Bind { tl_id: 0x102 }, 0x31, vec![2, 1]),
            (SlaveCommand::Unbind { tl_id: 0xffff }, 0x32, vec![255, 255]),
            (SlaveCommand::Dismiss { tl_id: 0x102 }, 0x30, vec![2, 1]),
            (
                SlaveCommand::Despawn { obj_id: 0xaabbcc },
                0x2f,
                vec![0xcc, 0xbb, 0xaa],
            ),
            (
                SlaveCommand::Rename {
                    tl_id: 0x102,
                    name: "Boat".into(),
                },
                0x34,
                vec![2, 1, 4, 0, b'B', b'o', b'a', b't'],
            ),
        ];
        for (command, opcode, body) in cases {
            assert!(command.validate().is_ok());
            let msg = ClientMsg::Slave(command);
            assert_eq!(msg.opcode(), opcode);
            assert_eq!(msg.encode_body(), body);
        }
    }

    fn cannon(slot: i32) -> Item {
        // A ship part: tag 10, 12 opaque detail bytes.
        Item {
            template_id: 28852,
            id: 0x1122334455667788,
            grade: 2,
            flags: 3,
            count: slot,
            detail: crate::model::ItemDetail::Other(10),
            detail_payload: Some((1..=12).collect()),
            create_time: 10,
            lifespan_mins: 11,
            made_unit_id: 12,
            world_id: 13,
            unsecure_time: 14,
            unpack_time: 15,
            ..Item::default()
        }
    }

    fn equip_request() -> SlaveEquipmentRequest {
        SlaveEquipmentRequest {
            character_id: 0x0a0b0c0d,
            slave_tl_id: 0x0102,
            db_slave_id: 0x11223344,
            bts: true,
            first_item: Some(cannon(1)),
            second_item: None,
            first_location: SlaveItemLocation { container: BAG_CONTAINER, slot: 9 },
            second_location: SlaveItemLocation { container: SLAVE_EQUIPMENT_CONTAINER, slot: 4 },
        }
    }

    #[test]
    fn change_slave_equipment_is_byte_exact_like_the_server_reader() {
        let command = SlaveCommand::ChangeEquipment(Box::new(equip_request()));
        assert!(command.validate().is_ok());
        let msg = ClientMsg::Slave(command);
        assert_eq!(msg.opcode(), 0x37);
        let mut expected = vec![0x0d, 0x0c, 0x0b, 0x0a, 2, 1, 0x44, 0x33, 0x22, 0x11, 1];
        // item1: template, id, grade, flags, count, detail tag 10 + 12 bytes, six trailing fields
        expected.extend([0xb4, 0x70, 0, 0, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 2, 3, 1, 0, 0, 0, 10]);
        expected.extend(1..=12);
        expected.extend([10, 0, 0, 0, 0, 0, 0, 0, 11, 0, 0, 0, 12, 0, 0, 0, 13]);
        expected.extend([14, 0, 0, 0, 0, 0, 0, 0, 15, 0, 0, 0, 0, 0, 0, 0]);
        // item2 empty, then bag 9 and slave slot 0xf2 / 4
        expected.extend([0, 0, 0, 0, 2, 9, 0xf2, 4]);
        assert_eq!(msg.encode_body(), expected);
    }

    #[test]
    fn unequip_echoes_the_slave_item_second_and_rejects_bad_locations() {
        let unequip = SlaveEquipmentRequest {
            first_item: None,
            second_item: Some(cannon(1)),
            ..equip_request()
        };
        assert!(unequip.validate().is_ok());
        let body = ClientMsg::Slave(SlaveCommand::ChangeEquipment(Box::new(unequip))).encode_body();
        assert_eq!(&body[11..15], &[0, 0, 0, 0], "first item empty");
        assert_eq!(&body[body.len() - 4..], &[2, 9, 0xf2, 4]);
        let bad = [
            SlaveEquipmentRequest { character_id: 0, ..equip_request() },
            SlaveEquipmentRequest { slave_tl_id: 0, ..equip_request() },
            SlaveEquipmentRequest { first_item: None, ..equip_request() },
            SlaveEquipmentRequest {
                second_location: SlaveItemLocation { container: BAG_CONTAINER, slot: 1 },
                ..equip_request()
            },
            SlaveEquipmentRequest {
                second_location: SlaveItemLocation { container: SLAVE_EQUIPMENT_CONTAINER, slot: 27 },
                ..equip_request()
            },
            SlaveEquipmentRequest {
                first_location: SlaveItemLocation { container: 1, slot: 0 },
                ..equip_request()
            },
            SlaveEquipmentRequest {
                first_item: Some(Item { detail_payload: None, ..cannon(1) }),
                ..equip_request()
            },
        ];
        for request in bad {
            assert!(request.validate().is_err(), "{request:?}");
        }
    }

    #[test]
    fn native_spawn_conditionally_writes_only_two_item_location_bytes() {
        let p = spawn();
        let base = vec![
            68, 0, 0, 0, 8, 7, 6, 5, 4, 3, 2, 1, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, 0,
            0, 0x60, 0x40, 0, 0, 0, 0x3f,
        ];
        let mut expected = base.clone();
        expected.extend([0; 8]);
        expected.push(1);
        let command = SlaveCommand::Spawn(p.clone());
        assert!(command.validate().is_ok());
        let msg = ClientMsg::Slave(command);
        assert_eq!(msg.opcode(), 0x2e);
        assert_eq!(msg.encode_body(), expected);
        assert_eq!(expected.len(), 37);
        let p = SlaveSpawnRequest {
            item_id: 0x0102030405060708,
            item_location: Some(SlaveItemLocation {
                container: 2,
                slot: 9,
            }),
            hide_spawn_effect: false,
            ..p
        };
        let mut expected = base;
        expected.extend([8, 7, 6, 5, 4, 3, 2, 1, 2, 9, 0]);
        assert_eq!(
            ClientMsg::Slave(SlaveCommand::Spawn(p)).encode_body(),
            expected
        );
        assert_eq!(expected.len(), 39);
        // The lane reader requires four slot bytes then hide, so it cannot read this native body.
        let mut r = crate::wire::Reader::new(&expected);
        r.skip(36).unwrap();
        assert!(r.bytes(5).is_err());
    }

    #[test]
    fn malformed_ids_names_geometry_and_conditional_item_locations_are_rejected() {
        let mut invalid = vec![
            SlaveCommand::Bind { tl_id: 0 },
            SlaveCommand::Unbind { tl_id: 0 },
            SlaveCommand::Dismiss { tl_id: 0 },
            SlaveCommand::Despawn { obj_id: 0 },
            SlaveCommand::Despawn { obj_id: 0x1000000 },
        ];
        for name in ["".into(), " ".into(), "x\0y".into(), "x".repeat(129)] {
            invalid.push(SlaveCommand::Rename { tl_id: 1, name });
        }
        let p = spawn();
        for p in [
            SlaveSpawnRequest {
                template_id: 0,
                ..p.clone()
            },
            SlaveSpawnRequest {
                z: f32::NAN,
                ..p.clone()
            },
            SlaveSpawnRequest {
                yaw: f32::INFINITY,
                ..p.clone()
            },
            SlaveSpawnRequest {
                item_id: 9,
                ..p.clone()
            },
            SlaveSpawnRequest {
                item_location: Some(SlaveItemLocation {
                    container: 2,
                    slot: 0,
                }),
                ..p
            },
        ] {
            invalid.push(SlaveCommand::Spawn(p));
        }
        for command in invalid {
            assert!(command.validate().is_err(), "{command:?}");
        }
        assert!(
            SlaveCommand::Rename {
                tl_id: 1,
                name: "x".repeat(128)
            }
            .validate()
            .is_ok()
        );
    }
}
