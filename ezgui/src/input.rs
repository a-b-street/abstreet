use crate::menu::{Menu, Position};
use crate::top_menu::TopMenu;
use crate::{Canvas, Event, InputResult, Key, ScreenPt, Text};
use std::collections::{BTreeMap, HashMap, HashSet};

// As we check for user input, record the input and the thing that would happen. This will let us
// build up some kind of OSD of possible actions.
pub struct UserInput {
    event: Event,
    event_consumed: bool,
    unimportant_actions: Vec<String>,
    important_actions: Vec<String>,
    // If two different callers both expect the same key, there's likely an unintentional conflict.
    reserved_keys: HashMap<Key, String>,

    // When context or top menus are active, most methods lie about having input.
    // TODO This is hacky, but if we consume_event in things like get_moved_mouse, then canvas
    // dragging and UI mouseover become mutex. :\
    // TODO Logically these are borrowed, but I think that requires lots of lifetime plumbing right
    // now...
    pub(crate) context_menu: ContextMenu,
    pub(crate) top_menu: Option<TopMenu>,
    pub(crate) modal_state: ModalMenuState,

    // This could be from context_menu or modal_state.
    // TODO Is that potentially confusing?
    pub(crate) chosen_action: Option<String>,
    pub(crate) set_mode_called: bool,
}

pub enum ContextMenu {
    Inactive,
    Building(ScreenPt, BTreeMap<Key, String>),
    Displaying(Menu<Key>),
    Clicked(Key),
}

impl ContextMenu {
    pub fn maybe_build(self, canvas: &Canvas) -> ContextMenu {
        match self {
            ContextMenu::Building(origin, actions) => {
                if actions.is_empty() {
                    ContextMenu::Inactive
                } else {
                    ContextMenu::Displaying(Menu::new(
                        None,
                        actions
                            .into_iter()
                            .map(|(hotkey, action)| (Some(hotkey), action, hotkey))
                            .collect(),
                        false,
                        Position::TopLeftAt(origin),
                        canvas,
                    ))
                }
            }
            _ => self,
        }
    }
}

impl UserInput {
    pub(crate) fn new(
        event: Event,
        context_menu: ContextMenu,
        mut top_menu: Option<TopMenu>,
        modal_state: ModalMenuState,
        canvas: &Canvas,
    ) -> UserInput {
        let mut input = UserInput {
            event,
            event_consumed: false,
            unimportant_actions: Vec::new(),
            important_actions: Vec::new(),
            context_menu,
            // Don't move it in yet!
            top_menu: None,
            modal_state,
            reserved_keys: HashMap::new(),
            chosen_action: None,
            set_mode_called: false,
        };

        if let Some(ref mut menu) = top_menu {
            match menu.event(&mut input, canvas) {
                // Keep going; the input hasn't been consumed.
                InputResult::Canceled => {
                    // Create the context menu here, even if one already existed.
                    if input.right_mouse_button_pressed() {
                        assert!(!input.event_consumed);
                        input.event_consumed = true;
                        input.context_menu = ContextMenu::Building(
                            canvas.get_cursor_in_screen_space(),
                            BTreeMap::new(),
                        );
                    } else {
                        match input.context_menu {
                            ContextMenu::Inactive => {
                                if let Some((_, ref mut menu)) = input.modal_state.active {
                                    // context_menu is borrowed, so can't call methods on input.
                                    match menu.event(input.event) {
                                        // TODO Only consume the input if it was a mouse on top of
                                        // the menu... because we don't want to also mouseover
                                        // stuff underneath
                                        InputResult::Canceled | InputResult::StillActive => {}
                                        InputResult::Done(action, _) => {
                                            assert!(!input.event_consumed);
                                            input.event_consumed = true;
                                            input.chosen_action = Some(action);
                                        }
                                    }
                                }
                            }
                            ContextMenu::Displaying(ref mut menu) => {
                                // Can't call consume_event() because context_menu is borrowed.
                                assert!(!input.event_consumed);
                                input.event_consumed = true;
                                match menu.event(input.event) {
                                    InputResult::Canceled => {
                                        input.context_menu = ContextMenu::Inactive;
                                    }
                                    InputResult::StillActive => {}
                                    InputResult::Done(_, hotkey) => {
                                        input.context_menu = ContextMenu::Clicked(hotkey);
                                    }
                                }
                            }
                            ContextMenu::Building(_, _) | ContextMenu::Clicked(_) => {
                                panic!("UserInput::new given a ContextMenu in an impossible state");
                            }
                        }
                    }
                }
                // The context menu can't coexist.
                InputResult::StillActive => {}
                InputResult::Done(action, _) => {
                    input.chosen_action = Some(action);
                }
            }
            menu.valid_actions.clear();
        }
        input.top_menu = top_menu;

        input
    }

    pub fn number_chosen(&mut self, num_options: usize, action: &str) -> Option<usize> {
        assert!(num_options >= 1 && num_options <= 9);

        if self.context_menu_active() {
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
        self.important_actions
            .push(format!("Press {} to {}", key.describe(), action));
        false
    }

    pub fn contextual_action(&mut self, hotkey: Key, action: &str) -> bool {
        match self.context_menu {
            ContextMenu::Inactive => {
                // If the menu's not active (the user hasn't right-clicked yet), then still allow the
                // legacy behavior of just pressing the hotkey.
                return self.unimportant_key_pressed(hotkey, &format!("CONTEXTUAL: {}", action));
            }
            ContextMenu::Building(_, ref mut actions) => {
                // The event this round was the right click, so don't check if the right keypress
                // happened.
                if let Some(prev_action) = actions.get(&hotkey) {
                    if prev_action != action {
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
                    self.context_menu = ContextMenu::Inactive;
                    return true;
                }
            }
            ContextMenu::Clicked(key) => {
                if key == hotkey {
                    self.context_menu = ContextMenu::Inactive;
                    return true;
                }
            }
        }
        false
    }

    pub fn action_chosen(&mut self, action: &str) -> bool {
        if self.chosen_action == Some(action.to_string()) {
            self.chosen_action = None;
            return true;
        }

        if let Some(ref mut menu) = self.top_menu {
            if let Some(key) = menu.actions.get(action).cloned() {
                menu.valid_actions.insert(key);
                self.unimportant_key_pressed(key, action)
            } else {
                panic!(
                    "action_chosen(\"{}\") doesn't match actions in the TopMenu!",
                    action
                );
            }
        } else {
            panic!("action_chosen(\"{}\") without a TopMenu defined!", action);
        }
    }

    pub fn set_mode(&mut self, mode: &str, prompt: String, canvas: &Canvas) {
        self.set_mode_called = true;
        if let Some((ref existing_mode, ref mut menu)) = self.modal_state.active {
            if existing_mode != mode {
                panic!("set_mode called on both {} and {}", existing_mode, mode);
            }
            menu.mark_all_inactive();
        } else {
            if let Some(ref m) = self.modal_state.modes.get(mode) {
                let mut menu = Menu::new(
                    Some(prompt),
                    m.actions
                        .iter()
                        .map(|(key, action)| (Some(*key), action.to_string(), *key))
                        .collect(),
                    false,
                    Position::TopRightOfScreen,
                    canvas,
                );
                menu.mark_all_inactive();
                self.modal_state.active = Some((mode.to_string(), menu));
            } else {
                panic!("set_mode called on unknown {}", mode);
            }
        }
    }

    pub fn modal_action(&mut self, action: &str) -> bool {
        if let Some((ref mode, ref mut menu)) = self.modal_state.active {
            if self.chosen_action == Some(action.to_string()) {
                self.chosen_action = None;
                return true;
            }

            if let Some(key) = self.modal_state.modes[mode].get_key(action) {
                menu.mark_active(key);
                self.unimportant_key_pressed(key, action)
            } else {
                panic!("modal_action {} undefined in mode {}", action, mode);
            }
        } else {
            panic!("modal_action({}) without set_mode", action);
        }
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
        self.unimportant_actions
            .push(format!("Press {} to {}", key.describe(), action));
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
    pub(crate) fn left_mouse_button_pressed(&mut self) -> bool {
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

    pub fn is_update_event(&mut self) -> bool {
        if self.context_menu_active() {
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

    fn context_menu_active(&self) -> bool {
        match self.context_menu {
            ContextMenu::Inactive => false,
            _ => true,
        }
    }
}

pub struct ModalMenu {
    name: String,
    actions: Vec<(Key, String)>,
}

impl ModalMenu {
    pub fn new(name: &str, raw_actions: Vec<(Key, &str)>) -> ModalMenu {
        let mut keys: HashSet<Key> = HashSet::new();
        let mut action_names: HashSet<String> = HashSet::new();
        let mut actions = Vec::new();
        for (key, action) in raw_actions {
            if keys.contains(&key) {
                panic!("ModalMenu {} uses {:?} twice", name, key);
            }
            keys.insert(key);

            if action_names.contains(action) {
                panic!("ModalMenu {} defines \"{}\" twice", name, action);
            }
            action_names.insert(action.to_string());

            actions.push((key, action.to_string()));
        }

        ModalMenu {
            name: name.to_string(),
            actions,
        }
    }

    fn get_key(&self, action: &str) -> Option<Key> {
        // TODO Could precompute hash
        for (key, a) in &self.actions {
            if action == a {
                return Some(*key);
            }
        }
        None
    }
}

pub struct ModalMenuState {
    modes: HashMap<String, ModalMenu>,
    pub(crate) active: Option<(String, Menu<Key>)>,
}

impl ModalMenuState {
    pub fn new(modes: Vec<ModalMenu>) -> ModalMenuState {
        // TODO Make sure mode names aren't repeated
        ModalMenuState {
            modes: modes.into_iter().map(|m| (m.name.clone(), m)).collect(),
            active: None,
        }
    }
}
