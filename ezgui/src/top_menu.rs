use crate::text::LINE_HEIGHT;
use crate::{Canvas, Color, GfxCtx, Key, Text};
use geom::{Polygon, Pt2D};
use std::collections::HashSet;

pub struct TopMenu {
    folders: Vec<Folder>,

    txt: Text,
}

impl TopMenu {
    pub fn new(folders: Vec<Folder>) -> TopMenu {
        let mut keys: HashSet<Key> = HashSet::new();
        for f in &folders {
            for (key, _) in &f.actions {
                if keys.contains(key) {
                    panic!("TopMenu uses {:?} twice", key);
                }
                keys.insert(*key);
            }
        }

        let mut txt = Text::with_bg_color(None);
        for f in &folders {
            txt.append(format!("{}     ", f.name), Color::WHITE, None);
        }

        TopMenu { folders, txt }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        let old_ctx = g.fork_screenspace();
        g.draw_polygon(
            Color::BLACK.alpha(0.5),
            &Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                canvas.window_size.width as f64,
                LINE_HEIGHT,
            ),
        );
        g.unfork(old_ctx);

        canvas.draw_text_at_screenspace_topleft(g, self.txt.clone(), (0.0, 0.0));
    }
}

pub struct Folder {
    name: String,
    actions: Vec<(Key, String)>,
}

impl Folder {
    pub fn new(name: &str, actions: Vec<(Key, &str)>) -> Folder {
        Folder {
            name: name.to_string(),
            actions: actions
                .into_iter()
                .map(|(key, action)| (key, action.to_string()))
                .collect(),
        }
    }
}
