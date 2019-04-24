use crate::widgets::{Menu, Position};
use crate::{text, Canvas, Event, InputResult, Key, ScreenPt, Text, TopMenu};
use std::collections::{BTreeMap, HashMap, HashSet};

// As we check for user input, record the input and the thing that would happen. This will let us
// build up some kind of OSD of possible actions.
pub struct UserInput {
    event: Event,
    event_consumed: bool,
    important_actions: Vec<(Key, String)>,
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
    pub(crate) set_mode_called: HashSet<String>,
    current_mode: Option<String>,
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
        canvas: &mut Canvas,
    ) -> UserInput {
        let mut input = UserInput {
            event,
            event_consumed: false,
            important_actions: Vec::new(),
            context_menu,
            // Don't move it in yet!
            top_menu: None,
            modal_state,
            reserved_keys: HashMap::new(),
            chosen_action: None,
            set_mode_called: HashSet::new(),
            current_mode: None,
        };

        // First things first...
        if let Event::WindowResized(width, height) = input.event {
            canvas.window_width = width;
            canvas.window_height = height;
        }

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
                                for (_, menu) in input.modal_state.active.iter_mut() {
                                    // context_menu is borrowed, so can't call methods on input.
                                    match menu.event(input.event, canvas) {
                                        // TODO Only consume the input if it was a mouse on top of
                                        // the menu... because we don't want to also mouseover
                                        // stuff underneath
                                        InputResult::Canceled | InputResult::StillActive => {}
                                        InputResult::Done(action, _) => {
                                            assert!(!input.event_consumed);
                                            input.event_consumed = true;
                                            input.chosen_action = Some(action);
                                            break;
                                        }
                                    }
                                }
                            }
                            ContextMenu::Displaying(ref mut menu) => {
                                // Can't call consume_event() because context_menu is borrowed.
                                assert!(!input.event_consumed);
                                input.event_consumed = true;
                                match menu.event(input.event, canvas) {
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
            if let Some(maybe_key) = menu.actions.get(action).cloned() {
                menu.valid_actions.insert(action.to_string());
                if let Some(key) = maybe_key {
                    self.unimportant_key_pressed(key, action)
                } else {
                    false
                }
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

    // Returns the bottom left of the modal menu.
    // TODO It'd be nice to scope the return value to the next draw()s only.
    pub fn set_mode_with_extra(
        &mut self,
        mode: &str,
        prompt: Text,
        canvas: &Canvas,
        extra_width: f64,
        _extra_height: f64,
    ) -> ScreenPt {
        self.set_mode_called.insert(mode.to_string());
        self.current_mode = Some(mode.to_string());
        if let Some(ref mut menu) = self.modal_state.mut_active_mode(mode) {
            menu.mark_all_inactive();
            menu.change_prompt(prompt);
            menu.get_bottom_left()
        } else {
            if let Some(ref m) = self.modal_state.modes.get(mode) {
                let mut menu = Menu::new(
                    Some(prompt),
                    m.actions
                        .iter()
                        .map(|(key, action)| (Some(*key), action.to_string(), *key))
                        .collect(),
                    false,
                    Position::TopRightOfScreen(extra_width),
                    canvas,
                );
                menu.mark_all_inactive();
                let corner = menu.get_bottom_left();
                self.modal_state.active.push((mode.to_string(), menu));
                corner
            } else {
                panic!("set_mode called on unknown {}", mode);
            }
        }
    }

    pub fn set_mode_with_prompt(
        &mut self,
        mode: &str,
        prompt: String,
        canvas: &Canvas,
    ) -> ScreenPt {
        let mut txt = Text::new();
        txt.add_styled_line(prompt, None, Some(text::PROMPT_COLOR), None);
        self.set_mode_with_extra(mode, txt, canvas, 0.0, 0.0)
    }

    pub fn set_mode_with_new_prompt(
        &mut self,
        mode: &str,
        prompt: Text,
        canvas: &Canvas,
    ) -> ScreenPt {
        self.set_mode_with_extra(mode, prompt, canvas, 0.0, 0.0)
    }

    pub fn set_mode(&mut self, mode: &str, canvas: &Canvas) -> ScreenPt {
        self.set_mode_with_prompt(mode, mode.to_string(), canvas)
    }

    pub fn modal_action(&mut self, action: &str) -> bool {
        if let Some(ref mode) = self.current_mode {
            if self.chosen_action == Some(action.to_string()) {
                self.chosen_action = None;
                return true;
            }
            // TODO When an action is chosen, the plugin short-circuits and we don't mark other
            // items active. And here, we don't even mark the chosen action as active again. This
            // is semantically correct (think about holding down the key for deleting the current
            // cycle), but causes annoying flickering.

            if self.modal_state.modes[mode].get_key(action).is_some() {
                self.modal_state
                    .mut_active_mode(mode)
                    .unwrap()
                    .mark_active(action);
                // Don't check for the keypress here; Menu's event() will have already processed it
                // and set chosen_action.
                false
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

    pub(crate) fn window_gained_cursor(&mut self) -> bool {
        self.event == Event::WindowGainedCursor
    }
    pub fn window_lost_cursor(&mut self) -> bool {
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
        for (key, a) in self.important_actions.drain(..) {
            osd.add_line("Press ".to_string());
            osd.append(key.describe(), Some(text::HOTKEY_COLOR));
            osd.append(format!(" to {}", a), None);
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

    pub(crate) active: Vec<(String, Menu<Key>)>,
}

impl ModalMenuState {
    pub fn new(modes: Vec<ModalMenu>) -> ModalMenuState {
        // TODO Make sure mode names aren't repeated
        ModalMenuState {
            modes: modes.into_iter().map(|m| (m.name.clone(), m)).collect(),
            active: Vec::new(),
        }
    }

    fn mut_active_mode(&mut self, mode: &str) -> Option<&mut Menu<Key>> {
        for (name, menu) in self.active.iter_mut() {
            if mode == name {
                return Some(menu);
            }
        }
        None
    }
}
