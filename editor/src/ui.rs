use crate::objects::{DrawCtx, RenderingHints, ID};
use crate::render::{
    draw_vehicle, AgentCache, DrawPedestrian, RenderOptions, Renderable, MIN_ZOOM_FOR_DETAIL,
};
use crate::state::UIState;
use abstutil;
use ezgui::{Canvas, Color, EventCtx, EventLoopMode, GfxCtx, Prerender, Text, BOTTOM_LEFT};
use geom::{Bounds, Circle, Distance, Polygon};
use map_model::{BuildingID, IntersectionID, LaneID, Traversable};
use serde_derive::{Deserialize, Serialize};
use sim::GetDrawAgents;
use std::collections::{HashMap, HashSet};

// TODO Collapse stuff!
pub struct UI {
    pub hints: RenderingHints,
    pub state: UIState,
}

impl UI {
    pub fn new(state: UIState, canvas: &mut Canvas) -> UI {
        match abstutil::read_json::<EditorState>("../editor_state") {
            Ok(ref loaded) if state.primary.map.get_name() == &loaded.map_name => {
                println!("Loaded previous editor_state");
                canvas.cam_x = loaded.cam_x;
                canvas.cam_y = loaded.cam_y;
                canvas.cam_zoom = loaded.cam_zoom;
            }
            _ => {
                println!("Couldn't load editor_state or it's for a different map, so just focusing on an arbitrary building");
                let focus_pt = ID::Building(BuildingID(0))
                    .canonical_point(
                        &state.primary.map,
                        &state.primary.sim,
                        &state.primary.draw_map,
                    )
                    .or_else(|| {
                        ID::Lane(LaneID(0)).canonical_point(
                            &state.primary.map,
                            &state.primary.sim,
                            &state.primary.draw_map,
                        )
                    })
                    .expect("Can't get canonical_point of BuildingID(0) or Road(0)");
                canvas.center_on_map_pt(focus_pt);
            }
        }

        UI {
            state,
            hints: RenderingHints {
                mode: EventLoopMode::InputOnly,
                osd: Text::new(),
                suppress_traffic_signal_details: None,
                hide_turn_icons: HashSet::new(),
            },
        }
    }

    pub fn new_draw(
        &self,
        g: &mut GfxCtx,
        show_turn_icons_for: Option<IntersectionID>,
        override_color: HashMap<ID, Color>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
    ) {
        let ctx = DrawCtx {
            cs: &self.state.cs,
            map: &self.state.primary.map,
            draw_map: &self.state.primary.draw_map,
            sim: &self.state.primary.sim,
            hints: &self.hints,
        };
        let mut sample_intersection: Option<String> = None;

        g.clear(self.state.cs.get_def("true background", Color::BLACK));
        g.redraw(&self.state.primary.draw_map.boundary_polygon);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL && !g.is_screencap() {
            // Unzoomed mode
            let layers = show_objs.layers();
            if layers.show_areas {
                g.redraw(&self.state.primary.draw_map.draw_all_areas);
            }
            if layers.show_lanes {
                g.redraw(&self.state.primary.draw_map.draw_all_thick_roads);
            }
            if layers.show_intersections {
                g.redraw(&self.state.primary.draw_map.draw_all_unzoomed_intersections);
            }
            if layers.show_buildings {
                g.redraw(&self.state.primary.draw_map.draw_all_buildings);
            }

            // Still show area selection when zoomed out.
            if let Some(ID::Area(id)) = self.state.primary.current_selection {
                g.draw_polygon(
                    self.state.cs.get("selected"),
                    &fill_to_boundary_polygon(ctx.draw_map.get_a(id).get_outline(&ctx.map)),
                );
            }

            self.state
                .primary
                .sim
                .draw_unzoomed(g, &self.state.primary.map);
        } else {
            let mut cache = self.state.primary.draw_map.agents.borrow_mut();
            let objects = self.get_renderables_back_to_front(
                g.get_screen_bounds(),
                &g.prerender,
                &mut cache,
                show_turn_icons_for,
                source,
                show_objs,
            );

            let mut drawn_all_buildings = false;
            let mut drawn_all_areas = false;

            for obj in objects {
                match obj.get_id() {
                    ID::Building(_) => {
                        if !drawn_all_buildings {
                            g.redraw(&self.state.primary.draw_map.draw_all_buildings);
                            drawn_all_buildings = true;
                        }
                    }
                    ID::Area(_) => {
                        if !drawn_all_areas {
                            g.redraw(&self.state.primary.draw_map.draw_all_areas);
                            drawn_all_areas = true;
                        }
                    }
                    _ => {}
                };
                let opts = RenderOptions {
                    color: override_color.get(&obj.get_id()).cloned(),
                    debug_mode: show_objs.layers().geom_debug_mode,
                };
                obj.draw(g, opts, &ctx);

                if self.state.primary.current_selection == Some(obj.get_id()) {
                    g.draw_polygon(
                        self.state.cs.get_def("selected", Color::YELLOW.alpha(0.4)),
                        &fill_to_boundary_polygon(obj.get_outline(&ctx.map)),
                    );
                }

                if g.is_screencap() && sample_intersection.is_none() {
                    if let ID::Intersection(id) = obj.get_id() {
                        sample_intersection = Some(format!("_i{}", id.0));
                    }
                }
            }
        }

        if !g.is_screencap() {
            // Not happy about cloning, but probably will make the OSD a first-class ezgui concept
            // soon, so meh
            let mut osd = self.hints.osd.clone();
            // TODO Only in some kind of debug mode
            osd.add_line(format!(
                "{} things uploaded, {} things drawn",
                abstutil::prettyprint_usize(g.get_num_uploads()),
                abstutil::prettyprint_usize(g.num_draw_calls),
            ));
            g.draw_blocking_text(&osd, BOTTOM_LEFT);
        }

        if let Some(i) = sample_intersection {
            g.set_screencap_naming_hint(i);
        }
    }

    // Because we have to sometimes borrow part of self for GetDrawAgents, this just returns the
    // Option<ID> that the caller should assign. When this monolithic UI nonsense is dismantled,
    // this weirdness goes away.
    pub fn handle_mouseover(
        &self,
        ctx: &mut EventCtx,
        show_turn_icons_for: Option<IntersectionID>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
        debug_areas: bool,
    ) -> Option<ID> {
        if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
            return self.mouseover_something(
                &ctx,
                show_turn_icons_for,
                source,
                show_objs,
                debug_areas,
            );
        }
        if ctx.input.window_lost_cursor() {
            return None;
        }
        self.state.primary.current_selection
    }

    fn mouseover_something(
        &self,
        ctx: &EventCtx,
        show_turn_icons_for: Option<IntersectionID>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
        debug_areas: bool,
    ) -> Option<ID> {
        // Unzoomed mode. Ignore when debugging areas.
        if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL && !debug_areas {
            return None;
        }

        let pt = ctx.canvas.get_cursor_in_map_space()?;

        let mut cache = self.state.primary.draw_map.agents.borrow_mut();
        let mut objects = self.get_renderables_back_to_front(
            Circle::new(pt, Distance::meters(3.0)).get_bounds(),
            ctx.prerender,
            &mut cache,
            show_turn_icons_for,
            source,
            show_objs,
        );
        objects.reverse();

        for obj in objects {
            // Don't mouseover areas.
            // TODO Might get fancier rules in the future, so we can't mouseover irrelevant things
            // in intersection editor mode, for example.
            match obj.get_id() {
                ID::Area(_) if !debug_areas => {}
                // Thick roads are only shown when unzoomed, when we don't mouseover at all.
                ID::Road(_) => {}
                _ => {
                    if obj.get_outline(&self.state.primary.map).contains_pt(pt) {
                        return Some(obj.get_id());
                    }
                }
            };
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
        show_turn_icons_for: Option<IntersectionID>,
        source: &GetDrawAgents,
        show_objs: &ShowObject,
    ) -> Vec<Box<&'a Renderable>> {
        let map = &self.state.primary.map;
        let draw_map = &self.state.primary.draw_map;

        let mut areas: Vec<Box<&Renderable>> = Vec::new();
        let mut lanes: Vec<Box<&Renderable>> = Vec::new();
        let mut roads: Vec<Box<&Renderable>> = Vec::new();
        let mut intersections: Vec<Box<&Renderable>> = Vec::new();
        let mut buildings: Vec<Box<&Renderable>> = Vec::new();
        let mut extra_shapes: Vec<Box<&Renderable>> = Vec::new();
        let mut bus_stops: Vec<Box<&Renderable>> = Vec::new();
        let mut turn_icons: Vec<Box<&Renderable>> = Vec::new();
        let mut agents_on: Vec<Traversable> = Vec::new();

        for id in draw_map.get_matching_objects(bounds) {
            if !show_objs.show(id) {
                continue;
            }
            match id {
                ID::Area(id) => areas.push(Box::new(draw_map.get_a(id))),
                ID::Lane(id) => {
                    lanes.push(Box::new(draw_map.get_l(id)));
                    let lane = map.get_l(id);
                    if show_turn_icons_for == Some(lane.dst_i) {
                        for (t, _) in map.get_next_turns_and_lanes(id, lane.dst_i) {
                            turn_icons.push(Box::new(draw_map.get_t(t.id)));
                        }
                    } else {
                        // TODO Bug: pedestrians on front paths aren't selectable.
                        agents_on.push(Traversable::Lane(id));
                    }
                    for bs in &lane.bus_stops {
                        bus_stops.push(Box::new(draw_map.get_bs(*bs)));
                    }
                }
                ID::Road(id) => {
                    roads.push(Box::new(draw_map.get_r(id)));
                }
                ID::Intersection(id) => {
                    intersections.push(Box::new(draw_map.get_i(id)));
                    for t in &map.get_i(id).turns {
                        if show_turn_icons_for != Some(id) {
                            agents_on.push(Traversable::Turn(*t));
                        }
                    }
                }
                // TODO front paths will get drawn over buildings, depending on quadtree order.
                // probably just need to make them go around other buildings instead of having
                // two passes through buildings.
                ID::Building(id) => buildings.push(Box::new(draw_map.get_b(id))),
                ID::ExtraShape(id) => extra_shapes.push(Box::new(draw_map.get_es(id))),

                ID::BusStop(_) | ID::Turn(_) | ID::Car(_) | ID::Pedestrian(_) | ID::Trip(_) => {
                    panic!("{:?} shouldn't be in the quadtree", id)
                }
            }
        }

        // From background to foreground Z-order
        let mut borrows: Vec<Box<&Renderable>> = Vec::new();
        borrows.extend(areas);
        borrows.extend(lanes);
        borrows.extend(roads);
        borrows.extend(intersections);
        borrows.extend(buildings);
        borrows.extend(extra_shapes);
        borrows.extend(bus_stops);
        borrows.extend(turn_icons);

        // Expand all of the Traversables into agents, populating the cache if needed.
        {
            let time = source.time();

            for on in &agents_on {
                if !agents.has(time, *on) {
                    let mut list: Vec<Box<Renderable>> = Vec::new();
                    for c in source.get_draw_cars(*on, map).into_iter() {
                        list.push(draw_vehicle(c, map, prerender, &self.state.cs));
                    }
                    for p in source.get_draw_peds(*on, map).into_iter() {
                        list.push(Box::new(DrawPedestrian::new(
                            p,
                            map,
                            prerender,
                            &self.state.cs,
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

fn fill_to_boundary_polygon(poly: Polygon) -> Polygon {
    // TODO This looks awful for lanes, oops.
    //PolyLine::make_polygons_for_boundary(poly.points().clone(), Distance::meters(0.5))
    poly
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
