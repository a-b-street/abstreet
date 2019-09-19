mod bus_explorer;
mod chokepoints;
mod color_picker;
mod connected_roads;
mod floodfill;
mod neighborhood_summary;
mod objects;
mod polygons;
mod routes;

use crate::common::CommonState;
use crate::game::{State, Transition, WizardState};
use crate::helpers::ID;
use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::{ShowLayers, ShowObject, UI};
use abstutil::Timer;
use ezgui::{
    hotkey, Color, Drawable, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, Line, ModalMenu,
    Text, Wizard,
};
use geom::Duration;
use std::collections::HashSet;

pub struct DebugMode {
    menu: ModalMenu,
    common: CommonState,
    chokepoints: Option<chokepoints::ChokepointsFinder>,
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
                        (hotkey(Key::Num7), "show/hide labels"),
                        (hotkey(Key::N), "show/hide neighborhood summaries"),
                        (hotkey(Key::R), "show/hide route for all agents"),
                        (None, "show strongly-connected component roads"),
                    ],
                    vec![
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
                        (hotkey(Key::Escape), "return to previous mode"),
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
            txt.add(Line("Showing chokepoints"));
        }
        if !self.hidden.is_empty() {
            txt.add(Line(format!("Hiding {} things", self.hidden.len())));
        }
        if let Some(ref results) = self.search_results {
            txt.add(Line(format!(
                "Search for {} has {} results",
                results.query,
                results.ids.len()
            )));
        }
        if self.neighborhood_summary.active {
            txt.add(Line("Showing neighborhood summaries"));
        }
        if let routes::AllRoutesViewer::Active(_, ref traces) = self.all_routes {
            txt.add(Line(format!("Showing {} routes", traces.len())));
        }
        self.menu.handle_event(ctx, Some(txt));

        ctx.canvas.handle_event(ctx.input);
        if let Some(t) = self.common.event(ctx, ui, &mut self.menu) {
            return t;
        }

        if self.menu.action("return to previous mode") {
            return Transition::Pop;
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
        match ui.primary.current_selection {
            Some(ID::Lane(_)) | Some(ID::Intersection(_)) | Some(ID::ExtraShape(_)) => {
                let id = ui.primary.current_selection.clone().unwrap();
                if ctx
                    .input
                    .contextual_action(Key::H, format!("hide {:?}", id))
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

        if let Some(ID::Car(id)) = ui.primary.current_selection {
            if ctx
                .input
                .contextual_action(Key::Backspace, "forcibly kill this car")
            {
                ui.primary.sim.kill_stuck_car(id, &ui.primary.map);
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
                ui.primary.current_selection = None;
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
            } else if self.menu.action("show/hide labels") {
                self.layers.show_labels = !self.layers.show_labels;
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
        if let Some(floodfiller) = floodfill::Floodfiller::new(ctx, ui, &mut self.menu) {
            return Transition::Push(floodfiller);
        }

        Transition::Keep
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = self.common.draw_options(ui);
        opts.label_buildings = self.layers.show_labels;
        opts.label_roads = self.layers.show_labels;
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
