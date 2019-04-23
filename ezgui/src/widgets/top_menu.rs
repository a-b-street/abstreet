use crate::screen_geom::ScreenRectangle;
use crate::text;
use crate::widgets::{Menu, Position};
use crate::{Canvas, GfxCtx, InputResult, Key, ScreenPt, Text, UserInput};
use geom::{Polygon, Pt2D};
use std::collections::{HashMap, HashSet};

const HORIZONTAL_PADDING_BETWEEN_ITEMS: f64 = 50.0;

pub struct TopMenu {
    folders: Vec<Folder>,
    pub(crate) actions: HashMap<String, Option<Key>>,

    highlighted: Option<usize>,
    submenu: Option<(usize, Menu<()>)>,
    // Reset every round
    pub(crate) valid_actions: HashSet<String>,
}

impl TopMenu {
    pub fn new(mut folders: Vec<Folder>, canvas: &Canvas) -> TopMenu {
        let mut keys: HashSet<Key> = HashSet::new();
        let mut actions: HashMap<String, Option<Key>> = HashMap::new();
        for f in &folders {
            for (maybe_key, action) in &f.actions {
                if let Some(key) = maybe_key {
                    if keys.contains(key) {
                        panic!("TopMenu uses {:?} twice", key);
                    }
                    keys.insert(*key);
                }

                if actions.contains_key(action) {
                    panic!("TopMenu assigns \"{:?}\" twice", action);
                }
                actions.insert(action.to_string(), *maybe_key);
            }
        }

        // Calculate rectangles for the folders
        {
            let mut x1 = 0.0;
            for f in folders.iter_mut() {
                let (w, h) = canvas.text_dims(&Text::from_line(f.name.to_string()));
                f.rectangle.x1 = x1;
                f.rectangle.x2 = x1 + w;
                f.rectangle.y2 = h;

                x1 += w + HORIZONTAL_PADDING_BETWEEN_ITEMS;
            }
        }

        TopMenu {
            folders,
            actions,
            highlighted: None,
            submenu: None,
            valid_actions: HashSet::new(),
        }
    }

    // Canceled means the top menu isn't blocking input, still active means it is, and done means
    // something was clicked!
    pub fn event(&mut self, input: &mut UserInput, canvas: &Canvas) -> InputResult<()> {
        if let Some(cursor) = input.get_moved_mouse() {
            // TODO Could quickly filter out by checking y
            self.highlighted = self
                .folders
                .iter()
                .position(|f| f.rectangle.contains(cursor));
        }

        if let Some(idx) = self.highlighted {
            if input.left_mouse_button_pressed()
                || self
                    .submenu
                    .as_ref()
                    .map(|(existing_idx, _)| idx != *existing_idx)
                    .unwrap_or(false)
            {
                let f = &self.folders[idx];
                let mut menu = Menu::new(
                    None,
                    f.actions
                        .iter()
                        .map(|(maybe_key, action)| (*maybe_key, action.to_string(), ()))
                        .collect(),
                    false,
                    Position::TopLeftAt(ScreenPt::new(f.rectangle.x1, f.rectangle.y2)),
                    canvas,
                );
                menu.mark_all_inactive();
                // valid_actions can't change once this submenu is created, so determine what
                // actions are valid right now.
                for action in &self.valid_actions {
                    menu.mark_active(action);
                }
                self.submenu = Some((idx, menu));
                return InputResult::StillActive;
            }
        }

        if let Some((_, ref mut submenu)) = self.submenu {
            if let Some(ev) = input.use_event_directly() {
                match submenu.event(ev, canvas) {
                    InputResult::StillActive => {}
                    InputResult::Canceled => {
                        self.submenu = None;
                        self.highlighted = None;
                    }
                    InputResult::Done(action, _) => {
                        self.submenu = None;
                        self.highlighted = None;
                        return InputResult::Done(action, ());
                    }
                };
            }
            return InputResult::StillActive;
        }

        InputResult::Canceled
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: 0.0,
            y1: 0.0,
            x2: g.canvas.window_width,
            y2: g.canvas.top_menu_height(),
        });

        g.fork_screenspace();
        g.draw_polygon(
            text::BG_COLOR,
            &Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                g.canvas.window_width,
                g.canvas.top_menu_height(),
            ),
        );
        g.unfork();

        for (idx, f) in self.folders.iter().enumerate() {
            let mut txt = Text::with_bg_color(if Some(idx) == self.highlighted {
                Some(text::SELECTED_COLOR)
            } else {
                None
            });
            txt.add_line(f.name.to_string());
            g.draw_text_at_screenspace_topleft(&txt, ScreenPt::new(f.rectangle.x1, f.rectangle.y1));
        }

        if let Some((_, ref menu)) = self.submenu {
            menu.draw(g);
        }
    }
}

pub struct Folder {
    name: String,
    actions: Vec<(Option<Key>, String)>,

    rectangle: ScreenRectangle,
}

impl Folder {
    pub fn new(name: &str, actions: Vec<(Option<Key>, &str)>) -> Folder {
        Folder {
            name: name.to_string(),
            actions: actions
                .into_iter()
                .map(|(key, action)| (key, action.to_string()))
                .collect(),
            // TopMenu::new will calculate this.
            rectangle: ScreenRectangle {
                x1: 0.0,
                y1: 0.0,
                x2: 0.0,
                y2: 0.0,
            },
        }
    }
}
