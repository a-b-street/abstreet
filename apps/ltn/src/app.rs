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
use map_model::{osm, CrossingType, IntersectionID, Map, RoutingParams};
use widgetry::tools::URLManager;
use widgetry::{Canvas, Drawable, EventCtx, GfxCtx, SharedAppState, State, Warper};

use crate::logic::Partitioning;
use crate::{logic, pages, render, Edits, FilterType, NeighbourhoodID};

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
    pub impact: logic::Impact,

    pub consultation: Option<NeighbourhoodID>,
    pub consultation_id: Option<String>,

    pub draw_all_filters: render::Toggle3Zoomed,
    pub draw_major_road_labels: DrawSimpleRoadLabels,
    pub draw_all_local_road_labels: Option<DrawSimpleRoadLabels>,
    pub draw_poi_icons: Drawable,
    pub draw_bus_routes: Drawable,

    pub current_trip_name: Option<String>,
}

impl PerMap {
    fn new(
        ctx: &mut EventCtx,
        mut map: Map,
        opts: &Options,
        cs: &ColorScheme,
        timer: &mut Timer,
    ) -> Self {
        // Do this before creating the default partitioning. Non-driveable roads in OSM get turned
        // into driveable roads and a filter here, and we want the partitioning to "see" those
        // roads.
        let edits = logic::transform_existing_filters(&mut map, timer);
        let mut proposals = crate::save::Proposals::new(&map, edits, timer);

        let mut routing_params_before_changes = map.routing_params().clone();
        proposals
            .current_proposal
            .edits
            .update_routing_params(&mut routing_params_before_changes);

        let draw_all_filters = proposals.current_proposal.edits.draw(ctx, &map);

        logic::populate_existing_crossings(&map, &mut proposals.current_proposal.edits);

        // Create DrawMap after transform_existing_filters, which modifies road widths
        let draw_map = DrawMap::new(ctx, &map, opts, cs, timer);
        let draw_poi_icons = render::render_poi_icons(ctx, &map);
        let draw_bus_routes = render::render_bus_routes(ctx, &map);

        let per_map = Self {
            map,
            draw_map,

            current_neighbourhood: None,

            routing_params_before_changes,
            proposals,
            impact: logic::Impact::empty(ctx),

            consultation: None,
            consultation_id: None,

            draw_all_filters,
            draw_major_road_labels: DrawSimpleRoadLabels::empty(ctx),
            draw_all_local_road_labels: None,
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
    pub edit_mode: pages::EditMode,
    pub filter_type: FilterType,
    pub crossing_type: CrossingType,

    // Remember form settings in different tabs.
    // Pick areas:
    pub draw_neighbourhood_style: pages::PickAreaStyle,
    // Plan a route:
    pub main_road_penalty: f64,
    pub show_walking_cycling_routes: bool,
    // Select boundary:
    pub add_intermediate_blocks: bool,

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
        self.per_map.draw_major_road_labels =
            DrawSimpleRoadLabels::only_major_roads(ctx, self, render::colors::MAIN_ROAD_LABEL);
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
        init_states: F,
    ) -> (App, Vec<Box<dyn State<App>>>) {
        abstutil::logger::setup();
        ctx.canvas.settings = opts.canvas_settings.clone();

        let session = Session {
            edit_mode: pages::EditMode::Filters,
            filter_type: FilterType::WalkCycleOnly,
            crossing_type: CrossingType::Unsignalized,

            draw_neighbourhood_style: pages::PickAreaStyle::Simple,
            main_road_penalty: 1.0,
            show_walking_cycling_routes: false,
            add_intermediate_blocks: true,

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

    pub fn calculate_draw_all_local_road_labels(&mut self, ctx: &mut EventCtx) {
        if self.per_map.draw_all_local_road_labels.is_none() {
            self.per_map.draw_all_local_road_labels = Some(DrawSimpleRoadLabels::new(
                ctx,
                self,
                render::colors::LOCAL_ROAD_LABEL,
                Box::new(|r| r.get_rank() == osm::RoadRank::Local && !r.is_light_rail()),
            ));
        }
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
