// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::ColorScheme;
//use cpuprofiler;
use ezgui::{Canvas, Color, EventLoopMode, GfxCtx, Text, UserInput, BOTTOM_LEFT, GUI};
use kml;
use map_model::{BuildingID, IntersectionID, LaneID, Map};
use objects::{Ctx, RenderingHints, ID, ROOT_MENU};
use piston::input::Key;
use plugins;
use plugins::edit_mode::EditMode;
use plugins::hider::Hider;
use plugins::layers::ToggleableLayers;
use plugins::time_travel::TimeTravel;
use plugins::Plugin;
use render::{DrawMap, RenderOptions};
use sim;
use sim::{GetDrawAgents, Sim, SimFlags, Tick};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::panic;
use std::process;

const MIN_ZOOM_FOR_MOUSEOVER: f64 = 4.0;

pub struct UI {
    primary: PerMapUI,
    primary_plugins: PluginsPerMap,
    // When running an A/B test, this is populated too.
    secondary: Option<(PerMapUI, PluginsPerMap)>,

    plugins: PluginsPerUI,

    // TODO describe An index into plugin_handlers.
    active_plugin: Option<usize>,

    canvas: Canvas,
    // TODO mutable ColorScheme to slurp up defaults is NOT ideal.
    cs: RefCell<ColorScheme>,

    // Remember this to support loading a new PerMapUI
    kml: Option<String>,
}

impl GUI<RenderingHints> for UI {
    fn event(&mut self, input: UserInput) -> (EventLoopMode, RenderingHints) {
        match panic::catch_unwind(panic::AssertUnwindSafe(|| self.inner_event(input))) {
            Ok(hints) => (hints.mode, hints),
            Err(err) => {
                error!("********************************************************************************");
                error!("UI broke! Primary sim:");
                self.primary.sim.dump_before_abort();
                if let Some((s, _)) = &self.secondary {
                    error!("Secondary sim:");
                    s.sim.dump_before_abort();
                }
                self.save_editor_state();
                panic::resume_unwind(err);
            }
        }
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, hints: RenderingHints) {
        g.clear(
            self.cs
                .borrow_mut()
                .get("map background", Color::rgb(242, 239, 233)),
        );

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bounds(),
            self.primary_plugins.hider(),
            &self.primary.map,
            self.get_draw_agent_source(),
            self.plugins.layers(),
            self,
        );
        for obj in statics.into_iter() {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id(), &hints),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.plugins.layers().debug_mode.is_enabled(),
            };
            obj.draw(
                g,
                opts,
                Ctx {
                    cs: &mut self.cs.borrow_mut(),
                    map: &self.primary.map,
                    draw_map: &self.primary.draw_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                    hints: &hints,
                },
            );
        }
        for obj in dynamics.into_iter() {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id(), &hints),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.plugins.layers().debug_mode.is_enabled(),
            };
            obj.draw(
                g,
                opts,
                Ctx {
                    cs: &mut self.cs.borrow_mut(),
                    map: &self.primary.map,
                    draw_map: &self.primary.draw_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                    hints: &hints,
                },
            );
        }

        if let Some(p) = self.get_active_plugin() {
            p.draw(
                g,
                Ctx {
                    cs: &mut self.cs.borrow_mut(),
                    map: &self.primary.map,
                    draw_map: &self.primary.draw_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                    hints: &hints,
                },
            );
        } else {
            // TODO Ew, this is a weird ambient plugin that doesn't consume input but might want to
            // draw stuff... only if another plugin isn't already active (aka, this is a hack to
            // turn this off when traffic signal editor is on.)
            self.primary_plugins.turn_cycler().draw(
                g,
                Ctx {
                    cs: &mut self.cs.borrow_mut(),
                    map: &self.primary.map,
                    draw_map: &self.primary.draw_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                    hints: &hints,
                },
            );
        }

        self.canvas.draw_text(g, hints.osd, BOTTOM_LEFT);
    }
}

// All of the state that's bound to a specific map+edit has to live here.
// TODO How can we arrange the code so that we statically know that we don't pass anything from UI
// to something in PerMapUI?
pub struct PerMapUI {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub recalculate_current_selection: bool,
    pub current_flags: SimFlags,
}

pub struct PluginsPerMap {
    // Anything that holds onto any kind of ID has to live here!
    list: Vec<Box<Plugin>>,
}

impl PluginsPerMap {
    fn hider(&self) -> &Hider {
        self.list[0].downcast_ref::<Hider>().unwrap()
    }

    fn show_owner(&self) -> &Box<Plugin> {
        &self.list[1]
    }

    fn turn_cycler(&self) -> &Box<Plugin> {
        &self.list[2]
    }

    fn time_travel(&self) -> &TimeTravel {
        self.list[3].downcast_ref::<TimeTravel>().unwrap()
    }
}

impl PerMapUI {
    pub fn new(flags: SimFlags, kml: &Option<String>) -> (PerMapUI, PluginsPerMap) {
        let mut timer = abstutil::Timer::new("setup PerMapUI");

        let (map, sim) = sim::load(flags.clone(), Some(Tick::from_seconds(30)), &mut timer);
        let extra_shapes: Vec<kml::ExtraShape> = if let Some(path) = kml {
            if path.ends_with(".kml") {
                kml::load(&path, &map.get_gps_bounds(), &mut timer)
                    .expect("Couldn't load extra KML shapes")
                    .shapes
            } else {
                let shapes: kml::ExtraShapes =
                    abstutil::read_binary(&path, &mut timer).expect("Couldn't load ExtraShapes");
                shapes.shapes
            }
        } else {
            Vec::new()
        };

        timer.start("draw_map");
        let draw_map = DrawMap::new(&map, extra_shapes, &mut timer);
        timer.stop("draw_map");

        let steepness_viz = plugins::steep::SteepnessVisualizer::new(&map);
        let neighborhood_summary =
            plugins::neighborhood_summary::NeighborhoodSummary::new(&map, &draw_map, &mut timer);

        timer.done();

        let state = PerMapUI {
            map,
            draw_map,
            sim,

            current_selection: None,
            recalculate_current_selection: false,
            current_flags: flags,
        };
        let plugins = PluginsPerMap {
            list: vec![
                Box::new(Hider::new()),
                Box::new(plugins::show_owner::ShowOwnerState::new()),
                Box::new(plugins::turn_cycler::TurnCyclerState::new()),
                Box::new(plugins::time_travel::TimeTravel::new()),
                Box::new(plugins::debug_objects::DebugObjectsState::new()),
                Box::new(plugins::follow::FollowState::new()),
                Box::new(plugins::show_route::ShowRouteState::new()),
                Box::new(plugins::show_activity::ShowActivityState::new()),
                Box::new(plugins::floodfill::Floodfiller::new()),
                Box::new(steepness_viz),
                Box::new(plugins::geom_validation::Validator::new()),
                Box::new(plugins::chokepoints::ChokepointsFinder::new()),
                Box::new(neighborhood_summary),
            ],
        };
        (state, plugins)
    }
}

// aka plugins that don't depend on map
struct PluginsPerUI {
    list: Vec<Box<Plugin>>,
}

impl PluginsPerUI {
    fn edit_mode(&self) -> &EditMode {
        self.list[0].downcast_ref::<EditMode>().unwrap()
    }

    fn layers(&self) -> &ToggleableLayers {
        self.list[1].downcast_ref::<ToggleableLayers>().unwrap()
    }

    fn layers_mut(&mut self) -> &mut ToggleableLayers {
        self.list[1].downcast_mut::<ToggleableLayers>().unwrap()
    }
}

impl UI {
    pub fn new(flags: SimFlags, kml: Option<String>) -> UI {
        // Do this first, so anything logged by sim::load isn't lost.
        let logs = plugins::logs::DisplayLogs::new();

        let (primary, primary_plugins) = PerMapUI::new(flags, &kml);
        let mut ui = UI {
            primary,
            primary_plugins,
            secondary: None,

            plugins: PluginsPerUI {
                list: vec![
                    Box::new(EditMode::new()),
                    Box::new(ToggleableLayers::new()),
                    Box::new(plugins::search::SearchState::new()),
                    Box::new(plugins::warp::WarpState::new()),
                    Box::new(plugins::classification::OsmClassifier::new()),
                    Box::new(logs),
                    Box::new(plugins::diff_all::DiffAllState::new()),
                    Box::new(plugins::diff_worlds::DiffWorldsState::new()),
                    Box::new(plugins::sim_controls::SimController::new()),
                ],
            },

            active_plugin: None,

            canvas: Canvas::new(),
            cs: RefCell::new(ColorScheme::load().unwrap()),

            kml,
        };

        match abstutil::read_json::<EditorState>("editor_state") {
            Ok(ref state) if ui.primary.map.get_name().to_string() == state.map_name => {
                info!("Loaded previous editor_state");
                ui.canvas.cam_x = state.cam_x;
                ui.canvas.cam_y = state.cam_y;
                ui.canvas.cam_zoom = state.cam_zoom;
            }
            _ => {
                warn!("Couldn't load editor_state or it's for a different map, so just focusing on an arbitrary building");
                // TODO window_size isn't set yet, so this actually kinda breaks
                let focus_pt = ID::Building(BuildingID(0))
                    .canonical_point(&ui.primary.map, &ui.primary.sim, &ui.primary.draw_map)
                    .or_else(|| {
                        ID::Lane(LaneID(0)).canonical_point(
                            &ui.primary.map,
                            &ui.primary.sim,
                            &ui.primary.draw_map,
                        )
                    }).expect("Can't get canonical_point of BuildingID(0) or Road(0)");
                ui.canvas.center_on_map_pt(focus_pt);
            }
        }

        ui.plugins
            .layers_mut()
            .handle_zoom(-1.0, ui.canvas.cam_zoom);

        ui
    }

    fn inner_event(&mut self, mut input: UserInput) -> RenderingHints {
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
        self.canvas.handle_event(&mut input);
        let new_zoom = self.canvas.cam_zoom;
        self.plugins.layers_mut().handle_zoom(old_zoom, new_zoom);

        // Always handle mouseover
        if old_zoom >= MIN_ZOOM_FOR_MOUSEOVER && new_zoom < MIN_ZOOM_FOR_MOUSEOVER {
            self.primary.current_selection = None;
        }
        if !self.canvas.is_dragging()
            && input.get_moved_mouse().is_some()
            && new_zoom >= MIN_ZOOM_FOR_MOUSEOVER
        {
            self.primary.current_selection = self.mouseover_something();
        }

        // TODO Normally we'd return InputOnly here if there was an active plugin, but actually, we
        // want some keys to always be pressable (sim controller stuff, quitting the game?)

        // If there's an active plugin, just run it.
        if let Some(idx) = self.active_plugin {
            if !self.run_plugin(idx, &mut input, &mut hints) {
                self.active_plugin = None;
            }
        } else {
            // Run each plugin, short-circuiting if the plugin claimed it was active.
            for idx in 0..self.plugins.list.len() + self.primary_plugins.list.len() {
                if self.run_plugin(idx, &mut input, &mut hints) {
                    self.active_plugin = Some(idx);
                    break;
                }
            }
        }

        if input.unimportant_key_pressed(Key::Escape, ROOT_MENU, "quit") {
            self.save_editor_state();
            self.cs.borrow().save();
            info!("Saved color_scheme");
            //cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
            process::exit(0);
        }

        if self.primary.recalculate_current_selection {
            self.primary.recalculate_current_selection = false;
            self.primary.current_selection = self.mouseover_something();
        }

        input.populate_osd(&mut hints.osd);

        hints
    }

    fn mouseover_something(&self) -> Option<ID> {
        let pt = self.canvas.get_cursor_in_map_space();

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bounds(),
            self.primary_plugins.hider(),
            &self.primary.map,
            self.get_draw_agent_source(),
            self.plugins.layers(),
            self,
        );
        // Check front-to-back
        for obj in dynamics.into_iter() {
            if obj.contains_pt(pt) {
                return Some(obj.get_id());
            }
        }
        for obj in statics.into_iter().rev() {
            if obj.contains_pt(pt) {
                return Some(obj.get_id());
            }
        }

        None
    }

    fn color_obj(&self, id: ID, hints: &RenderingHints) -> Option<Color> {
        if Some(id) == self.primary.current_selection {
            return Some(self.cs.borrow_mut().get("selected", Color::BLUE));
        }

        let ctx = Ctx {
            cs: &mut self.cs.borrow_mut(),
            map: &self.primary.map,
            draw_map: &self.primary.draw_map,
            canvas: &self.canvas,
            sim: &self.primary.sim,
            hints,
        };
        if let Some(p) = self.get_active_plugin() {
            return p.color_for(id, ctx);
        }

        // TODO Ew, this is a weird ambient plugin that doesn't consume input but has an opinion on
        // color.
        self.primary_plugins.show_owner().color_for(id, ctx)
    }

    fn get_active_plugin(&self) -> Option<&Box<Plugin>> {
        let idx = self.active_plugin?;
        let len = self.plugins.list.len();
        if idx < len {
            Some(&self.plugins.list[idx])
        } else {
            Some(&self.primary_plugins.list[idx - len])
        }
    }

    fn run_plugin(
        &mut self,
        idx: usize,
        input: &mut UserInput,
        hints: &mut RenderingHints,
    ) -> bool {
        let active = {
            let mut ctx = PluginCtx {
                primary: &mut self.primary,
                primary_plugins: None,
                secondary: &mut self.secondary,
                canvas: &mut self.canvas,
                cs: &mut self.cs.borrow_mut(),
                input,
                hints,
                kml: &self.kml,
            };
            let len = self.plugins.list.len();
            if idx < len {
                ctx.primary_plugins = Some(&mut self.primary_plugins);
                self.plugins.list[idx].event(ctx)
            } else {
                self.primary_plugins.list[idx - len].event(ctx)
            }
        };
        active
    }

    fn save_editor_state(&self) {
        let state = EditorState {
            map_name: self.primary.map.get_name().clone(),
            cam_x: self.canvas.cam_x,
            cam_y: self.canvas.cam_y,
            cam_zoom: self.canvas.cam_zoom,
        };
        // TODO maybe make state line up with the map, so loading from a new map doesn't break
        abstutil::write_json("editor_state", &state).expect("Saving editor_state failed");
        info!("Saved editor_state");
    }

    fn get_draw_agent_source(&self) -> Box<&GetDrawAgents> {
        let tt = self.primary_plugins.time_travel();
        if tt.is_active() {
            Box::new(tt)
        } else {
            Box::new(&self.primary.sim)
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub map_name: String,
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,
}

pub trait ShowTurnIcons {
    fn show_icons_for(&self, id: IntersectionID) -> bool;
}

impl ShowTurnIcons for UI {
    fn show_icons_for(&self, id: IntersectionID) -> bool {
        self.plugins.layers().show_all_turn_icons.is_enabled()
            || self.plugins.edit_mode().show_turn_icons(id)
            || {
                if let Some(ID::Turn(t)) = self.primary.current_selection {
                    t.parent == id
                } else {
                    false
                }
            }
    }
}

// This mirrors many, but not all, of the fields in UI.
pub struct PluginCtx<'a> {
    pub primary: &'a mut PerMapUI,
    // Only filled out for PluginsPerUI, not for PluginsPerMap.
    pub primary_plugins: Option<&'a mut PluginsPerMap>,
    pub secondary: &'a mut Option<(PerMapUI, PluginsPerMap)>,
    pub canvas: &'a mut Canvas,
    pub cs: &'a mut ColorScheme,
    pub input: &'a mut UserInput,
    pub hints: &'a mut RenderingHints,
    pub kml: &'a Option<String>,
}
