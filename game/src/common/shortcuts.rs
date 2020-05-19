use crate::app::App;
use crate::common::Warping;
use crate::game::{State, Transition, WizardState};
use abstutil::{Cloneable, Timer};
use ezgui::{Choice, EventCtx, Key, Wizard};
use geom::{LonLat, Pt2D};
use serde::{Deserialize, Serialize};

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

fn choose_shortcut(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let center = ctx
        .canvas
        .center_to_map_pt()
        .forcibly_to_gps(&app.primary.map.get_gps_bounds());
    let cam_zoom = ctx.canvas.cam_zoom;

    let mut wizard = wiz.wrap(ctx);
    let (_, mut s) = wizard.choose("Jump to which shortcut?", || {
        // TODO Allow deleting
        let mut shortcuts = vec![Shortcut {
            name: "Create a new shortcut here".to_string(),
            center,
            cam_zoom,
        }];
        let mut timer = Timer::new("load shortcuts");
        for name in abstutil::list_all_objects(abstutil::path_all_shortcuts()) {
            let s: Shortcut = abstutil::read_json(abstutil::path_shortcut(&name), &mut timer);
            if app
                .primary
                .map
                .get_boundary_polygon()
                .contains_pt(Pt2D::forcibly_from_gps(
                    s.center,
                    &app.primary.map.get_gps_bounds(),
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
                    // TODO Handle >9
                    Choice::new(s.name.clone(), s).key(Key::NUM_KEYS[idx - 1])
                }
            })
            .collect()
    })?;
    if s.name == "Create a new shortcut here" {
        // TODO Enforce non-empty, unique names
        let name = wizard.input_string("Name this shortcut")?;
        s.name = name;
        abstutil::write_json(abstutil::path_shortcut(&s.name), &s);
        wizard.abort();
        Some(Transition::Pop)
    } else {
        Some(Transition::Replace(Warping::new(
            ctx,
            Pt2D::forcibly_from_gps(s.center, &app.primary.map.get_gps_bounds()),
            Some(s.cam_zoom),
            None,
            &mut app.primary,
        )))
    }
}
