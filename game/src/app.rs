use std::cell::RefCell;
use std::collections::BTreeMap;

use maplit::btreemap;
use rand::seq::SliceRandom;

use abstutil::Timer;
use geom::{Bounds, Circle, Distance, Duration, Pt2D, Time};
use map_model::{IntersectionID, Map, Traversable};
use sim::{Analytics, Sim, SimCallback, SimFlags};
use widgetry::{Color, EventCtx, GfxCtx, Prerender};

use crate::challenges::HighScore;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::layer::Layer;
use crate::options::Options;
use crate::render::{
    unzoomed_agent_radius, AgentCache, DrawMap, DrawOptions, Renderable, UnzoomedAgents,
};
use crate::sandbox::{GameplayMode, TutorialState};

/// The top-level data that lasts through the entire game, no matter what state the game is in.
pub struct App {
    // Naming is from older days when there was an A/B test, "side-by-side" mode. Keeping this
    // naming, because that mode will return someday.
    pub primary: PerMap,
    pub cs: ColorScheme,
    // TODO This is a bit weird to keep here; it's controlled almost entirely by the minimap panel.
    // It has no meaning in edit mode.
    pub unzoomed_agents: UnzoomedAgents,
    pub opts: Options,

    pub per_obj: PerObjectActions,

    /// Static data that lasts the entire session. Use sparingly.
    pub session: SessionState,
}

impl App {
    pub fn new(flags: Flags, opts: Options, ctx: &mut EventCtx, splash: bool) -> App {
        let cs = ColorScheme::new(opts.color_scheme);
        ctx.set_style(cs.gui_style.clone());

        let primary = ctx.loading_screen("load map", |ctx, mut timer| {
            let (map, sim, _) = flags.sim_flags.load(timer);
            PerMap::map_loaded(map, sim, splash, flags, &opts, &cs, ctx, &mut timer)
        });

        App {
            primary,
            unzoomed_agents: UnzoomedAgents::new(&cs),
            cs,
            opts,
            per_obj: PerObjectActions::new(),
            session: SessionState::empty(),
        }
    }

    // TODO Should the prebaked methods be on primary along with the data?
    pub fn has_prebaked(&self) -> Option<(&String, &String)> {
        self.primary.prebaked.as_ref().map(|(m, s, _)| (m, s))
    }
    pub fn prebaked(&self) -> &Analytics {
        &self.primary.prebaked.as_ref().unwrap().2
    }
    pub fn set_prebaked(&mut self, prebaked: Option<(String, String, Analytics)>) {
        self.primary.prebaked = prebaked;

        if false {
            if let Some((_, _, ref a)) = self.primary.prebaked {
                use abstutil::{prettyprint_usize, serialized_size_bytes};
                println!(
                    "- road_thruput: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.road_thruput))
                );
                println!(
                    "- intersection_thruput: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.intersection_thruput))
                );
                println!(
                    "- traffic_signal_thruput: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.traffic_signal_thruput))
                );
                println!(
                    "- demand : {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.demand))
                );
                println!(
                    "- bus_arrivals : {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.bus_arrivals))
                );
                println!(
                    "- passengers_boarding: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.passengers_boarding))
                );
                println!(
                    "- passengers_alighting: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.passengers_alighting))
                );
                println!(
                    "- started_trips: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.started_trips))
                );
                println!(
                    "- finished_trips: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.finished_trips))
                );
                println!(
                    "- trip_log: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.trip_log))
                );
                println!(
                    "- intersection_delays: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.intersection_delays))
                );
                println!(
                    "- parking_lane_changes: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.parking_lane_changes))
                );
                println!(
                    "- parking_lot_changes: {} bytes",
                    prettyprint_usize(serialized_size_bytes(&a.parking_lot_changes))
                );
            }
        }
    }

    pub fn map_switched(&mut self, ctx: &mut EventCtx, map: Map, sim: Sim, timer: &mut Timer) {
        ctx.canvas.save_camera_state(self.primary.map.get_name());
        self.primary = PerMap::map_loaded(
            map,
            sim,
            false,
            self.primary.current_flags.clone(),
            &self.opts,
            &self.cs,
            ctx,
            timer,
        )
    }

    pub fn draw(&self, g: &mut GfxCtx, opts: DrawOptions, show_objs: &dyn ShowObject) {
        let map = &self.primary.map;
        let draw_map = &self.primary.draw_map;

        let mut sample_intersection: Option<String> = None;

        g.clear(self.cs.void_background);
        g.redraw(&draw_map.boundary_polygon);

        if g.canvas.cam_zoom < self.opts.min_zoom_for_detail && !g.is_screencap() {
            // Unzoomed mode
            let layers = show_objs.layers();
            if layers.show_areas {
                g.redraw(&draw_map.draw_all_areas);
            }
            if layers.show_parking_lots {
                g.redraw(&draw_map.draw_all_unzoomed_parking_lots);
            }
            if layers.show_intersections || layers.show_lanes {
                g.redraw(
                    &self
                        .primary
                        .draw_map
                        .draw_all_unzoomed_roads_and_intersections,
                );
            }
            if layers.show_buildings {
                g.redraw(&draw_map.draw_all_buildings);
                // Not the building paths
            }

            // Still show some shape selection when zoomed out.
            // TODO Refactor! Ideally use get_obj
            if let Some(ID::Area(id)) = self.primary.current_selection {
                g.draw_polygon(self.cs.selected, draw_map.get_a(id).get_outline(map));
            } else if let Some(ID::Road(id)) = self.primary.current_selection {
                g.draw_polygon(self.cs.selected, draw_map.get_r(id).get_outline(map));
            } else if let Some(ID::Intersection(id)) = self.primary.current_selection {
                // Actually, don't use get_outline here! Full polygon is easier to see.
                g.draw_polygon(self.cs.selected, map.get_i(id).polygon.clone());
            } else if let Some(ID::Building(id)) = self.primary.current_selection {
                g.draw_polygon(self.cs.selected, map.get_b(id).polygon.clone());
            }

            let mut cache = self.primary.draw_map.agents.borrow_mut();
            cache.draw_unzoomed_agents(g, self);

            if let Some(a) = self
                .primary
                .current_selection
                .as_ref()
                .and_then(|id| id.agent_id())
            {
                if let Some(pt) = self.primary.sim.canonical_pt_for_agent(a, map) {
                    // Usually we show selection with an outline, but no thickness/color is really
                    // visible for these tiny crowded dots.
                    g.draw_polygon(
                        Color::CYAN,
                        Circle::new(pt, unzoomed_agent_radius(a.to_vehicle_type())).to_polygon(),
                    );
                }
            }
        } else {
            let mut cache = draw_map.agents.borrow_mut();
            let objects = self.get_renderables_back_to_front(
                g.get_screen_bounds(),
                &g.prerender,
                &mut cache,
                show_objs,
            );

            let mut drawn_all_buildings = false;
            let mut drawn_all_areas = false;

            for obj in objects {
                obj.draw(g, self, &opts);

                match obj.get_id() {
                    ID::Building(_) => {
                        if !drawn_all_buildings {
                            g.redraw(&draw_map.draw_all_building_paths);
                            g.redraw(&draw_map.draw_all_buildings);
                            g.redraw(&draw_map.draw_all_building_outlines);
                            drawn_all_buildings = true;
                        }
                    }
                    ID::Area(_) => {
                        if !drawn_all_areas {
                            g.redraw(&draw_map.draw_all_areas);
                            drawn_all_areas = true;
                        }
                    }
                    _ => {}
                };

                if self.primary.current_selection == Some(obj.get_id()) {
                    g.draw_polygon(self.cs.selected, obj.get_outline(map));
                }

                if g.is_screencap() && sample_intersection.is_none() {
                    if let ID::Intersection(id) = obj.get_id() {
                        sample_intersection = Some(format!("_i{}", id.0));
                    }
                }
            }
        }

        if let Some(i) = sample_intersection {
            g.set_screencap_naming_hint(i);
        }
    }

    /// Assumes some defaults.
    pub fn recalculate_current_selection(&mut self, ctx: &EventCtx) {
        self.primary.current_selection =
            self.calculate_current_selection(ctx, &ShowEverything::new(), false, false, false);
    }

    pub fn mouseover_unzoomed_roads_and_intersections(&self, ctx: &EventCtx) -> Option<ID> {
        self.calculate_current_selection(ctx, &ShowEverything::new(), false, true, false)
    }
    pub fn mouseover_unzoomed_buildings(&self, ctx: &EventCtx) -> Option<ID> {
        self.calculate_current_selection(ctx, &ShowEverything::new(), false, false, true)
    }
    pub fn mouseover_unzoomed_everything(&self, ctx: &EventCtx) -> Option<ID> {
        self.calculate_current_selection(ctx, &ShowEverything::new(), false, true, true)
    }
    pub fn mouseover_debug_mode(&self, ctx: &EventCtx, show_objs: &dyn ShowObject) -> Option<ID> {
        self.calculate_current_selection(ctx, show_objs, true, false, false)
    }

    fn calculate_current_selection(
        &self,
        ctx: &EventCtx,
        show_objs: &dyn ShowObject,
        debug_mode: bool,
        unzoomed_roads_and_intersections: bool,
        unzoomed_buildings: bool,
    ) -> Option<ID> {
        // Unzoomed mode. Ignore when debugging areas.
        if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail
            && !(debug_mode || unzoomed_roads_and_intersections || unzoomed_buildings)
        {
            return None;
        }

        let pt = ctx.canvas.get_cursor_in_map_space()?;

        let mut cache = self.primary.draw_map.agents.borrow_mut();
        let mut objects = self.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            ctx.prerender,
            &mut cache,
            show_objs,
        );
        objects.reverse();

        for obj in objects {
            match obj.get_id() {
                ID::Area(_) => {
                    if !debug_mode {
                        continue;
                    }
                }
                ID::Road(_) => {
                    if !unzoomed_roads_and_intersections
                        || ctx.canvas.cam_zoom >= self.opts.min_zoom_for_detail
                    {
                        continue;
                    }
                }
                ID::Intersection(_) => {
                    if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail
                        && !unzoomed_roads_and_intersections
                    {
                        continue;
                    }
                }
                ID::Building(_) => {
                    if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail && !unzoomed_buildings {
                        continue;
                    }
                }
                _ => {
                    if ctx.canvas.cam_zoom < self.opts.min_zoom_for_detail {
                        continue;
                    }
                }
            }
            if obj.contains_pt(pt, &self.primary.map) {
                return Some(obj.get_id());
            }
        }
        None
    }

    // TODO This could probably belong to DrawMap again, but it's annoying to plumb things that
    // State does, like show_icons_for() and show().
    fn get_renderables_back_to_front<'a>(
        &'a self,
        bounds: Bounds,
        prerender: &Prerender,
        agents: &'a mut AgentCache,
        show_objs: &dyn ShowObject,
    ) -> Vec<&'a (dyn Renderable + 'a)> {
        let map = &self.primary.map;
        let draw_map = &self.primary.draw_map;

        let mut areas: Vec<&dyn Renderable> = Vec::new();
        let mut parking_lots: Vec<&dyn Renderable> = Vec::new();
        let mut lanes: Vec<&dyn Renderable> = Vec::new();
        let mut roads: Vec<&dyn Renderable> = Vec::new();
        let mut intersections: Vec<&dyn Renderable> = Vec::new();
        let mut buildings: Vec<&dyn Renderable> = Vec::new();
        let mut bus_stops: Vec<&dyn Renderable> = Vec::new();
        let mut agents_on: Vec<Traversable> = Vec::new();

        for id in draw_map.get_matching_objects(bounds) {
            if !show_objs.show(&id) {
                continue;
            }
            match id {
                ID::Area(id) => areas.push(draw_map.get_a(id)),
                ID::Lane(id) => {
                    lanes.push(draw_map.get_l(id));
                    agents_on.push(Traversable::Lane(id));
                    for bs in &map.get_l(id).bus_stops {
                        if show_objs.show(&ID::BusStop(*bs)) {
                            bus_stops.push(draw_map.get_bs(*bs));
                        }
                    }
                }
                ID::Road(id) => {
                    roads.push(draw_map.get_r(id));
                }
                ID::Intersection(id) => {
                    intersections.push(draw_map.get_i(id));
                    for t in &map.get_i(id).turns {
                        agents_on.push(Traversable::Turn(*t));
                    }
                }
                ID::Building(id) => buildings.push(draw_map.get_b(id)),
                ID::ParkingLot(id) => {
                    parking_lots.push(draw_map.get_pl(id));
                    // Slight hack
                    agents_on.push(Traversable::Lane(map.get_pl(id).driving_pos.lane()));
                }

                ID::BusStop(_) | ID::Car(_) | ID::Pedestrian(_) | ID::PedCrowd(_) => {
                    panic!("{:?} shouldn't be in the quadtree", id)
                }
            }
        }

        // From background to foreground Z-order
        let mut borrows: Vec<&dyn Renderable> = Vec::new();
        borrows.extend(areas);
        borrows.extend(parking_lots);
        borrows.extend(lanes);
        borrows.extend(roads);
        borrows.extend(intersections);
        borrows.extend(buildings);
        borrows.extend(bus_stops);

        // Expand all of the Traversables into agents, populating the cache if needed.
        {
            for on in &agents_on {
                agents.populate_if_needed(*on, map, &self.primary.sim, &self.cs, prerender);
            }
        }

        for on in agents_on {
            for obj in agents.get(on) {
                borrows.push(obj);
            }
        }

        borrows.retain(|x| x.get_zorder() <= self.primary.show_zorder);

        // This is a stable sort.
        borrows.sort_by_key(|x| x.get_zorder());

        borrows
    }
}

pub struct ShowLayers {
    pub show_buildings: bool,
    pub show_parking_lots: bool,
    pub show_intersections: bool,
    pub show_lanes: bool,
    pub show_areas: bool,
    pub show_labels: bool,
}

impl ShowLayers {
    pub fn new() -> ShowLayers {
        ShowLayers {
            show_buildings: true,
            show_parking_lots: true,
            show_intersections: true,
            show_lanes: true,
            show_areas: true,
            show_labels: false,
        }
    }
}

pub trait ShowObject {
    fn show(&self, obj: &ID) -> bool;
    fn layers(&self) -> &ShowLayers;
}

pub struct ShowEverything {
    layers: ShowLayers,
}

impl ShowEverything {
    pub fn new() -> ShowEverything {
        ShowEverything {
            layers: ShowLayers::new(),
        }
    }
}

impl ShowObject for ShowEverything {
    fn show(&self, _: &ID) -> bool {
        true
    }

    fn layers(&self) -> &ShowLayers {
        &self.layers
    }
}

#[derive(Clone)]
pub struct Flags {
    pub sim_flags: SimFlags,
    /// Number of agents to generate when requested. If unspecified, trips to/from borders will be
    /// included.
    pub num_agents: Option<usize>,
    /// If true, all map edits immediately apply to the live simulation. Otherwise, most edits
    /// require resetting to midnight.
    pub live_map_edits: bool,
}

/// All of the state that's bound to a specific map.
pub struct PerMap {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub current_flags: Flags,
    pub last_warped_from: Option<(Pt2D, f64)>,
    pub sim_cb: Option<Box<dyn SimCallback>>,
    pub show_zorder: isize,
    pub zorder_range: (isize, isize),
    /// If we ever left edit mode and resumed without restarting from midnight, this is true.
    pub dirty_from_edits: bool,
    /// Any ScenarioModifiers in effect?
    pub has_modified_trips: bool,

    /// Sometimes we need the map before any edits have been applied. Cache it here.
    pub unedited_map: RefCell<Option<Map>>,

    pub layer: Option<Box<dyn Layer>>,
    /// Only filled out in edit mode. Stored here once to avoid lots of clones. Used for preview.
    pub suspended_sim: Option<Sim>,
    /// Only exists in some gameplay modes. Must be carefully reset otherwise. Has the map and
    /// scenario name too.
    // TODO Embed that in Analytics directly instead.
    prebaked: Option<(String, String, Analytics)>,
}

impl PerMap {
    fn map_loaded(
        map: Map,
        sim: Sim,
        splash: bool,
        flags: Flags,
        opts: &Options,
        cs: &ColorScheme,
        ctx: &mut EventCtx,
        timer: &mut Timer,
    ) -> PerMap {
        timer.start("draw_map");
        let (draw_map, zorder_range) = DrawMap::new(&map, opts, cs, ctx, timer);
        timer.stop("draw_map");

        let per_map = PerMap {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags,
            last_warped_from: None,
            sim_cb: None,
            zorder_range,
            show_zorder: zorder_range.1,
            dirty_from_edits: false,
            has_modified_trips: false,
            unedited_map: RefCell::new(None),
            layer: None,
            suspended_sim: None,
            prebaked: None,
        };

        let mut rng = per_map.current_flags.sim_flags.make_rng();
        let rand_focus_pt = per_map
            .map
            .all_buildings()
            .choose(&mut rng)
            .and_then(|b| ID::Building(b.id).canonical_point(&per_map))
            .or_else(|| {
                per_map
                    .map
                    .all_lanes()
                    .choose(&mut rng)
                    .and_then(|l| ID::Lane(l.id).canonical_point(&per_map))
            })
            .expect("Can't get canonical_point of a random building or lane");
        let bounds = per_map.map.get_bounds();
        ctx.canvas.map_dims = (bounds.width(), bounds.height());

        if splash {
            ctx.canvas.center_on_map_pt(rand_focus_pt);
        } else {
            if !ctx.canvas.load_camera_state(per_map.map.get_name()) {
                println!("Couldn't load camera state, just focusing on an arbitrary building");
                ctx.canvas.center_on_map_pt(rand_focus_pt);
            }
        }

        per_map
    }

    /// Returns whatever was there
    pub fn clear_sim(&mut self) -> Sim {
        self.dirty_from_edits = false;
        std::mem::replace(
            &mut self.sim,
            Sim::new(
                &self.map,
                self.current_flags.sim_flags.opts.clone(),
                &mut Timer::new("reset simulation"),
            ),
        )
    }

    /// If needed, makes sure the unedited_map is populated. Callers can then do
    /// `self.unedited_map.borrow().unwrap_or(&self.map)`.
    // TODO I'd like to return &Map or something here, but can't get the borrow checker to work.
    pub fn calculate_unedited_map(&self) {
        if self.map.get_edits().commands.is_empty() {
            return;
        }

        let mut orig_map = self.unedited_map.borrow_mut();
        if orig_map.is_none() {
            let mut timer = Timer::new("load unedited map");
            *orig_map = Some(Map::new(
                abstutil::path_map(self.map.get_name()),
                &mut timer,
            ));
        }
    }
}

// TODO Serialize these, but in a very careful, future-compatible way
pub struct SessionState {
    pub tutorial: Option<TutorialState>,
    pub high_scores: BTreeMap<GameplayMode, Vec<HighScore>>,
    pub info_panel_tab: BTreeMap<&'static str, &'static str>,
}

impl SessionState {
    pub fn empty() -> SessionState {
        SessionState {
            tutorial: None,
            high_scores: BTreeMap::new(),
            info_panel_tab: btreemap! {
                "lane" => "info",
                "intersection" => "info",
                "bldg" => "info",
                "person" => "trips",
                "bus" => "status",
            },
        }
    }
}

// TODO Reconsider this; maybe it does belong in widgetry.
pub struct PerObjectActions {
    pub click_action: Option<String>,
}

impl PerObjectActions {
    pub fn new() -> PerObjectActions {
        PerObjectActions { click_action: None }
    }

    pub fn reset(&mut self) {
        self.click_action = None;
    }

    pub fn left_click<S: Into<String>>(&mut self, ctx: &mut EventCtx, label: S) -> bool {
        assert!(self.click_action.is_none());
        self.click_action = Some(label.into());
        ctx.normal_left_click()
    }
}

pub struct FindDelayedIntersections {
    pub halt_limit: Duration,
    pub report_limit: Duration,

    pub currently_delayed: Vec<(IntersectionID, Time)>,
}

impl SimCallback for FindDelayedIntersections {
    fn run(&mut self, sim: &Sim, _: &Map) -> bool {
        self.currently_delayed = sim.delayed_intersections(self.report_limit);
        if let Some((_, t)) = self.currently_delayed.get(0) {
            sim.time() - *t >= self.halt_limit
        } else {
            false
        }
    }
}
