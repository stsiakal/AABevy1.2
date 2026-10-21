//! Scripted input for placement and other pointer users.
//!
//! The real client's placement reads the OS cursor and mouse buttons. Automated runs must not move or click the
//! owner's mouse, and `--offscreen-shot` has no window cursor at all, so the script runner writes a [`VirtualInput`]
//! resource instead: [`cursor_position`] returns it when a virtual cursor is set and the real window cursor
//! otherwise; [`take_click`], [`take_wheels`] and [`take_keys`] hand queued one-frame edges to the consumer.
//!
//! The script commands are documented in `docs/aa-client-script.md`.

use bevy::prelude::*;
use bevy::window::Window;

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

/// The script's pointer and button state. Empty means "use the real input".
#[derive(Resource, Default)]
pub struct VirtualInput {
    cursor: Option<VirtualCursor>,
    click: Option<MouseButton>,
    wheels: Vec<VirtualWheel>,
    keys: Vec<KeyCode>,
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

    /// `click left|right`: one press in the next frame the consumer reads.
    pub fn queue_click(&mut self, button: MouseButton) {
        self.click = Some(button);
    }

    pub fn queue_wheel(&mut self, steps: f32, shift: bool, alt: bool, ctrl: bool) {
        self.wheels.push(VirtualWheel { steps, shift, alt, ctrl });
    }

    pub fn queue_key(&mut self, key: KeyCode) {
        self.keys.push(key);
    }

    /// `cursor <name>` for named keys (`escape`, `tab`, ...). Unknown names are ignored and reported by the caller.
    pub fn queue_key_name(&mut self, name: &str) -> bool {
        let Some(key) = key_code(name) else { return false };
        self.queue_key(key);
        true
    }

    /// The virtual cursor in window pixels, or the real one when none was set.
    pub fn cursor_position(&self, window: Option<&Window>) -> Option<Vec2> {
        match self.cursor {
            Some(VirtualCursor::Pixels(p)) => Some(p),
            Some(VirtualCursor::Fraction(f)) => window.map(|w| Vec2::new(f.x * w.width(), f.y * w.height())),
            None => window.and_then(|w| w.cursor_position()),
        }
    }

    pub fn take_click(&mut self) -> Option<MouseButton> {
        self.click.take()
    }

    pub fn take_wheels(&mut self) -> Vec<VirtualWheel> {
        std::mem::take(&mut self.wheels)
    }

    pub fn take_keys(&mut self) -> Vec<KeyCode> {
        std::mem::take(&mut self.keys)
    }

    /// Drop queued edges when a new session starts: input meant for the previous mode must not carry in.
    pub fn clear_edges(&mut self) {
        self.click = None;
        self.wheels.clear();
        self.keys.clear();
    }
}

/// The `KeyCode` names a script can use. Placement only needs Escape today; the rest are for future use.
fn key_code(name: &str) -> Option<KeyCode> {
    Some(match name.to_ascii_lowercase().as_str() {
        "escape" | "esc" => KeyCode::Escape,
        "tab" => KeyCode::Tab,
        "enter" | "return" => KeyCode::Enter,
        "space" => KeyCode::Space,
        "arrowleft" | "left" => KeyCode::ArrowLeft,
        "arrowright" | "right" => KeyCode::ArrowRight,
        _ => return None,
    })
}

pub fn plugin(app: &mut App) {
    app.init_resource::<VirtualInput>();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window() -> Window {
        let mut w = Window::default();
        w.resolution.set(1920.0, 1080.0);
        w
    }

    #[test]
    fn a_script_cursor_is_in_pixels_or_a_window_fraction() {
        let mut v = VirtualInput::default();
        v.set_cursor(0.5, 0.6);
        assert_eq!(v.cursor_position(Some(&window())), Some(Vec2::new(960.0, 648.0)));
        v.set_cursor(1250.0, 1000.0);
        assert_eq!(v.cursor_position(Some(&window())), Some(Vec2::new(1250.0, 1000.0)));
        v.clear_cursor();
        assert_eq!(v.cursor_position(None), None);
    }

    #[test]
    fn edges_are_consumed_once_and_cleared_on_a_new_session() {
        let mut v = VirtualInput::default();
        v.queue_click(MouseButton::Left);
        v.queue_wheel(1.0, false, false, false);
        v.queue_key(KeyCode::Escape);
        assert_eq!(v.take_click(), Some(MouseButton::Left));
        assert_eq!(v.take_click(), None);
        assert_eq!(v.take_wheels().len(), 1);
        assert!(v.take_wheels().is_empty());
        assert_eq!(v.take_keys(), vec![KeyCode::Escape]);
        v.queue_click(MouseButton::Right);
        v.clear_edges();
        assert_eq!(v.take_click(), None);
    }
}
