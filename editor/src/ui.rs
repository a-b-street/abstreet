// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

// TODO this should just be a way to handle interactions between plugins

use abstutil;
use colors::ColorScheme;
use control::ControlMap;
//use cpuprofiler;
use ezgui::{Canvas, Color, EventLoopMode, GfxCtx, Text, UserInput, BOTTOM_LEFT, GUI};
use flame;
use kml;
use map_model::{IntersectionID, Map};
use objects::{Ctx, ID, ROOT_MENU};
use piston::input::Key;
use plugins;
use plugins::hider::Hider;
use plugins::layers::ToggleableLayers;
use plugins::sim_controls::SimController;
use plugins::stop_sign_editor::StopSignEditor;
use plugins::traffic_signal_editor::TrafficSignalEditor;
use plugins::Plugin;
use render::{DrawMap, RenderOptions};
use sim;
use sim::{Sim, SimFlags};
use std::cell::RefCell;
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

impl GUI for UI {
    fn event(&mut self, mut input: UserInput, osd: &mut Text) -> EventLoopMode {
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
            if !self.run_plugin(idx, &mut input, osd) {
                self.active_plugin = None;
            }
        } else {
            // Run each plugin, short-circuiting if the plugin claimed it was active.
            for idx in 0..self.plugins.list.len() + self.primary_plugins.list.len() {
                if self.run_plugin(idx, &mut input, osd) {
                    self.active_plugin = Some(idx);
                    break;
                }
            }
        }

        if input.unimportant_key_pressed(Key::Escape, ROOT_MENU, "quit") {
            let state = EditorState {
                map_name: self.primary.map.get_name().clone(),
                cam_x: self.canvas.cam_x,
                cam_y: self.canvas.cam_y,
                cam_zoom: self.canvas.cam_zoom,
            };
            // TODO maybe make state line up with the map, so loading from a new map doesn't break
            abstutil::write_json("editor_state", &state).expect("Saving editor_state failed");
            self.cs.borrow().save();
            info!("Saved editor_state and color_scheme");
            //cpuprofiler::PROFILER.lock().unwrap().stop().unwrap();
            process::exit(0);
        }

        // Sim controller plugin is kind of always active? If nothing else ran, let it use keys.
        let result = self.plugins.sim_ctrl.event(
            &mut input,
            &mut self.primary,
            &mut self.primary_plugins,
            &mut self.secondary,
            osd,
        );

        if self.primary.recalculate_current_selection {
            self.primary.recalculate_current_selection = false;
            self.primary.current_selection = self.mouseover_something();
        }

        input.populate_osd(osd);
        result
    }

    fn get_mut_canvas(&mut self) -> &mut Canvas {
        &mut self.canvas
    }

    fn draw(&self, g: &mut GfxCtx, osd: Text) {
        g.clear(self.cs.borrow_mut().get("map background", Color::WHITE));

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            self.primary_plugins.hider(),
            &self.primary.map,
            &self.primary.sim,
            self.plugins.layers(),
            self,
        );
        for obj in statics.into_iter() {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id()),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.plugins.layers().debug_mode.is_enabled(),
            };
            obj.draw(
                g,
                opts,
                Ctx {
                    cs: &mut self.cs.borrow_mut(),
                    map: &self.primary.map,
                    control_map: &self.primary.control_map,
                    draw_map: &self.primary.draw_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                },
            );
        }
        for obj in dynamics.into_iter() {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id()),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.plugins.layers().debug_mode.is_enabled(),
            };
            obj.draw(
                g,
                opts,
                Ctx {
                    cs: &mut self.cs.borrow_mut(),
                    map: &self.primary.map,
                    control_map: &self.primary.control_map,
                    draw_map: &self.primary.draw_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                },
            );
        }

        if let Some(p) = self.get_active_plugin() {
            p.draw(
                g,
                Ctx {
                    cs: &mut self.cs.borrow_mut(),
                    map: &self.primary.map,
                    control_map: &self.primary.control_map,
                    draw_map: &self.primary.draw_map,
                    canvas: &self.canvas,
                    sim: &self.primary.sim,
                },
            );
        }
        self.plugins.sim_ctrl.draw(g, &self.canvas);

        self.canvas.draw_text(g, osd, BOTTOM_LEFT);
    }
}

// All of the state that's bound to a specific map+edit has to live here.
// TODO How can we arrange the code so that we statically know that we don't pass anything from UI
// to something in PerMapUI?
pub struct PerMapUI {
    pub map: Map,
    pub draw_map: DrawMap,
    pub control_map: ControlMap,
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

    fn stop_sign_editor(&self) -> &StopSignEditor {
        self.list[2].downcast_ref::<StopSignEditor>().unwrap()
    }

    fn traffic_signal_editor(&self) -> &TrafficSignalEditor {
        self.list[3].downcast_ref::<TrafficSignalEditor>().unwrap()
    }
}

impl PerMapUI {
    pub fn new(
        flags: SimFlags,
        kml: &Option<String>,
        cs: &mut ColorScheme,
    ) -> (PerMapUI, PluginsPerMap) {
        flame::start("setup");
        let (map, control_map, sim) = sim::load(flags.clone(), Some(sim::Tick::from_seconds(30)));
        let extra_shapes = if let Some(path) = kml {
            kml::load(&path, &map.get_gps_bounds()).expect("Couldn't load extra KML shapes")
        } else {
            Vec::new()
        };

        flame::start("draw_map");
        let draw_map = DrawMap::new(&map, &control_map, extra_shapes, cs);
        flame::end("draw_map");

        flame::end("setup");
        flame::dump_stdout();

        let steepness_viz = plugins::steep::SteepnessVisualizer::new(&map);
        let neighborhood_summary = plugins::neighborhood_summary::NeighborhoodSummary::new(&map);

        let state = PerMapUI {
            map,
            draw_map,
            control_map,
            sim,

            current_selection: None,
            recalculate_current_selection: false,
            current_flags: flags,
        };
        let plugins = PluginsPerMap {
            list: vec![
                Box::new(Hider::new()),
                Box::new(plugins::show_owner::ShowOwnerState::new()),
                Box::new(StopSignEditor::new()),
                Box::new(TrafficSignalEditor::new()),
                Box::new(plugins::debug_objects::DebugObjectsState::new()),
                Box::new(plugins::follow::FollowState::Empty),
                Box::new(plugins::show_route::ShowRouteState::Empty),
                Box::new(plugins::floodfill::Floodfiller::new()),
                Box::new(steepness_viz),
                Box::new(plugins::geom_validation::Validator::new()),
                Box::new(plugins::turn_cycler::TurnCyclerState::new()),
                Box::new(plugins::draw_neighborhoods::DrawNeighborhoodState::new()),
                Box::new(plugins::scenarios::ScenarioManager::new()),
                Box::new(plugins::map_edits::EditsManager::new()),
                Box::new(plugins::chokepoints::ChokepointsFinder::new()),
                Box::new(neighborhood_summary),
            ],
        };
        (state, plugins)
    }
}

// aka plugins that don't depend on map
struct PluginsPerUI {
    sim_ctrl: SimController,
    list: Vec<Box<Plugin>>,
}

impl PluginsPerUI {
    fn layers(&self) -> &ToggleableLayers {
        self.list[0].downcast_ref::<ToggleableLayers>().unwrap()
    }

    fn layers_mut(&mut self) -> &mut ToggleableLayers {
        self.list[0].downcast_mut::<ToggleableLayers>().unwrap()
    }
}

impl UI {
    pub fn new(flags: SimFlags, kml: Option<String>) -> UI {
        // Do this first, so anything logged by sim::load isn't lost.
        let logs = plugins::logs::DisplayLogs::new();

        let mut cs = ColorScheme::load().unwrap();
        let (primary, primary_plugins) = PerMapUI::new(flags, &kml, &mut cs);
        let mut ui = UI {
            primary,
            primary_plugins,
            secondary: None,

            plugins: PluginsPerUI {
                sim_ctrl: SimController::new(),
                list: vec![
                    Box::new(ToggleableLayers::new()),
                    Box::new(plugins::search::SearchState::Empty),
                    Box::new(plugins::warp::WarpState::Empty),
                    Box::new(plugins::classification::OsmClassifier::new()),
                    Box::new(plugins::color_picker::ColorPicker::new()),
                    Box::new(plugins::a_b_tests::ABTestManager::new()),
                    Box::new(logs),
                    Box::new(plugins::diff_all::DiffAllState::new()),
                    Box::new(plugins::diff_worlds::DiffWorldsState::new()),
                    Box::new(plugins::road_editor::RoadEditor::new()),
                ],
            },

            active_plugin: None,

            canvas: Canvas::new(),
            cs: RefCell::new(cs),

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
                warn!("Couldn't load editor_state or it's for a different map, so just centering initial view");
                ui.canvas.center_on_map_pt(ui.primary.draw_map.center_pt);
            }
        }

        ui.plugins
            .layers_mut()
            .handle_zoom(-1.0, ui.canvas.cam_zoom);

        ui
    }

    fn mouseover_something(&self) -> Option<ID> {
        let pt = self.canvas.get_cursor_in_map_space();

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bbox(),
            self.primary_plugins.hider(),
            &self.primary.map,
            &self.primary.sim,
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

    fn color_obj(&self, id: ID) -> Option<Color> {
        if Some(id) == self.primary.current_selection {
            return Some(self.cs.borrow_mut().get("selected", Color::BLUE));
        }

        let ctx = Ctx {
            cs: &mut self.cs.borrow_mut(),
            map: &self.primary.map,
            control_map: &self.primary.control_map,
            draw_map: &self.primary.draw_map,
            canvas: &self.canvas,
            sim: &self.primary.sim,
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

    fn run_plugin(&mut self, idx: usize, input: &mut UserInput, osd: &mut Text) -> bool {
        let mut new_primary_plugins: Option<PluginsPerMap> = None;
        let active = {
            let ctx = PluginCtx {
                primary: &mut self.primary,
                secondary: &mut self.secondary,
                canvas: &mut self.canvas,
                cs: &mut self.cs.borrow_mut(),
                input,
                osd,
                kml: &self.kml,
                new_primary_plugins: &mut new_primary_plugins,
            };
            let len = self.plugins.list.len();
            if idx < len {
                self.plugins.list[idx].event(ctx)
            } else {
                self.primary_plugins.list[idx - len].event(ctx)
            }
        };
        if let Some(new_plugins) = new_primary_plugins {
            self.primary_plugins = new_plugins;
        }
        active
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
            || self.primary_plugins.stop_sign_editor().show_turn_icons(id)
            || self
                .primary_plugins
                .traffic_signal_editor()
                .show_turn_icons(id)
    }
}

// This mirrors many, but not all, of the fields in UI.
pub struct PluginCtx<'a> {
    pub primary: &'a mut PerMapUI,
    pub secondary: &'a mut Option<(PerMapUI, PluginsPerMap)>,
    pub canvas: &'a mut Canvas,
    pub cs: &'a mut ColorScheme,
    pub input: &'a mut UserInput,
    pub osd: &'a mut Text,
    pub kml: &'a Option<String>,

    // Unfortunately we have to use an output parameter here, but it's pretty isolated to
    // run_plugin
    pub new_primary_plugins: &'a mut Option<PluginsPerMap>,
}
