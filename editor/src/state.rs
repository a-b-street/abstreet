use crate::colors::ColorScheme;
use crate::objects::{Ctx, RenderingHints, ID};
use crate::plugins;
use crate::plugins::debug;
use crate::plugins::edit;
use crate::plugins::view;
use crate::plugins::{Plugin, PluginCtx};
use crate::render::{DrawMap, Renderable};
use abstutil::Timer;
use ezgui::{Canvas, Color, GfxCtx, UserInput};
use map_model::{IntersectionID, Map};
use sim::{GetDrawAgents, Sim, SimFlags, Tick};

pub trait UIState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64);
    fn set_current_selection(&mut self, obj: Option<ID>);
    fn is_current_selection(&self, obj: ID) -> bool;
    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    );
    fn get_objects_onscreen(
        &self,
        canvas: &Canvas,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>);
    fn is_debug_mode_enabled(&self) -> bool;
    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx);
    fn dump_before_abort(&self);
    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color>;
    fn primary(&self) -> &PerMapUI;
}

pub struct DefaultUIState {
    pub primary: PerMapUI,
    primary_plugins: PluginsPerMap,
    // When running an A/B test, this is populated too.
    secondary: Option<(PerMapUI, PluginsPerMap)>,

    // These are all mutually exclusive and, if present, override everything else.
    exclusive_blocking_plugin: Option<Box<Plugin>>,
    // These are all mutually exclusive, but don't override other stuff.
    exclusive_nonblocking_plugin: Option<Box<Plugin>>,

    // These are stackable modal plugins. They can all coexist, and they don't block other modal
    // plugins or ambient plugins.
    show_score: Option<plugins::sim::show_score::ShowScoreState>,

    // Ambient plugins always exist, and they never block anything.
    pub sim_controls: plugins::sim::controls::SimControls,
    layers: debug::layers::ToggleableLayers,
}

impl DefaultUIState {
    pub fn new(flags: SimFlags, kml: Option<String>, canvas: &Canvas) -> DefaultUIState {
        // Do this first to trigger the log console initialization, so anything logged by sim::load
        // isn't lost.
        view::logs::DisplayLogs::initialize();
        let (primary, primary_plugins) = PerMapUI::new(flags, kml);
        let mut state = DefaultUIState {
            primary,
            primary_plugins,
            secondary: None,
            exclusive_blocking_plugin: None,
            exclusive_nonblocking_plugin: None,
            show_score: None,
            sim_controls: plugins::sim::controls::SimControls::new(),
            layers: debug::layers::ToggleableLayers::new(),
        };
        state.layers.handle_zoom(-1.0, canvas.cam_zoom);
        state
    }
}

impl UIState for DefaultUIState {
    fn handle_zoom(&mut self, old_zoom: f64, new_zoom: f64) {
        self.layers.handle_zoom(old_zoom, new_zoom);
    }

    fn set_current_selection(&mut self, obj: Option<ID>) {
        self.primary.current_selection = obj;
    }

    fn is_current_selection(&self, obj: ID) -> bool {
        self.primary.current_selection == Some(obj)
    }

    fn event(
        &mut self,
        input: &mut UserInput,
        hints: &mut RenderingHints,
        recalculate_current_selection: &mut bool,
        cs: &mut ColorScheme,
        canvas: &mut Canvas,
    ) {
        // Exclusive blocking plugins first
        {
            let mut ctx = PluginCtx {
                primary: &mut self.primary,
                primary_plugins: None,
                secondary: &mut self.secondary,
                canvas,
                cs,
                input,
                hints,
                recalculate_current_selection,
            };

            // Special cases of weird blocking exclusive plugins!
            if self
                .primary_plugins
                .search
                .as_ref()
                .map(|p| p.is_blocking())
                .unwrap_or(false)
            {
                if !self
                    .primary_plugins
                    .search
                    .as_mut()
                    .unwrap()
                    .blocking_event(&mut ctx)
                {
                    self.primary_plugins.search = None;
                }
                return;
            }
            // Always run this here, to let it scrape sim state.
            self.primary_plugins.time_travel.event(&mut ctx);
            if self.primary_plugins.time_travel.is_active() {
                return;
            }

            ctx.primary_plugins = Some(&mut self.primary_plugins);
            if self.exclusive_blocking_plugin.is_some() {
                if !self
                    .exclusive_blocking_plugin
                    .as_mut()
                    .unwrap()
                    .blocking_event(&mut ctx)
                {
                    self.exclusive_blocking_plugin = None;
                }
                return;
            }

            if let Some(p) = view::logs::DisplayLogs::new(&mut ctx) {
                self.exclusive_blocking_plugin = Some(Box::new(p));
            } else if let Some(p) = view::search::SearchState::new(&mut ctx) {
                self.primary_plugins.search = Some(p);
            } else if let Some(p) = debug::chokepoints::ChokepointsFinder::new(&mut ctx) {
                self.exclusive_blocking_plugin = Some(Box::new(p));
            } else if let Some(p) = debug::classification::OsmClassifier::new(&mut ctx) {
                self.exclusive_blocking_plugin = Some(Box::new(p));
            } else if let Some(p) = debug::floodfill::Floodfiller::new(&mut ctx) {
                self.exclusive_blocking_plugin = Some(Box::new(p));
            } else if let Some(p) = debug::geom_validation::Validator::new(&mut ctx) {
                self.exclusive_blocking_plugin = Some(Box::new(p));
            } else if let Some(p) = view::warp::WarpState::new(&mut ctx) {
                self.exclusive_blocking_plugin = Some(Box::new(p));
            } else if ctx.secondary.is_none() {
                if let Some(p) = edit::a_b_tests::ABTestManager::new(&mut ctx) {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                } else if let Some(p) = edit::color_picker::ColorPicker::new(&mut ctx) {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                } else if let Some(p) =
                    edit::draw_neighborhoods::DrawNeighborhoodState::new(&mut ctx)
                {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                } else if let Some(p) = edit::map_edits::EditsManager::new(&mut ctx) {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                } else if let Some(p) = edit::road_editor::RoadEditor::new(&mut ctx) {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                } else if let Some(p) = edit::scenarios::ScenarioManager::new(&mut ctx) {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                } else if let Some(p) = edit::stop_sign_editor::StopSignEditor::new(&mut ctx) {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                } else if let Some(p) =
                    edit::traffic_signal_editor::TrafficSignalEditor::new(&mut ctx)
                {
                    self.exclusive_blocking_plugin = Some(Box::new(p));
                }
            }
            if self
                .primary_plugins
                .search
                .as_ref()
                .map(|p| p.is_blocking())
                .unwrap_or(false)
                || self.exclusive_blocking_plugin.is_some()
            {
                return;
            }
        }

        // Exclusive nonblocking plugins
        {
            let mut ctx = PluginCtx {
                primary: &mut self.primary,
                primary_plugins: None,
                secondary: &mut self.secondary,
                canvas,
                cs,
                input,
                hints,
                recalculate_current_selection,
            };

            if self.exclusive_nonblocking_plugin.is_some() {
                if !self
                    .exclusive_nonblocking_plugin
                    .as_mut()
                    .unwrap()
                    .blocking_event(&mut ctx)
                {
                    self.exclusive_nonblocking_plugin = None;
                }
            } else if ctx.secondary.is_some() {
                // TODO This is per UI, so it's never reloaded. Make sure to detect new loads, even
                // when the initial time is 0? But we probably have no state then, so...
                if let Some(p) = plugins::sim::diff_all::DiffAllState::new(&mut ctx) {
                    self.exclusive_nonblocking_plugin = Some(Box::new(p));
                } else if let Some(p) = plugins::sim::diff_trip::DiffTripState::new(&mut ctx) {
                    self.exclusive_nonblocking_plugin = Some(Box::new(p));
                }
            }
        }

        // Stackable modal plugins
        let mut ctx = PluginCtx {
            primary: &mut self.primary,
            primary_plugins: None,
            secondary: &mut self.secondary,
            canvas,
            cs,
            input,
            hints,
            recalculate_current_selection,
        };

        if self.show_score.is_some() {
            if !self
                .show_score
                .as_mut()
                .unwrap()
                .nonblocking_event(&mut ctx)
            {
                self.show_score = None;
            }
        } else if let Some(p) = plugins::sim::show_score::ShowScoreState::new(&mut ctx) {
            self.show_score = Some(p);
        }
        if self
            .primary_plugins
            .search
            .as_ref()
            .map(|p| !p.is_blocking())
            .unwrap_or(false)
        {
            if !self
                .primary_plugins
                .search
                .as_mut()
                .unwrap()
                .blocking_event(&mut ctx)
            {
                self.primary_plugins.search = None;
            }
            return;
        }

        if self.primary_plugins.hider.is_some() {
            if !self
                .primary_plugins
                .hider
                .as_mut()
                .unwrap()
                .nonblocking_event(&mut ctx)
            {
                self.primary_plugins.hider = None;
            }
        } else if let Some(p) = debug::hider::Hider::new(&mut ctx) {
            self.primary_plugins.hider = Some(p);
        }

        // Ambient plugins. Do the primary_plugins group first, since we later borrow the plugins
        // for sim_controls.
        for p in self.primary_plugins.ambient_plugins.iter_mut() {
            p.ambient_event(&mut ctx);
        }
        {
            ctx.primary_plugins = Some(&mut self.primary_plugins);
            self.sim_controls.ambient_event(&mut ctx);
        }
        self.layers.ambient_event(&mut ctx);
    }

    fn get_objects_onscreen(
        &self,
        canvas: &Canvas,
    ) -> (Vec<Box<&Renderable>>, Vec<Box<Renderable>>) {
        let draw_agent_source: &GetDrawAgents = {
            let tt = &self.primary_plugins.time_travel;
            if tt.is_active() {
                tt
            } else {
                &self.primary.sim
            }
        };

        self.primary.draw_map.get_objects_onscreen(
            canvas.get_screen_bounds(),
            &self.primary.map,
            draw_agent_source,
            self,
        )
    }

    fn is_debug_mode_enabled(&self) -> bool {
        self.layers.debug_mode.is_enabled()
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        if let Some(ref plugin) = self.primary_plugins.search {
            plugin.draw(g, ctx);
            if plugin.is_blocking() {
                return;
            }
        }
        if let Some(ref plugin) = self.exclusive_blocking_plugin {
            plugin.draw(g, ctx);
            return;
        }

        if let Some(ref plugin) = self.exclusive_nonblocking_plugin {
            plugin.draw(g, ctx);
        }

        // Stackable modals
        if let Some(ref p) = self.show_score {
            p.draw(g, ctx);
        }
        // Hider doesn't draw

        // Ambient
        self.sim_controls.draw(g, ctx);
        // Layers doesn't draw
        for p in &self.primary_plugins.ambient_plugins {
            p.draw(g, ctx);
        }
    }

    fn dump_before_abort(&self) {
        error!("********************************************************************************");
        error!("UI broke! Primary sim:");
        self.primary.sim.dump_before_abort();
        if let Some((s, _)) = &self.secondary {
            error!("Secondary sim:");
            s.sim.dump_before_abort();
        }
    }

    fn color_obj(&self, id: ID, ctx: &Ctx) -> Option<Color> {
        match id {
            ID::Turn(_) => {}
            _ => {
                if Some(id) == self.primary.current_selection {
                    return Some(ctx.cs.get_def("selected", Color::BLUE));
                }
            }
        };

        if let Some(ref plugin) = self.primary_plugins.search {
            if let Some(c) = plugin.color_for(id, ctx) {
                return Some(c);
            }
        }
        if let Some(ref plugin) = self.exclusive_blocking_plugin {
            return plugin.color_for(id, ctx);
        }

        // The exclusive_nonblocking_plugins don't color_obj.

        // show_score, hider, sim_controls, and layers don't color_obj.
        for p in &self.primary_plugins.ambient_plugins {
            if let Some(c) = p.color_for(id, ctx) {
                return Some(c);
            }
        }

        None
    }

    fn primary(&self) -> &PerMapUI {
        &self.primary
    }
}

pub trait ShowObjects {
    fn show_icons_for(&self, id: IntersectionID) -> bool;
    fn show(&self, obj: ID) -> bool;
}

impl ShowObjects for DefaultUIState {
    fn show_icons_for(&self, id: IntersectionID) -> bool {
        if let Some(ref plugin) = self.exclusive_blocking_plugin {
            if let Ok(p) = plugin.downcast_ref::<edit::stop_sign_editor::StopSignEditor>() {
                return p.show_turn_icons(id);
            }
            if let Ok(p) = plugin.downcast_ref::<edit::traffic_signal_editor::TrafficSignalEditor>()
            {
                return p.show_turn_icons(id);
            }
        }

        self.layers.show_all_turn_icons.is_enabled() || {
            // TODO This sounds like some old hack, probably remove this?
            if let Some(ID::Turn(t)) = self.primary.current_selection {
                t.parent == id
            } else {
                false
            }
        }
    }

    fn show(&self, obj: ID) -> bool {
        if let Some(ref p) = self.primary_plugins.hider {
            if !p.show(obj) {
                return false;
            }
        }
        self.layers.show(obj)
    }
}

// All of the state that's bound to a specific map+edit has to live here.
pub struct PerMapUI {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub current_flags: SimFlags,
}

impl PerMapUI {
    pub fn new(flags: SimFlags, kml: Option<String>) -> (PerMapUI, PluginsPerMap) {
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

        let state = PerMapUI {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags,
        };
        let plugins = PluginsPerMap::new(&state, &mut timer);
        timer.done();
        (state, plugins)
    }
}

// Anything that holds onto any kind of ID has to live here!
pub struct PluginsPerMap {
    // These are stackable modal plugins. They can all coexist, and they don't block other modal
    // plugins or ambient plugins.
    hider: Option<debug::hider::Hider>,

    // When present, this either acts like exclusive blocking or like stackable modal. :\
    search: Option<view::search::SearchState>,

    // This acts like exclusive blocking when active.
    time_travel: plugins::sim::time_travel::TimeTravel,

    ambient_plugins: Vec<Box<Plugin>>,
}

impl PluginsPerMap {
    pub fn new(state: &PerMapUI, timer: &mut Timer) -> PluginsPerMap {
        PluginsPerMap {
            hider: None,
            search: None,
            ambient_plugins: vec![
                Box::new(view::debug_objects::DebugObjectsState::new()),
                Box::new(view::follow::FollowState::new()),
                Box::new(view::neighborhood_summary::NeighborhoodSummary::new(
                    &state.map,
                    &state.draw_map,
                    timer,
                )),
                // TODO Could be a little simpler to instantiate this lazily, stop representing
                // inactive state.
                Box::new(view::show_activity::ShowActivityState::new()),
                Box::new(view::show_associated::ShowAssociatedState::new()),
                Box::new(view::show_route::ShowRouteState::new()),
                Box::new(view::turn_cycler::TurnCyclerState::new()),
            ],
            time_travel: plugins::sim::time_travel::TimeTravel::new(),
        }
    }
}
