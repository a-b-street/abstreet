use crate::common::warp::Warping;
use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use abstutil::Cloneable;
use ezgui::{hotkey, EventCtx, EventLoopMode, Key, Warper, Wizard};
use geom::Pt2D;
use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Shortcut {
    name: String,
    center: Pt2D,
    cam_zoom: f64,
}

impl Cloneable for Shortcut {}

pub struct ChoosingShortcut;
impl ChoosingShortcut {
    pub fn new() -> Box<dyn State> {
        WizardState::new(Box::new(choose_shortcut))
    }
}

fn choose_shortcut(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let center = ctx.canvas.center_to_map_pt();
    let cam_zoom = ctx.canvas.cam_zoom;

    let mut wizard = wiz.wrap(ctx);
    let (_, mut s) = wizard.choose_something_hotkeys("Jump to which shortcut?", || {
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

        let mut shortcuts = vec![Shortcut {
            name: "Create a new shortcut here".to_string(),
            center,
            cam_zoom,
        }];
        shortcuts.extend(
            abstutil::load_all_objects::<Shortcut>(abstutil::SHORTCUTS, ui.primary.map.get_name())
                .into_iter()
                .map(|(_, s)| s),
        );
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
        Some(Transition::Pop)
    } else {
        Some(Transition::ReplaceWithMode(
            Box::new(Warping {
                warper: Warper::new(ctx, s.center, Some(s.cam_zoom)),
                id: None,
            }),
            EventLoopMode::Animation,
        ))
    }
}
