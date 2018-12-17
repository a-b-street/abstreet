use crate::colors::ColorScheme;
use abstutil;
//use cpuprofiler;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::render::RenderOptions;
use crate::state::UIState;
use ezgui::{
    Canvas, Color, EventLoopMode, Folder, GfxCtx, Key, Text, TopMenu, UserInput, BOTTOM_LEFT, GUI,
};
use kml;
use map_model::{BuildingID, LaneID};
use serde_derive::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::{HashMap, HashSet};
use std::process;

const MIN_ZOOM_FOR_MOUSEOVER: f64 = 4.0;

pub struct UI<S: UIState> {
    state: S,
    canvas: Canvas,
    cs: ColorScheme,
}

impl<S: UIState> GUI<RenderingHints> for UI<S> {
    fn top_menu(canvas: &Canvas) -> Option<TopMenu> {
        Some(TopMenu::new(
            vec![
                Folder::new(
                    "File",
                    vec![(Key::Comma, "show log console"), (Key::Escape, "quit")],
                ),
                Folder::new(
                    "Debug",
                    vec![
                        (Key::C, "find chokepoints"),
                        (Key::I, "validate map geometry"),
                        (Key::K, "unhide everything"),
                        (Key::Num1, "show/hide buildings"),
                        (Key::Num2, "show/hide intersections"),
                        (Key::Num3, "show/hide lanes"),
                        (Key::Num4, "show/hide parcels"),
                        (Key::Num5, "show/hide road steepness"),
                        (Key::Num6, "show OSM colors"),
                        (Key::Num7, "show/hide extra shapes"),
                        (Key::Num9, "show/hide all turn icons"),
                        (Key::G, "show/hide geometry debug mode"),
                    ],
                ),
                Folder::new(
                    "Edit",
                    vec![
                        (Key::B, "manage A/B tests"),
                        (Key::Num8, "configure colors"),
                        (Key::N, "manage neighborhoods"),
                        (Key::Q, "manage map edits"),
                        (Key::E, "edit roads"),
                        (Key::W, "manage scenarios"),
                    ],
                ),
                Folder::new(
                    "Sim",
                    vec![
                        (Key::LeftBracket, "slow down sim"),
                        (Key::RightBracket, "speed up sim"),
                        (Key::O, "save sim state"),
                        (Key::Y, "load previous sim state"),
                        (Key::U, "load next sim state"),
                        (Key::S, "seed the sim with agents"),
                        (Key::Space, "run/pause sim"),
                        (Key::M, "run one step of sim"),
                        (Key::Dot, "show sim info sidepanel"),
                        (Key::T, "start time traveling"),
                    ],
                ),
                Folder::new(
                    "View",
                    vec![
                        (Key::Z, "show neighborhood summaries"),
                        (Key::Slash, "search for something"),
                        (Key::A, "show lanes with active traffic"),
                        (Key::J, "warp to an object"),
                    ],
                ),
            ],
            canvas,
        ))
    }

    fn event(&mut self, input: &mut UserInput) -> (EventLoopMode, RenderingHints) {
        let mut hints = RenderingHints {
            mode: EventLoopMode::InputOnly,
            osd: Text::new(),
            suppress_intersection_icon: None,
            color_crosswalks: HashMap::new(),
            hide_crosswalks: HashSet::new(),
            hide_turn_icons: HashSet::new(),
        };

        // First update the camera and handle zoom
        let old_zoom = self.canvas.cam_zoom;
        self.canvas.handle_event(input);
        let new_zoom = self.canvas.cam_zoom;
        self.state.handle_zoom(old_zoom, new_zoom);

        // Always handle mouseover
        if old_zoom >= MIN_ZOOM_FOR_MOUSEOVER && new_zoom < MIN_ZOOM_FOR_MOUSEOVER {
            self.state.set_current_selection(None);
        }
        if !self.canvas.is_dragging()
            && input.get_moved_mouse().is_some()
            && new_zoom >= MIN_ZOOM_FOR_MOUSEOVER
        {
            self.state.set_current_selection(self.mouseover_something());
        }

        let mut recalculate_current_selection = false;
        self.state.event(
            input,
            &mut hints,
            &mut recalculate_current_selection,
            &mut self.cs,
            &mut self.canvas,
        );
        if recalculate_current_selection {
            self.state.set_current_selection(self.mouseover_something());
        }

        // Can do this at any time.
        if input.action_chosen("quit") {
            self.save_editor_state();
            self.cs.save();
            info!("Saved color_scheme");
            //cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
            process::exit(0);
        }

        input.populate_osd(&mut hints.osd);

        (hints.mode, hints)
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, hints: &RenderingHints) {
        g.clear(self.cs.get_def("map background", Color::rgb(242, 239, 233)));

        let ctx = Ctx {
            cs: &self.cs,
            map: &self.state.primary().map,
            draw_map: &self.state.primary().draw_map,
            canvas: &self.canvas,
            sim: &self.state.primary().sim,
            hints: &hints,
        };

        let (statics, dynamics) = self.state.get_objects_onscreen(&self.canvas);
        for obj in statics
            .into_iter()
            .chain(dynamics.iter().map(|obj| Box::new(obj.borrow())))
        {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id(), &ctx),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.state.is_debug_mode_enabled(),
            };
            obj.draw(g, opts, &ctx);
        }

        self.state.draw(g, &ctx);

        // Not happy about cloning, but probably will make the OSD a first-class ezgui concept
        // soon, so meh
        self.canvas.draw_text(g, hints.osd.clone(), BOTTOM_LEFT);
    }

    fn dump_before_abort(&self) {
        self.state.dump_before_abort();
        self.save_editor_state();
    }
}

impl<S: UIState> UI<S> {
    pub fn new(state: S, canvas: Canvas) -> UI<S> {
        let mut ui = UI {
            state,
            canvas,
            cs: ColorScheme::load().unwrap(),
        };

        match abstutil::read_json::<EditorState>("editor_state") {
            Ok(ref state) if ui.state.primary().map.get_name() == &state.map_name => {
                info!("Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
            }
            _ => {
                warn!("Couldn't load editor_state or it's for a different map, so just focusing on an arbitrary building");
                // TODO window_size isn't set yet, so this actually kinda breaks
                let focus_pt = ID::Building(BuildingID(0))
                    .canonical_point(
                        &ui.state.primary().map,
                        &ui.state.primary().sim,
                        &ui.state.primary().draw_map,
                    )
                    .or_else(|| {
                        ID::Lane(LaneID(0)).canonical_point(
                            &ui.state.primary().map,
                            &ui.state.primary().sim,
                            &ui.state.primary().draw_map,
                        )
                    })
                    .expect("Can't get canonical_point of BuildingID(0) or Road(0)");
                ui.canvas.center_on_map_pt(focus_pt);
            }
        }

        ui
    }

    fn mouseover_something(&self) -> Option<ID> {
        let pt = self.canvas.get_cursor_in_map_space();

        let (statics, dynamics) = self.state.get_objects_onscreen(&self.canvas);
        // Check front-to-back
        for obj in dynamics
            .iter()
            .map(|obj| Box::new(obj.borrow()))
            .chain(statics.into_iter().rev())
        {
            if obj.contains_pt(pt) {
                return Some(obj.get_id());
            }
        }

        None
    }

    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color> {
        self.state.color_obj(id, ctx)
    }

    fn save_editor_state(&self) {
        let state = EditorState {
            map_name: self.state.primary().map.get_name().clone(),
            cam_x: self.canvas.cam_x,
            cam_y: self.canvas.cam_y,
            cam_zoom: self.canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("editor_state", &state).expect("Saving editor_state failed");
        info!("Saved editor_state");
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub map_name: String,
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,
}
