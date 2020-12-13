use geom::Duration;

use crate::{Canvas, Event, Key, MultiKey, ScreenPt};

// As we check for user input, record the input and the thing that would happen. This will let us
// build up some kind of OSD of possible actions.
pub struct UserInput {
    pub(crate) event: Event,
    pub(crate) event_consumed: bool,

    lctrl_held: bool,
}

impl UserInput {
    pub(crate) fn new(event: Event, canvas: &Canvas) -> UserInput {
        UserInput {
            event,
            event_consumed: false,
            lctrl_held: canvas.keys_held.contains(&Key::LeftControl),
        }
    }

    pub fn pressed<MK: Into<Option<MultiKey>>>(&mut self, multikey: MK) -> bool {
        let mk = if let Some(mk) = multikey.into() {
            mk
        } else {
            return false;
        };
        if self.event_consumed {
            return false;
        }

        if let Event::KeyPress(pressed) = self.event {
            let same = match mk {
                MultiKey::Normal(key) => pressed == key && !self.lctrl_held,
                MultiKey::LCtrl(key) => pressed == key && self.lctrl_held,
                MultiKey::Any(keys) => !self.lctrl_held && keys.contains(&pressed),
            };
            if same {
                self.consume_event();
                return true;
            }
        }
        false
    }

    pub(crate) fn any_pressed(&mut self) -> Option<Key> {
        if self.event_consumed {
            return None;
        }

        if let Event::KeyPress(key) = self.event {
            self.consume_event();
            return Some(key);
        }
        None
    }

    pub fn key_released(&mut self, key: Key) -> bool {
        if self.event_consumed {
            return false;
        }

        if self.event == Event::KeyRelease(key) {
            self.consume_event();
            return true;
        }
        false
    }

    // No consuming for these?
    // Only places looking at special drag behavior should use these two, otherwise prefer
    // normal_left_click in EventCtx
    pub fn left_mouse_button_pressed(&mut self) -> bool {
        self.event == Event::LeftMouseButtonDown
    }
    pub fn left_mouse_button_released(&mut self) -> bool {
        self.event == Event::LeftMouseButtonUp
    }

    pub fn window_lost_cursor(&self) -> bool {
        self.event == Event::WindowLostCursor
    }

    pub fn get_moved_mouse(&self) -> Option<ScreenPt> {
        if let Event::MouseMovedTo(pt) = self.event {
            return Some(pt);
        }
        None
    }

    pub fn get_mouse_scroll(&self) -> Option<(f64, f64)> {
        if let Event::MouseWheelScroll(dx, dy) = self.event {
            return Some((dx, dy));
        }
        None
    }

    pub fn is_window_resized(&self) -> bool {
        match self.event {
            Event::WindowResized(_) => true,
            _ => false,
        }
    }

    pub fn nonblocking_is_update_event(&mut self) -> Option<Duration> {
        if self.event_consumed {
            return None;
        }

        if let Event::Update(dt) = self.event {
            Some(dt)
        } else {
            None
        }
    }
    pub fn use_update_event(&mut self) {
        self.consume_event();
        match self.event {
            Event::Update(_) => {}
            _ => panic!("Not an update event"),
        }
    }

    pub(crate) fn consume_event(&mut self) {
        assert!(!self.event_consumed);
        self.event_consumed = true;
    }
    pub(crate) fn unconsume_event(&mut self) {
        assert!(self.event_consumed);
        self.event_consumed = false;
    }

    pub(crate) fn has_been_consumed(&self) -> bool {
        self.event_consumed
    }
}
