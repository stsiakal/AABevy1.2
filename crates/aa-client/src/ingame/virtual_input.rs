//! Scripted input that feeds the client's real input pipeline.
//!
//! A script (`docs/aa-client-script.md`) must be able to do what the mouse and the keyboard do, in
//! `--headless` runs with no OS window and in `--offscreen-shot` runs, without moving or clicking the
//! owner's devices. The script writes a [`VirtualInput`]; [`inject`] runs in `PreUpdate` before
//! `bevy::input::InputSystems`, writes the same `KeyboardInput` / `MouseButtonInput` / `MouseWheel`
//! messages winit would, and moves the primary window's cursor. Bevy then updates
//! `ButtonInput<KeyCode>` / `ButtonInput<MouseButton>` / `AccumulatedMouseScroll` as for a real
//! device, `settings::update_input` turns the original `hotkey.g` bindings into `Actions`, and every
//! consumer (movement, the mount/vehicle drivers, the action bar, Escape, the HUD) sees the input
//! exactly like a physical key.
//!
//! `key f` is a tap (press and release in one frame, so `just_pressed` fires), `hold w 3` keeps the
//! key down for three seconds, `keydown` / `keyup` hold and release explicitly.

use std::collections::HashSet;

use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input::mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

/// A cursor set by the script: window pixels, or a fraction of the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VirtualCursor {
    Pixels(Vec2),
    Fraction(Vec2),
}

/// A wheel notch with the modifiers the script asked for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VirtualWheel {
    pub steps: f32,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum KeyEdge {
    Down(KeyCode),
    Up(KeyCode),
}

/// The script's queued input, from now until it is injected as real input. Empty means "use the real
/// devices" (the cursor: the real OS cursor).
#[derive(Resource, Default)]
pub struct VirtualInput {
    cursor: Option<VirtualCursor>,
    /// Key edges waiting for the next `PreUpdate`.
    keys: Vec<KeyEdge>,
    /// Keys the injector has pressed and the script has not released.
    down: HashSet<KeyCode>,
    /// Keys with a press queued but not injected yet (a second `key_down` is a no-op).
    queued: HashSet<KeyCode>,
    /// `hold`: key and how long to keep it down from the moment it is injected.
    holds: Vec<(KeyCode, f32)>,
    /// Scheduled releases, seconds on the app clock.
    releases: Vec<(KeyCode, f32)>,
    clicks: Vec<MouseButton>,
    wheels: Vec<VirtualWheel>,
}

impl VirtualInput {
    /// `cursor X Y`: both in 0..=1 are a fraction of the window, otherwise logical window pixels.
    pub fn set_cursor(&mut self, x: f32, y: f32) {
        self.cursor = Some(if (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y) {
            VirtualCursor::Fraction(Vec2::new(x, y))
        } else {
            VirtualCursor::Pixels(Vec2::new(x, y))
        });
    }

    #[cfg(test)]
    pub fn clear_cursor(&mut self) {
        self.cursor = None;
    }

    /// `key NAME`: press and release in the next injected frame (a tap: `just_pressed` fires).
    pub fn tap(&mut self, key: KeyCode) {
        self.key_down(key);
        self.key_up(key);
    }

    /// `key NAME` by the `hotkey.g` name (`w`, `f`, `escape`, `number1`, ...). False when unknown.
    pub fn tap_name(&mut self, name: &str) -> bool {
        let Some(key) = key_code(name) else { return false };
        self.tap(key);
        true
    }

    /// `keydown NAME`: hold the key until `key_up` (or the end of the run).
    pub fn key_down(&mut self, key: KeyCode) {
        if self.down.contains(&key) || !self.queued.insert(key) {
            return;
        }
        self.keys.push(KeyEdge::Down(key));
    }

    /// `keyup NAME`: release a key `key_down` or `hold` pressed.
    pub fn key_up(&mut self, key: KeyCode) {
        let queued = self.queued.remove(&key);
        self.holds.retain(|(k, _)| *k != key);
        self.releases.retain(|(k, _)| *k != key);
        if self.down.contains(&key) || queued {
            self.keys.push(KeyEdge::Up(key));
        }
    }

    /// `hold NAME SECONDS`: press now and release when the time is up.
    pub fn hold(&mut self, key: KeyCode, seconds: f32) {
        if self.down.contains(&key) || self.queued.contains(&key) {
            return;
        }
        self.queued.insert(key);
        self.keys.push(KeyEdge::Down(key));
        self.holds.push((key, seconds.max(0.0)));
    }

    /// `click left|right`: one press and release in the next injected frame.
    pub fn queue_click(&mut self, button: MouseButton) {
        self.clicks.push(button);
    }

    /// `wheel STEPS [shift] [alt] [ctrl]`: one wheel event with those modifiers held for its frame.
    pub fn queue_wheel(&mut self, steps: f32, shift: bool, alt: bool, ctrl: bool) {
        self.wheels.push(VirtualWheel { steps, shift, alt, ctrl });
    }

    /// The virtual cursor in window pixels, or the real one when none was set.
    pub fn cursor_position(&self, window: Option<&Window>) -> Option<Vec2> {
        match self.cursor {
            Some(VirtualCursor::Pixels(p)) => Some(p),
            Some(VirtualCursor::Fraction(f)) => window.map(|w| Vec2::new(f.x * w.width(), f.y * w.height())),
            None => window.and_then(|w| w.cursor_position()),
        }
    }

    /// Drop queued edges when a new session starts: input meant for the previous mode must not carry
    /// in. Keys already held are released on the next injected frame.
    pub fn clear_edges(&mut self) {
        self.keys.clear();
        self.queued.clear();
        self.holds.clear();
        self.releases.clear();
        self.clicks.clear();
        self.wheels.clear();
        let held: Vec<KeyCode> = self.down.iter().copied().collect();
        for key in held {
            self.keys.push(KeyEdge::Up(key));
        }
    }
}

/// Injects the queued edges as the messages and resources a real device would produce. Runs before
/// `InputSystems`, so Bevy's own keyboard/mouse systems update `ButtonInput` from them in this frame.
#[allow(clippy::too_many_arguments)]
pub fn inject(
    time: Res<Time>,
    mut input: ResMut<VirtualInput>,
    mut windows: Query<(Entity, &mut Window), With<PrimaryWindow>>,
    mut keyboard: MessageWriter<KeyboardInput>,
    mut mouse: MessageWriter<MouseButtonInput>,
    mut wheel: MessageWriter<MouseWheel>,
) {
    let window_entity = windows.iter().next().map_or(Entity::PLACEHOLDER, |(entity, _)| entity);
    if let Ok((_, mut window)) = windows.single_mut() {
        let cursor = match input.cursor {
            Some(VirtualCursor::Pixels(p)) => Some(p),
            Some(VirtualCursor::Fraction(f)) => Some(Vec2::new(f.x * window.width(), f.y * window.height())),
            None => None,
        };
        if let Some(position) = cursor
            && window.cursor_position() != Some(position)
        {
            window.set_cursor_position(Some(position));
        }
    }

    let now = time.elapsed_secs();
    let due: Vec<KeyCode> = input.releases.iter().filter(|(_, at)| *at <= now).map(|(key, _)| *key).collect();
    input.releases.retain(|(_, at)| *at > now);
    for key in due {
        if input.down.contains(&key) || input.queued.contains(&key) {
            input.key_up(key);
        }
    }

    let edges: Vec<KeyEdge> = std::mem::take(&mut input.keys);
    for edge in edges {
        match edge {
            KeyEdge::Down(key) => {
                input.queued.remove(&key);
                if !input.down.insert(key) {
                    continue;
                }
                keyboard.write(keyboard_input(key, true, window_entity));
                if let Some(i) = input.holds.iter().position(|(held, _)| *held == key) {
                    let (_, seconds) = input.holds.remove(i);
                    input.releases.push((key, now + seconds));
                }
            }
            KeyEdge::Up(key) => {
                input.queued.remove(&key);
                if input.down.remove(&key) {
                    keyboard.write(keyboard_input(key, false, window_entity));
                }
            }
        }
    }

    for button in std::mem::take(&mut input.clicks) {
        mouse.write(MouseButtonInput { button, state: ButtonState::Pressed, window: window_entity });
        mouse.write(MouseButtonInput { button, state: ButtonState::Released, window: window_entity });
    }

    for notch in std::mem::take(&mut input.wheels) {
        if notch.steps == 0.0 {
            continue;
        }
        // A real wheel is turned with the modifiers held: press them for this frame (unless the
        // script already holds them) and let go of the ones pressed here on the next frame.
        let modifiers = [
            (notch.ctrl, KeyCode::ControlLeft),
            (notch.alt, KeyCode::AltLeft),
            (notch.shift, KeyCode::ShiftLeft),
        ];
        for (asked, key) in modifiers {
            if asked && !input.down.contains(&key) {
                input.down.insert(key);
                keyboard.write(keyboard_input(key, true, window_entity));
                input.releases.push((key, now + 0.001));
            }
        }
        wheel.write(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: notch.steps,
            window: window_entity,
            phase: TouchPhase::Moved,
        });
    }
}

fn keyboard_input(key: KeyCode, pressed: bool, window: Entity) -> KeyboardInput {
    let logical_key = match printable(key) {
        Some(text) => Key::Character(text.into()),
        None => named_key(key),
    };
    KeyboardInput {
        key_code: key,
        logical_key,
        state: if pressed { ButtonState::Pressed } else { ButtonState::Released },
        text: pressed.then(|| printable(key)).flatten().map(Into::into),
        repeat: false,
        window,
    }
}

/// The text a real press of this key would carry (`None` for modifiers, Escape, arrows, ...).
fn printable(key: KeyCode) -> Option<&'static str> {
    use KeyCode::*;
    Some(match key {
        KeyA => "a", KeyB => "b", KeyC => "c", KeyD => "d", KeyE => "e",
        KeyF => "f", KeyG => "g", KeyH => "h", KeyI => "i", KeyJ => "j",
        KeyK => "k", KeyL => "l", KeyM => "m", KeyN => "n", KeyO => "o",
        KeyP => "p", KeyQ => "q", KeyR => "r", KeyS => "s", KeyT => "t",
        KeyU => "u", KeyV => "v", KeyW => "w", KeyX => "x", KeyY => "y",
        KeyZ => "z",
        Digit0 => "0", Digit1 => "1", Digit2 => "2", Digit3 => "3", Digit4 => "4",
        Digit5 => "5", Digit6 => "6", Digit7 => "7", Digit8 => "8", Digit9 => "9",
        Space => " ",
        Comma => ",", Period => ".", Slash => "/", Semicolon => ";", Quote => "'",
        Minus => "-", Equal => "=", Backquote => "`", BracketLeft => "[",
        BracketRight => "]", Backslash => "\\",
        _ => return None,
    })
}

fn named_key(key: KeyCode) -> Key {
    use KeyCode::*;
    match key {
        Enter | NumpadEnter => Key::Enter,
        Escape => Key::Escape,
        Tab => Key::Tab,
        Backspace => Key::Backspace,
        Delete => Key::Delete,
        Insert => Key::Insert,
        Home => Key::Home,
        End => Key::End,
        PageUp => Key::PageUp,
        PageDown => Key::PageDown,
        ArrowLeft => Key::ArrowLeft,
        ArrowRight => Key::ArrowRight,
        ArrowUp => Key::ArrowUp,
        ArrowDown => Key::ArrowDown,
        ShiftLeft | ShiftRight => Key::Shift,
        ControlLeft | ControlRight => Key::Control,
        AltLeft | AltRight => Key::Alt,
        Space => Key::Space,
        _ => Key::Unidentified(NativeKey::Unidentified),
    }
}

/// The `KeyCode` names a script can use: the `hotkey.g` names (`w`, `f`, `escape`, `number1`, ...)
/// and friendly aliases (`esc`, `return`, `left`, ...).
pub fn key_code(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    let name = name.trim().to_ascii_lowercase();
    if name.chars().count() == 1 {
        return Some(match name.as_str() {
            "a" => KeyA, "b" => KeyB, "c" => KeyC, "d" => KeyD, "e" => KeyE,
            "f" => KeyF, "g" => KeyG, "h" => KeyH, "i" => KeyI, "j" => KeyJ,
            "k" => KeyK, "l" => KeyL, "m" => KeyM, "n" => KeyN, "o" => KeyO,
            "p" => KeyP, "q" => KeyQ, "r" => KeyR, "s" => KeyS, "t" => KeyT,
            "u" => KeyU, "v" => KeyV, "w" => KeyW, "x" => KeyX, "y" => KeyY,
            "z" => KeyZ,
            "0" => Digit0, "1" => Digit1, "2" => Digit2, "3" => Digit3, "4" => Digit4,
            "5" => Digit5, "6" => Digit6, "7" => Digit7, "8" => Digit8, "9" => Digit9,
            "," => Comma, "." => Period, "/" => Slash, ";" => Semicolon, "'" => Quote,
            "-" => Minus, "=" => Equal, "`" => Backquote, "[" => BracketLeft,
            "]" => BracketRight, "\\" => Backslash,
            _ => return None,
        });
    }
    Some(match name.as_str() {
        "escape" | "esc" => Escape,
        "tab" => Tab,
        "enter" | "return" => Enter,
        "space" => Space,
        "backspace" => Backspace,
        "delete" | "del" => Delete,
        "insert" | "ins" => Insert,
        "home" => Home,
        "end" => End,
        "pageup" | "pgup" => PageUp,
        "pagedown" | "pgdn" => PageDown,
        "arrowleft" | "left" => ArrowLeft,
        "arrowright" | "right" => ArrowRight,
        "arrowup" | "up" => ArrowUp,
        "arrowdown" | "down" => ArrowDown,
        "shift" | "lshift" => ShiftLeft,
        "ctrl" | "control" | "lctrl" => ControlLeft,
        "alt" | "lalt" => AltLeft,
        "f1" => F1, "f2" => F2, "f3" => F3, "f4" => F4, "f5" => F5, "f6" => F6,
        "f7" => F7, "f8" => F8, "f9" => F9, "f10" => F10, "f11" => F11, "f12" => F12,
        "number0" | "num0" | "numpad0" => Numpad0,
        "number1" | "num1" | "numpad1" => Numpad1,
        "number2" | "num2" | "numpad2" => Numpad2,
        "number3" | "num3" | "numpad3" => Numpad3,
        "number4" | "num4" | "numpad4" => Numpad4,
        "number5" | "num5" | "numpad5" => Numpad5,
        "number6" | "num6" | "numpad6" => Numpad6,
        "number7" | "num7" | "numpad7" => Numpad7,
        "number8" | "num8" | "numpad8" => Numpad8,
        "number9" | "num9" | "numpad9" => Numpad9,
        "numlock" => NumLock,
        "capslock" => CapsLock,
        _ => return None,
    })
}

pub fn plugin(app: &mut App) {
    app.init_resource::<VirtualInput>()
        .add_systems(PreUpdate, inject.before(bevy::input::InputSystems));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Actions;
    use bevy::input::mouse::AccumulatedMouseScroll;
    use bevy::input::InputPlugin;
    use std::time::Duration;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(InputPlugin)
            .insert_resource(Time::<()>::default())
            .init_resource::<VirtualInput>()
            .init_resource::<Actions>()
            .init_resource::<crate::hud::HudPointerOverUi>()
            .insert_non_send(crate::hud::api::HudBridge::default())
            .add_systems(PreUpdate, inject.before(bevy::input::InputSystems))
            .add_systems(Update, crate::settings::update_input);
        app
    }

    fn actions(app: &App) -> &Actions {
        app.world().resource::<Actions>()
    }

    #[test]
    fn a_scripted_key_tap_fires_do_interaction_1_through_the_bindings() {
        let mut app = app();
        assert!(app.world_mut().resource_mut::<VirtualInput>().tap_name("f"));
        app.update();
        assert!(actions(&app).just_pressed("do_interaction_1", 0));
        assert!(!actions(&app).pressed("do_interaction_1", 0), "a tap is a press and a release");
        app.update();
        assert!(!actions(&app).just_pressed("do_interaction_1", 0), "the edge lasts one frame");
    }

    #[test]
    fn a_held_w_stays_moveforward_until_the_hold_ends() {
        let mut app = app();
        app.world_mut().resource_mut::<VirtualInput>().hold(KeyCode::KeyW, 0.25);
        app.update();
        assert!(actions(&app).pressed("moveforward", 0));
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(100));
        app.update();
        assert!(actions(&app).pressed("moveforward", 0), "still held before the time is up");
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(300));
        app.update();
        assert!(!actions(&app).pressed("moveforward", 0), "released after `hold w 0.25`");
    }

    /// A stand-in for `mount::drive_mount` / `vehicle`: the drivers read exactly this action state
    /// (`actions.pressed("moveforward", 0)`) and the shift key from `ButtonInput`; with a mount
    /// seated, the scripted hold moves it.
    #[derive(Component, Default)]
    struct TestMount {
        seated: bool,
        travelled: f32,
    }

    fn drive_test_mount(actions: Res<Actions>, mut mounts: Query<&mut TestMount>) {
        let Ok(mut mount) = mounts.single_mut() else { return };
        if mount.seated && actions.pressed("moveforward", 0) {
            mount.travelled += 1.0;
        }
    }

    #[test]
    fn a_seated_mount_driver_reads_the_scripted_hold() {
        let mut app = app();
        app.add_systems(Update, drive_test_mount.after(crate::settings::update_input));
        app.world_mut().spawn(TestMount { seated: true, travelled: 0.0 });
        app.world_mut().resource_mut::<VirtualInput>().hold(KeyCode::KeyW, 0.5);
        for _ in 0..3 {
            app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(50));
            app.update();
        }
        let mount = app.world_mut().query::<&TestMount>().single(app.world()).unwrap();
        assert!(mount.travelled > 0.0, "the seated mount saw moveforward");
    }

    #[test]
    fn a_scripted_click_is_a_real_mouse_edge() {
        let mut app = app();
        app.world_mut().resource_mut::<VirtualInput>().queue_click(MouseButton::Left);
        app.update();
        let mouse = app.world().resource::<ButtonInput<MouseButton>>();
        assert!(mouse.just_pressed(MouseButton::Left) && mouse.just_released(MouseButton::Left));
        assert!(!mouse.pressed(MouseButton::Left));
    }

    #[test]
    fn a_scripted_wheel_reaches_the_scroll_and_holds_its_modifier_for_the_frame() {
        let mut app = app();
        app.world_mut().resource_mut::<VirtualInput>().queue_wheel(2.0, false, false, false);
        app.update();
        assert_eq!(app.world().resource::<AccumulatedMouseScroll>().delta.y, 2.0);
        assert_eq!(actions(&app).zoom_lines(), 2.0, "the binding layer saw the wheel action");

        app.world_mut().resource_mut::<VirtualInput>().queue_wheel(1.0, true, false, false);
        app.update();
        assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::ShiftLeft));
        app.world_mut().resource_mut::<Time>().advance_by(Duration::from_millis(20));
        app.update();
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::ShiftLeft), "the wheel's modifier is let go");
    }

    #[test]
    fn a_scripted_cursor_moves_the_window_cursor() {
        let mut app = app();
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.world_mut().resource_mut::<VirtualInput>().set_cursor(120.0, 240.0);
        app.update();
        let window = app.world_mut().query::<&Window>().single(app.world()).unwrap();
        assert_eq!(window.cursor_position(), Some(Vec2::new(120.0, 240.0)));
        app.world_mut().resource_mut::<VirtualInput>().clear_cursor();
        assert_eq!(app.world().resource::<VirtualInput>().cursor_position(None), None);
    }

    #[test]
    fn the_script_key_names_map_to_the_hotkey_letters() {
        assert_eq!(key_code("f"), Some(KeyCode::KeyF));
        assert_eq!(key_code("Escape"), Some(KeyCode::Escape));
        assert_eq!(key_code("slot3"), None);
        assert_eq!(key_code("number1"), Some(KeyCode::Numpad1));
    }

    #[test]
    fn a_new_session_releases_the_held_keys() {
        let mut app = app();
        app.world_mut().resource_mut::<VirtualInput>().key_down(KeyCode::KeyW);
        app.update();
        assert!(app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::KeyW));
        app.world_mut().resource_mut::<VirtualInput>().clear_edges();
        app.update();
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::KeyW));
    }
}
