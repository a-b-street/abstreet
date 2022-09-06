use abstio::MapName;
use abstutil::Timer;
use geom::{Duration, Pt2D, Time};
use map_gui::colors::ColorScheme;
use map_gui::load::MapLoader;
use map_gui::options::Options;
use map_gui::render::{DrawMap, DrawOptions};
use map_gui::tools::CameraState;
use map_gui::tools::DrawSimpleRoadLabels;
use map_gui::{AppLike, ID};
use map_model::{IntersectionID, Map, RoutingParams};
use widgetry::tools::URLManager;
use widgetry::{Canvas, Drawable, EventCtx, GfxCtx, SharedAppState, State, Warper};

use crate::{Edits, FilterType, NeighbourhoodID, Partitioning, Toggle3Zoomed};

pub type Transition = widgetry::Transition<App>;

pub struct App {
    pub per_map: PerMap,
    pub cs: ColorScheme,
    pub opts: Options,

    pub session: Session,
}

pub struct PerMap {
    pub map: Map,
    pub draw_map: DrawMap,
}

impl PerMap {
    fn new(ctx: &mut EventCtx, app: &App, map: Map, timer: &mut Timer) -> Self {
        let draw_map = DrawMap::new(ctx, &map, &app.opts, &app.cs, timer);
        let per_map = Self { map, draw_map };
        if !CameraState::load(ctx, per_map.map.get_name()) {
            // If we didn't restore a previous camera position, start zoomed out, centered on the
            // map's center.
            ctx.canvas.cam_zoom = ctx.canvas.min_zoom();
            ctx.canvas
                .center_on_map_pt(per_map.map.get_boundary_polygon().center());
        }
        per_map
    }
}

// TODO Tension: Many of these are per-map. game::App nicely wraps these up. Time to stop abusing
// SimpleApp?
pub struct Session {
    // These come from a save::Proposal
    pub proposal_name: Option<String>,
    pub partitioning: Partitioning,
    pub edits: Edits,
    // These capture modal filters that exist in the map already. Whenever we pathfind in this app
    // in the "before changes" case, we have to use these. Do NOT use the map's built-in
    // pathfinder. (https://github.com/a-b-street/abstreet/issues/852 would make this more clear)
    pub routing_params_before_changes: RoutingParams,
    pub draw_all_road_labels: Option<DrawSimpleRoadLabels>,
    pub draw_poi_icons: Drawable,
    pub draw_bus_routes: Drawable,

    pub alt_proposals: crate::save::AltProposals,
    pub draw_all_filters: Toggle3Zoomed,
    pub impact: crate::impact::Impact,

    pub edit_mode: crate::edit::EditMode,
    pub filter_type: FilterType,

    // Remember form settings in different tabs.
    // Browse neighbourhoods:
    pub draw_neighbourhood_style: crate::browse::Style,
    // Editing:
    pub heuristic: crate::filters::auto::Heuristic,
    // Pathfinding
    pub main_road_penalty: f64,
    pub show_walking_cycling_routes: bool,

    pub current_trip_name: Option<String>,

    pub consultation: Option<NeighbourhoodID>,
    pub consultation_id: Option<String>,
    // The current consultation should always be based off a built-in proposal
    pub consultation_proposal_path: Option<String>,

    // Shared in all modes
    pub layers: crate::components::Layers,
}

impl AppLike for App {
    #[inline]
    fn map(&self) -> &Map {
        &self.per_map.map
    }
    #[inline]
    fn sim(&self) -> &sim::Sim {
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
        &self.per_map.draw_map
    }
    #[inline]
    fn mut_draw_map(&mut self) -> &mut DrawMap {
        &mut self.per_map.draw_map
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
        CameraState::save(ctx.canvas, self.per_map.map.get_name());
        self.per_map = PerMap::new(ctx, self, map, timer);
        self.opts.units.metric = self.per_map.map.get_name().city.uses_metric();
    }

    fn draw_with_opts(&self, g: &mut GfxCtx, _l: DrawOptions) {
        self.draw_with_layering(g, |_| {});
    }
    fn make_warper(
        &mut self,
        ctx: &EventCtx,
        pt: Pt2D,
        target_cam_zoom: Option<f64>,
        _: Option<ID>,
    ) -> Box<dyn State<App>> {
        Box::new(SimpleWarper {
            warper: Warper::new(ctx, pt, target_cam_zoom),
        })
    }

    fn sim_time(&self) -> Time {
        Time::START_OF_DAY
    }

    fn current_stage_and_remaining_time(&self, _: IntersectionID) -> (usize, Duration) {
        (0, Duration::ZERO)
    }
}

impl SharedAppState for App {
    fn draw_default(&self, g: &mut GfxCtx) {
        self.draw_with_opts(g, DrawOptions::new());
    }

    fn dump_before_abort(&self, canvas: &Canvas) {
        CameraState::save(canvas, self.per_map.map.get_name());
    }

    fn before_quit(&self, canvas: &Canvas) {
        CameraState::save(canvas, self.per_map.map.get_name());
    }

    fn free_memory(&mut self) {
        self.per_map.draw_map.free_memory();
    }
}

impl App {
    pub fn new<F: 'static + Fn(&mut EventCtx, &mut App) -> Vec<Box<dyn State<App>>>>(
        ctx: &mut EventCtx,
        opts: Options,
        map_name: MapName,
        cam: Option<String>,
        session: Session,
        init_states: F,
    ) -> (App, Vec<Box<dyn State<App>>>) {
        abstutil::logger::setup();
        ctx.canvas.settings = opts.canvas_settings.clone();

        let cs = ColorScheme::new(ctx, opts.color_scheme);
        // Start with a blank map
        let map = Map::blank();
        let draw_map = DrawMap::new(ctx, &map, &opts, &cs, &mut Timer::throwaway());
        let app = App {
            per_map: PerMap { map, draw_map },
            cs,
            opts,
            session,
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

    /// Draw unzoomed, but after the water/park areas layer, draw something custom.
    pub fn draw_with_layering<F: Fn(&mut GfxCtx)>(&self, g: &mut GfxCtx, custom: F) {
        g.clear(self.cs.void_background);
        g.redraw(&self.per_map.draw_map.boundary_polygon);
        g.redraw(&self.per_map.draw_map.draw_all_areas);
        custom(g);
        g.redraw(&self.per_map.draw_map.draw_all_unzoomed_parking_lots);
        g.redraw(
            &self
                .per_map
                .draw_map
                .draw_all_unzoomed_roads_and_intersections,
        );
        g.redraw(&self.per_map.draw_map.draw_all_buildings);
        g.redraw(&self.per_map.draw_map.draw_all_building_outlines);
    }
}

struct SimpleWarper {
    warper: Warper,
}

impl State<App> for SimpleWarper {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        if self.warper.event(ctx) {
            Transition::Keep
        } else {
            Transition::Pop
        }
    }

    fn draw(&self, _: &mut GfxCtx, _: &App) {}
}
