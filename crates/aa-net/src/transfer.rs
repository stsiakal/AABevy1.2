//! `CSBoardingTransfer` (carriages, ferries and airships on fixed paths): the target's timeline id and
//! the attach point the original client picks, or 0 to leave. The server's 1.2 reader reads exactly
//! `{tl: u16, ap: u8}`; no other field is on the wire.

use crate::opcodes::cs;
use crate::wire::Writer;

/// The first passenger attach point of a transfer (`FUN_393546b0`: the free one of 2..=7).
pub const FIRST_PASSENGER_POINT: u8 = 2;
/// One past the last passenger attach point of a transfer.
pub const LAST_PASSENGER_POINT: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferCommand {
    /// Board the targeted transfer at the first free attach point 2..=7.
    Board { tl_id: u16, ap: u8 },
    /// Leave the transfer: the same opcode with attach point 0.
    Leave { tl_id: u16 },
}

impl TransferCommand {
    pub fn validate(&self) -> std::result::Result<(), &'static str> {
        match self {
            Self::Board { tl_id, ap } => {
                if *tl_id == 0 {
                    return Err("transfer timeline ID must be nonzero");
                }
                if !(FIRST_PASSENGER_POINT..=LAST_PASSENGER_POINT).contains(ap) {
                    return Err("a transfer boarding point is 2..=7");
                }
            }
            Self::Leave { tl_id } if *tl_id == 0 => {
                return Err("transfer timeline ID must be nonzero");
            }
            Self::Leave { .. } => {}
        }
        Ok(())
    }

    pub fn opcode(&self) -> u16 {
        cs::CSBoardingTransferPacket
    }

    pub fn write(&self, w: &mut Writer) {
        match self {
            Self::Board { tl_id, ap } => {
                w.u16(*tl_id).u8(*ap);
            }
            Self::Leave { tl_id } => {
                w.u16(*tl_id).u8(0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::ClientMsg;
    use crate::wire::Reader;

    /// The AAEmu server's `CSBoardingTransferPacket` reader: `tl = read_u16()`, `ap = read_u8()`.
    #[test]
    fn the_writer_matches_the_server_reader() {
        let cases = [
            (TransferCommand::Board { tl_id: 0x0102, ap: 2 }, vec![2, 1, 2]),
            (TransferCommand::Board { tl_id: 0xFFFE, ap: 7 }, vec![0xFE, 0xFF, 7]),
            (TransferCommand::Leave { tl_id: 0x0102 }, vec![2, 1, 0]),
        ];
        for (command, body) in cases {
            assert!(command.validate().is_ok());
            let msg = ClientMsg::Transfer(command);
            assert_eq!(msg.opcode(), 0x067);
            assert_eq!(msg.name(), "CSBoardingTransferPacket");
            assert_eq!(msg.encode_body(), body);
            let mut r = Reader::new(&body);
            assert_eq!(r.u16().unwrap(), match command { TransferCommand::Board { tl_id, .. } | TransferCommand::Leave { tl_id } => tl_id });
            assert_eq!(r.u8().unwrap(), match command { TransferCommand::Board { ap, .. } => ap, TransferCommand::Leave { .. } => 0 });
            assert!(r.is_empty());
        }
    }

    #[test]
    fn invalid_timeline_ids_and_boarding_points_are_rejected() {
        for command in [
            TransferCommand::Board { tl_id: 0, ap: 2 },
            TransferCommand::Leave { tl_id: 0 },
            TransferCommand::Board { tl_id: 1, ap: 0 },
            TransferCommand::Board { tl_id: 1, ap: 1 },
            TransferCommand::Board { tl_id: 1, ap: 8 },
            TransferCommand::Board { tl_id: 1, ap: u8::MAX },
        ] {
            assert!(command.validate().is_err(), "{command:?}");
        }
        for ap in FIRST_PASSENGER_POINT..=LAST_PASSENGER_POINT {
            assert!(TransferCommand::Board { tl_id: 1, ap }.validate().is_ok());
        }
    }
}
