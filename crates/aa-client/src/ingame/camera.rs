//! The third person camera, like ArcheAge's: it orbits a pivot just above the player's head.
//!
//! * hold the left mouse button and drag: orbit the camera around the character
//! * hold the right mouse button and drag: orbit and turn the character with the camera
//! * wheel: zoom, as 1.2's `camera::update_distance` does it ([`zoom_notch`], [`approach_distance`]): a notch moves the wanted
//!   distance by 0.5 x clamp(0.3 x wanted, 1, 20) between 0 and the active mode's far limit ([`mode_max_distance`]); under
//!   1 m it is first person and the own character is hidden; the camera follows the wanted distance at 5 + gap^1.8 m/s
//!
//! The view at world entry (the real client's spawn camera, `reference/ingame/spawn/camera.json`) stands level with the pivot
//! `PIVOT_HEIGHT` above the feet, vertical field of view 42.75 degrees, at 1.2's start distance by race and gender
//! ([`start_distance`]: 4 m for a Nuian man).
//!
//! Walls: the arm from the pivot to the camera is swept with a sphere against the terrain and the world's big structures
//! ([`WorldCollision::clip_camera`]); the camera stops before the first hit at once and comes back out smoothly
//! ([`ARM_RETURN`]) when the hit clears.
//!
//! Blows and spells shake it: [`CameraShake`] holds the shakes of the effect data (`fx_shake_cameras`), `follow_player` adds
//! their sum to the pose.
//!
//! The mode byte of [`CameraMode`] picks the camera (`CameraManager::Update`): `screenshotcamera` enters the free camera
//! (mode 2), and under the water the camera dives to `camera_dive_pitch`, held at `camera_dive_angle`.

use aa_ui::UiBackgroundCamera;
use aa_ui::render::UiCamera;
use aa_world::{TerrainQuery, WorldCollision, bevy_to_cry, cry_to_bevy};
use bevy::camera::{Hdr, RenderTarget};
use bevy::input::mouse::AccumulatedMouseMotion;
use bevy::pbr::{ScreenSpaceTransmission, ScreenSpaceTransmissionQuality};
use bevy::prelude::*;
use bevy::render::view::Msaa;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};

use super::dev::DevOptions;
use super::player::{LocalPlayer, Mover, height_or};
use crate::Screen;

/// Height of the camera pivot above the character's feet (m): the real client's camera sits 1.754 m
/// above the ground at pitch 0.
pub const PIVOT_HEIGHT: f32 = 1.754;
/// The distance without a character to take it from (the prewarm camera): 1.2's start for a Nuian man.
pub const DEFAULT_DISTANCE: f32 = 4.0;
/// The action camera mode's `camera_max_dist` (`option_camera_fov_set.cfg` group 1, `FUN_3915e7a0`).
pub const ACTION_MAX_DISTANCE: f32 = 10.0;
/// The classic camera mode's `camera_max_dist` (group 2): the running client's option 65 is 2, so this is its far limit.
pub const CLASSIC_MAX_DISTANCE: f32 = 18.0;
/// Under this wanted distance the camera is first person and the own character is hidden (`tw_first_person_dist`).
pub const FIRST_PERSON_BELOW: f32 = 1.0;
/// 1.2's camera update caps its frame step at 33 ms (`CameraManager::Update`): the zoom never settles faster after a long frame.
const CAMERA_STEP_CAP: f32 = 0.033;
/// Radius of the sphere the camera arm is swept with (m): more than the near plane (0.2) reaches.
const CAMERA_RADIUS: f32 = 0.3;
/// How fast the camera comes back out after a wall let go of it (1 / s of an exponential approach).
const ARM_RETURN: f32 = 5.0;
/// The classic camera mode's `cl_fov` (`option_camera_fov_set.cfg` group 2), the default.
const FOV_DEG: f32 = 42.75;
/// The action camera mode's `cl_fov` (group 1).
const ACTION_FOV_DEG: f32 = 60.0;
/// Radians per mouse count.
const MOUSE_SENSITIVITY: f32 = 0.004;

/// The six camera modes of `X2::GameClient::CameraManager::Update` (`FUN_3916aab0`), by the byte at `+4`: 1 is the unit
/// third-person camera (`FUN_39169860`), 2 the free camera (`FUN_391632d0`); 3-6 (`FUN_3916a260`, `FUN_39164100`, no per-frame
/// update, `FUN_39166390`) are unnamed and keep the unit camera here.
#[allow(dead_code)] // modes 3-6 are the original's unnamed ones; only 1 and 2 have behaviour here
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraMode {
    #[default]
    Unit,
    Free,
    Mode3,
    Mode4,
    Mode5,
    Mode6,
}

#[allow(dead_code)] // the mode byte, used by the tests and the manager hook
impl CameraMode {
    /// The manager's mode byte.
    pub fn number(self) -> u8 {
        match self {
            CameraMode::Unit => 1,
            CameraMode::Free => 2,
            CameraMode::Mode3 => 3,
            CameraMode::Mode4 => 4,
            CameraMode::Mode5 => 5,
            CameraMode::Mode6 => 6,
        }
    }

    /// The mode of a manager byte; 1-6 are the original's, anything else the unit camera.
    pub fn from_number(number: u8) -> Self {
        match number {
            2 => CameraMode::Free,
            3 => CameraMode::Mode3,
            4 => CameraMode::Mode4,
            5 => CameraMode::Mode5,
            6 => CameraMode::Mode6,
            _ => CameraMode::Unit,
        }
    }
}

/// The camera manager's current mode, on the world camera.
#[derive(Component, Default)]
pub struct CameraModeState(pub CameraMode);

/// The free camera (manager mode 2): its own position and look, detached from the character (`FUN_391632d0`).
#[derive(Component, Clone, Copy, Debug)]
pub struct FreeCamera {
    /// Game position (metres, Cry axes).
    pub pos: Vec3,
    /// Game yaw (radians counter-clockwise from north), as the orbit camera's.
    pub yaw: f32,
    /// Radians, positive looks up.
    pub pitch: f32,
}

impl FreeCamera {
    fn transform(self) -> Transform {
        let rotation = Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(self.pitch);
        Transform { translation: cry_to_bevy(self.pos), rotation, scale: Vec3::ONE }
    }
}

/// `fr_xspeed`, `fr_yspeed`, `fr_zspeed`, `fr_speed_scale` and `fr_fspeed_scale` (`FUN_390f7b40:28-49`): 40 m/s, 3x while Shift.
pub const FREE_X_SPEED: f32 = 40.0;
pub const FREE_Y_SPEED: f32 = 40.0;
pub const FREE_Z_SPEED: f32 = 40.0;
pub const FREE_SPEED_SCALE: f32 = 1.0;
pub const FREE_FAST_SPEED_SCALE: f32 = 3.0;
/// `fr_xturn`, `fr_yturn`, `fr_turn_scale` and `fr_fturn_scale`: 60 degrees a second, 2x while Shift.
pub const FREE_X_TURN: f32 = 60.0;
pub const FREE_Y_TURN: f32 = 60.0;
pub const FREE_TURN_SCALE: f32 = 1.0;
pub const FREE_FAST_TURN_SCALE: f32 = 2.0;
/// `camera_dive_enable`, `camera_dive_angle` and `camera_dive_pitch` (`FUN_390f7b40:543-551`): under the surface the camera
/// pitches to 20 degrees down, no further down than `camera_dive_angle` (30 degrees).
pub const CAMERA_DIVE_ENABLE: bool = true;
pub const CAMERA_DIVE_ANGLE_DEG: f32 = 30.0;
pub const CAMERA_DIVE_PITCH_DEG: f32 = 20.0;
/// `camera_target_ground_align` (1): the camera stays over the terrain (the clamp [`follow_player`] always had).
pub const CAMERA_TARGET_GROUND_ALIGN: bool = true;

/// One frame of the free camera: the axis inputs (-1..1), the keyboard turns (-1..1) and the mouse motion in counts.
#[derive(Clone, Copy, Debug, Default)]
pub struct FreeInput {
    pub forward: f32,
    pub right: f32,
    pub up: f32,
    pub turn: f32,
    pub look: f32,
    pub mouse: Vec2,
    pub fast: bool,
}

/// One free-camera step (`FUN_391632d0`): the axes move at their cvar speeds and the mouse/keyboard turns at the turn speeds,
/// both scaled while Shift is held. `dt` is the frame's, in seconds.
pub fn free_camera_step(pos: Vec3, yaw: f32, pitch: f32, input: FreeInput, dt: f32) -> (Vec3, f32, f32) {
    let speed = if input.fast { FREE_FAST_SPEED_SCALE } else { FREE_SPEED_SCALE } * dt;
    let turn = if input.fast { FREE_FAST_TURN_SCALE } else { FREE_TURN_SCALE } * dt;
    let f = super::player::forward(yaw);
    let r = super::player::right(yaw);
    let mut pos = pos;
    pos += Vec3::new(f.x, f.y, 0.0) * (input.forward * FREE_Y_SPEED * speed);
    pos += Vec3::new(r.x, r.y, 0.0) * (input.right * FREE_X_SPEED * speed);
    pos.z += input.up * FREE_Z_SPEED * speed;
    let yaw = yaw - input.mouse.x * MOUSE_SENSITIVITY + input.turn * FREE_X_TURN.to_radians() * turn;
    let pitch = (pitch - input.mouse.y * MOUSE_SENSITIVITY + input.look * FREE_Y_TURN.to_radians() * turn).clamp(-1.3, 1.45);
    (pos, yaw, pitch)
}

/// The dive camera's pitch (`camera_dive_*`): entering the water snaps to [`CAMERA_DIVE_PITCH_DEG`] down, afterwards looking
/// down is held at [`CAMERA_DIVE_ANGLE_DEG`]; out of the water the orbit's own pitch is used untouched.
pub fn dive_pitch(pitch: f32, underwater: bool, entered: bool) -> f32 {
    if !CAMERA_DIVE_ENABLE || !underwater {
        return pitch;
    }
    if entered {
        -CAMERA_DIVE_PITCH_DEG.to_radians()
    } else {
        pitch.max(-CAMERA_DIVE_ANGLE_DEG.to_radians())
    }
}

#[derive(Component, Clone, Copy)]
pub struct OrbitCamera {
    /// Heading in game terms: radians counter-clockwise from north (the character's yaw).
    pub yaw: f32,
    /// Radians, positive looks up.
    pub pitch: f32,
    /// The distance the camera stands at now (m), before a wall shortens it; it follows [`OrbitCamera::wanted`]
    /// ([`approach_distance`]).
    pub distance: f32,
    /// The distance the wheel asks for (m): 0 is first person.
    pub wanted: f32,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera { yaw: 0.0, pitch: 0.0, distance: DEFAULT_DISTANCE, wanted: DEFAULT_DISTANCE }
    }
}

/// 1.2's distance at world entry (`camera::start_dist_by_race` x2game+0x9c0c58, indexed race x 3 + gender with the server's race
/// and gender numbers): set at once, no easing, never past the active mode's far limit. Every other row, gender 0 too, is 10 m.
pub fn start_distance(race: u8, gender: u8) -> f32 {
    use aa_net::model::{gender::*, race::*};
    let start = match (race, gender) {
        (NUIAN, MALE) => 4.0,
        (NUIAN, FEMALE) => 3.2,
        (ELF, MALE) => 3.8,
        (ELF, FEMALE) => 3.4,
        (HARIHARAN, MALE) => 3.6,
        (HARIHARAN, FEMALE) => 3.2,
        (FERRE, MALE) => 4.0,
        (FERRE, FEMALE) => 3.4,
        _ => 10.0,
    };
    f32::min(start, mode_max_distance())
}

/// The far limit of the active camera mode: `camera_max_dist` from the cfg group option 65 picks (`option_camera_fov_set.cfg`,
/// `FUN_3915e7a0`): 10 m for the action mode (1) and 18 m for classic (2, the running client's default).
pub fn mode_max_distance() -> f32 {
    mode_max_distance_for(crate::settings::option_number("n:65").or_else(|| crate::settings::native_default(65)))
}

/// The far limit of a mode number (1 action, 2 classic), the option's value without the store.
pub fn mode_max_distance_for(mode: Option<f64>) -> f32 {
    if mode.is_some_and(|m| m.round() as i64 == 1) { ACTION_MAX_DISTANCE } else { CLASSIC_MAX_DISTANCE }
}

/// The wanted distance after one wheel notch (`camera::update_distance` x2game+0x16c2b0, a notch being one frame of the action):
/// `notch` 1 zooms in (1.2's `zoom_in`, an input of -0.5), -1 out. The step is 0.5 x clamp(0.3 x wanted, 1, 20), the result held
/// between 0 and the active mode's far limit ([`mode_max_distance`]); under [`FIRST_PERSON_BELOW`] it snaps to 0 (first person),
/// or to 1 m when zooming out.
pub fn zoom_notch(wanted: f32, notch: f32) -> f32 {
    zoom_notch_with(wanted, notch, mode_max_distance())
}

/// [`zoom_notch`] with the far limit given (the action mode's 10 m or classic's 18 m).
pub fn zoom_notch_with(wanted: f32, notch: f32, max: f32) -> f32 {
    if notch == 0.0 {
        return wanted;
    }
    let input = -0.5 * notch.signum();
    let next = (wanted + input * (0.3 * wanted).clamp(1.0, 20.0)).clamp(0.0, max);
    match next < FIRST_PERSON_BELOW {
        true if notch < 0.0 => FIRST_PERSON_BELOW,
        true => 0.0,
        false => next,
    }
}

/// The drawn distance moved towards the wanted one for `seconds` (capped at 33 ms, as 1.2's camera update caps its step) at
/// 5 + gap^1.8 m/s, never past it (`tw_zoom_speed` 5, `tw_zoom_speed_power` 1.8).
pub fn approach_distance(current: f32, wanted: f32, seconds: f32) -> f32 {
    let gap = wanted - current;
    if gap == 0.0 || seconds <= 0.0 {
        return current;
    }
    let step = (5.0 + gap.abs().powf(1.8)) * seconds.min(CAMERA_STEP_CAP);
    if gap.abs() <= step { wanted } else { current + gap.signum() * step }
}

/// First person: the wanted distance is under 1 m, and 1.2 hides the own character (`ProcessUnitCamFadeOut`).
pub fn first_person(wanted: f32) -> bool {
    wanted < FIRST_PERSON_BELOW
}

/// Screen space transmission, which aa-fx's refraction particles read the scene through. aa-fx puts it on every camera the first time
/// a refraction effect shows up (`ensure_transmission`), and the component is part of what a render pipeline is keyed on (the view
/// layout), so a camera that got it in the middle of the game would make every material compile again. The cameras that draw the
/// world (this one and the prewarm camera) have it from the start, exactly as aa-fx would give it; without a transmissive item it
/// costs nothing (no copy of the scene, no texture).
pub fn transmission() -> ScreenSpaceTransmission {
    ScreenSpaceTransmission { steps: 1, quality: ScreenSpaceTransmissionQuality::Low }
}

/// The in-game camera's lens (also the prewarm camera's, `prewarm`).
pub fn projection() -> Projection {
    projection_fov(mode_fov())
}

/// The vertical field of view (degrees) of the Options window's camera mode (`OPTION_ITEM_CAMERA_FOV_SET`: 1 action, 2 classic).
pub fn mode_fov() -> f32 {
    let mode = crate::settings::option_number("n:65").or_else(|| crate::settings::native_default(65));
    if mode.is_some_and(|m| m.round() as i64 == 1) { ACTION_FOV_DEG } else { FOV_DEG }
}

/// The world camera's own vertical field of view (degrees), what a buff's view item widens (`super::cam_fov`): the camera mode's,
/// or `--cam-fov`.
pub fn base_fov(dev: &DevOptions) -> f32 {
    dev.cam_fov.unwrap_or_else(mode_fov)
}

/// The lens with another vertical field of view (degrees): `--cam-fov`, to line a shot up with a capture of the real client's `cl_fov`.
fn projection_fov(degrees: f32) -> Projection {
    Projection::Perspective(PerspectiveProjection { fov: degrees.to_radians(), near: 0.2, far: 20000.0, ..default() })
}

/// The world camera still draws into an image behind the loading screen ([`show_world`] puts it on the window).
#[derive(Component)]
pub struct BehindLoading;

/// Spawns the camera behind the player (the world streams around the first `Camera3d`). On the loading screen it renders into
/// an image the size of the window instead: the world draws, compiles its pipelines and loads its models behind the loading
/// UI, and the screen switches to the game when that is done.
#[allow(clippy::too_many_arguments)]
pub fn spawn_camera(
    mut commands: Commands,
    player: Query<&Mover, With<LocalPlayer>>,
    data: Res<aa_net::state::Player>,
    existing: Query<(), With<OrbitCamera>>,
    anchors: Query<Entity, With<crate::login::WorldAnchor>>,
    screen: Res<State<Screen>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    dev: Res<DevOptions>,
) {
    let (Ok(mover), true) = (player.single(), existing.is_empty()) else { return };
    // 1.2 sets the wanted and the drawn distance at once when its own unit is bound, by the race and gender the server sent
    let distance = dev.cam_dist.unwrap_or_else(|| {
        let (race, gender) = data.spawn.as_ref().map_or((0, 0), |s| (s.character().race, s.character().gender));
        let start = data.spawn.as_ref().map_or(DEFAULT_DISTANCE, |_| start_distance(race, gender));
        info!("camera start distance {start:.2} m (race {race}, gender {gender})");
        start
    });
    let distance = distance.min(mode_max_distance());
    let orbit = OrbitCamera {
        yaw: mover.facing + dev.cam_yaw.unwrap_or(0.0),
        pitch: dev.cam_pitch.unwrap_or(0.0),
        distance,
        wanted: distance,
    };
    let transform = camera_transform(&orbit, mover.pos);
    let mut camera = commands.spawn((
        Name::new("camera"),
        Camera3d::default(),
        dev.cam_fov.map_or_else(projection, projection_fov),
        transmission(),
        transform,
        orbit,
        CameraModeState::default(),
    ));
    if *screen.get() == Screen::Loading
        && let Ok(window) = windows.single()
    {
        let image = Image::new_target_texture(window.physical_width(), window.physical_height(), bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb, None);
        camera.insert((RenderTarget::Image(images.add(image).into()), BehindLoading));
    }
    // this camera is the world's focus from now on (the anchor kept the world off the login stage camera)
    for e in &anchors {
        commands.entity(e).despawn();
    }
}

#[allow(clippy::type_complexity)]
/// Puts the world camera on the window once the UI cameras agree with it (HDR and MSAA, see `hud::cameras`; cameras that
/// disagree get separate main textures and the UI would replace the world), or after a second.
pub fn show_world(
    mut commands: Commands,
    time: Res<Time>,
    mut waited: Local<f32>,
    hidden: Query<(Entity, Has<Hdr>, Option<&Msaa>), (With<BehindLoading>, With<OrbitCamera>)>,
    ui: Query<(Has<Hdr>, Option<&Msaa>), (With<UiCamera>, Without<UiBackgroundCamera>)>,
) {
    let Ok((entity, hdr, msaa)) = hidden.single() else {
        *waited = 0.0;
        return;
    };
    *waited += time.delta_secs();
    let msaa = msaa.copied().unwrap_or_default();
    let agree = ui.iter().all(|(ui_hdr, ui_msaa)| ui_hdr == hdr && ui_msaa.copied() == Some(msaa));
    if agree || *waited > 1.0 {
        commands.entity(entity).insert(RenderTarget::default()).remove::<BehindLoading>();
        info!("world camera on the window (UI cameras agree: {agree})");
    }
}

/// Camera pose for an orbit around the pivot above `pos` (game coordinates of the feet).
pub fn camera_transform(orbit: &OrbitCamera, pos: Vec3) -> Transform {
    let pivot = cry_to_bevy(pos + Vec3::Z * PIVOT_HEIGHT);
    let rotation = Quat::from_rotation_y(orbit.yaw) * Quat::from_rotation_x(orbit.pitch);
    Transform { translation: pivot + rotation * Vec3::Z * orbit.distance, rotation, scale: Vec3::ONE }
}

/// 1.2's fixed views (`input::front_camera` and the five other handlers, x2game+0x408450..0x408620): the camera's yaw is an
/// offset from the character's heading, set at once, pitch and distance unchanged.
pub const FRONT_VIEW: f32 = std::f32::consts::PI;
pub const LEFT_VIEW: f32 = std::f32::consts::FRAC_PI_2;
pub const RIGHT_VIEW: f32 = 3.0 * std::f32::consts::FRAC_PI_2;
pub const BACK_VIEW: f32 = 0.0;

/// The offset a fixed-view action sets (Insert `front_camera`, Delete `left_camera`, Home `right_camera`, End `back_camera`).
pub fn fixed_view_offset(action: &str) -> Option<f32> {
    match action {
        "front_camera" => Some(FRONT_VIEW),
        "left_camera" => Some(LEFT_VIEW),
        "right_camera" => Some(RIGHT_VIEW),
        "back_camera" => Some(BACK_VIEW),
        _ => None,
    }
}

/// Page Down (`cycle_camera_clockwise`) steps the **kept** last fixed value by minus a quarter turn, Page Up by plus: from the
/// value kept for it, not from where the mouse has turned the camera since. Which side is which is the action's name (a guess).
pub fn cycle_view(kept: f32, clockwise: bool) -> f32 {
    let step = std::f32::consts::FRAC_PI_2;
    (if clockwise { kept - step } else { kept + step }).rem_euclid(std::f32::consts::TAU)
}

/// The camera's world yaw for a character heading and an offset.
pub fn view_yaw(facing: f32, offset: f32) -> f32 {
    facing + offset
}

/// Mouse orbit / zoom. Runs before the player moves so the character turns with the same input.
pub fn orbit_input(
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    actions: Res<crate::settings::Actions>,
    mut cameras: Query<(&mut OrbitCamera, Option<&FreeCamera>)>,
    mut window: Query<&mut CursorOptions, With<PrimaryWindow>>,
    over_ui: Option<Res<crate::hud::HudPointerOverUi>>,
    mut notches: Local<f32>,
    player: Query<&Mover, With<LocalPlayer>>,
    // the last fixed view's offset, which Page Down / Page Up step from (`input::g_fixed_view_yaw`)
    mut kept: Local<f32>,
    placement: Option<Res<super::placement::Placement>>,
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<bevy::input::mouse::AccumulatedMouseScroll>,
) {
    let Ok((mut orbit, free)) = cameras.single_mut() else { return };
    // the free camera (manager mode 2) reads the same keys and mouse itself
    if free.is_some() {
        return;
    }
    // the fixed views: the yaw is set at once relative to the character's heading
    if let Ok(mover) = player.single() {
        let mut set = None;
        for action in ["front_camera", "left_camera", "right_camera", "back_camera"] {
            if actions.just_pressed(action, 0) {
                set = fixed_view_offset(action);
            }
        }
        if actions.just_pressed("cycle_camera_clockwise", 0) {
            set = Some(cycle_view(*kept, true));
        }
        if actions.just_pressed("cycle_camera_counter_clockwise", 0) {
            set = Some(cycle_view(*kept, false));
        }
        if let Some(offset) = set {
            *kept = offset;
            orbit.yaw = view_yaw(mover.facing, offset);
        }
    }
    // a drag that began on a HUD window (or the wheel over a list) is the HUD's
    let over_ui = over_ui.is_some_and(|o| o.0);
    let dragging = !over_ui && (mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Right));
    if dragging {
        orbit.yaw -= motion.delta.x * MOUSE_SENSITIVITY;
        orbit.pitch = (orbit.pitch - motion.delta.y * MOUSE_SENSITIVITY).clamp(-1.3, 1.45);
    }
    // a whole notch is one step of 1.2's zoom; the fractions a touchpad sends add up to notches
    // the wheel that turns a placement ghost (planting, a rotatable house) is not also the zoom
    let turning_ghost = placement.as_ref().and_then(|p| p.session.as_ref()).is_some_and(|s| s.wheel_rotates());
    if !over_ui && !turning_ghost {
        *notches += actions.zoom_lines();
    } else if !over_ui && keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        // retail `builder_rotate_mode`: CTRL-wheel is `builder_zoom_in/out`, the camera zoom, while the plain wheel turns the model
        *notches += match scroll.unit {
            bevy::input::mouse::MouseScrollUnit::Line => scroll.delta.y,
            bevy::input::mouse::MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
    }
    while notches.abs() >= 1.0 {
        let notch = notches.signum();
        let before = orbit.wanted;
        orbit.wanted = zoom_notch(orbit.wanted, notch);
        *notches -= notch;
        if first_person(orbit.wanted) != first_person(before) {
            debug!("camera: {} (wanted {:.2} m)", if first_person(orbit.wanted) { "first person" } else { "third person" }, orbit.wanted);
        }
    }
    // the pointer disappears and stays put while a button drags the view, like the real client
    if let Ok(mut cursor) = window.single_mut() {
        let want = if dragging { (CursorGrabMode::Locked, false) } else { (CursorGrabMode::None, true) };
        if cursor.grab_mode != want.0 || cursor.visible != want.1 {
            cursor.grab_mode = want.0;
            cursor.visible = want.1;
        }
    }
}

/// `screenshotcamera` enters and leaves the free camera of the manager's mode 2 (`FUN_391632d0`), keeping the pose it had.
pub fn toggle_free_camera(
    actions: Res<crate::settings::Actions>,
    typing: Option<Res<crate::hud::HudKeyboardFocus>>,
    mut commands: Commands,
    mut cameras: Query<(Entity, &mut CameraModeState, &OrbitCamera, &Transform, Option<&FreeCamera>)>,
) {
    if typing.is_some_and(|f| f.0) || !actions.just_pressed("screenshotcamera", 0) {
        return;
    }
    let Ok((entity, mut state, orbit, transform, free)) = cameras.single_mut() else { return };
    if free.is_some() {
        state.0 = CameraMode::Unit;
        commands.entity(entity).remove::<FreeCamera>();
        info!("camera: mode 1 (unit camera)");
    } else {
        state.0 = CameraMode::Free;
        let camera = FreeCamera { pos: bevy_to_cry(transform.translation), yaw: orbit.yaw, pitch: orbit.pitch };
        commands.entity(entity).insert(camera);
        info!("camera: mode 2 (free camera) at ({:.1}, {:.1}, {:.2})", camera.pos.x, camera.pos.y, camera.pos.z);
    }
}

/// The free camera's keys (`FUN_391632d0`): `np_add` / `np_subtract` (and W / S) forward and back, A / D strafe, `np_multiply`
/// / `np_divide` (and Space / X, Home / End) up and down, Shift fast, the mouse turns it.
#[allow(clippy::too_many_arguments)]
pub fn free_camera_input(
    actions: Res<crate::settings::Actions>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    typing: Option<Res<crate::hud::HudKeyboardFocus>>,
    over_ui: Option<Res<crate::hud::HudPointerOverUi>>,
    time: Res<Time>,
    mut cameras: Query<(&mut FreeCamera, &mut Transform)>,
    mut window: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    let Ok((mut free, mut transform)) = cameras.single_mut() else { return };
    let typing = typing.is_some_and(|f| f.0);
    let over_ui = over_ui.is_some_and(|o| o.0);
    let held = |action: &str| !typing && actions.pressed(action, 0);
    let key = |code: KeyCode| !typing && keys.pressed(code);
    let axis = |plus: bool, minus: bool| plus as i32 as f32 - minus as i32 as f32;
    let forward = axis(held("moveforward") || key(KeyCode::NumpadAdd) || key(KeyCode::KeyW), held("moveback") || key(KeyCode::NumpadSubtract) || key(KeyCode::KeyS));
    let right = axis(held("moveright") || key(KeyCode::KeyD), held("moveleft") || key(KeyCode::KeyA));
    let up = axis(
        held("jump") || key(KeyCode::NumpadMultiply) || key(KeyCode::Home),
        held("down") || key(KeyCode::NumpadDivide) || key(KeyCode::End),
    );
    let dragging = !over_ui && (buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right));
    let fast = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let input = FreeInput {
        forward,
        right,
        up,
        turn: 0.0,
        look: 0.0,
        mouse: if dragging { motion.delta } else { Vec2::ZERO },
        fast,
    };
    let (pos, yaw, pitch) = free_camera_step(free.pos, free.yaw, free.pitch, input, time.delta_secs());
    *free = FreeCamera { pos, yaw, pitch };
    *transform = free.transform();
    if let Ok(mut cursor) = window.single_mut() {
        let want = if dragging { (CursorGrabMode::Locked, false) } else { (CursorGrabMode::None, true) };
        if cursor.grab_mode != want.0 || cursor.visible != want.1 {
            cursor.grab_mode = want.0;
            cursor.visible = want.1;
        }
    }
}

/// How strong the shakes of the effect data are here: the data is made for a camera that sits in the head of the character,
/// this one is 4 m behind it, so a full shake would throw the whole scene around.
const SHAKE_SCALE: f32 = 0.5;
/// The most a shake may move (m) or turn (degrees) the camera, whatever the data says.
const SHAKE_MAX_SHIFT: f32 = 0.2;
const SHAKE_MAX_ANGLE: f32 = 2.0;
/// Shakes running at once; the oldest goes when there are more.
const SHAKES: usize = 6;

/// The camera shakes that are running. A shake is a wave that alternates every `step` seconds (the data's `frequency`) between
/// two extremes, each a little off by `randomness`, and fades out linearly over `duration`.
#[derive(Resource, Default)]
pub struct CameraShake {
    active: Vec<Running>,
    /// Counts the shakes, so that two of them do not wobble alike.
    count: u32,
}

struct Running {
    ang: Vec3,
    shift: Vec3,
    step: f32,
    randomness: f32,
    duration: f32,
    age: f32,
    seed: u32,
}

/// A number in 0..1 from `x`.
fn hash01(mut x: u32) -> f32 {
    x = x.wrapping_mul(0x9E37_79B1) ^ (x >> 15);
    x = x.wrapping_mul(0x85EB_CA6B) ^ (x >> 13);
    x = x.wrapping_mul(0xC2B2_AE35) ^ (x >> 16);
    (x & 0xFFFF) as f32 / 65535.0
}

impl Running {
    /// The wave (-1..1) of one axis at the shake's age.
    fn wave(&self, axis: u32) -> f32 {
        let x = self.age / self.step;
        let k = x.floor() as u32;
        let sample = |k: u32| {
            let sign = if (k + axis).is_multiple_of(2) { 1.0 } else { -1.0 };
            sign * (1.0 - self.randomness * hash01(self.seed ^ k.wrapping_mul(31) ^ axis.wrapping_mul(0x51)))
        };
        let f = x - x.floor();
        let smooth = f * f * (3.0 - 2.0 * f);
        sample(k) + (sample(k + 1) - sample(k)) * smooth
    }
}

impl CameraShake {
    /// Starts a shake: `ang` in degrees and `shift` in metres (cry axes, the data's amplitudes), `frequency` the seconds
    /// between the two extremes of the wave.
    pub fn add(&mut self, ang: Vec3, shift: Vec3, frequency: f32, randomness: f32, duration: f32) {
        if duration <= 0.0 {
            return;
        }
        if self.active.len() >= SHAKES {
            self.active.remove(0);
        }
        self.count = self.count.wrapping_add(1);
        self.active.push(Running {
            ang,
            shift,
            step: frequency.max(0.03),
            randomness: randomness.clamp(0.0, 1.0),
            duration,
            age: 0.0,
            seed: self.count.wrapping_mul(0x2545_F491),
        });
    }

    /// Moves the shakes on by `dt` seconds; returns the camera's displacement in its own axes and the turn.
    pub fn advance(&mut self, dt: f32) -> (Vec3, Quat) {
        let mut shift = Vec3::ZERO;
        let mut ang = Vec3::ZERO;
        for s in &mut self.active {
            s.age += dt;
            let fade = (1.0 - s.age / s.duration).max(0.0);
            let w = |axis| s.wave(axis) * fade * SHAKE_SCALE;
            shift += Vec3::new(s.shift.x * w(0), s.shift.y * w(1), s.shift.z * w(2));
            ang += Vec3::new(s.ang.x * w(3), s.ang.y * w(4), s.ang.z * w(5));
        }
        self.active.retain(|s| s.age < s.duration);
        // cry x is right, y forward (into the screen), z up; a turn about x tilts, about y rolls, about z yaws
        let shift = (Vec3::new(shift.x, shift.z, -shift.y)).clamp_length_max(SHAKE_MAX_SHIFT);
        let ang = ang.clamp(Vec3::splat(-SHAKE_MAX_ANGLE), Vec3::splat(SHAKE_MAX_ANGLE)).map(f32::to_radians);
        (shift, Quat::from_euler(EulerRot::YXZ, ang.z, ang.x, ang.y))
    }

    pub fn is_idle(&self) -> bool {
        self.active.is_empty()
    }
}

/// Dev: `AA_COMBAT_CHASE=1` puts the camera behind the projectile in flight (the combat code sets the pose), to look at the arrow
/// or the spell from a few metres without guessing where the fight will be.
#[derive(Resource, Default)]
pub struct Chase(pub Option<Transform>);

/// Dev: `AA_COMBAT_CAM_REL=-147` keeps the camera at that heading from the way the player faces (degrees, level; the real client's
/// `right_camera` is -90 until the character turns to its target), to line a fight up with a capture of the real client.
#[derive(Resource)]
pub struct CamRel(pub f32);

/// How far the wall (or the ground) has shortened the camera arm (m), the part of it that is still being given back.
#[derive(Default)]
pub struct ArmClip {
    shortened: f32,
    /// A wall holds the camera (for the log line when that starts and stops).
    held: bool,
    /// `AA_CAMERA_NO_CLIP=1`: the walls do not stop the camera (for before / after shots), read on the first frame.
    off: Option<bool>,
}

impl ArmClip {
    /// Takes the shortening the walls ask for this frame: more of it at once, less of it smoothly over `dt` seconds.
    fn update(&mut self, wanted: f32, dt: f32) -> f32 {
        if wanted >= self.shortened {
            self.shortened = wanted;
        } else {
            self.shortened += (wanted - self.shortened) * (1.0 - (-ARM_RETURN * dt).exp());
        }
        self.shortened
    }
}

/// Puts the camera on its orbit around the player; stops it at walls and keeps it above the ground.
#[allow(clippy::too_many_arguments)]
pub fn follow_player(
    time: Res<Time>,
    mut shake: ResMut<CameraShake>,
    chase: Option<Res<Chase>>,
    rel: Option<Res<CamRel>>,
    mut player: Query<(&Mover, &mut Visibility), With<LocalPlayer>>,
    mut camera: Query<(&mut OrbitCamera, &mut Transform, Option<&FreeCamera>)>,
    terrain: Res<TerrainQuery>,
    collision: Res<WorldCollision>,
    water: Res<aa_world::WaterQuery>,
    lag: Option<Res<super::skill_motion::CameraLag>>,
    mut clip: Local<ArmClip>,
    mut hidden: Local<bool>,
    mut underwater: Local<bool>,
) {
    let (Ok((mover, mut visibility)), Ok((mut orbit, mut transform, free))) = (player.single_mut(), camera.single_mut()) else { return };
    // the free camera (mode 2) writes its own pose; the orbit stays where it was for the way back
    if free.is_some() {
        return;
    }
    // a mode change can lower the far limit under the wanted distance
    let max = mode_max_distance();
    orbit.wanted = orbit.wanted.min(max);
    orbit.distance = orbit.distance.min(max);
    // the zoom settles at 1.2's pace; in first person the own character is hidden (only this system's own decision is written)
    let distance = approach_distance(orbit.distance, orbit.wanted, time.delta_secs());
    if distance != orbit.distance {
        orbit.distance = distance;
    }
    // a player entity rebuilt while in first person comes back visible: hide it again
    let fp = first_person(orbit.wanted);
    if fp != *hidden || (fp && *visibility != Visibility::Hidden) {
        *hidden = fp;
        *visibility = if fp { Visibility::Hidden } else { Visibility::Inherited };
    }
    // the pivot trails the player through a charge or a blink (`skill_motion`) and catches up
    let pivot_pos = mover.pos + lag.map_or(Vec3::ZERO, |l| l.0);
    if let Some(pose) = chase.and_then(|c| c.0) {
        *transform = pose;
        return;
    }
    if let Some(rel) = rel {
        orbit.yaw = mover.facing + rel.0.to_radians();
        orbit.pitch = 0.0;
    }
    let mut orbit = *orbit;
    let mut t = camera_transform(&orbit, pivot_pos);
    // underwater the camera dives: entering snaps its pitch down, afterwards it may not look further down than the dive angle
    let camera_game = bevy_to_cry(t.translation);
    let wet = water.water_at(camera_game).is_some_and(|(surface, _)| camera_game.z < surface);
    let pitch = dive_pitch(orbit.pitch, wet, wet && !*underwater);
    *underwater = wet;
    if pitch != orbit.pitch {
        orbit.pitch = pitch;
        t = camera_transform(&orbit, pivot_pos);
    }
    // the arm from the pivot above the head to the wanted place stops before the first wall
    let pivot_game = pivot_pos + Vec3::Z * PIVOT_HEIGHT;
    let pivot = cry_to_bevy(pivot_game);
    let wanted = bevy_to_cry(t.translation);
    let full = wanted.distance(pivot_game);
    let off = *clip.off.get_or_insert_with(|| std::env::var_os("AA_CAMERA_NO_CLIP").is_some());
    // in first person the camera stands on the pivot: there is no arm to sweep
    let free = if off || full < 1e-3 { full } else { collision.clip_camera(pivot_game, wanted, CAMERA_RADIUS).distance(pivot_game) };
    let wanted_cut = (full - free).max(0.0);
    let shortened = clip.update(wanted_cut, time.delta_secs());
    if (wanted_cut > 0.02) != clip.held {
        clip.held = wanted_cut > 0.02;
        if clip.held {
            info!("camera: a wall stops the arm at {free:.2} m of {full:.2} m (player {:.1}, {:.1}, {:.2})", mover.pos.x, mover.pos.y, mover.pos.z);
        } else {
            debug!("camera: the arm is free again ({full:.2} m)");
        }
    }
    if shortened > 1e-4 && full > 1e-4 {
        t.translation = pivot + (t.translation - pivot) * ((full - shortened).max(0.0) / full);
    }
    // not below the terrain (bevy y = game z), whatever the sweep did on a steep slope
    let ground = height_or(&terrain, t.translation.x, -t.translation.z, mover.pos.z) + 0.15;
    if CAMERA_TARGET_GROUND_ALIGN && t.translation.y < ground {
        t.translation.y = ground;
        // look at the pivot from the corrected position (on the pivot, first person, the orbit's own rotation stays)
        if t.translation.distance(pivot) > 1e-3 {
            t.look_at(pivot, Vec3::Y);
        }
    }
    if !shake.is_idle() {
        let (shift, turn) = shake.advance(time.delta_secs());
        t.translation += t.rotation * shift;
        t.rotation *= turn;
    }
    *transform = t;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shake_moves_the_camera_and_dies_out() {
        let mut shake = CameraShake::default();
        assert!(shake.is_idle());
        shake.add(Vec3::new(2.0, 4.0, 2.0), Vec3::new(0.0, 0.2, 0.0), 0.01, 0.3, 1.3);
        let mut moved = 0.0f32;
        let mut t = 0.0;
        while t < 1.0 {
            let (shift, turn) = shake.advance(1.0 / 60.0);
            assert!(shift.length() <= SHAKE_MAX_SHIFT + 1e-6);
            assert!(turn.is_normalized());
            moved = moved.max(shift.length()).max(turn.angle_between(Quat::IDENTITY));
            t += 1.0 / 60.0;
        }
        assert!(moved > 0.001, "{moved}");
        assert!(!shake.is_idle());
        for _ in 0..30 {
            shake.advance(1.0 / 60.0);
        }
        assert!(shake.is_idle());
        assert_eq!(shake.advance(0.1), (Vec3::ZERO, Quat::IDENTITY));
    }

    #[test]
    fn a_huge_shake_is_held_in() {
        let mut shake = CameraShake::default();
        shake.add(Vec3::splat(40.0), Vec3::splat(5.0), 0.05, 0.0, 2.0);
        for _ in 0..60 {
            let (shift, turn) = shake.advance(1.0 / 60.0);
            assert!(shift.length() <= SHAKE_MAX_SHIFT + 1e-6);
            assert!(turn.angle_between(Quat::IDENTITY) < 0.2);
        }
    }

    #[test]
    fn only_a_few_shakes_run_at_once() {
        let mut shake = CameraShake::default();
        for _ in 0..20 {
            shake.add(Vec3::ONE, Vec3::ONE, 0.05, 0.1, 1.0);
        }
        assert_eq!(shake.active.len(), SHAKES);
        shake.add(Vec3::ONE, Vec3::ONE, 0.05, 0.1, 0.0);
        assert_eq!(shake.active.len(), SHAKES);
    }
}

#[cfg(test)]
mod zoom_tests {
    use super::*;

    #[test]
    fn the_start_distance_is_1_2s_table_by_race_and_gender() {
        use aa_net::model::{gender::*, race::*};
        let table = [(NUIAN, 4.0, 3.2), (ELF, 3.8, 3.4), (HARIHARAN, 3.6, 3.2), (FERRE, 4.0, 3.4)];
        for (race, male, female) in table {
            assert_eq!(start_distance(race, MALE), male, "race {race} male");
            assert_eq!(start_distance(race, FEMALE), female, "race {race} female");
        }
        // any other row of the table, gender 0 too, is 10 m, never past the active mode's far limit
        assert_eq!(start_distance(2, MALE), 10.0);
        assert_eq!(start_distance(NUIAN, 0), 10.0);
        assert!(start_distance(0, 0) <= mode_max_distance());
    }

    #[test]
    fn the_far_limit_is_the_active_modes_camera_max_dist() {
        // option 65: 1 is the action mode's 10 m, 2 (the running client's default) classic's 18 m
        assert_eq!(mode_max_distance_for(Some(1.0)), ACTION_MAX_DISTANCE);
        assert_eq!(mode_max_distance_for(Some(2.0)), CLASSIC_MAX_DISTANCE);
        assert_eq!(mode_max_distance_for(None), CLASSIC_MAX_DISTANCE);
        assert_eq!(zoom_notch_with(9.5, -1.0, CLASSIC_MAX_DISTANCE), 10.925);
        let mut wanted = 9.5;
        for _ in 0..40 {
            wanted = zoom_notch_with(wanted, -1.0, CLASSIC_MAX_DISTANCE);
        }
        assert_eq!(wanted, CLASSIC_MAX_DISTANCE, "classic zooms out to 18 m");
        for _ in 0..40 {
            wanted = zoom_notch_with(wanted, -1.0, ACTION_MAX_DISTANCE);
        }
        assert_eq!(wanted, ACTION_MAX_DISTANCE, "action stops at 10 m");
    }

    #[test]
    fn a_notch_steps_as_a_running_1_2_showed_it() {
        // every value of 1.2's camera floats sampled notch by notch (2026-10-20): one in from 3.6, out to the far limit, in to
        // first person, out twice (the action mode's 10 m)
        let mut wanted = zoom_notch_with(3.6, 1.0, ACTION_MAX_DISTANCE);
        assert!((wanted - 3.06).abs() < 0.002, "{wanted}");
        for expected in [3.56, 4.094, 4.708, 5.414, 6.226, 7.160, 8.235, 9.470, 10.0] {
            wanted = zoom_notch_with(wanted, -1.0, ACTION_MAX_DISTANCE);
            assert!((wanted - expected).abs() < 0.002, "{wanted} against 1.2's {expected}");
        }
        assert_eq!(zoom_notch_with(wanted, -1.0, ACTION_MAX_DISTANCE), ACTION_MAX_DISTANCE, "the far limit holds");
        for expected in [8.5, 7.225, 6.141, 5.220, 4.437, 3.772, 3.206, 2.706, 2.206, 1.706, 1.206, 0.0] {
            wanted = zoom_notch_with(wanted, 1.0, ACTION_MAX_DISTANCE);
            assert!((wanted - expected).abs() < 0.002, "{wanted} against 1.2's {expected}");
        }
        for expected in [1.0, 1.5] {
            wanted = zoom_notch_with(wanted, -1.0, ACTION_MAX_DISTANCE);
            assert!((wanted - expected).abs() < 0.002, "{wanted} against 1.2's {expected}");
        }
    }

    #[test]
    fn first_person_is_a_snap_under_one_metre() {
        // 1.2: 1.206 -> 0 zooming in; out of first person 0 -> 1.0 -> 1.5
        assert_eq!(zoom_notch_with(1.206, 1.0, CLASSIC_MAX_DISTANCE), 0.0);
        assert_eq!(zoom_notch_with(0.0, -1.0, CLASSIC_MAX_DISTANCE), 1.0);
        assert_eq!(zoom_notch_with(1.0, -1.0, CLASSIC_MAX_DISTANCE), 1.5);
        assert_eq!(zoom_notch_with(0.0, 1.0, CLASSIC_MAX_DISTANCE), 0.0);
        assert!(first_person(0.0) && !first_person(1.0));
    }

    #[test]
    fn from_a_nuian_mans_start_the_sixth_notch_in_is_first_person_and_the_notches_take_it_out_to_the_far_limit() {
        let mut wanted = start_distance(aa_net::model::race::NUIAN, aa_net::model::gender::MALE);
        for notch in 1..=6 {
            wanted = zoom_notch_with(wanted, 1.0, CLASSIC_MAX_DISTANCE);
            assert_eq!(first_person(wanted), notch == 6, "notch {notch}: {wanted}");
        }
        let mut notches = 0;
        while wanted < CLASSIC_MAX_DISTANCE {
            wanted = zoom_notch_with(wanted, -1.0, CLASSIC_MAX_DISTANCE);
            notches += 1;
            if notches == 13 {
                assert!((wanted - 9.310).abs() < 0.002, "{wanted}");
            }
        }
        assert_eq!(notches, 18, "first person (0 m) to classic's 18 m");
    }

    #[test]
    fn the_camera_settles_as_fast_as_1_2s() {
        // 1.2's notch steps and settle times from its run of 2026-10-19 (step m, ms), at 1.2's own 7 ms frames: within 10 %
        for (gap, ms) in [(0.700, 134.0), (0.805, 153.0), (0.926, 170.0), (1.066, 198.0), (0.904, 170.0), (0.769, 147.0)] {
            let (mut distance, mut t, mut last) = (5.0f32, 0.0f32, 5.0f32);
            let wanted = 5.0 - gap;
            while distance != wanted {
                distance = approach_distance(distance, wanted, 0.007);
                assert!(distance <= last && distance >= wanted, "never past the wanted distance");
                last = distance;
                t += 0.007;
            }
            assert!((t * 1000.0 - ms).abs() <= ms * 0.1, "{gap} m settled in {:.0} ms, 1.2 {ms}", t * 1000.0);
        }
    }

    #[test]
    fn each_fixed_view_key_sets_its_offset_from_the_heading_at_once() {
        use std::f32::consts::{FRAC_PI_2, PI};
        assert_eq!(fixed_view_offset("front_camera"), Some(PI));
        assert_eq!(fixed_view_offset("left_camera"), Some(FRAC_PI_2));
        assert_eq!(fixed_view_offset("right_camera"), Some(3.0 * FRAC_PI_2));
        assert_eq!(fixed_view_offset("back_camera"), Some(0.0));
        assert_eq!(fixed_view_offset("zoom"), None);
        // relative to the heading: facing 0.5 rad, the front view looks back at the face
        assert!((view_yaw(0.5, FRONT_VIEW) - (0.5 + PI)).abs() < 1e-6);
        // the character turns with A or D by d and the camera by d too: the offset is kept
        let (facing, yaw) = (0.5f32, view_yaw(0.5, LEFT_VIEW));
        let d = 0.3;
        assert!(((yaw + d) - (facing + d) - LEFT_VIEW).abs() < 1e-6);
    }

    #[test]
    fn the_cycle_keys_step_from_the_last_fixed_value_not_from_the_mouse() {
        use std::f32::consts::{FRAC_PI_2, PI, TAU};
        let close = |a: f32, b: f32| (a - b).rem_euclid(TAU).min((b - a).rem_euclid(TAU)) < 1e-5;
        // Insert, then the mouse drags the camera elsewhere (the kept value stays PI), then Page Down
        let kept = FRONT_VIEW;
        let next = cycle_view(kept, true);
        assert!(close(next, PI - FRAC_PI_2));
        // three Page Downs from the back view go round: 3pi/2, pi, pi/2
        let mut v = BACK_VIEW;
        let mut seen = Vec::new();
        for _ in 0..3 {
            v = cycle_view(v, true);
            seen.push(v);
        }
        assert!(close(seen[0], 3.0 * FRAC_PI_2) && close(seen[1], PI) && close(seen[2], FRAC_PI_2));
        assert!(close(cycle_view(BACK_VIEW, false), FRAC_PI_2));
    }

    #[test]
    fn the_six_manager_modes_round_trip_their_bytes() {
        let modes = [CameraMode::Unit, CameraMode::Free, CameraMode::Mode3, CameraMode::Mode4, CameraMode::Mode5, CameraMode::Mode6];
        for (i, mode) in modes.iter().enumerate() {
            assert_eq!(mode.number(), i as u8 + 1);
            assert_eq!(CameraMode::from_number(i as u8 + 1), *mode);
        }
        assert_eq!(CameraMode::from_number(0), CameraMode::Unit);
        assert_eq!(CameraMode::from_number(7), CameraMode::Unit);
    }

    #[test]
    fn the_free_camera_moves_and_turns_at_its_cvars() {
        let at = Vec3::new(100.0, 200.0, 10.0);
        let input = FreeInput { forward: 1.0, right: 0.0, up: 0.5, ..Default::default() };
        let (pos, yaw, pitch) = free_camera_step(at, 0.0, 0.0, input, 1.0);
        assert!((pos.y - (200.0 + FREE_Y_SPEED)).abs() < 1e-4, "{pos:?}");
        assert!((pos.z - (10.0 + FREE_Z_SPEED * 0.5)).abs() < 1e-4, "{pos:?}");
        assert_eq!((yaw, pitch), (0.0, 0.0));
        // Shift is the fast scale on every axis
        let (fast, _, _) = free_camera_step(at, 0.0, 0.0, FreeInput { fast: true, ..input }, 1.0);
        assert!((fast.y - (200.0 + FREE_Y_SPEED * FREE_FAST_SPEED_SCALE)).abs() < 1e-4, "{fast:?}");
        // a mouse drag turns it; 1000 counts is 4 radians at 0.004
        let (_, yaw, pitch) = free_camera_step(at, 0.0, 0.0, FreeInput { mouse: Vec2::new(1000.0, 0.0), ..Default::default() }, 0.016);
        assert!((yaw + 4.0).abs() < 1e-4, "{yaw}");
        assert_eq!(pitch, 0.0);
        // the keyboard turn speed: a whole second of a held turn is 60 degrees
        let (_, yaw, _) = free_camera_step(at, 0.0, 0.0, FreeInput { turn: 1.0, ..Default::default() }, 1.0);
        assert!((yaw - FREE_X_TURN.to_radians()).abs() < 1e-4, "{yaw}");
    }

    #[test]
    fn underwater_the_camera_dives_to_the_dive_pitch_and_holds_the_dive_angle() {
        assert_eq!(dive_pitch(0.4, false, false), 0.4, "dry: untouched");
        let entering = dive_pitch(0.4, true, true);
        assert!((entering + CAMERA_DIVE_PITCH_DEG.to_radians()).abs() < 1e-6, "{entering}");
        let limit = -CAMERA_DIVE_ANGLE_DEG.to_radians();
        assert_eq!(dive_pitch(-1.2, true, false), limit, "no further down than the dive angle");
        let up = 0.7;
        assert_eq!(dive_pitch(up, true, false), up, "looking up stays free");
    }

    #[test]
    fn a_long_frame_moves_the_camera_no_further_than_33_ms() {
        assert_eq!(approach_distance(4.0, 10.0, 0.25), approach_distance(4.0, 10.0, 0.033));
        assert_eq!(approach_distance(4.0, 4.0, 0.1), 4.0);
        assert_eq!(approach_distance(4.0, 3.9, 0.033), 3.9, "a small gap closes in one frame");
    }
}
