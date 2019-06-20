use crate::helpers::{ColorScheme, ID};
use crate::render::{
    draw_vehicle, AgentCache, DrawCtx, DrawMap, DrawOptions, DrawPedestrian, Renderable,
    MIN_ZOOM_FOR_DETAIL,
};
use abstutil;
use abstutil::{MeasureMemory, Timer};
use ezgui::{Color, EventCtx, GeomBatch, GfxCtx, Prerender};
use geom::{Bounds, Circle, Distance, Duration};
use map_model::{Map, Traversable};
use rand::seq::SliceRandom;
use serde_derive::{Deserialize, Serialize};
use sim::{GetDrawAgents, Sim, SimFlags};
use structopt::StructOpt;

// TODO Collapse stuff!
pub struct UI {
    pub primary: PerMapUI,
    pub cs: ColorScheme,
}

impl UI {
    pub fn new(flags: Flags, ctx: &mut EventCtx, splash: bool) -> UI {
        let cs = ColorScheme::load().unwrap();
        let primary = ctx.loading_screen("load map", |ctx, mut timer| {
            PerMapUI::new(flags, &cs, ctx, &mut timer)
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

        if splash {
            ctx.canvas.center_on_map_pt(rand_focus_pt);
        } else {
            match abstutil::read_json::<EditorState>("../editor_state.json") {
                Ok(ref loaded) if primary.map.get_name() == &loaded.map_name => {
                    println!("Loaded previous editor_state.json");
                    ctx.canvas.cam_x = loaded.cam_x;
                    ctx.canvas.cam_y = loaded.cam_y;
                    ctx.canvas.cam_zoom = loaded.cam_zoom;
                }
                _ => {
                    println!("Couldn't load editor_state.json or it's for a different map, so just focusing on an arbitrary building");
                    ctx.canvas.center_on_map_pt(rand_focus_pt);
                }
            }
        }

        UI { primary, cs }
    }

    pub fn draw(
        &self,
        g: &mut GfxCtx,
        opts: DrawOptions,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
    ) {
        let ctx = DrawCtx {
            cs: &self.cs,
            map: &self.primary.map,
            draw_map: &self.primary.draw_map,
            sim: &self.primary.sim,
        };
        let mut sample_intersection: Option<String> = None;

        g.clear(self.cs.get_def("true background", Color::BLACK));
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
            }

            if layers.show_extra_shapes {
                for es in &self.primary.draw_map.extra_shapes {
                    if show_objs.show(es.get_id()) {
                        es.draw(g, &opts, &ctx);
                    }
                }
            }

            // Still show area/extra shape selection when zoomed out.
            if let Some(ID::Area(id)) = self.primary.current_selection {
                g.draw_polygon(
                    self.cs.get("selected"),
                    &ctx.draw_map.get_a(id).get_outline(&ctx.map),
                );
            } else if let Some(ID::ExtraShape(id)) = self.primary.current_selection {
                g.draw_polygon(
                    self.cs.get("selected"),
                    &ctx.draw_map.get_es(id).get_outline(&ctx.map),
                );
            }

            let (cars, bikes, buses, peds) =
                self.primary.sim.get_unzoomed_agents(&self.primary.map);
            let mut batch = GeomBatch::new();
            let radius = Distance::meters(10.0) / g.canvas.cam_zoom;
            for (color, agents) in vec![
                (self.cs.get_def("unzoomed car", Color::RED.alpha(0.5)), cars),
                (
                    self.cs.get_def("unzoomed bike", Color::GREEN.alpha(0.5)),
                    bikes,
                ),
                (
                    self.cs.get_def("unzoomed bus", Color::BLUE.alpha(0.5)),
                    buses,
                ),
                (
                    self.cs
                        .get_def("unzoomed pedestrian", Color::ORANGE.alpha(0.5)),
                    peds,
                ),
            ] {
                for pt in agents {
                    batch.push(color, Circle::new(pt, radius).to_polygon());
                }
            }
            batch.draw(g);
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
                match obj.get_id() {
                    ID::Building(_) => {
                        if !drawn_all_buildings {
                            g.redraw(&self.primary.draw_map.draw_all_buildings);
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
                obj.draw(g, &opts, &ctx);

                if self.primary.current_selection == Some(obj.get_id()) {
                    g.draw_polygon(
                        self.cs.get_def("selected", Color::RED.alpha(0.7)),
                        &obj.get_outline(&ctx.map),
                    );
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

    // Because we have to sometimes borrow part of self for GetDrawAgents, this just returns the
    // Option<ID> that the caller should assign. When this monolithic UI nonsense is dismantled,
    // this weirdness goes away.
    pub fn recalculate_current_selection(
        &self,
        ctx: &EventCtx,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
        debug_mode: bool,
    ) -> Option<ID> {
        // Unzoomed mode. Ignore when debugging areas and extra shapes.
        if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL && !debug_mode {
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
            // In unzoomed mode, can only mouseover areas
            match obj.get_id() {
                ID::Area(_) | ID::ExtraShape(_) => {
                    if !debug_mode {
                        continue;
                    }
                }
                // Never mouseover these
                ID::Road(_) => {
                    continue;
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
        source: &GetDrawAgents,
        show_objs: &ShowObject,
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
            if !show_objs.show(id) {
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

                ID::BusStop(_) | ID::Turn(_) | ID::Car(_) | ID::Pedestrian(_) | ID::Trip(_) => {
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
            let time = source.time();
            let step_count = source.step_count();

            for on in &agents_on {
                if !agents.has(time, *on) {
                    let mut list: Vec<Box<Renderable>> = Vec::new();
                    for c in source.get_draw_cars(*on, map).into_iter() {
                        list.push(draw_vehicle(c, map, prerender, &self.cs));
                    }
                    for p in source.get_draw_peds(*on, map).into_iter() {
                        list.push(Box::new(DrawPedestrian::new(
                            p, step_count, map, prerender, &self.cs,
                        )));
                    }
                    agents.put(time, *on, list);
                }
            }
        }

        for on in agents_on {
            for obj in agents.get(on) {
                borrows.push(obj);
            }
        }

        // This is a stable sort.
        borrows.sort_by_key(|r| r.get_zorder());

        borrows
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EditorState {
    pub map_name: String,
    pub cam_x: f64,
    pub cam_y: f64,
    pub cam_zoom: f64,
}

pub struct ShowLayers {
    pub show_buildings: bool,
    pub show_intersections: bool,
    pub show_lanes: bool,
    pub show_areas: bool,
    pub show_extra_shapes: bool,
    pub geom_debug_mode: bool,
}

impl ShowLayers {
    pub fn new() -> ShowLayers {
        ShowLayers {
            show_buildings: true,
            show_intersections: true,
            show_lanes: true,
            show_areas: true,
            show_extra_shapes: true,
            geom_debug_mode: false,
        }
    }
}

pub trait ShowObject {
    fn show(&self, obj: ID) -> bool;
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
    fn show(&self, _: ID) -> bool {
        true
    }

    fn layers(&self) -> &ShowLayers {
        &self.layers
    }
}

#[derive(StructOpt, Debug, Clone)]
#[structopt(name = "editor")]
pub struct Flags {
    #[structopt(flatten)]
    pub sim_flags: SimFlags,

    /// Extra KML or ExtraShapes to display
    #[structopt(long = "kml")]
    pub kml: Option<String>,

    // TODO Ideally these'd be phrased positively, but can't easily make them default to true.
    /// Should lane markings be drawn? Sometimes they eat too much GPU memory.
    #[structopt(long = "dont_draw_lane_markings")]
    pub dont_draw_lane_markings: bool,

    /// Enable cpuprofiler?
    #[structopt(long = "enable_profiler")]
    pub enable_profiler: bool,

    /// Number of agents to generate when requested. If unspecified, trips to/from borders will be
    /// included.
    #[structopt(long = "num_agents")]
    pub num_agents: Option<usize>,

    /// Don't start with the splash screen and menu
    #[structopt(long = "no_splash")]
    pub no_splash: bool,
}

// All of the state that's bound to a specific map+edit has to live here.
pub struct PerMapUI {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub current_flags: Flags,
}

impl PerMapUI {
    pub fn new(flags: Flags, cs: &ColorScheme, ctx: &mut EventCtx, timer: &mut Timer) -> PerMapUI {
        let mut mem = MeasureMemory::new();
        let (map, sim, _) = flags.sim_flags.load(Some(Duration::minutes(30)), timer);
        mem.reset("Map and Sim", timer);

        timer.start("draw_map");
        let draw_map = DrawMap::new(&map, &flags, cs, ctx.prerender, timer);
        timer.stop("draw_map");
        mem.reset("DrawMap", timer);

        PerMapUI {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags.clone(),
        }
    }

    pub fn reset_sim(&mut self) {
        // TODO savestate_every gets lost
        self.sim = Sim::new(
            &self.map,
            self.current_flags
                .sim_flags
                .run_name
                .clone()
                .unwrap_or_else(|| "unnamed".to_string()),
            None,
        );
    }
}
