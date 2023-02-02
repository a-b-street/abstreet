use abstio::MapName;
use abstutil::Timer;
use geom::{Distance, Duration, Pt2D, Time};
use map_gui::colors::ColorScheme;
use map_gui::load::MapLoader;
use map_gui::options::Options;
use map_gui::render::{DrawMap, DrawOptions};
use map_gui::tools::CameraState;
use map_gui::tools::DrawSimpleRoadLabels;
use map_gui::{AppLike, ID};
use map_model::{AmenityType, CrossingType, IntersectionID, Map, RoutingParams};
use widgetry::tools::URLManager;
use widgetry::{
    Canvas, Color, Drawable, EventCtx, GeomBatch, GfxCtx, RewriteColor, SharedAppState, State,
    Warper,
};

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

    // The last edited neighbourhood
    pub current_neighbourhood: Option<NeighbourhoodID>,

    // These capture modal filters that exist in the map already. Whenever we pathfind in this app
    // in the "before changes" case, we have to use these. Do NOT use the map's built-in
    // pathfinder. (https://github.com/a-b-street/abstreet/issues/852 would make this more clear)
    pub routing_params_before_changes: RoutingParams,
    pub proposals: crate::save::Proposals,
    pub impact: crate::impact::Impact,

    pub consultation: Option<NeighbourhoodID>,
    pub consultation_id: Option<String>,

    pub draw_all_filters: Toggle3Zoomed,
    pub draw_major_road_labels: Option<DrawSimpleRoadLabels>,
    pub draw_all_road_labels: Option<DrawSimpleRoadLabels>,
    pub draw_poi_icons: Drawable,
    pub draw_bus_routes: Drawable,

    pub current_trip_name: Option<String>,
}

impl PerMap {
    fn new(
        ctx: &mut EventCtx,
        map: Map,
        opts: &Options,
        cs: &ColorScheme,
        timer: &mut Timer,
    ) -> Self {
        let draw_map = DrawMap::new(ctx, &map, opts, cs, timer);
        let draw_poi_icons = render_poi_icons(ctx, &map);
        let draw_bus_routes = render_bus_routes(ctx, &map);

        let proposals = crate::save::Proposals::new(&map, timer);

        let per_map = Self {
            map,
            draw_map,

            current_neighbourhood: None,

            routing_params_before_changes: RoutingParams::default(),
            proposals,
            impact: crate::impact::Impact::empty(ctx),

            consultation: None,
            consultation_id: None,

            draw_all_filters: Toggle3Zoomed::empty(ctx),
            draw_major_road_labels: None,
            draw_all_road_labels: None,
            draw_poi_icons,
            draw_bus_routes,

            current_trip_name: None,
        };

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

pub struct Session {
    pub edit_mode: crate::edit::EditMode,
    pub filter_type: FilterType,
    pub crossing_type: CrossingType,

    // Remember form settings in different tabs.
    // Pick areas:
    pub draw_neighbourhood_style: crate::pick_area::Style,
    // Plan a route:
    pub main_road_penalty: f64,
    pub show_walking_cycling_routes: bool,

    // Shared in all modes
    pub layers: crate::components::Layers,
    pub manage_proposals: bool,
}

impl AppLike for App {
    #[inline]
    fn map(&self) -> &Map {
        &self.per_map.map
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
        self.per_map = PerMap::new(ctx, map, &self.opts, &self.cs, timer);
        self.opts.units.metric = self.per_map.map.get_name().city.uses_metric();

        // These two logically belong in PerMap::new, but it's easier to have the full App
        crate::filters::transform_existing_filters(ctx, self, timer);
        self.per_map.draw_all_filters = self
            .per_map
            .proposals
            .current_proposal
            .edits
            .draw(ctx, &self.per_map.map);

        crate::crossings::populate_existing_crossings(self);
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
        init_states: F,
    ) -> (App, Vec<Box<dyn State<App>>>) {
        abstutil::logger::setup();
        ctx.canvas.settings = opts.canvas_settings.clone();

        let session = Session {
            edit_mode: crate::edit::EditMode::Filters,
            filter_type: FilterType::WalkCycleOnly,
            crossing_type: CrossingType::Unsignalized,

            draw_neighbourhood_style: crate::pick_area::Style::Simple,
            main_road_penalty: 1.0,
            show_walking_cycling_routes: false,

            layers: crate::components::Layers::new(ctx),
            manage_proposals: false,
        };

        let cs = ColorScheme::new(ctx, opts.color_scheme);
        let app = App {
            // Start with a blank map
            per_map: PerMap::new(
                ctx,
                Map::almost_blank(),
                &opts,
                &cs,
                &mut Timer::throwaway(),
            ),
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

    pub fn edits(&self) -> &Edits {
        &self.per_map.proposals.current_proposal.edits
    }
    pub fn partitioning(&self) -> &Partitioning {
        &self.per_map.proposals.current_proposal.partitioning
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
