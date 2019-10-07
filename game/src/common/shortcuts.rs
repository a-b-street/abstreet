use crate::common::Warping;
use crate::game::{State, Transition, WizardState};
use crate::ui::UI;
use abstutil::{Cloneable, Timer};
use ezgui::{Choice, EventCtx, EventLoopMode, Key, Wizard};
use geom::{LonLat, Pt2D};
use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
struct Shortcut {
    name: String,
    center: LonLat,
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
    let center = ctx
        .canvas
        .center_to_map_pt()
        .forcibly_to_gps(&ui.primary.map.get_gps_bounds());
    let cam_zoom = ctx.canvas.cam_zoom;

    let mut wizard = wiz.wrap(ctx);
    let (_, mut s) = wizard.choose("Jump to which shortcut?", || {
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
        let mut timer = Timer::new("load shortcuts");
        for name in abstutil::list_all_objects(abstutil::SHORTCUTS, "") {
            let s: Shortcut =
                abstutil::read_json(&abstutil::path_shortcut(&name), &mut timer).unwrap();
            if ui
                .primary
                .map
                .get_boundary_polygon()
                .contains_pt(Pt2D::forcibly_from_gps(
                    s.center,
                    &ui.primary.map.get_gps_bounds(),
                ))
            {
                shortcuts.push(s);
            }
        }
        shortcuts
            .into_iter()
            .enumerate()
            .map(|(idx, s)| {
                if idx == 0 {
                    Choice::new(s.name.clone(), s)
                } else {
                    Choice::new(s.name.clone(), s).key(keys[idx - 1])
                }
            })
            .collect()
    })?;
    if s.name == "Create a new shortcut here" {
        // TODO Enforce non-empty, unique names
        let name = wizard.input_string("Name this shortcut")?;
        s.name = name;
        abstutil::write_json(&abstutil::path_shortcut(&s.name), &s).unwrap();
        wizard.abort();
        Some(Transition::Pop)
    } else {
        Some(Transition::ReplaceWithMode(
            Warping::new(
                ctx,
                Pt2D::forcibly_from_gps(s.center, &ui.primary.map.get_gps_bounds()),
                Some(s.cam_zoom),
                None,
                &mut ui.primary,
            ),
            EventLoopMode::Animation,
        ))
    }
}
