use crate::{Canvas, Color, Event, GfxCtx, Key, Text, TEXT_FG_COLOR};
use geom::{Polygon, Pt2D};
use std::collections::{BTreeMap, HashMap};

// As we check for user input, record the input and the thing that would happen. This will let us
// build up some kind of OSD of possible actions.
pub struct UserInput {
    event: Event,
    event_consumed: bool,
    unimportant_actions: Vec<String>,
    important_actions: Vec<String>,

    // While this is present, UserInput lies about anything happening.
    pub(crate) context_menu: Option<ContextMenu>,

    // If two different callers both expect the same key, there's likely an unintentional conflict.
    reserved_keys: HashMap<Key, String>,
}

impl UserInput {
    pub(crate) fn new(
        event: Event,
        context_menu: Option<ContextMenu>,
        canvas: &Canvas,
    ) -> UserInput {
        let mut input = UserInput {
            event,
            event_consumed: false,
            unimportant_actions: Vec::new(),
            important_actions: Vec::new(),
            context_menu,
            reserved_keys: HashMap::new(),
        };

        // TODO Or left clicking outside of the menu
        // TODO If the user left clicks on a menu item, then mark that action as selected, and
        // ensure contextual_action is called this round.
        // TODO If the user hovers on a menu item, mark it for later highlighting.

        // Create the context menu here, even if one already existed.
        if input.right_mouse_button_pressed() {
            input.context_menu = Some(ContextMenu {
                actions: BTreeMap::new(),
                origin: canvas.get_cursor_in_map_space(),
                geometry: None,
                selected: None,
                clicked: None,
            });
        } else if let Some(ref mut menu) = input.context_menu {
            if let Some((ref row, height)) = menu.geometry {
                // We have to directly look at stuff here; all of input's methods lie and pretend
                // nothing is happening.
                // TODO Would it be cleaner to just consume the event? But then contextual_action will
                // be confused.
                if input.event == Event::KeyPress(Key::Escape) {
                    input.context_menu = None;
                    input.consume_event();
                } else if input.event == Event::LeftMouseButtonDown {
                    if let Some(i) = menu.selected {
                        menu.clicked = Some(*menu.actions.keys().nth(i).unwrap());
                    } else {
                        input.context_menu = None;
                        input.consume_event();
                    }
                } else if let Event::MouseMovedTo(x, y) = input.event {
                    let cursor_pt = canvas.screen_to_map((x, y));
                    let mut matched = false;
                    for i in 0..menu.actions.len() {
                        if row
                            .translate(0.0, (i as f64) * height)
                            .contains_pt(cursor_pt)
                        {
                            menu.selected = Some(i);
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        menu.selected = None;
                    }
                }
            }
        }

        input
    }

    pub fn number_chosen(&mut self, num_options: usize, action: &str) -> Option<usize> {
        assert!(num_options >= 1 && num_options <= 9);

        if self.context_menu.is_some() {
            return None;
        }

        if num_options >= 1 {
            self.reserve_key(Key::Num1, action);
        }
        if num_options >= 2 {
            self.reserve_key(Key::Num2, action);
        }
        if num_options >= 3 {
            self.reserve_key(Key::Num3, action);
        }
        if num_options >= 4 {
            self.reserve_key(Key::Num4, action);
        }
        if num_options >= 5 {
            self.reserve_key(Key::Num5, action);
        }
        if num_options >= 6 {
            self.reserve_key(Key::Num6, action);
        }
        if num_options >= 7 {
            self.reserve_key(Key::Num7, action);
        }
        if num_options >= 8 {
            self.reserve_key(Key::Num8, action);
        }
        if num_options >= 9 {
            self.reserve_key(Key::Num9, action);
        }

        if self.event_consumed {
            return None;
        }

        let num = if let Event::KeyPress(key) = self.event {
            match key {
                Key::Num1 => Some(1),
                Key::Num2 => Some(2),
                Key::Num3 => Some(3),
                Key::Num4 => Some(4),
                Key::Num5 => Some(5),
                Key::Num6 => Some(6),
                Key::Num7 => Some(7),
                Key::Num8 => Some(8),
                Key::Num9 => Some(9),
                _ => None,
            }
        } else {
            None
        };
        match num {
            Some(n) if n <= num_options => {
                self.consume_event();
                Some(n)
            }
            _ => {
                self.important_actions.push(String::from(action));
                None
            }
        }
    }

    pub fn key_pressed(&mut self, key: Key, action: &str) -> bool {
        if self.context_menu.is_some() {
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
        self.important_actions
            .push(format!("Press {} to {}", key.describe(), action));
        false
    }

    pub fn contextual_action(&mut self, hotkey: Key, action: &str) -> bool {
        if let Some(ref mut menu) = self.context_menu.as_mut() {
            if menu.clicked == Some(hotkey) {
                self.consume_event();
                self.context_menu = None;
                return true;
            }

            // We could be initially populating the menu because the user just right-clicked, or
            // this could be a later round.
            if let Some(prev_action) = menu.actions.get(&hotkey) {
                if prev_action != action {
                    panic!(
                        "Context menu uses hotkey {:?} for both {} and {}",
                        hotkey, prev_action, action
                    );
                }
            } else {
                menu.actions.insert(hotkey, action.to_string());
            }

            if self.event_consumed {
                return false;
            }

            if self.event == Event::KeyPress(hotkey) {
                self.consume_event();
                self.context_menu = None;
                return true;
            }
            false
        } else {
            // If the menu's not active (the user hasn't right-clicked yet), then still allow the
            // legacy behavior of just pressing the hotkey.
            self.key_pressed(hotkey, &format!("CONTEXTUAL: {}", action))
        }
    }

    pub fn unimportant_key_pressed(&mut self, key: Key, action: &str) -> bool {
        if self.context_menu.is_some() {
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
        self.unimportant_actions
            .push(format!("Press {} to {}", key.describe(), action));
        false
    }

    pub fn key_released(&mut self, key: Key) -> bool {
        if self.context_menu.is_some() {
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
    pub(crate) fn left_mouse_button_pressed(&mut self) -> bool {
        if self.context_menu.is_some() {
            return false;
        }
        self.event == Event::LeftMouseButtonDown
    }
    pub(crate) fn left_mouse_button_released(&mut self) -> bool {
        if self.context_menu.is_some() {
            return false;
        }
        self.event == Event::LeftMouseButtonUp
    }
    pub(crate) fn right_mouse_button_pressed(&mut self) -> bool {
        if self.context_menu.is_some() {
            return false;
        }
        self.event == Event::RightMouseButtonDown
    }

    pub fn get_moved_mouse(&self) -> Option<(f64, f64)> {
        if self.context_menu.is_some() {
            return None;
        }

        if let Event::MouseMovedTo(x, y) = self.event {
            return Some((x, y));
        }
        None
    }

    pub(crate) fn get_mouse_scroll(&self) -> Option<f64> {
        if self.context_menu.is_some() {
            return None;
        }

        if let Event::MouseWheelScroll(dy) = self.event {
            return Some(dy);
        }
        None
    }

    pub fn is_update_event(&mut self) -> bool {
        if self.context_menu.is_some() {
            return false;
        }

        if self.event_consumed {
            return false;
        }

        if self.event == Event::Update {
            self.consume_event();
            return true;
        }

        false
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

    pub fn populate_osd(&mut self, osd: &mut Text) {
        for a in &self.important_actions {
            osd.add_line(a.clone());
        }
    }

    fn reserve_key(&mut self, key: Key, action: &str) {
        if let Some(prev_action) = self.reserved_keys.get(&key) {
            println!("both {} and {} read key {:?}", prev_action, action, key);
        }
        self.reserved_keys.insert(key, action.to_string());
    }
}

pub(crate) struct ContextMenu {
    actions: BTreeMap<Key, String>,
    origin: Pt2D,
    // The rectangle representing the top row of the menu, then the height of one row
    geometry: Option<(Polygon, f64)>,
    selected: Option<usize>,
    clicked: Option<Key>,
}

impl ContextMenu {
    pub(crate) fn calculate_geometry(&mut self, g: &mut GfxCtx, canvas: &Canvas) {
        if self.geometry.is_some() {
            return;
        }

        let mut txt = Text::new();
        for (hotkey, action) in &self.actions {
            txt.add_line(format!("{} - {}", hotkey.describe(), action));
        }
        let (screen_width, screen_height) = txt.dims(g);
        let map_width = screen_width / canvas.cam_zoom;
        let map_height = screen_height / canvas.cam_zoom;
        let top_left = Pt2D::new(
            self.origin.x() - (map_width / 2.0),
            self.origin.y() - (map_height / 2.0),
        );
        let row_height = map_height / (self.actions.len() as f64);
        self.geometry = Some((
            Polygon::rectangle_topleft(top_left, map_width, row_height),
            row_height,
        ));
    }

    pub(crate) fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        let mut txt = Text::new();
        for (idx, (hotkey, action)) in self.actions.iter().enumerate() {
            let bg = if Some(idx) == self.selected {
                Some(Color::WHITE)
            } else {
                None
            };
            txt.add_styled_line(hotkey.describe(), Color::BLUE, bg);
            txt.append(format!(" - {}", action), TEXT_FG_COLOR, bg);
        }
        canvas.draw_text_at(g, txt, self.origin);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}
