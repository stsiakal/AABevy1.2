//! The pure rules of how a rider gets on and off a mount in the original 1.2 client (`docs/research/re-mounts.md` section 2):
//! the entry side from the `SCUnitAttached` reason byte, the clips of each side and where the rider stands when the clip starts.
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

    /// The facing of a rider that starts the get-on clip on this side: the clip walks the root forward (1 m for the sides, 2 m for
    /// back and front) towards the saddle, so the rider faces the mount from its left, right, rear or front.
    pub fn rider_facing(self, mount_facing: f32) -> f32 {
        match self {
            Side::Left => mount_facing - std::f32::consts::FRAC_PI_2,
            Side::Right => mount_facing + std::f32::consts::FRAC_PI_2,
            Side::Back => mount_facing,
            Side::Front => mount_facing + std::f32::consts::PI,
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

/// Where the rider entity stands for a get-on clip whose root offset at its end is `end`, so that the clip finishes on the
/// saddle `seat`: the approach point, one or two metres out on the side and down on the ground.
pub fn approach_point(seat: Vec3, rider_facing: f32, end: Vec3, scale: f32) -> Vec3 {
    seat - to_world(end, rider_facing, scale)
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

    #[test]
    fn rider_starts_on_the_side_it_gets_on_from() {
        let seat = Vec3::new(100.0, 200.0, 50.0);
        // the mount faces +y; the side clips end 1 m forward and 1.888 m up, back and front 2 m
        let side_end = Vec3::new(0.0, 1.0, 1.888);
        let left = approach_point(seat, Side::Left.rider_facing(0.0), side_end, 1.0);
        assert!((left - Vec3::new(99.0, 200.0, 48.112)).length() < 1e-4, "left of the saddle: {left}");
        let right = approach_point(seat, Side::Right.rider_facing(0.0), side_end, 1.0);
        assert!((right - Vec3::new(101.0, 200.0, 48.112)).length() < 1e-4, "right of the saddle: {right}");
        let long_end = Vec3::new(0.0, 2.0, 1.888);
        let back = approach_point(seat, Side::Back.rider_facing(0.0), long_end, 1.0);
        assert!((back - Vec3::new(100.0, 198.0, 48.112)).length() < 1e-4, "behind the mount: {back}");
        let front = approach_point(seat, Side::Front.rider_facing(0.0), long_end, 1.0);
        assert!((front - Vec3::new(100.0, 202.0, 48.112)).length() < 1e-4, "in front of the mount: {front}");
    }

    #[test]
    fn approach_turns_with_the_mount() {
        // the mount faces +x (yaw -90 degrees); its left is +y
        let yaw = -std::f32::consts::FRAC_PI_2;
        let p = approach_point(Vec3::ZERO, Side::Left.rider_facing(yaw), Vec3::new(0.0, 1.0, 0.0), 1.0);
        assert!((p - Vec3::new(0.0, 1.0, 0.0)).length() < 1e-4, "{p}");
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
