use crate::widgets::ContextMenu;
use crate::{hotkey, lctrl, text, Canvas, Event, InputResult, Key, Line, MultiKey, ScreenPt, Text};
use std::collections::{BTreeMap, HashMap};

// As we check for user input, record the input and the thing that would happen. This will let us
// build up some kind of OSD of possible actions.
pub struct UserInput {
    pub(crate) event: Event,
    pub(crate) event_consumed: bool,
    important_actions: Vec<(Key, String)>,
    // If two different callers both expect the same key, there's likely an unintentional conflict.
    reserved_keys: HashMap<Key, String>,

    // When context menu is active, most methods lie about having input.
    // TODO This is hacky, but if we consume_event in things like get_moved_mouse, then canvas
    // dragging and UI mouseover become mutex. :\
    // TODO Logically these are borrowed, but I think that requires lots of lifetime plumbing right
    // now...
    pub(crate) context_menu: ContextMenu,

    lctrl_held: bool,
}

impl UserInput {
    pub(crate) fn new(event: Event, context_menu: ContextMenu, canvas: &mut Canvas) -> UserInput {
        let mut input = UserInput {
            event,
            event_consumed: false,
            important_actions: Vec::new(),
            context_menu,
            reserved_keys: HashMap::new(),
            lctrl_held: canvas.lctrl_held,
        };

        // First things first...
        if let Event::WindowResized(width, height) = input.event {
            canvas.window_width = width;
            canvas.window_height = height;
        }

        if input.event == Event::KeyPress(Key::LeftControl) {
            canvas.lctrl_held = true;
        }
        if input.event == Event::KeyRelease(Key::LeftControl) {
            canvas.lctrl_held = false;
        }

        if let Some(pt) = input.get_moved_mouse() {
            canvas.cursor_x = pt.x;
            canvas.cursor_y = pt.y;

            // OK to update this here; the drag has to be initiated from canvas.handle_event, which
            // the caller must invoke.
            if let Some(click) = canvas.left_mouse_drag_from {
                canvas.cam_x += click.x - pt.x;
                canvas.cam_y += click.y - pt.y;
                canvas.left_mouse_drag_from = Some(pt);
            }
        }
        if input.event == Event::WindowGainedCursor {
            canvas.window_has_cursor = true;
        }
        if input.window_lost_cursor() {
            canvas.window_has_cursor = false;
        }

        // Create the context menu here, even if one already existed.
        if input.right_mouse_button_pressed() {
            assert!(!input.event_consumed);
            input.event_consumed = true;
            input.context_menu =
                ContextMenu::Building(canvas.get_cursor_in_screen_space(), BTreeMap::new());
            return input;
        }
        match input.context_menu {
            ContextMenu::Inactive(_) => {}
            ContextMenu::Displaying(ref mut menu) => {
                // Can't call consume_event() because context_menu is borrowed.
                assert!(!input.event_consumed);
                input.event_consumed = true;
                match menu.event(input.event, canvas) {
                    InputResult::Canceled => {
                        input.context_menu = ContextMenu::new();
                    }
                    InputResult::StillActive => {}
                    InputResult::Done(action, _) => {
                        input.context_menu = ContextMenu::Clicked(action);
                    }
                }
                return input;
            }
            ContextMenu::Building(_, _) | ContextMenu::Clicked(_) => {
                panic!("UserInput::new given a ContextMenu in an impossible state");
            }
        }

        input
    }

    pub fn key_pressed(&mut self, key: Key, action: &str) -> bool {
        if self.context_menu_active() {
            return false;
        }

        self.reserve_key(key, action);

        self.important_actions.push((key, action.to_string()));

        if self.event_consumed {
            return false;
        }

        if self.event == Event::KeyPress(key) {
            self.consume_event();
            return true;
        }
        false
    }

    pub fn contextual_action<S: Into<String>>(&mut self, hotkey: Key, raw_action: S) -> bool {
        let action = raw_action.into();
        match self.context_menu {
            ContextMenu::Inactive(ref mut keys) => {
                // If the menu's not active (the user hasn't right-clicked yet), then still allow the
                // legacy behavior of just pressing the hotkey.
                keys.insert(hotkey);
                return self.unimportant_key_pressed(hotkey, &format!("CONTEXTUAL: {}", action));
            }
            ContextMenu::Building(_, ref mut actions) => {
                // The event this round was the right click, so don't check if the right keypress
                // happened.
                if let Some(prev_action) = actions.get(&hotkey) {
                    if prev_action != &action {
                        panic!(
                            "Context menu uses hotkey {:?} for both {} and {}",
                            hotkey, prev_action, action
                        );
                    }
                } else {
                    actions.insert(hotkey, action.to_string());
                }
            }
            ContextMenu::Displaying(_) => {
                if self.event == Event::KeyPress(hotkey) {
                    self.context_menu = ContextMenu::new();
                    return true;
                }
            }
            ContextMenu::Clicked(ref this_action) => {
                if &action == this_action {
                    self.context_menu = ContextMenu::new();
                    return true;
                }
            }
        }
        false
    }

    pub fn unimportant_key_pressed(&mut self, key: Key, action: &str) -> bool {
        if self.context_menu_active() {
            return false;
        }

        self.reserve_key(key, action);

        if self.event_consumed {
            return false;
        }

        if self.event == Event::KeyPress(key) {
            self.consume_event();
            return true;
        }
        false
    }

    pub(crate) fn new_was_pressed(&mut self, multikey: MultiKey) -> bool {
        if self.context_menu_active() {
            return false;
        }

        // TODO Reserve?

        if self.event_consumed {
            return false;
        }

        if let Event::KeyPress(pressed) = self.event {
            let mk = if self.lctrl_held {
                lctrl(pressed)
            } else {
                hotkey(pressed)
            }
            .unwrap();
            if mk == multikey {
                self.consume_event();
                return true;
            }
        }
        false
    }

    pub fn key_released(&mut self, key: Key) -> bool {
        if self.context_menu_active() {
            return false;
        }

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
    pub fn left_mouse_button_pressed(&mut self) -> bool {
        if self.context_menu_active() {
            return false;
        }
        self.event == Event::LeftMouseButtonDown
    }
    pub(crate) fn left_mouse_button_released(&mut self) -> bool {
        if self.context_menu_active() {
            return false;
        }
        self.event == Event::LeftMouseButtonUp
    }
    pub(crate) fn right_mouse_button_pressed(&mut self) -> bool {
        if self.context_menu_active() {
            return false;
        }
        self.event == Event::RightMouseButtonDown
    }

    pub fn window_lost_cursor(&self) -> bool {
        self.event == Event::WindowLostCursor
    }

    pub fn get_moved_mouse(&self) -> Option<ScreenPt> {
        if self.context_menu_active() {
            return None;
        }

        if let Event::MouseMovedTo(pt) = self.event {
            return Some(pt);
        }
        None
    }

    pub(crate) fn get_mouse_scroll(&self) -> Option<f64> {
        if self.context_menu_active() {
            return None;
        }

        if let Event::MouseWheelScroll(dy) = self.event {
            return Some(dy);
        }
        None
    }

    pub fn nonblocking_is_update_event(&mut self) -> bool {
        if self.context_menu_active() {
            return false;
        }

        if self.event_consumed {
            return false;
        }

        self.event == Event::Update
    }
    pub fn use_update_event(&mut self) {
        self.consume_event();
        assert!(self.event == Event::Update)
    }

    pub fn nonblocking_is_keypress_event(&mut self) -> bool {
        if self.context_menu_active() {
            return false;
        }

        if self.event_consumed {
            return false;
        }

        match self.event {
            Event::KeyPress(_) => true,
            _ => false,
        }
    }

    // TODO I'm not sure this is even useful anymore
    pub(crate) fn use_event_directly(&mut self) -> Option<Event> {
        if self.event_consumed {
            return None;
        }
        self.consume_event();
        Some(self.event)
    }

    fn consume_event(&mut self) {
        assert!(!self.event_consumed);
        self.event_consumed = true;
    }

    // Just for Wizard
    pub(crate) fn has_been_consumed(&self) -> bool {
        self.event_consumed
    }

    pub fn populate_osd(&mut self) -> Text {
        let mut txt = Text::new();
        for (key, a) in self.important_actions.drain(..) {
            txt.add_appended(vec![
                Line("Press "),
                Line(key.describe()).fg(text::HOTKEY_COLOR),
                Line(format!(" to {}", a)),
            ]);
        }
        txt
    }

    fn reserve_key(&mut self, key: Key, action: &str) {
        if let Some(prev_action) = self.reserved_keys.get(&key) {
            println!("both {} and {} read key {:?}", prev_action, action, key);
        }
        self.reserved_keys.insert(key, action.to_string());
    }

    fn context_menu_active(&self) -> bool {
        match self.context_menu {
            ContextMenu::Inactive(_) => false,
            _ => true,
        }
    }
}
