mod bus_explorer;
mod chokepoints;
mod color_picker;
mod connected_roads;
mod neighborhood_summary;
mod objects;
mod polygons;
mod routes;

use crate::common::CommonState;
use crate::edit::EditMode;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::sandbox::SandboxMode;
use crate::ui::{ShowLayers, ShowObject, UI};
use abstutil::wraparound_get;
use abstutil::Timer;
use ezgui::{
    hotkey, lctrl, Color, Drawable, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, ModalMenu,
    Text, Wizard,
};
use geom::{Distance, PolyLine, Polygon};
use map_model::{IntersectionID, Map, RoadID};
use std::collections::HashSet;

pub struct DebugMode {
    menu: ModalMenu,
    common: CommonState,
    chokepoints: Option<chokepoints::ChokepointsFinder>,
    show_original_roads: HashSet<RoadID>,
    intersection_geom: HashSet<IntersectionID>,
    connected_roads: connected_roads::ShowConnectedRoads,
    objects: objects::ObjectDebugger,
    hidden: HashSet<ID>,
    layers: ShowLayers,
    search_results: Option<SearchResults>,
    neighborhood_summary: neighborhood_summary::NeighborhoodSummary,
    all_routes: routes::AllRoutesViewer,
}

impl DebugMode {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> DebugMode {
        DebugMode {
            menu: ModalMenu::new(
                "Debug Mode",
                vec![
                    vec![
                        (hotkey(Key::C), "show/hide chokepoints"),
                        (hotkey(Key::Num1), "show/hide buildings"),
                        (hotkey(Key::Num2), "show/hide intersections"),
                        (hotkey(Key::Num3), "show/hide lanes"),
                        (hotkey(Key::Num4), "show/hide areas"),
                        (hotkey(Key::Num5), "show/hide extra shapes"),
                        (hotkey(Key::Num6), "show/hide geometry debug mode"),
                        (hotkey(Key::N), "show/hide neighborhood summaries"),
                        (hotkey(Key::R), "show/hide route for all agents"),
                    ],
                    vec![
                        (hotkey(Key::O), "clear original roads shown"),
                        (hotkey(Key::G), "clear intersection geometry"),
                        (hotkey(Key::H), "unhide everything"),
                        (hotkey(Key::M), "clear OSM search results"),
                    ],
                    vec![
                        (None, "screenshot everything"),
                        (hotkey(Key::Slash), "search OSM metadata"),
                        (hotkey(Key::S), "configure colors"),
                        (None, "explore a bus route"),
                    ],
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (lctrl(Key::S), "sandbox mode"),
                        (lctrl(Key::E), "edit mode"),
                        (hotkey(Key::J), "warp"),
                        (hotkey(Key::K), "navigate"),
                        (hotkey(Key::SingleQuote), "shortcuts"),
                        (hotkey(Key::F1), "take a screenshot"),
                    ],
                ],
                ctx,
            ),
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
            all_routes: routes::AllRoutesViewer::Inactive,
        }
    }
}

impl State for DebugMode {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        if ctx.redo_mouseover() {
            ui.primary.current_selection =
                ui.calculate_current_selection(ctx, &ui.primary.sim, self, true);
        }

        let mut txt = Text::prompt("Debug Mode");
        if self.chokepoints.is_some() {
            txt.add_line("Showing chokepoints".to_string());
        }
        if !self.show_original_roads.is_empty() {
            txt.add_line(format!(
                "Showing {} original roads",
                self.show_original_roads.len()
            ));
        }
        if !self.intersection_geom.is_empty() {
            txt.add_line(format!(
                "Showing {} attempts at intersection geometry",
                self.intersection_geom.len()
            ));
        }
        if !self.hidden.is_empty() {
            txt.add_line(format!("Hiding {} things", self.hidden.len()));
        }
        if let Some(ref results) = self.search_results {
            txt.add_line(format!(
                "Search for {} has {} results",
                results.query,
                results.ids.len()
            ));
        }
        if self.neighborhood_summary.active {
            txt.add_line("Showing neighborhood summaries".to_string());
        }
        if let routes::AllRoutesViewer::Active(_, ref traces) = self.all_routes {
            txt.add_line(format!("Showing {} routes", traces.len()));
        }
        self.menu.handle_event(ctx, Some(txt));

        ctx.canvas.handle_event(ctx.input);
        if let Some(t) = self.common.event(ctx, ui, &mut self.menu) {
            return t;
        }

        if self.menu.action("quit") {
            return Transition::Pop;
        }
        if self.menu.action("sandbox mode") {
            return Transition::Replace(Box::new(SandboxMode::new(ctx)));
        }
        if self.menu.action("edit mode") {
            return Transition::Replace(Box::new(EditMode::new(ctx, ui)));
        }

        if self.menu.action("show/hide chokepoints") {
            if self.chokepoints.is_some() {
                self.chokepoints = None;
            } else {
                // TODO Nothing will actually exist. ;)
                self.chokepoints = Some(chokepoints::ChokepointsFinder::new(&ui.primary.sim));
            }
        }
        self.all_routes.event(ui, &mut self.menu);
        if !self.show_original_roads.is_empty() {
            if self.menu.action("clear original roads shown") {
                self.show_original_roads.clear();
            }
        }
        if !self.intersection_geom.is_empty() && ui.primary.current_selection.is_none() {
            if self.menu.action("clear intersection geometry") {
                self.intersection_geom.clear();
            }
        }
        match ui.primary.current_selection {
            Some(ID::Lane(_)) | Some(ID::Intersection(_)) | Some(ID::ExtraShape(_)) => {
                let id = ui.primary.current_selection.clone().unwrap();
                if ctx
                    .input
                    .contextual_action(Key::H, &format!("hide {:?}", id))
                {
                    println!("Hiding {:?}", id);
                    ui.primary.current_selection = None;
                    self.hidden.insert(id);
                }
            }
            None => {
                if !self.hidden.is_empty() && self.menu.action("unhide everything") {
                    self.hidden.clear();
                    ui.primary.current_selection =
                        ui.calculate_current_selection(ctx, &ui.primary.sim, self, true);
                }
            }
            _ => {}
        }

        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            let id = ui.primary.map.get_l(l).parent;
            if !self.show_original_roads.contains(&id)
                && ctx
                    .input
                    .contextual_action(Key::V, &format!("show original geometry of {}", id))
            {
                self.show_original_roads.insert(id);
            }
        }
        if let Some(ID::Intersection(i)) = ui.primary.current_selection {
            if !self.intersection_geom.contains(&i)
                && ctx.input.contextual_action(
                    Key::G,
                    &format!("recalculate intersection geometry of {}", i),
                )
            {
                self.intersection_geom.insert(i);
            }
        }
        self.connected_roads.event(ctx, ui);
        self.objects.event(ctx, ui);
        self.neighborhood_summary.event(ui, &mut self.menu);

        if let Some(debugger) = polygons::PolygonDebugger::new(ctx, ui) {
            return Transition::Push(Box::new(debugger));
        }

        {
            let mut changed = true;
            if self.menu.action("show/hide buildings") {
                self.layers.show_buildings = !self.layers.show_buildings;
            } else if self.menu.action("show/hide intersections") {
                self.layers.show_intersections = !self.layers.show_intersections;
            } else if self.menu.action("show/hide lanes") {
                self.layers.show_lanes = !self.layers.show_lanes;
            } else if self.menu.action("show/hide areas") {
                self.layers.show_areas = !self.layers.show_areas;
            } else if self.menu.action("show/hide extra shapes") {
                self.layers.show_extra_shapes = !self.layers.show_extra_shapes;
            } else if self.menu.action("show/hide geometry debug mode") {
                self.layers.geom_debug_mode = !self.layers.geom_debug_mode;
            } else {
                changed = false;
            }

            if changed {
                ui.primary.current_selection =
                    ui.calculate_current_selection(ctx, &ui.primary.sim, self, true);
            }
        }

        if self.menu.action("screenshot everything") {
            let bounds = ui.primary.map.get_bounds();
            assert!(bounds.min_x == 0.0 && bounds.min_y == 0.0);
            return Transition::KeepWithMode(EventLoopMode::ScreenCaptureEverything {
                dir: abstutil::path_pending_screenshots(ui.primary.map.get_name()),
                zoom: 3.0,
                max_x: bounds.max_x,
                max_y: bounds.max_y,
            });
        }

        if self.search_results.is_some() {
            if self.menu.action("clear OSM search results") {
                self.search_results = None;
            }
        } else if self.menu.action("search OSM metadata") {
            return Transition::Push(WizardState::new(Box::new(search_osm)));
        } else if self.menu.action("configure colors") {
            return Transition::Push(color_picker::ColorChooser::new());
        }

        if let Some(explorer) = bus_explorer::BusRouteExplorer::new(ctx, ui) {
            return Transition::PushWithMode(explorer, EventLoopMode::Animation);
        }
        if let Some(picker) = bus_explorer::BusRoutePicker::new(ui, &mut self.menu) {
            return Transition::Push(picker);
        }

        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw_as_base_for_substates(&self) -> bool {
        true
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = self.common.draw_options(ui);
        opts.label_buildings = true;
        opts.geom_debug_mode = self.layers.geom_debug_mode;
        if let Some(ref chokepoints) = self.chokepoints {
            let color = ui.cs.get_def("chokepoint", Color::RED);
            for l in &chokepoints.lanes {
                opts.override_colors.insert(ID::Lane(*l), color);
            }
            for i in &chokepoints.intersections {
                opts.override_colors.insert(ID::Intersection(*i), color);
            }
        }
        for l in &self.connected_roads.lanes {
            opts.override_colors.insert(
                ID::Lane(*l),
                ui.cs.get("something associated with something else"),
            );
        }
        if g.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL {
            if let Some(ref results) = self.search_results {
                for id in &results.ids {
                    opts.override_colors
                        .insert(id.clone(), ui.cs.get("search result"));
                }
            }
        }

        ui.draw(g, opts, &ui.primary.sim, self);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            if let Some(ref results) = self.search_results {
                g.redraw(&results.unzoomed);
            }
        }

        self.common.draw(g, ui);

        for id in &self.show_original_roads {
            let r = ui.primary.map.get_r(*id);
            if let Some(pair) = r.get_center_for_side(true) {
                let (pl, width) = pair.unwrap();
                g.draw_polygon(
                    ui.cs
                        .get_def("original road forwards", Color::RED.alpha(0.5)),
                    &pl.make_polygons(width),
                );
            }
            if let Some(pair) = r.get_center_for_side(false) {
                let (pl, width) = pair.unwrap();
                g.draw_polygon(
                    ui.cs
                        .get_def("original road backwards", Color::BLUE.alpha(0.5)),
                    &pl.make_polygons(width),
                );
            }
        }
        for id in &self.intersection_geom {
            recalc_intersection_geom(*id, &ui.primary.map, g);
        }

        self.objects.draw(g, ui);
        self.neighborhood_summary.draw(g);
        self.all_routes.draw(g, ui);

        if !g.is_screencap() {
            self.menu.draw(g);
        }
    }
}

impl ShowObject for DebugMode {
    fn show(&self, obj: &ID) -> bool {
        if self.hidden.contains(obj) {
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
        // TODO This used to be the original OSM center-line node. No longer!
        let common_pt = map.get_i(id).polygon.center();
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
            g.draw_polygon(Color::RED.alpha(0.4), &poly);
        }
    }
}

fn search_osm(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let filter = wiz.wrap(ctx).input_string("Search for what?")?;
    let mut ids = HashSet::new();
    let mut batch = GeomBatch::new();

    let map = &ui.primary.map;
    let color = ui.cs.get_def("search result", Color::RED);
    for r in map.all_roads() {
        if r.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
            || format!("{}", r.osm_way_id).contains(&filter)
        {
            for l in r.all_lanes() {
                ids.insert(ID::Lane(l));
            }
            batch.push(color, r.get_thick_polygon().unwrap());
        }
    }
    for b in map.all_buildings() {
        if b.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
            || format!("{}", b.osm_way_id).contains(&filter)
        {
            ids.insert(ID::Building(b.id));
            batch.push(color, b.polygon.clone());
        }
    }

    let results = SearchResults {
        query: filter,
        ids,
        unzoomed: ctx.prerender.upload(batch),
    };

    Some(Transition::PopWithData(Box::new(|state, _, _| {
        state.downcast_mut::<DebugMode>().unwrap().search_results = Some(results);
    })))
}

struct SearchResults {
    query: String,
    ids: HashSet<ID>,
    unzoomed: Drawable,
}
