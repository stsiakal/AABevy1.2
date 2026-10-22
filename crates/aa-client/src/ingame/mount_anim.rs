//! The pure rules of how a rider gets on and off a mount in the original 1.2 client (`docs/research/re-mounts.md` section 2):
//! the entry side from the `SCUnitAttached` reason byte and the clips of each side. The rider is bound to the seat bone for the whole
//! get-on (the clips use movement control 1, Entity: no root motion reaches the entity); the clip's own skeletal pose starts beside the horse.
//! No Bevy system here; [`super::mount`] plays what these rules choose.

use aa_anim::pose::{BoundClip, Rig};
use bevy::prelude::*;

use super::player::{forward, right};

/// The side of the mount a rider gets on from (`VehicleSeat` of the rider graph: 1 left, 2 right, 3 back, 4 front).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Left,
    Right,
    Back,
    Front,
}

impl Side {
    /// The `reason` byte of `CSMountMate` / `SCUnitAttached`: 2, 3 and 4 stay, anything else is the left side (native `VehicleSeat`).
    pub fn of_reason(reason: u8) -> Side {
        match reason {
            2 => Side::Right,
            3 => Side::Back,
            4 => Side::Front,
            _ => Side::Left,
        }
    }

    /// The letter in the clip name `fist_ac_<pose>_<l|r|b|f>_geton`.
    pub fn letter(self) -> &'static str {
        match self {
            Side::Left => "l",
            Side::Right => "r",
            Side::Back => "b",
            Side::Front => "f",
        }
    }
}

/// The clip names to try for a get-on from `side`, best first: the side clip of the pose, then the same side of the horse, then the
/// left side of the pose.
pub fn geton_names(prefix: &str, side: Side) -> Vec<String> {
    let l = side.letter();
    let mut names = vec![format!("fist_ac_{prefix}_{l}_geton")];
    if prefix != "horse" {
        names.push(format!("fist_ac_horse_{l}_geton"));
    }
    if side != Side::Left {
        names.push(format!("fist_ac_{prefix}_l_geton"));
    }
    names
}

/// The three clips of a dismount, played as a jump (`JumpMain`, `JumpLoop`, `JumpEnd`), best first for each part.
pub fn getoff_names(prefix: &str) -> [Vec<String>; 3] {
    let names = |part: &str| {
        let mut v = vec![format!("fist_ac_{prefix}_l_getoff_{part}")];
        if prefix != "horse" {
            v.push(format!("fist_ac_horse_l_getoff_{part}"));
        }
        v
    };
    [names("start"), names("loop"), names("end")]
}

/// The root bone position in `clip` at `t` seconds, relative to its first key and scaled like the clip is played (model space,
/// x right, y forward, z up). Zero for a clip without a root track.
pub fn root_offset(rig: &Rig, clip: &BoundClip, t: f32) -> Vec3 {
    let Some(track) = rig.skeleton.bones.get(rig.root).and_then(|b| clip.clip.track(b.controller_id)) else { return Vec3::ZERO };
    let (Some(first), Some(at)) = (track.sample_pos(0.0), track.sample_pos(t.clamp(0.0, clip.clip.duration))) else { return Vec3::ZERO };
    (at - first) * clip.options.position_scale
}

/// A model-space offset turned to the world by a unit facing `facing` and scaled `scale`.
pub fn to_world(offset: Vec3, facing: f32, scale: f32) -> Vec3 {
    let xy = right(facing) * offset.x + forward(facing) * offset.y;
    Vec3::new(xy.x, xy.y, offset.z) * scale
}

/// The jump-style dismount: the clips play one after another, each starting where the root motion of the one before left the body.
pub struct Hop {
    /// Clips not played yet, last first.
    pub queue: Vec<BoundClip>,
    /// Where the entity stands for the clip that plays now (the body is `anchor` plus that clip root offset).
    pub anchor: Vec3,
    pub facing: f32,
    pub scale: f32,
    /// Seconds the clip that plays now has left.
    pub remaining: f32,
    /// The clip that plays now.
    pub current: Option<BoundClip>,
}

impl Hop {
    pub fn new(clips: Vec<BoundClip>, seat: Vec3, facing: f32, scale: f32) -> Hop {
        let mut queue = clips;
        queue.reverse();
        Hop { queue, anchor: seat, facing, scale, remaining: 0.0, current: None }
    }

    /// Takes the next clip. The root motion the finished one made is carried into the anchor so the body does not jump back.
    /// `None` once the hop is over (`anchor` is then where the unit stands).
    pub fn next(&mut self, rig: &Rig) -> Option<BoundClip> {
        if let Some(done) = self.current.take() {
            self.anchor += to_world(root_offset(rig, &done, done.clip.duration), self.facing, self.scale);
        }
        let clip = self.queue.pop()?;
        self.remaining = clip.clip.duration;
        self.current = Some(clip.clone());
        Some(clip)
    }
}

// ---- PseudoSpeed bands (`horse_full_body.xml`, `player_full_body.xml`) ----

/// The gait band a mount or a rider is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Idle,
    Walk,
    Run,
}

/// The `pseudoSpeed` the x2game movement request carries (`FUN_39404d50`; crygame `FUN_3906a7f0` writes it to the graph input, clamped
/// to 0.4 under actor flag bit0). It is a requested urgency, not a speed: 0 idle, 1.0 moving, 0.4 with the walk modifier, 1.5 for
/// move-to-point and auto-move (`FUN_393ba480`). cryaction `FUN_306747f0` zeroes it when the ground speed is under 0.5 m/s.
/// `speed`/`run_speed` only tell a walk from a run here (the walk modifier is not sent to other clients). How a sprint value above
/// 1.0 comes about for a mount outside auto-move is not found (open).
pub fn pseudo_speed(speed: f32, run_speed: f32, auto_move: bool) -> f32 {
    if speed < IDLE_SPEED || run_speed <= 0.0 {
        0.0
    } else if auto_move {
        AUTO_PSEUDO
    } else if speed / run_speed < WALK_SHARE {
        WALK_PSEUDO
    } else {
        MOVE_PSEUDO
    }
}

/// Under this ground speed (m/s) the request is zeroed (cryaction `FUN_306747f0`).
pub const IDLE_SPEED: f32 = 0.5;
/// The requested PseudoSpeed of a move, of a move with the walk modifier and of move-to-point / auto-move.
pub const MOVE_PSEUDO: f32 = 1.0;
pub const WALK_PSEUDO: f32 = 0.4;
pub const AUTO_PSEUDO: f32 = 1.5;
/// A move under this share of the run speed counts as the walk modifier (the horse walks at `game_walk_multiplier` 0.2, runs at 1).
const WALK_SHARE: f32 = 0.6;

/// The band between two limits keeps the band the unit was in (the graph's `PseudoSpeed` ranges leave gaps).
fn band(ps: f32, idle_max: f32, walk: (f32, f32), run_min: f32, prev: Band) -> Band {
    if ps <= idle_max {
        Band::Idle
    } else if ps >= run_min {
        Band::Run
    } else if (walk.0..=walk.1).contains(&ps) {
        Band::Walk
    } else {
        prev
    }
}

/// A mount: idle 0..0.1, walk 0.11..0.7, run from 0.8 (`horse_full_body.xml`).
pub fn mount_band(ps: f32, prev: Band) -> Band {
    band(ps, 0.10, (0.11, 0.7), 0.8, prev)
}

/// A rider on a mount: idle 0..0.1, walk 0.15..0.6, run from 0.7 (`player_full_body.xml`, `Horse_Driver*`).
pub fn rider_band(ps: f32, prev: Band) -> Band {
    band(ps, 0.10, (0.15, 0.6), 0.7, prev)
}

/// From this PseudoSpeed on a mount runs its sprint tier: only the 1.5 of move-to-point / auto-move reaches it (the band threshold in
/// the graph XML is kept as it was).
pub const SPRINT_PSEUDO: f32 = 1.25;

/// The turn rate (degrees/s, the unit of `RequestedTurnSpeedLZ` = change of rotation angle in degrees / dt, cryaction `FUN_30665830`)
/// from which a unit plays its turn clips: turning in place on the mount, the run turn blend on the rider and the mount. The value is
/// the earlier 0.6 rad/s (inferred; the graph's own thresholds were not read).
pub const TURN_MIN: f32 = 0.6 * 180.0 / std::f32::consts::PI;

/// -1 for a right turn, 1 for a left turn, 0 for none (yaw grows to the left).
pub fn turn_dir(rate: f32) -> i8 {
    if rate > TURN_MIN {
        1
    } else if rate < -TURN_MIN {
        -1
    } else {
        0
    }
}

/// A mate's gait: its run speed (m/s) and the rate it turns at.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct MateGait {
    pub run_speed: f32,
    last_yaw: Option<f32>,
    /// The smoothed turn rate (`RequestedTurnSpeedLZ`, degrees/s), positive to the left.
    pub turn: f32,
}

impl MateGait {
    pub fn new(run_speed: f32) -> MateGait {
        MateGait { run_speed, last_yaw: None, turn: 0.0 }
    }

    pub fn pseudo(&self, speed: f32, auto_move: bool) -> f32 {
        pseudo_speed(speed, self.run_speed, auto_move)
    }

    /// Follows the facing: the turn rate is the smoothed change of `yaw` over `dt`, in degrees/s.
    pub fn track_turn(&mut self, yaw: f32, dt: f32) {
        if let (Some(last), true) = (self.last_yaw, dt > 1e-4) {
            let d = (yaw - last + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            self.turn += (d.to_degrees() / dt - self.turn) * 0.3;
        }
        self.last_yaw = Some(yaw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_byte_selects_the_side_like_vehicle_seat() {
        assert_eq!(Side::of_reason(1), Side::Left);
        assert_eq!(Side::of_reason(2), Side::Right);
        assert_eq!(Side::of_reason(3), Side::Back);
        assert_eq!(Side::of_reason(4), Side::Front);
        assert_eq!(Side::of_reason(0), Side::Left, "anything else is the left side");
        assert_eq!(Side::of_reason(9), Side::Left);
    }

    #[test]
    fn clip_names_per_side_fall_back_to_the_horse_and_the_left() {
        assert_eq!(geton_names("horse", Side::Right), ["fist_ac_horse_r_geton", "fist_ac_horse_l_geton"]);
        assert_eq!(geton_names("horse", Side::Left), ["fist_ac_horse_l_geton"]);
        assert_eq!(geton_names("lion", Side::Front), ["fist_ac_lion_f_geton", "fist_ac_horse_f_geton", "fist_ac_lion_l_geton"]);
        assert_eq!(getoff_names("horse")[0], ["fist_ac_horse_l_getoff_start"]);
        assert_eq!(getoff_names("bear")[2], ["fist_ac_bear_l_getoff_end", "fist_ac_horse_l_getoff_end"]);
    }
}

// ---- the mount flow controller (`MountHorse Controller`, FUN_390a5f00 / FUN_390aaf30) ----

/// A mate is mounted from at most this far (25 m; the native check is `distance squared <= 625`).
pub const MOUNT_RANGE: f32 = 25.0;
/// The shared cooldown of the vehicle actions (ms).
pub const COOLDOWN_MS: u64 = 2000;
/// `CSMountMate` goes out once the player is within 3 m of the mate (`dist^2 < 9.0`).
pub const SEND_RANGE: f32 = 3.0;
/// The controller gives up after 10 s.
pub const LIFETIME: f32 = 10.0;
/// After the request it waits 2 s for the server's attach.
pub const WAIT: f32 = 2.0;
/// The auto-walk stops this close (horizontally) to its approach point; it slows down inside 1.5 m.
pub const ARRIVE: f32 = 0.8;
pub const SLOW_RANGE: f32 = 1.5;

/// Native `FUN_3909ae90`, the entry side as the `CSMountMate` reason byte. `d` is the unit direction from the player to the mate and
/// `f` the **mate**'s forward: `d.f >= 0.96` is the player behind the mate (3; 1 when the passenger seat is taken), `< -0.96` in
/// front of it (4; 1 when the driver seat is taken), between the mate's left/right side by the sign of `d.y*f.x - d.x*f.y` (2 right,
/// 1 left) with no occupancy override.
pub fn entry_reason(player: Vec3, mate: Vec3, mate_yaw: f32, driver_taken: bool, passenger_taken: bool) -> u8 {
    let diff = (mate - player).truncate();
    let len = diff.length();
    if len <= 1e-5 {
        return 2;
    }
    let (d, f) = (diff / len, forward(mate_yaw));
    let dot = d.dot(f);
    if 0.96 <= dot {
        if passenger_taken { 1 } else { 3 }
    } else if dot < -0.96 {
        if driver_taken { 1 } else { 4 }
    } else if d.y * f.x - d.x * f.y >= 0.0 {
        2
    } else {
        1
    }
}

/// What the controller does this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// Walk to this point (`slow`: inside 1.5 m of it, at half speed).
    Walk { to: Vec2, slow: bool },
    /// Within 3 m of the mate: send `CSMountMate` now.
    Send,
    /// The request is out; the controller waits for the server.
    Wait,
    /// Done: attached, timed out or the walk arrived without the request being possible.
    Stop,
}

/// The point at the mate's side the auto-walk heads for (1 m from its middle, on the side the player stands on).
pub fn approach_target(player: Vec3, mate: Vec3, mate_yaw: f32) -> Vec2 {
    let side = right(mate_yaw);
    let sign = if (player - mate).truncate().dot(side) >= 0.0 { 1.0 } else { -1.0 };
    mate.truncate() + side * sign
}

/// One frame of the controller. `elapsed` is the seconds since it started, `sent` the elapsed time of the request when it went out.
pub fn flow_step(player: Vec3, mate: Vec3, mate_yaw: f32, elapsed: f32, sent: Option<f32>) -> Step {
    if elapsed >= LIFETIME {
        return Step::Stop;
    }
    if let Some(at) = sent {
        return if elapsed - at >= WAIT { Step::Stop } else { Step::Wait };
    }
    let flat = (mate - player).truncate();
    if flat.length_squared() < SEND_RANGE * SEND_RANGE {
        return Step::Send;
    }
    let to = approach_target(player, mate, mate_yaw);
    let away = (to - player.truncate()).length();
    if away <= ARRIVE {
        return Step::Stop;
    }
    Step::Walk { to, slow: away < SLOW_RANGE }
}

/// The facing (radians, 0 = north, [`forward`]) towards `to` from `from`.
pub fn bearing(from: Vec2, to: Vec2) -> f32 {
    let d = to - from;
    (-d.x).atan2(d.y)
}

#[cfg(test)]
mod flow_tests {
    use super::*;

    #[test]
    fn entry_reason_uses_the_mates_forward() {
        let player = Vec3::ZERO;
        // the mate stands north of the player and faces north: the player is behind it
        assert_eq!(entry_reason(player, Vec3::new(0.0, 3.0, 0.0), 0.0, false, false), 3);
        assert_eq!(entry_reason(player, Vec3::new(0.0, 3.0, 0.0), 0.0, false, true), 1);
        // facing south: the player is in front of it
        assert_eq!(entry_reason(player, Vec3::new(0.0, 3.0, 0.0), std::f32::consts::PI, true, false), 1);
        assert_eq!(entry_reason(player, Vec3::new(0.0, 3.0, 0.0), std::f32::consts::PI, false, false), 4);
        // the mate to the east facing north: the player is on its left
        assert_eq!(entry_reason(player, Vec3::new(3.0, 0.0, 0.0), 0.0, true, true), 1);
        assert_eq!(entry_reason(player, Vec3::new(-3.0, 0.0, 0.0), 0.0, true, true), 2);
    }

    #[test]
    fn the_controller_walks_in_sends_within_three_metres_then_waits() {
        let mate = Vec3::new(0.0, 10.0, 0.0);
        let far = flow_step(Vec3::ZERO, mate, 0.0, 0.0, None);
        assert!(matches!(far, Step::Walk { slow: false, .. }), "{far:?}");
        assert_eq!(flow_step(Vec3::new(0.0, 7.5, 0.0), mate, 0.0, 1.0, None), Step::Send, "2.5 m away");
        assert!(matches!(flow_step(Vec3::new(0.0, 6.9, 0.0), mate, 0.0, 1.0, None), Step::Walk { .. }), "3.1 m away still walks");
        assert_eq!(flow_step(Vec3::new(0.0, 7.5, 0.0), mate, 0.0, 1.5, Some(1.0)), Step::Wait);
        assert_eq!(flow_step(Vec3::new(0.0, 7.5, 0.0), mate, 0.0, 3.1, Some(1.0)), Step::Stop, "2 s wait ran out");
        assert_eq!(flow_step(Vec3::ZERO, mate, 0.0, 10.0, None), Step::Stop, "10 s lifetime");
    }

    #[test]
    fn the_walk_aims_at_the_side_the_player_is_on() {
        let target = approach_target(Vec3::new(-8.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0), 0.0);
        assert!((target - Vec2::new(-1.0, 0.0)).length() < 1e-5, "{target}");
        assert!(bearing(Vec2::ZERO, Vec2::new(0.0, 5.0)).abs() < 1e-5, "north is facing 0");
        assert!((bearing(Vec2::ZERO, Vec2::new(5.0, 0.0)) + std::f32::consts::FRAC_PI_2).abs() < 1e-5, "east is -90 degrees");
    }
}

#[cfg(test)]
mod band_tests {
    use super::*;

    #[test]
    fn the_mount_bands_follow_the_pseudo_speed_with_gaps_holding_the_band() {
        assert_eq!(mount_band(0.0, Band::Run), Band::Idle);
        assert_eq!(mount_band(0.1, Band::Run), Band::Idle);
        assert_eq!(mount_band(0.2, Band::Idle), Band::Walk, "a walk at 1.8 of 9 m/s");
        assert_eq!(mount_band(0.7, Band::Idle), Band::Walk);
        assert_eq!(mount_band(0.75, Band::Walk), Band::Walk, "the gap holds the walk");
        assert_eq!(mount_band(0.75, Band::Run), Band::Run, "and the run");
        assert_eq!(mount_band(0.8, Band::Walk), Band::Run);
        assert_eq!(mount_band(1.5, Band::Idle), Band::Run);
    }

    #[test]
    fn the_requested_pseudo_speed_is_discrete() {
        assert_eq!(pseudo_speed(0.3, 9.0, false), 0.0, "under 0.5 m/s the request is zeroed");
        assert_eq!(pseudo_speed(0.0, 9.0, true), 0.0);
        assert_eq!(pseudo_speed(1.8, 9.0, false), WALK_PSEUDO, "the walk modifier asks 0.4");
        assert_eq!(pseudo_speed(9.0, 9.0, false), MOVE_PSEUDO);
        assert_eq!(pseudo_speed(13.5, 9.0, false), MOVE_PSEUDO, "no sprint value outside auto-move");
        assert_eq!(pseudo_speed(9.0, 9.0, true), AUTO_PSEUDO);
        assert_eq!(mount_band(pseudo_speed(1.8, 9.0, false), Band::Idle), Band::Walk);
        assert_eq!(mount_band(pseudo_speed(9.0, 9.0, false), Band::Idle), Band::Run);
    }

    #[test]
    fn the_rider_bands_are_wider_at_the_walk() {
        assert_eq!(rider_band(0.12, Band::Idle), Band::Idle, "0.1..0.15 holds");
        assert_eq!(rider_band(0.12, Band::Walk), Band::Walk);
        assert_eq!(rider_band(0.15, Band::Idle), Band::Walk);
        assert_eq!(rider_band(0.65, Band::Walk), Band::Walk, "0.6..0.7 holds the walk");
        assert_eq!(rider_band(0.7, Band::Walk), Band::Run);
        // the horse walks at 0.2 and runs at 1: the rider walks and runs with it
        assert_eq!(rider_band(pseudo_speed(1.8, 9.0, false), Band::Idle), Band::Walk);
        assert_eq!(rider_band(pseudo_speed(9.0, 9.0, false), Band::Idle), Band::Run);
    }

    #[test]
    fn a_turning_mate_reports_the_direction_of_the_turn() {
        let mut gait = MateGait::new(9.0);
        gait.track_turn(0.0, 0.016);
        for i in 1..40 {
            gait.track_turn(i as f32 * 0.03, 0.016);
        }
        assert_eq!(turn_dir(gait.turn), 1, "yaw grows to the left: {}", gait.turn);
        assert_eq!(turn_dir(-40.0), -1, "degrees per second");
        assert_eq!(turn_dir(20.0), 0, "under the 0.6 rad/s (34.4 deg/s) threshold");
        assert!((gait.turn - 0.03 / 0.016 * 180.0 / std::f32::consts::PI).abs() < 5.0, "0.03 rad per 16 ms is about 107 deg/s: {}", gait.turn);
        // across the +-pi seam the turn stays small
        let mut seam = MateGait::new(9.0);
        seam.track_turn(3.13, 0.016);
        seam.track_turn(-3.13, 0.016);
        assert!((seam.turn - 0.3 * (0.0232 / 0.016 * 180.0 / std::f32::consts::PI)).abs() < 0.5, "{}", seam.turn);
    }
}
