use crate::challenges::HighScore;
use crate::colors::ColorScheme;
use crate::helpers::ID;
use crate::layer::Layers;
use crate::options::Options;
use crate::render::{
    AgentCache, AgentColorScheme, DrawMap, DrawOptions, Renderable, MIN_ZOOM_FOR_DETAIL,
};
use crate::sandbox::{GameplayMode, TutorialState};
use abstutil::{MeasureMemory, Timer};
use ezgui::{EventCtx, GfxCtx, Prerender};
use geom::{Bounds, Circle, Distance, Pt2D};
use map_model::{Map, Traversable};
use rand::seq::SliceRandom;
use sim::{Analytics, GetDrawAgents, Sim, SimFlags};
use std::collections::HashMap;

pub struct App {
    pub primary: PerMap,
    // Invariant: This is Some(...) iff we're in A/B test mode or a sub-state.
    pub secondary: Option<PerMap>,
    // Only exists in some gameplay modes. Must be carefully reset otherwise. Has the map and
    // scenario name too. TODO Embed that in Analytics directly instead.
    prebaked: Option<(String, String, Analytics)>,
    pub cs: ColorScheme,
    // TODO This is a bit weird to keep here; it's controlled almost entirely by the minimap panel.
    // It has no meaning in edit mode.
    pub agent_cs: AgentColorScheme,
    pub opts: Options,

    pub per_obj: PerObjectActions,
    pub layer: Layers,

    // Static data that lasts the entire session. Use sparingly.
    pub session: SessionState,
}

impl App {
    pub fn new(flags: Flags, opts: Options, ctx: &mut EventCtx, splash: bool) -> App {
        let cs = ColorScheme::new(opts.color_scheme);
        ctx.set_style(cs.gui_style.clone());

        let primary = ctx.loading_screen("load map", |ctx, mut timer| {
            PerMap::new(flags, &cs, ctx, &mut timer)
        });

        let mut rng = primary.current_flags.sim_flags.make_rng();
        let rand_focus_pt = primary
            .map
            .all_buildings()
            .choose(&mut rng)
            .and_then(|b| ID::Building(b.id).canonical_point(&primary))
            .or_else(|| {
                primary
                    .map
                    .all_lanes()
                    .choose(&mut rng)
                    .and_then(|l| ID::Lane(l.id).canonical_point(&primary))
            })
            .expect("Can't get canonical_point of a random building or lane");
        let bounds = primary.map.get_bounds();
        ctx.canvas.map_dims = (bounds.width(), bounds.height());

        if splash {
            ctx.canvas.center_on_map_pt(rand_focus_pt);
        } else {
            if !ctx.canvas.load_camera_state(primary.map.get_name()) {
                println!("Couldn't load camera state, just focusing on an arbitrary building");
                ctx.canvas.center_on_map_pt(rand_focus_pt);
            }
        }

        App {
            primary,
            secondary: None,
            prebaked: None,
            agent_cs: AgentColorScheme::new(&cs),
            cs,
            opts,
            per_obj: PerObjectActions::new(),
            layer: Layers::Inactive,
            session: SessionState::empty(),
        }
    }

    pub fn has_prebaked(&self) -> Option<(&String, &String)> {
        self.prebaked.as_ref().map(|(m, s, _)| (m, s))
    }
    pub fn prebaked(&self) -> &Analytics {
        &self.prebaked.as_ref().unwrap().2
    }
    pub fn set_prebaked(&mut self, prebaked: Option<(String, String, Analytics)>) {
        self.prebaked = prebaked;
    }

    pub fn switch_map(&mut self, ctx: &mut EventCtx, load: String) {
        ctx.canvas.save_camera_state(self.primary.map.get_name());
        let mut flags = self.primary.current_flags.clone();
        flags.sim_flags.load = load;
        let session = std::mem::replace(&mut self.session, SessionState::empty());
        *self = App::new(flags, self.opts.clone(), ctx, false);
        self.session = session;
    }

    pub fn draw(
        &self,
        g: &mut GfxCtx,
        opts: DrawOptions,
        source: &dyn GetDrawAgents,
        show_objs: &dyn ShowObject,
    ) {
        let mut sample_intersection: Option<String> = None;

        g.clear(self.cs.void_background);
        g.redraw(&self.primary.draw_map.boundary_polygon);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL && !g.is_screencap() {
            // Unzoomed mode
            let layers = show_objs.layers();
            if layers.show_areas {
                g.redraw(&self.primary.draw_map.draw_all_areas);
            }
            if layers.show_lanes {
                g.redraw(&self.primary.draw_map.draw_all_thick_roads);
            }
            if layers.show_intersections {
                g.redraw(&self.primary.draw_map.draw_all_unzoomed_intersections);
            }
            if layers.show_buildings {
                g.redraw(&self.primary.draw_map.draw_all_buildings);
                // Not the building paths
            }

            if layers.show_extra_shapes {
                for es in &self.primary.draw_map.extra_shapes {
                    if show_objs.show(&es.get_id()) {
                        es.draw(g, self, &opts);
                    }
                }
            }

            // Still show some shape selection when zoomed out.
            // TODO Refactor! Ideally use get_obj
            if let Some(ID::Area(id)) = self.primary.current_selection {
                g.draw_polygon(
                    self.cs.selected,
                    &self
                        .primary
                        .draw_map
                        .get_a(id)
                        .get_outline(&self.primary.map),
                );
            } else if let Some(ID::ExtraShape(id)) = self.primary.current_selection {
                g.draw_polygon(
                    self.cs.selected,
                    &self
                        .primary
                        .draw_map
                        .get_es(id)
                        .get_outline(&self.primary.map),
                );
            } else if let Some(ID::Road(id)) = self.primary.current_selection {
                g.draw_polygon(
                    self.cs.selected,
                    &self
                        .primary
                        .draw_map
                        .get_r(id)
                        .get_outline(&self.primary.map),
                );
            } else if let Some(ID::Intersection(id)) = self.primary.current_selection {
                // Actually, don't use get_outline here! Full polygon is easier to see.
                g.draw_polygon(self.cs.selected, &self.primary.map.get_i(id).polygon);
            }

            let mut cache = self.primary.draw_map.agents.borrow_mut();
            cache.draw_unzoomed_agents(
                source,
                &self.primary.map,
                &self.agent_cs,
                g,
                Distance::meters(10.0) / g.canvas.cam_zoom,
            );
        } else {
            let mut cache = self.primary.draw_map.agents.borrow_mut();
            let objects = self.get_renderables_back_to_front(
                g.get_screen_bounds(),
                &g.prerender,
                &mut cache,
                source,
                show_objs,
            );

            let mut drawn_all_buildings = false;
            let mut drawn_all_areas = false;

            for obj in objects {
                obj.draw(g, self, &opts);

                match obj.get_id() {
                    ID::Building(_) => {
                        if !drawn_all_buildings {
                            g.redraw(&self.primary.draw_map.draw_all_buildings);
                            g.redraw(&self.primary.draw_map.draw_all_building_paths);
                            drawn_all_buildings = true;
                        }
                    }
                    ID::Area(_) => {
                        if !drawn_all_areas {
                            g.redraw(&self.primary.draw_map.draw_all_areas);
                            drawn_all_areas = true;
                        }
                    }
                    _ => {}
                };

                if self.primary.current_selection == Some(obj.get_id()) {
                    g.draw_polygon(self.cs.selected, &obj.get_outline(&self.primary.map));
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

    // Assumes some defaults.
    pub fn recalculate_current_selection(&mut self, ctx: &EventCtx) {
        self.primary.current_selection = self.calculate_current_selection(
            ctx,
            &self.primary.sim,
            &ShowEverything::new(),
            false,
            false,
        );
    }

    // Because we have to sometimes borrow part of self for GetDrawAgents, this just returns the
    // Option<ID> that the caller should assign. When this monolithic UI nonsense is dismantled,
    // this weirdness goes away.
    pub fn calculate_current_selection(
        &self,
        ctx: &EventCtx,
        source: &dyn GetDrawAgents,
        show_objs: &dyn ShowObject,
        debug_mode: bool,
        unzoomed_roads_and_intersections: bool,
    ) -> Option<ID> {
        // Unzoomed mode. Ignore when debugging areas and extra shapes.
        if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL
            && !(debug_mode || unzoomed_roads_and_intersections)
        {
            return None;
        }

        let pt = ctx.canvas.get_cursor_in_map_space()?;

        let mut cache = self.primary.draw_map.agents.borrow_mut();
        let mut objects = self.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            ctx.prerender,
            &mut cache,
            source,
            show_objs,
        );
        objects.reverse();

        for obj in objects {
            match obj.get_id() {
                ID::Area(_) | ID::ExtraShape(_) => {
                    if !debug_mode {
                        continue;
                    }
                }
                ID::Road(_) => {
                    if !unzoomed_roads_and_intersections
                        || ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL
                    {
                        continue;
                    }
                }
                ID::Intersection(_) => {
                    if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL
                        && !unzoomed_roads_and_intersections
                    {
                        continue;
                    }
                }
                _ => {
                    if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
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
        source: &dyn GetDrawAgents,
        show_objs: &dyn ShowObject,
    ) -> Vec<&'a (dyn Renderable + 'a)> {
        let map = &self.primary.map;
        let draw_map = &self.primary.draw_map;

        let mut areas: Vec<&dyn Renderable> = Vec::new();
        let mut lanes: Vec<&dyn Renderable> = Vec::new();
        let mut roads: Vec<&dyn Renderable> = Vec::new();
        let mut intersections: Vec<&dyn Renderable> = Vec::new();
        let mut buildings: Vec<&dyn Renderable> = Vec::new();
        let mut extra_shapes: Vec<&dyn Renderable> = Vec::new();
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
                        bus_stops.push(draw_map.get_bs(*bs));
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
                // TODO front paths will get drawn over buildings, depending on quadtree order.
                // probably just need to make them go around other buildings instead of having
                // two passes through buildings.
                ID::Building(id) => buildings.push(draw_map.get_b(id)),
                ID::ExtraShape(id) => extra_shapes.push(draw_map.get_es(id)),

                ID::BusStop(_) | ID::Turn(_) | ID::Car(_) | ID::Pedestrian(_) | ID::PedCrowd(_) => {
                    panic!("{:?} shouldn't be in the quadtree", id)
                }
            }
        }

        // From background to foreground Z-order
        let mut borrows: Vec<&dyn Renderable> = Vec::new();
        borrows.extend(areas);
        borrows.extend(lanes);
        borrows.extend(roads);
        borrows.extend(intersections);
        borrows.extend(buildings);
        borrows.extend(extra_shapes);
        borrows.extend(bus_stops);

        // Expand all of the Traversables into agents, populating the cache if needed.
        {
            for on in &agents_on {
                agents.populate_if_needed(*on, map, source, &self.cs, prerender);
            }
        }

        for on in agents_on {
            for obj in agents.get(on) {
                borrows.push(obj);
            }
        }

        // This is a stable sort.
        borrows.sort_by_key(|x| x.get_zorder());

        borrows
    }
}

pub struct ShowLayers {
    pub show_buildings: bool,
    pub show_intersections: bool,
    pub show_lanes: bool,
    pub show_areas: bool,
    pub show_extra_shapes: bool,
    pub show_labels: bool,
}

impl ShowLayers {
    pub fn new() -> ShowLayers {
        ShowLayers {
            show_buildings: true,
            show_intersections: true,
            show_lanes: true,
            show_areas: true,
            show_extra_shapes: true,
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
    pub kml: Option<String>,
    pub draw_lane_markings: bool,
    // Number of agents to generate when requested. If unspecified, trips to/from borders will be
    // included.
    pub num_agents: Option<usize>,
}

// All of the state that's bound to a specific map+edit has to live here.
pub struct PerMap {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub current_flags: Flags,
    pub last_warped_from: Option<(Pt2D, f64)>,
}

impl PerMap {
    pub fn new(flags: Flags, cs: &ColorScheme, ctx: &mut EventCtx, timer: &mut Timer) -> PerMap {
        let mut mem = MeasureMemory::new();
        let (map, sim, _) = flags.sim_flags.load(timer);
        mem.reset("Map and Sim", timer);

        timer.start("draw_map");
        let draw_map = DrawMap::new(&map, &flags, cs, ctx, timer);
        timer.stop("draw_map");
        mem.reset("DrawMap", timer);

        PerMap {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags.clone(),
            last_warped_from: None,
        }
    }

    // Returns whatever was there
    pub fn clear_sim(&mut self) -> Sim {
        std::mem::replace(
            &mut self.sim,
            Sim::new(
                &self.map,
                self.current_flags.sim_flags.opts.clone(),
                &mut Timer::new("reset simulation"),
            ),
        )
    }
}

// TODO Serialize these, but in a very careful, future-compatible way
pub struct SessionState {
    pub tutorial: Option<TutorialState>,
    pub high_scores: HashMap<GameplayMode, Vec<HighScore>>,
}

impl SessionState {
    pub fn empty() -> SessionState {
        SessionState {
            tutorial: None,
            high_scores: HashMap::new(),
        }
    }
}

// TODO Reconsider this; maybe it does belong in ezgui.
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
