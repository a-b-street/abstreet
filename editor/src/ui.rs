use crate::colors::ColorScheme;
use abstutil;
//use cpuprofiler;
use crate::objects::{Ctx, RenderingHints, ID, ROOT_MENU};
use crate::plugins;
use crate::plugins::debug::layers::ToggleableLayers;
use crate::plugins::debug::DebugMode;
use crate::plugins::edit::EditMode;
use crate::plugins::time_travel::TimeTravel;
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{DrawMap, RenderOptions};
use ezgui::{Canvas, Color, EventLoopMode, GfxCtx, Text, UserInput, BOTTOM_LEFT, GUI};
use kml;
use map_model::{BuildingID, IntersectionID, LaneID, Map};
use piston::input::Key;
use serde_derive::{Deserialize, Serialize};
use sim;
use sim::{GetDrawAgents, Sim, SimFlags, Tick};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::process;

const MIN_ZOOM_FOR_MOUSEOVER: f64 = 4.0;

pub struct UI {
    primary: PerMapUI,
    primary_plugins: PluginsPerMap,
    // When running an A/B test, this is populated too.
    secondary: Option<(PerMapUI, PluginsPerMap)>,

    plugins: PluginsPerUI,
    active_plugin: Option<usize>,

    canvas: Canvas,
    // TODO mutable ColorScheme to slurp up defaults is NOT ideal.
    cs: RefCell<ColorScheme>,

    // Remember this to support loading a new PerMapUI
    kml: Option<String>,
}

impl GUI<RenderingHints> for UI {
    fn event(&mut self, mut input: UserInput) -> (EventLoopMode, RenderingHints) {
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
        self.primary_plugins
            .layers_mut()
            .handle_zoom(old_zoom, new_zoom);

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

        // Can do this at any time.
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

        (hints.mode, hints)
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

        let mut ctx = Ctx {
            cs: &mut self.cs.borrow_mut(),
            map: &self.primary.map,
            draw_map: &self.primary.draw_map,
            canvas: &self.canvas,
            sim: &self.primary.sim,
            hints: &hints,
        };

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bounds(),
            self.primary_plugins.debug_mode(),
            &self.primary.map,
            self.get_draw_agent_source(),
            self,
        );
        for obj in statics
            .into_iter()
            .chain(dynamics.iter().map(|obj| Box::new(obj.borrow())))
        {
            let opts = RenderOptions {
                color: self.color_obj(obj.get_id(), &mut ctx),
                cam_zoom: self.canvas.cam_zoom,
                debug_mode: self.primary_plugins.layers().debug_mode.is_enabled(),
            };
            obj.draw(g, opts, &mut ctx);
        }

        if let Some(p) = self.get_active_plugin() {
            p.draw(g, &mut ctx);
        } else {
            // If no other mode was active, give the ambient plugins in ViewMode and SimMode a
            // chance.
            self.primary_plugins.view_mode().draw(g, &mut ctx);
            self.plugins.sim_mode().draw(g, &mut ctx);
        }

        self.canvas.draw_text(g, hints.osd, BOTTOM_LEFT);
    }

    fn dump_before_abort(&self) {
        error!("********************************************************************************");
        error!("UI broke! Primary sim:");
        self.primary.sim.dump_before_abort();
        if let Some((s, _)) = &self.secondary {
            error!("Secondary sim:");
            s.sim.dump_before_abort();
        }
        self.save_editor_state();
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
    fn debug_mode(&self) -> &DebugMode {
        self.list[0].downcast_ref::<DebugMode>().unwrap()
    }

    fn view_mode(&self) -> &Box<Plugin> {
        &self.list[1]
    }

    fn time_travel(&self) -> &TimeTravel {
        self.list[2].downcast_ref::<TimeTravel>().unwrap()
    }

    fn layers(&self) -> &ToggleableLayers {
        &self.list[0].downcast_ref::<DebugMode>().unwrap().layers
    }

    fn layers_mut(&mut self) -> &mut ToggleableLayers {
        &mut self.list[0].downcast_mut::<DebugMode>().unwrap().layers
    }
}

impl PerMapUI {
    pub fn new(
        flags: SimFlags,
        kml: &Option<String>,
        canvas: &Canvas,
    ) -> (PerMapUI, PluginsPerMap) {
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

        let debug_mode = DebugMode::new(&map);
        let view_mode = plugins::view::ViewMode::new(&map, &draw_map, &mut timer);

        timer.done();

        let state = PerMapUI {
            map,
            draw_map,
            sim,

            current_selection: None,
            recalculate_current_selection: false,
            current_flags: flags,
        };
        let mut plugins = PluginsPerMap {
            list: vec![
                Box::new(debug_mode),
                Box::new(view_mode),
                Box::new(plugins::time_travel::TimeTravel::new()),
            ],
        };
        plugins.layers_mut().handle_zoom(-1.0, canvas.cam_zoom);

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

    fn sim_mode(&self) -> &Box<Plugin> {
        &self.list[1]
    }
}

impl UI {
    pub fn new(flags: SimFlags, kml: Option<String>) -> UI {
        // Do this first, so anything logged by sim::load isn't lost.
        let logs = plugins::logs::DisplayLogs::new();

        let canvas = Canvas::new();
        let (primary, primary_plugins) = PerMapUI::new(flags, &kml, &canvas);
        let mut ui = UI {
            primary,
            primary_plugins,
            secondary: None,

            plugins: PluginsPerUI {
                list: vec![
                    Box::new(EditMode::new()),
                    Box::new(plugins::sim::SimMode::new()),
                    Box::new(logs),
                ],
            },

            active_plugin: None,

            canvas,
            cs: RefCell::new(ColorScheme::load().unwrap()),

            kml,
        };
        // TODO Hacktastic way of sneaking this in!
        if ui.primary.current_flags.load == "../data/raw_maps/ban_left_turn.abst".to_string() {
            ui.plugins
                .list
                .push(Box::new(plugins::tutorial::TutorialMode::new()));
        }

        match abstutil::read_json::<EditorState>("editor_state") {
            Ok(ref state) if ui.primary.map.get_name() == &state.map_name => {
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
                    })
                    .expect("Can't get canonical_point of BuildingID(0) or Road(0)");
                ui.canvas.center_on_map_pt(focus_pt);
            }
        }

        ui
    }

    fn mouseover_something(&self) -> Option<ID> {
        let pt = self.canvas.get_cursor_in_map_space();

        let (statics, dynamics) = self.primary.draw_map.get_objects_onscreen(
            self.canvas.get_screen_bounds(),
            self.primary_plugins.debug_mode(),
            &self.primary.map,
            self.get_draw_agent_source(),
            self,
        );
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

    fn color_obj(&self, id: ID, ctx: &mut Ctx) -> Option<Color> {
        if Some(id) == self.primary.current_selection {
            return Some(ctx.cs.get("selected", Color::BLUE));
        }

        if let Some(p) = self.get_active_plugin() {
            p.color_for(id, ctx)
        } else {
            // If no other mode was active, give the ambient plugins in ViewMode a chance.
            self.primary_plugins.view_mode().color_for(id, ctx)
        }
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
            self.plugins.list[idx].blocking_event(&mut ctx)
        } else {
            self.primary_plugins.list[idx - len].blocking_event(&mut ctx)
        }
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

    fn get_draw_agent_source(&self) -> &GetDrawAgents {
        let tt = self.primary_plugins.time_travel();
        if tt.is_active() {
            tt
        } else {
            &self.primary.sim
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
        self.primary_plugins
            .layers()
            .show_all_turn_icons
            .is_enabled()
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
