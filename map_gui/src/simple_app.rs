use structopt::StructOpt;

use abstio::MapName;
use abstutil::Timer;
use geom::{Circle, Distance, Duration, Pt2D, Time};
use map_model::{IntersectionID, Map};
use sim::Sim;
use widgetry::tools::URLManager;
use widgetry::{Canvas, EventCtx, GfxCtx, Settings, SharedAppState, State, Transition, Warper};

use crate::colors::{ColorScheme, ColorSchemeChoice};
use crate::load::MapLoader;
use crate::options::Options;
use crate::render::DrawMap;
use crate::render::{DrawOptions, Renderable};
use crate::tools::CameraState;
use crate::{AppLike, ID};

/// Simple app state that just renders a static map, without any dynamic agents on the map.
pub struct SimpleApp<T> {
    pub map: Map,
    pub draw_map: DrawMap,
    pub cs: ColorScheme,
    pub opts: Options,
    pub current_selection: Option<ID>,
    /// Custom per-app state can be stored here
    pub session: T,
    /// If desired, this can be advanced to render traffic signals changing.
    pub time: Time,
}

// A SimpleApp can directly use this (`let args = SimpleAppArgs::from_iter(abstutil::cli_args())`)
// or embed in their own struct and define other flags.
#[derive(StructOpt)]
pub struct SimpleAppArgs {
    /// Path to a map to initially load. If not provided, load the last map used or a fixed
    /// default.
    #[structopt()]
    pub map_path: Option<String>,
    /// Initially position the camera here. The format is an OSM-style `zoom/lat/lon` string
    /// (https://wiki.openstreetmap.org/wiki/Browsing#Other_URL_tricks).
    #[structopt(long)]
    pub cam: Option<String>,
    /// Dev mode exposes experimental tools useful for debugging, but that'd likely confuse most
    /// players.
    #[structopt(long)]
    pub dev: bool,
    /// The color scheme for map elements, agents, and the UI.
    #[structopt(long, parse(try_from_str = ColorSchemeChoice::parse))]
    pub color_scheme: Option<ColorSchemeChoice>,
    /// When making a screen recording, enable this option to hide some UI elements
    #[structopt(long)]
    pub minimal_controls: bool,
    /// Override the monitor's auto-detected scale factor
    #[structopt(long)]
    pub scale_factor: Option<f64>,
}

impl SimpleAppArgs {
    /// Options are passed in by each app, usually seeded with defaults or from a config file.  For
    /// the few options that we allow to be specified by command-line, overwrite the values.
    pub fn override_options(&self, opts: &mut Options) {
        opts.dev = self.dev;
        opts.minimal_controls = self.minimal_controls;
        if let Some(cs) = self.color_scheme {
            opts.color_scheme = cs;
            opts.toggle_day_night_colors = false;
        }
    }

    pub fn update_widgetry_settings(&self, mut settings: Settings) -> Settings {
        settings = settings
            .read_svg(Box::new(abstio::slurp_bytes))
            .window_icon(abstio::path("system/assets/pregame/icon.png"));
        if let Some(s) = self.scale_factor {
            settings = settings.scale_factor(s);
        }
        settings
    }

    pub fn map_name(&self) -> MapName {
        self.map_path
            .as_ref()
            .map(|path| {
                MapName::from_path(path).unwrap_or_else(|| panic!("bad map path: {}", path))
            })
            .or_else(|| {
                abstio::maybe_read_json::<crate::tools::DefaultMap>(
                    abstio::path_player("maps.json"),
                    &mut Timer::throwaway(),
                )
                .ok()
                .map(|x| x.last_map)
            })
            .unwrap_or_else(|| MapName::seattle("montlake"))
    }
}

impl<T: 'static> SimpleApp<T> {
    pub fn new<
        F: 'static + Fn(&mut EventCtx, &mut SimpleApp<T>) -> Vec<Box<dyn State<SimpleApp<T>>>>,
    >(
        ctx: &mut EventCtx,
        opts: Options,
        map_name: MapName,
        cam: Option<String>,
        session: T,
        init_states: F,
    ) -> (SimpleApp<T>, Vec<Box<dyn State<SimpleApp<T>>>>) {
        abstutil::logger::setup();
        ctx.canvas.settings = opts.canvas_settings.clone();

        let cs = ColorScheme::new(ctx, opts.color_scheme);
        // Start with a blank map
        let map = Map::blank();
        let draw_map = DrawMap::new(ctx, &map, &opts, &cs, &mut Timer::throwaway());
        let app = SimpleApp {
            map,
            draw_map,
            cs,
            opts,
            current_selection: None,
            session,
            time: Time::START_OF_DAY,
        };

        let states = vec![MapLoader::new_state(
            ctx,
            &app,
            map_name,
            Box::new(move |ctx, app| {
                URLManager::change_camera(ctx, cam.as_ref(), app.map().get_gps_bounds());
                Transition::Clear(init_states(ctx, app))
            }),
        )];
        (app, states)
    }

    pub fn draw_unzoomed(&self, g: &mut GfxCtx) {
        g.clear(self.cs.void_background);
        g.redraw(&self.draw_map.boundary_polygon);
        g.redraw(&self.draw_map.draw_all_areas);
        g.redraw(&self.draw_map.draw_all_unzoomed_parking_lots);
        g.redraw(&self.draw_map.draw_all_unzoomed_roads_and_intersections);
        g.redraw(&self.draw_map.draw_all_buildings);
        g.redraw(&self.draw_map.draw_all_building_outlines);
        // Not the building paths

        // Still show some shape selection when zoomed out.
        // TODO Refactor! Ideally use get_obj
        if let Some(ID::Area(id)) = self.current_selection {
            g.draw_polygon(
                self.cs.selected,
                self.draw_map.get_a(id).get_outline(&self.map),
            );
        } else if let Some(ID::Road(id)) = self.current_selection {
            g.draw_polygon(
                self.cs.selected,
                self.draw_map.get_r(id).get_outline(&self.map),
            );
        } else if let Some(ID::Intersection(id)) = self.current_selection {
            // Actually, don't use get_outline here! Full polygon is easier to see.
            g.draw_polygon(self.cs.selected, self.map.get_i(id).polygon.clone());
        } else if let Some(ID::Building(id)) = self.current_selection {
            g.draw_polygon(self.cs.selected, self.map.get_b(id).polygon.clone());
        }
    }

    pub fn draw_zoomed(&self, g: &mut GfxCtx, opts: DrawOptions) {
        g.clear(self.cs.void_background);
        g.redraw(&self.draw_map.boundary_polygon);

        let objects = self
            .draw_map
            .get_renderables_back_to_front(g.get_screen_bounds(), &self.map);

        let mut drawn_all_buildings = false;
        let mut drawn_all_areas = false;

        for obj in objects {
            obj.draw(g, self, &opts);

            match obj.get_id() {
                ID::Building(_) => {
                    if !drawn_all_buildings {
                        g.redraw(&self.draw_map.draw_all_buildings);
                        g.redraw(&self.draw_map.draw_all_building_outlines);
                        drawn_all_buildings = true;
                    }
                }
                ID::Area(_) => {
                    if !drawn_all_areas {
                        g.redraw(&self.draw_map.draw_all_areas);
                        drawn_all_areas = true;
                    }
                }
                _ => {}
            }

            if self.current_selection == Some(obj.get_id()) {
                g.draw_polygon(self.cs.selected, obj.get_outline(&self.map));
            }
        }
    }

    /// Assumes some defaults.
    pub fn recalculate_current_selection(&mut self, ctx: &EventCtx) {
        self.current_selection = self.calculate_current_selection(ctx, false, false);
    }

    // TODO Returns anything; I think it should just return roads
    pub fn mouseover_unzoomed_roads_and_intersections(&self, ctx: &EventCtx) -> Option<ID> {
        self.calculate_current_selection(ctx, true, false)
    }
    /// Only select buildings, and work whether zoomed in or not.
    pub fn mouseover_unzoomed_buildings(&self, ctx: &EventCtx) -> Option<ID> {
        self.calculate_current_selection(ctx, false, true)
            .filter(|id| matches!(id, ID::Building(_)))
    }

    fn calculate_current_selection(
        &self,
        ctx: &EventCtx,
        unzoomed_roads_and_intersections: bool,
        unzoomed_buildings: bool,
    ) -> Option<ID> {
        // Unzoomed mode. Ignore when debugging areas.
        if ctx.canvas.is_unzoomed() && !(unzoomed_roads_and_intersections || unzoomed_buildings) {
            return None;
        }

        let pt = ctx.canvas.get_cursor_in_map_space()?;

        let mut objects = self.draw_map.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            &self.map,
        );
        objects.reverse();

        for obj in objects {
            match obj.get_id() {
                ID::Road(_) => {
                    if !unzoomed_roads_and_intersections || ctx.canvas.is_zoomed() {
                        continue;
                    }
                }
                ID::Intersection(_) => {
                    if ctx.canvas.is_unzoomed() && !unzoomed_roads_and_intersections {
                        continue;
                    }
                }
                ID::Building(_) => {
                    if ctx.canvas.is_unzoomed() && !unzoomed_buildings {
                        continue;
                    }
                }
                _ => {
                    if ctx.canvas.is_unzoomed() {
                        continue;
                    }
                }
            }
            if obj.contains_pt(pt, &self.map) {
                return Some(obj.get_id());
            }
        }
        None
    }
}

impl<T: 'static> AppLike for SimpleApp<T> {
    #[inline]
    fn map(&self) -> &Map {
        &self.map
    }
    #[inline]
    fn sim(&self) -> &Sim {
        unreachable!()
    }
    #[inline]
    fn cs(&self) -> &ColorScheme {
        &self.cs
    }
    #[inline]
    fn mut_cs(&mut self) -> &mut ColorScheme {
        &mut self.cs
    }
    #[inline]
    fn draw_map(&self) -> &DrawMap {
        &self.draw_map
    }
    #[inline]
    fn mut_draw_map(&mut self) -> &mut DrawMap {
        &mut self.draw_map
    }
    #[inline]
    fn opts(&self) -> &Options {
        &self.opts
    }
    #[inline]
    fn mut_opts(&mut self) -> &mut Options {
        &mut self.opts
    }

    fn map_switched(&mut self, ctx: &mut EventCtx, map: Map, timer: &mut Timer) {
        CameraState::save(ctx.canvas, self.map.get_name());
        self.map = map;
        self.draw_map = DrawMap::new(ctx, &self.map, &self.opts, &self.cs, timer);
        if !CameraState::load(ctx, self.map.get_name()) {
            // If we didn't restore a previous camera position, start zoomed out, centered on the
            // map's center.
            ctx.canvas.cam_zoom = ctx.canvas.min_zoom();
            ctx.canvas
                .center_on_map_pt(self.map.get_boundary_polygon().center());
        }
    }

    fn draw_with_opts(&self, g: &mut GfxCtx, opts: DrawOptions) {
        if g.canvas.is_unzoomed() {
            self.draw_unzoomed(g);
        } else {
            self.draw_zoomed(g, opts);
        }
    }

    fn make_warper(
        &mut self,
        ctx: &EventCtx,
        pt: Pt2D,
        target_cam_zoom: Option<f64>,
        _: Option<ID>,
    ) -> Box<dyn State<SimpleApp<T>>> {
        Box::new(SimpleWarper {
            warper: Warper::new(ctx, pt, target_cam_zoom),
        })
    }

    fn sim_time(&self) -> Time {
        self.time
    }

    fn current_stage_and_remaining_time(&self, id: IntersectionID) -> (usize, Duration) {
        let signal = self.map.get_traffic_signal(id);
        let mut time_left = (self.time - Time::START_OF_DAY) % signal.simple_cycle_duration();
        for (idx, stage) in signal.stages.iter().enumerate() {
            if time_left < stage.stage_type.simple_duration() {
                return (idx, time_left);
            }
            time_left -= stage.stage_type.simple_duration();
        }
        unreachable!()
    }
}

impl<T: 'static> SharedAppState for SimpleApp<T> {
    fn draw_default(&self, g: &mut GfxCtx) {
        self.draw_with_opts(g, DrawOptions::new());
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        CameraState::save(canvas, self.map.get_name());
    }

    fn before_quit(&self, canvas: &Canvas) {
        CameraState::save(canvas, self.map.get_name());
    }

    fn free_memory(&mut self) {
        self.draw_map.free_memory();
    }
}

struct SimpleWarper {
    warper: Warper,
}

impl<T> State<SimpleApp<T>> for SimpleWarper {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut SimpleApp<T>) -> Transition<SimpleApp<T>> {
        if self.warper.event(ctx) {
            Transition::Keep
        } else {
            Transition::Pop
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &SimpleApp<T>) {}
}
