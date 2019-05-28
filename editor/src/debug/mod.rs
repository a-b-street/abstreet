mod chokepoints;
mod color_picker;
mod connected_roads;
mod neighborhood_summary;
mod objects;
mod polygons;

use crate::common::CommonState;
use crate::game::{GameState, Mode};
use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::{ShowLayers, ShowObject, UI};
use abstutil::wraparound_get;
use abstutil::Timer;
use clipping::CPolygon;
use ezgui::{
    Color, EventCtx, EventLoopMode, GfxCtx, InputResult, Key, ModalMenu, ScrollingMenu, Text,
    TextBox, Wizard,
};
use geom::{Distance, PolyLine, Polygon, Pt2D};
use map_model::{IntersectionID, Map, RoadID};
use std::collections::HashSet;

pub struct DebugMode {
    state: State,
    common: CommonState,
    chokepoints: Option<chokepoints::ChokepointsFinder>,
    show_original_roads: HashSet<RoadID>,
    intersection_geom: HashSet<IntersectionID>,
    connected_roads: connected_roads::ShowConnectedRoads,
    objects: objects::ObjectDebugger,
    hidden: HashSet<ID>,
    layers: ShowLayers,
    search_results: Option<(String, HashSet<ID>)>,
    neighborhood_summary: neighborhood_summary::NeighborhoodSummary,
}

enum State {
    Exploring(ModalMenu),
    Polygons(polygons::PolygonDebugger),
    SearchOSM(TextBox),
    Colors(color_picker::ColorPicker),
}

impl DebugMode {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> DebugMode {
        DebugMode {
            state: DebugMode::exploring_state(ctx),
            common: CommonState::new(),
            chokepoints: None,
            show_original_roads: HashSet::new(),
            intersection_geom: HashSet::new(),
            connected_roads: connected_roads::ShowConnectedRoads::new(),
            objects: objects::ObjectDebugger::new(),
            hidden: HashSet::new(),
            layers: ShowLayers::new(),
            search_results: None,
            neighborhood_summary: neighborhood_summary::NeighborhoodSummary::new(
                &ui.primary.map,
                &ui.primary.draw_map,
                ctx.prerender,
                &mut Timer::new("set up DebugMode"),
            ),
        }
    }

    fn exploring_state(ctx: &EventCtx) -> State {
        State::Exploring(ModalMenu::new(
            "Debug Mode",
            vec![
                vec![
                    (Some(Key::Escape), "quit"),
                    (Some(Key::C), "show/hide chokepoints"),
                    (Some(Key::O), "clear original roads shown"),
                    (Some(Key::G), "clear intersection geometry"),
                    (Some(Key::H), "unhide everything"),
                    (Some(Key::Num1), "show/hide buildings"),
                    (Some(Key::Num2), "show/hide intersections"),
                    (Some(Key::Num3), "show/hide lanes"),
                    (Some(Key::Num4), "show/hide areas"),
                    (Some(Key::Num5), "show/hide extra shapes"),
                    (Some(Key::Num6), "show/hide geometry debug mode"),
                    (None, "screenshot everything"),
                    (Some(Key::Slash), "search OSM metadata"),
                    (Some(Key::M), "clear OSM search results"),
                    (Some(Key::S), "configure colors"),
                    (Some(Key::N), "show/hide neighborhood summaries"),
                ],
                CommonState::modal_menu_entries(),
            ]
            .concat(),
            ctx,
        ))
    }

    pub fn event(state: &mut GameState, ctx: &mut EventCtx) -> EventLoopMode {
        match state.mode {
            Mode::Debug(ref mut mode) => {
                // TODO Argh, bad hack! Can't do it below because menu is borrowed and ShowObject
                // is implemented on the entirety of DebugMode. :(
                if let State::Exploring(_) = mode.state {
                    state.ui.primary.current_selection =
                        state
                            .ui
                            .handle_mouseover(ctx, &state.ui.primary.sim, mode, true);
                }

                match mode.state {
                    State::Exploring(ref mut menu) => {
                        let mut txt = Text::prompt("Debug Mode");
                        if mode.chokepoints.is_some() {
                            txt.add_line("Showing chokepoints".to_string());
                        }
                        if !mode.show_original_roads.is_empty() {
                            txt.add_line(format!(
                                "Showing {} original roads",
                                mode.show_original_roads.len()
                            ));
                        }
                        if !mode.intersection_geom.is_empty() {
                            txt.add_line(format!(
                                "Showing {} attempts at intersection geometry",
                                mode.intersection_geom.len()
                            ));
                        }
                        if !mode.hidden.is_empty() {
                            txt.add_line(format!("Hiding {} things", mode.hidden.len()));
                        }
                        if let Some((ref search, ref results)) = mode.search_results {
                            txt.add_line(format!(
                                "Search for {} has {} results",
                                search,
                                results.len()
                            ));
                        }
                        if mode.neighborhood_summary.active {
                            txt.add_line("Showing neighborhood summaries".to_string());
                        }
                        menu.handle_event(ctx, Some(txt));

                        ctx.canvas.handle_event(ctx.input);
                        if let Some(evmode) = mode.common.event(ctx, &mut state.ui, menu) {
                            return evmode;
                        }

                        if menu.action("quit") {
                            state.mode = Mode::SplashScreen(Wizard::new(), None);
                            return EventLoopMode::InputOnly;
                        }

                        if menu.action("show/hide chokepoints") {
                            if mode.chokepoints.is_some() {
                                mode.chokepoints = None;
                            } else {
                                // TODO Nothing will actually exist. ;)
                                mode.chokepoints = Some(chokepoints::ChokepointsFinder::new(
                                    &state.ui.primary.sim,
                                ));
                            }
                        }
                        if !mode.show_original_roads.is_empty() {
                            if menu.action("clear original roads shown") {
                                mode.show_original_roads.clear();
                            }
                        }
                        if !mode.intersection_geom.is_empty()
                            && state.ui.primary.current_selection.is_none()
                        {
                            if menu.action("clear intersection geometry") {
                                mode.intersection_geom.clear();
                            }
                        }
                        match state.ui.primary.current_selection {
                            Some(ID::Lane(_))
                            | Some(ID::Intersection(_))
                            | Some(ID::ExtraShape(_)) => {
                                let id = state.ui.primary.current_selection.unwrap();
                                if ctx
                                    .input
                                    .contextual_action(Key::H, &format!("hide {:?}", id))
                                {
                                    println!("Hiding {:?}", id);
                                    //*ctx.recalculate_current_selection = true;
                                    state.ui.primary.current_selection = None;
                                    mode.hidden.insert(id);
                                }
                            }
                            None => {
                                if !mode.hidden.is_empty() && menu.action("unhide everything") {
                                    mode.hidden.clear();
                                    // TODO recalculate current_selection
                                }
                            }
                            _ => {}
                        }

                        if let Some(ID::Lane(l)) = state.ui.primary.current_selection {
                            let id = state.ui.primary.map.get_l(l).parent;
                            if !mode.show_original_roads.contains(&id)
                                && ctx.input.contextual_action(
                                    Key::V,
                                    &format!("show original geometry of {}", id),
                                )
                            {
                                mode.show_original_roads.insert(id);
                            }
                        }
                        if let Some(ID::Intersection(i)) = state.ui.primary.current_selection {
                            if !mode.intersection_geom.contains(&i)
                                && ctx.input.contextual_action(
                                    Key::G,
                                    &format!("recalculate intersection geometry of {}", i),
                                )
                            {
                                mode.intersection_geom.insert(i);
                            }
                        }
                        mode.connected_roads.event(ctx, &state.ui);
                        mode.objects.event(ctx, &state.ui);
                        mode.neighborhood_summary.event(&state.ui, menu);

                        if let Some(debugger) = polygons::PolygonDebugger::new(ctx, &state.ui) {
                            mode.state = State::Polygons(debugger);
                            return EventLoopMode::InputOnly;
                        }

                        // TODO recalc current selection...
                        if menu.action("show/hide buildings") {
                            mode.layers.show_buildings = !mode.layers.show_buildings;
                        } else if menu.action("show/hide intersections") {
                            mode.layers.show_intersections = !mode.layers.show_intersections;
                        } else if menu.action("show/hide lanes") {
                            mode.layers.show_lanes = !mode.layers.show_lanes;
                        } else if menu.action("show/hide areas") {
                            mode.layers.show_areas = !mode.layers.show_areas;
                        } else if menu.action("show/hide extra shapes") {
                            mode.layers.show_extra_shapes = !mode.layers.show_extra_shapes;
                        } else if menu.action("show/hide geometry debug mode") {
                            mode.layers.geom_debug_mode = !mode.layers.geom_debug_mode;
                        }

                        if menu.action("screenshot everything") {
                            let bounds = state.ui.primary.map.get_bounds();
                            assert!(bounds.min_x == 0.0 && bounds.min_y == 0.0);
                            return EventLoopMode::ScreenCaptureEverything {
                                dir: format!(
                                    "../data/screenshots/pending_{}",
                                    state.ui.primary.map.get_name()
                                ),
                                zoom: 3.0,
                                max_x: bounds.max_x,
                                max_y: bounds.max_y,
                            };
                        }

                        if mode.search_results.is_some() {
                            if menu.action("clear OSM search results") {
                                mode.search_results = None;
                            }
                        } else if menu.action("search OSM metadata") {
                            mode.state = State::SearchOSM(TextBox::new("Search for what?", None));
                        } else if menu.action("configure colors") {
                            mode.state = State::Colors(color_picker::ColorPicker::Choosing(
                                ScrollingMenu::new(
                                    "Pick a color to change",
                                    state.ui.cs.color_names(),
                                ),
                            ));
                        }

                        EventLoopMode::InputOnly
                    }
                    State::Polygons(ref mut debugger) => {
                        if debugger.event(ctx) {
                            mode.state = DebugMode::exploring_state(ctx);
                        }
                        EventLoopMode::InputOnly
                    }
                    State::SearchOSM(ref mut tb) => {
                        match tb.event(&mut ctx.input) {
                            InputResult::Canceled => {
                                mode.state = DebugMode::exploring_state(ctx);
                            }
                            InputResult::Done(filter, _) => {
                                mode.state = DebugMode::exploring_state(ctx);

                                let mut ids = HashSet::new();
                                let map = &state.ui.primary.map;
                                for r in map.all_roads() {
                                    if r.osm_tags
                                        .iter()
                                        .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
                                    {
                                        for l in r.all_lanes() {
                                            ids.insert(ID::Lane(l));
                                        }
                                    }
                                }
                                for b in map.all_buildings() {
                                    if b.osm_tags
                                        .iter()
                                        .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
                                    {
                                        ids.insert(ID::Building(b.id));
                                    }
                                }
                                mode.search_results = Some((filter, ids));
                            }
                            InputResult::StillActive => {}
                        }
                        EventLoopMode::InputOnly
                    }
                    State::Colors(ref mut picker) => {
                        if picker.event(ctx, &mut state.ui) {
                            mode.state = DebugMode::exploring_state(ctx);
                        }
                        EventLoopMode::InputOnly
                    }
                }
            }
            _ => unreachable!(),
        }
    }

    pub fn draw(state: &GameState, g: &mut GfxCtx) {
        match state.mode {
            Mode::Debug(ref mode) => match mode.state {
                State::Exploring(ref menu) => {
                    let mut opts = mode.common.draw_options(&state.ui);
                    opts.geom_debug_mode = mode.layers.geom_debug_mode;
                    if let Some(ref chokepoints) = mode.chokepoints {
                        let color = state.ui.cs.get_def("chokepoint", Color::RED);
                        for l in &chokepoints.lanes {
                            opts.override_colors.insert(ID::Lane(*l), color);
                        }
                        for i in &chokepoints.intersections {
                            opts.override_colors.insert(ID::Intersection(*i), color);
                        }
                    }
                    for l in &mode.connected_roads.lanes {
                        opts.override_colors.insert(
                            ID::Lane(*l),
                            state.ui.cs.get("something associated with something else"),
                        );
                    }
                    if let Some((_, ref results)) = mode.search_results {
                        for id in results {
                            opts.override_colors
                                .insert(*id, state.ui.cs.get_def("search result", Color::RED));
                        }
                    }
                    state.ui.draw(g, opts, &state.ui.primary.sim, mode);
                    mode.common.draw(g, &state.ui);

                    for id in &mode.show_original_roads {
                        let r = state.ui.primary.map.get_r(*id);
                        if let Some(pair) = r.get_center_for_side(true) {
                            let (pl, width) = pair.unwrap();
                            g.draw_polygon(
                                state
                                    .ui
                                    .cs
                                    .get_def("original road forwards", Color::RED.alpha(0.5)),
                                &pl.make_polygons(width),
                            );
                        }
                        if let Some(pair) = r.get_center_for_side(false) {
                            let (pl, width) = pair.unwrap();
                            g.draw_polygon(
                                state
                                    .ui
                                    .cs
                                    .get_def("original road backwards", Color::BLUE.alpha(0.5)),
                                &pl.make_polygons(width),
                            );
                        }
                    }
                    for id in &mode.intersection_geom {
                        recalc_intersection_geom(*id, &state.ui.primary.map, g);
                    }

                    mode.objects.draw(g, &state.ui);
                    mode.neighborhood_summary.draw(g);

                    if !g.is_screencap() {
                        menu.draw(g);
                    }
                }
                State::Polygons(ref debugger) => {
                    let mut opts = DrawOptions::new();
                    opts.geom_debug_mode = mode.layers.geom_debug_mode;
                    state.ui.draw(g, opts, &state.ui.primary.sim, mode);
                    debugger.draw(g, &state.ui);
                }
                State::SearchOSM(ref tb) => {
                    let mut opts = DrawOptions::new();
                    opts.geom_debug_mode = mode.layers.geom_debug_mode;
                    state.ui.draw(g, opts, &state.ui.primary.sim, mode);
                    tb.draw(g);
                }
                State::Colors(ref picker) => {
                    let mut opts = DrawOptions::new();
                    opts.geom_debug_mode = mode.layers.geom_debug_mode;
                    state.ui.draw(g, opts, &state.ui.primary.sim, mode);
                    picker.draw(g);
                }
            },
            _ => unreachable!(),
        }
    }
}

impl ShowObject for DebugMode {
    fn show(&self, obj: ID) -> bool {
        if self.hidden.contains(&obj) {
            return false;
        }

        match obj {
            ID::Road(_) | ID::Lane(_) => self.layers.show_lanes,
            ID::Building(_) => self.layers.show_buildings,
            ID::Intersection(_) => self.layers.show_intersections,
            ID::ExtraShape(_) => self.layers.show_extra_shapes,
            ID::Area(_) => self.layers.show_areas,
            _ => true,
        }
    }

    fn layers(&self) -> &ShowLayers {
        &self.layers
    }
}

fn recalc_intersection_geom(id: IntersectionID, map: &Map, g: &mut GfxCtx) {
    let mut all_polys = Vec::new();

    if false {
        // Get road center lines sorted by angle into the intersection, to find adjacent roads.
        // TODO Maybe do this by directed roads instead. Otherwise the corner is too big? But not
        // sure what the intersections will look like yet.
        let mut road_centers: Vec<(PolyLine, Distance)> = map
            .get_i(id)
            .roads
            .iter()
            .map(|r| {
                let road = map.get_r(*r);
                let (pl, width) = road.get_thick_polyline(true).unwrap();
                if road.dst_i == id {
                    (pl, width)
                } else {
                    (pl.reversed(), width)
                }
            })
            .collect();
        // TODO Right?
        let common_pt = map.get_i(id).point;
        // TODO Brittle because of f64->i64 and for short last lines
        road_centers.sort_by_key(|(pl, _)| {
            pl.last_line()
                .pt1()
                .angle_to(common_pt)
                .normalized_degrees() as i64
        });

        for idx in 0..road_centers.len() as isize {
            // pl1 to pl2 moves clockwise
            let (pl1, width1) = wraparound_get(&road_centers, idx);
            let (pl2, width2) = wraparound_get(&road_centers, idx + 1);

            let glued = pl1.clone().extend(pl2.reversed());
            let max_width = (*width1).max(*width2);
            let poly = Polygon::new(&glued.to_thick_boundary_pts(max_width));
            g.draw_polygon(Color::RED.alpha(0.4), &poly);
            all_polys.push(poly);
            //break;
        }

        if false {
            if let Some(p) = intersection_many(&all_polys) {
                g.draw_polygon(Color::GREEN.alpha(0.4), &p);
            }
        }
    }

    if true {
        for r in &map.get_i(id).roads {
            let road = map.get_r(*r);
            let (orig_pl, width) = road.get_thick_polyline(true).unwrap();
            let dir_pl = if road.dst_i == id {
                orig_pl
            } else {
                orig_pl.reversed()
            };
            // Extend the last line by, say, the length of the original road.
            let last_pt = dir_pl
                .last_line()
                .pt2()
                .project_away(dir_pl.length(), dir_pl.last_line().angle());
            let mut pts = dir_pl.points().clone();
            pts.pop();
            pts.push(last_pt);

            // This is different than pl.make_polygons(width) because of the order of the points!!!
            let poly = Polygon::new(&PolyLine::new(pts).to_thick_boundary_pts(width));
            //g.draw_polygon(Color::RED.alpha(0.4), &poly);
            all_polys.push(poly);
        }

        if let Some(p) = intersection_many(&all_polys) {
            g.draw_polygon(Color::GREEN.alpha(0.4), &p);
        }
    }

    if false {
        let mut all_pieces = Vec::new();
        for (idx1, p1) in all_polys.iter().enumerate() {
            for (idx2, p2) in all_polys.iter().enumerate() {
                if idx1 != idx2 {
                    all_pieces.extend(intersection(p1, p2));
                }
            }
        }
        for p in &all_pieces {
            g.draw_polygon(Color::BLUE.alpha(0.4), &p);
        }
        if false {
            if let Some(final_poly) = union(&all_pieces) {
                g.draw_polygon(Color::GREEN.alpha(0.4), &final_poly);
            }
        }
    }
}

fn poly_to_cpoly(poly: &Polygon) -> CPolygon {
    let mut pts: Vec<[f64; 2]> = poly.points().iter().map(|pt| [pt.x(), pt.y()]).collect();
    if pts[0] == *pts.last().unwrap() {
        pts.pop();
    }
    CPolygon::from_vec(&pts)
}

fn cpoly_to_poly(raw_pts: Vec<[f64; 2]>) -> Polygon {
    let mut pts: Vec<Pt2D> = raw_pts
        .into_iter()
        .map(|pt| Pt2D::new(pt[0], pt[1]))
        .collect();
    if pts[0] != *pts.last().unwrap() {
        pts.push(pts[0]);
    }
    Polygon::new(&pts)
}

fn intersection(p1: &Polygon, p2: &Polygon) -> Vec<Polygon> {
    let mut cp1 = poly_to_cpoly(p1);
    let mut cp2 = poly_to_cpoly(p2);
    cp1.intersection(&mut cp2)
        .into_iter()
        .map(|pts| cpoly_to_poly(pts))
        .collect()
}

fn union(polys: &Vec<Polygon>) -> Option<Polygon> {
    let mut result = poly_to_cpoly(&polys[0]);
    for p in polys.iter().skip(1) {
        let output = result.union(&mut poly_to_cpoly(p));
        if output.len() != 1 {
            println!("Argh, got {} pieces from union", output.len());
            return None;
        }
        result = CPolygon::from_vec(&output[0]);
    }
    Some(cpoly_to_poly(result.points()))
}

fn intersection_many(polys: &Vec<Polygon>) -> Option<Polygon> {
    let mut result = poly_to_cpoly(&polys[0]);
    for p in polys.iter().skip(1) {
        let output = result.intersection(&mut poly_to_cpoly(p));
        if output.len() != 1 {
            println!("Argh, got {} pieces from intersection", output.len());
            return None;
        }
        result = CPolygon::from_vec(&output[0]);
    }
    Some(cpoly_to_poly(result.points()))
}
