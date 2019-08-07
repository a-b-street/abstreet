use crate::common::warp::Warping;
use crate::game::{State, Transition};
use crate::ui::UI;
use abstutil::Cloneable;
use ezgui::{hotkey, EventCtx, EventLoopMode, GfxCtx, Key, Warper, Wizard, WrappedWizard};
use geom::Pt2D;
use serde_derive::{Deserialize, Serialize};

pub struct ChoosingShortcut {
    wizard: Wizard,
    shortcuts: Vec<Shortcut>,
}

impl ChoosingShortcut {
    pub fn new(ui: &UI) -> ChoosingShortcut {
        ChoosingShortcut {
            wizard: Wizard::new(),
            shortcuts: abstutil::load_all_objects::<Shortcut>(
                abstutil::SHORTCUTS,
                ui.primary.map.get_name(),
            )
            .into_iter()
            .map(|(_, s)| s)
            .collect(),
        }
    }
}

impl State for ChoosingShortcut {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        // TODO Ahh expensive
        let mut shortcuts = vec![Shortcut {
            name: "Create a new shortcut here".to_string(),
            center: ctx.canvas.center_to_map_pt(),
            cam_zoom: ctx.canvas.cam_zoom,
        }];
        shortcuts.extend(self.shortcuts.clone());

        if let Some(s) = choose_shortcut(&mut self.wizard.wrap(ctx), shortcuts, ui) {
            return Transition::ReplaceWithMode(
                Box::new(Warping {
                    warper: Warper::new(ctx, s.center, Some(s.cam_zoom)),
                    id: None,
                }),
                EventLoopMode::Animation,
            );
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.wizard.draw(g);
    }
}

fn choose_shortcut(
    wizard: &mut WrappedWizard,
    shortcuts: Vec<Shortcut>,
    ui: &UI,
) -> Option<Shortcut> {
    let (_, mut s) = wizard.new_choose_something("Jump to which shortcut?", || {
        // TODO Handle >9
        // TODO Allow deleting
        let keys = vec![
            Key::Num1,
            Key::Num2,
            Key::Num3,
            Key::Num4,
            Key::Num5,
            Key::Num6,
            Key::Num7,
            Key::Num8,
            Key::Num9,
        ];

        shortcuts
            .into_iter()
            .enumerate()
            .map(|(idx, s)| {
                if idx == 0 {
                    (None, s.name.clone(), s)
                } else {
                    (hotkey(keys[idx - 1]), s.name.clone(), s)
                }
            })
            .collect()
    })?;
    if s.name == "Create a new shortcut here" {
        // TODO Enforce non-empty, unique names
        let name = wizard.input_string("Name this shortcut")?;
        s.name = name;
        abstutil::save_json_object(abstutil::SHORTCUTS, ui.primary.map.get_name(), &s.name, &s);
        wizard.abort();
        None
    } else {
        Some(s)
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct Shortcut {
    name: String,
    center: Pt2D,
    cam_zoom: f64,
}

impl Cloneable for Shortcut {}
