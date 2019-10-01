use crate::helpers::{ColorScheme, ID};
use crate::render::{
    draw_vehicle, AgentCache, AgentColorScheme, DrawCtx, DrawMap, DrawOptions, DrawPedCrowd,
    DrawPedestrian, Renderable, MIN_ZOOM_FOR_DETAIL,
};
use abstutil;
use abstutil::{MeasureMemory, Timer};
use ezgui::{Canvas, Color, EventCtx, GfxCtx, Prerender};
use geom::{Bounds, Circle, Distance, Pt2D};
use map_model::{Map, Traversable};
use rand::seq::SliceRandom;
use serde_derive::{Deserialize, Serialize};
use sim::{GetDrawAgents, Sim, SimFlags, SimOptions};

pub struct UI {
    pub primary: PerMapUI,
    // Invariant: This is Some(...) iff we're in A/B test mode or a sub-state.
    pub secondary: Option<PerMapUI>,
    pub cs: ColorScheme,
    pub agent_cs: AgentColorScheme,
}

impl UI {
    pub fn new(flags: Flags, ctx: &mut EventCtx, splash: bool) -> UI {
        let cs = ColorScheme::load().unwrap();
        let primary = ctx.loading_screen("load map", |ctx, mut timer| {
            ctx.set_textures(
                flags.textures,
                vec![
                    ("assets/water_texture.png", Color::rgb(170, 211, 223)),
                    ("assets/grass_texture.png", Color::rgb(200, 250, 204)),
                ],
                &mut timer,
            );

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
            let path = abstutil::path_camera_state(primary.map.get_name());
            match abstutil::read_json::<CameraState>(&path, &mut Timer::throwaway()) {
                Ok(ref loaded) => {
                    println!("Loaded {}", path);
                    ctx.canvas.cam_x = loaded.cam_x;
                    ctx.canvas.cam_y = loaded.cam_y;
                    ctx.canvas.cam_zoom = loaded.cam_zoom;
                }
                _ => {
                    println!(
                        "Couldn't load {}, so just focusing on an arbitrary building",
                        path
                    );
                    ctx.canvas.center_on_map_pt(rand_focus_pt);
                }
            }
        }

        UI {
            primary,
            secondary: None,
            cs,
            agent_cs: AgentColorScheme::VehicleTypes,
        }
    }

    pub fn draw(
        &self,
        g: &mut GfxCtx,
        opts: DrawOptions,
        source: &dyn GetDrawAgents,
        show_objs: &dyn ShowObject,
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
                    if show_objs.show(&es.get_id()) {
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

            let mut cache = self.primary.draw_map.agents.borrow_mut();
            cache.draw_unzoomed_agents(&self.primary, self.agent_cs, &self.cs, g);
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
                obj.draw(g, &opts, &ctx);

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

    // Assumes some defaults.
    pub fn recalculate_current_selection(&mut self, ctx: &EventCtx) {
        self.primary.current_selection =
            self.calculate_current_selection(ctx, &self.primary.sim, &ShowEverything::new(), false);
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

                ID::BusStop(_)
                | ID::Turn(_)
                | ID::Car(_)
                | ID::Pedestrian(_)
                | ID::PedCrowd(_)
                | ID::Trip(_) => panic!("{:?} shouldn't be in the quadtree", id),
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
                    let mut list: Vec<Box<dyn Renderable>> = Vec::new();
                    for c in source.get_draw_cars(*on, map).into_iter() {
                        list.push(draw_vehicle(c, map, prerender, &self.cs, self.agent_cs));
                    }
                    let (loners, crowds) = source.get_draw_peds(*on, map);
                    for p in loners {
                        list.push(Box::new(DrawPedestrian::new(
                            p,
                            step_count,
                            map,
                            prerender,
                            &self.cs,
                            self.agent_cs,
                        )));
                    }
                    for c in crowds {
                        list.push(Box::new(DrawPedCrowd::new(c, map, prerender, &self.cs)));
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

    pub fn save_camera_state(&self, canvas: &Canvas) {
        let state = CameraState {
            map_name: self.primary.map.get_name().clone(),
            cam_x: canvas.cam_x,
            cam_y: canvas.cam_y,
            cam_zoom: canvas.cam_zoom,
        };
        let path = abstutil::path_camera_state(&state.map_name);
        abstutil::write_json(&path, &state).unwrap();
        println!("Saved {}", path);
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct CameraState {
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
            geom_debug_mode: false,
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
    pub enable_profiler: bool,
    // Number of agents to generate when requested. If unspecified, trips to/from borders will be
    // included.
    pub num_agents: Option<usize>,
    pub splash: bool,
    pub textures: bool,
}

// All of the state that's bound to a specific map+edit has to live here.
pub struct PerMapUI {
    pub map: Map,
    pub draw_map: DrawMap,
    pub sim: Sim,

    pub current_selection: Option<ID>,
    pub current_flags: Flags,
    pub last_warped_from: Option<(Pt2D, f64)>,
}

impl PerMapUI {
    pub fn new(flags: Flags, cs: &ColorScheme, ctx: &mut EventCtx, timer: &mut Timer) -> PerMapUI {
        let mut mem = MeasureMemory::new();
        let (map, sim, _) = flags.sim_flags.load(timer);
        mem.reset("Map and Sim", timer);

        timer.start("draw_map");
        let draw_map = DrawMap::new(&map, &flags, cs, ctx, timer);
        timer.stop("draw_map");
        mem.reset("DrawMap", timer);

        PerMapUI {
            map,
            draw_map,
            sim,
            current_selection: None,
            current_flags: flags.clone(),
            last_warped_from: None,
        }
    }

    pub fn reset_sim(&mut self) {
        let flags = &self.current_flags.sim_flags;

        self.sim = Sim::new(
            &self.map,
            SimOptions {
                run_name: flags
                    .run_name
                    .clone()
                    .unwrap_or_else(|| "unnamed".to_string()),
                savestate_every: flags.savestate_every,
                use_freeform_policy_everywhere: flags.freeform_policy,
                disable_block_the_box: flags.disable_block_the_box,
                record_stats: flags.record_stats,
            },
        );
    }
}
