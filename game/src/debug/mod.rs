mod associated;
mod connected_roads;
mod floodfill;
mod objects;
mod polygons;
mod routes;

use crate::common::{tool_panel, CommonState};
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::ui::{ShowLayers, ShowObject, UI};
use ezgui::{
    hotkey, Color, Drawable, EventCtx, EventLoopMode, GeomBatch, GfxCtx, Key, Line, ModalMenu,
    Text, Wizard,
};
use geom::Duration;
use sim::Sim;
use std::collections::HashSet;

pub struct DebugMode {
    menu: ModalMenu,
    common: CommonState,
    tool_panel: WrappedComposite,
    associated: associated::ShowAssociatedState,
    connected_roads: connected_roads::ShowConnectedRoads,
    objects: objects::ObjectDebugger,
    hidden: HashSet<ID>,
    layers: ShowLayers,
    search_results: Option<SearchResults>,
    all_routes: routes::AllRoutesViewer,
}

impl DebugMode {
    pub fn new(ctx: &mut EventCtx) -> DebugMode {
        DebugMode {
            menu: ModalMenu::new(
                "Debug Mode",
                vec![
                    (hotkey(Key::Num1), "hide buildings"),
                    (hotkey(Key::Num2), "hide intersections"),
                    (hotkey(Key::Num3), "hide lanes"),
                    (hotkey(Key::Num4), "hide areas"),
                    (hotkey(Key::Num5), "hide extra shapes"),
                    (hotkey(Key::Num6), "show labels"),
                    (hotkey(Key::R), "show route for all agents"),
                    (None, "screenshot everything"),
                    (hotkey(Key::Slash), "search OSM metadata"),
                    (hotkey(Key::O), "save sim state"),
                    (hotkey(Key::Y), "load previous sim state"),
                    (hotkey(Key::U), "load next sim state"),
                    (None, "pick a savestate to load"),
                ],
                ctx,
            ),
            common: CommonState::new(),
            tool_panel: tool_panel(ctx),
            associated: associated::ShowAssociatedState::Inactive,
            connected_roads: connected_roads::ShowConnectedRoads::new(),
            objects: objects::ObjectDebugger::new(),
            hidden: HashSet::new(),
            layers: ShowLayers::new(),
            search_results: None,
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

        {
            let mut txt = Text::new();
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
            if let routes::AllRoutesViewer::Active(_, ref traces) = self.all_routes {
                txt.add(Line(format!("Showing {} routes", traces.len())));
            }
            self.menu.set_info(ctx, txt);
        }
        self.menu.event(ctx);

        ctx.canvas_movement();
        self.associated.event(ui);

        if self.menu.action("save sim state") {
            ctx.loading_screen("savestate", |_, timer| {
                timer.start("save sim state");
                ui.primary.sim.save();
                timer.stop("save sim state");
            });
        }
        if self.menu.action("load previous sim state") {
            if let Some(t) = ctx.loading_screen("load previous savestate", |ctx, mut timer| {
                let prev_state = ui
                    .primary
                    .sim
                    .find_previous_savestate(ui.primary.sim.time());
                match prev_state
                    .clone()
                    .and_then(|path| Sim::load_savestate(path, &ui.primary.map, &mut timer).ok())
                {
                    Some(new_sim) => {
                        ui.primary.sim = new_sim;
                        ui.recalculate_current_selection(ctx);
                        None
                    }
                    None => Some(Transition::Push(msg(
                        "Error",
                        vec![format!("Couldn't load previous savestate {:?}", prev_state)],
                    ))),
                }
            }) {
                return t;
            }
        }
        if self.menu.action("load next sim state") {
            if let Some(t) = ctx.loading_screen("load next savestate", |ctx, mut timer| {
                let next_state = ui.primary.sim.find_next_savestate(ui.primary.sim.time());
                match next_state
                    .clone()
                    .and_then(|path| Sim::load_savestate(path, &ui.primary.map, &mut timer).ok())
                {
                    Some(new_sim) => {
                        ui.primary.sim = new_sim;
                        ui.recalculate_current_selection(ctx);
                        None
                    }
                    None => Some(Transition::Push(msg(
                        "Error",
                        vec![format!("Couldn't load next savestate {:?}", next_state)],
                    ))),
                }
            }) {
                return t;
            }
        }
        if self.menu.action("pick a savestate to load") {
            return Transition::Push(WizardState::new(Box::new(load_savestate)));
        }

        self.all_routes.event(ui, &mut self.menu, ctx);
        match ui.primary.current_selection {
            Some(ID::Lane(_)) | Some(ID::Intersection(_)) | Some(ID::ExtraShape(_)) => {
                let id = ui.primary.current_selection.clone().unwrap();
                if ui.per_obj.action(ctx, Key::H, format!("hide {:?}", id)) {
                    println!("Hiding {:?}", id);
                    ui.primary.current_selection = None;
                    if self.hidden.is_empty() {
                        self.menu
                            .push_action(hotkey(Key::H), "unhide everything", ctx);
                    }
                    self.hidden.insert(id);
                }
            }
            None => {
                if !self.hidden.is_empty() && self.menu.consume_action("unhide everything", ctx) {
                    self.hidden.clear();
                    ui.primary.current_selection =
                        ui.calculate_current_selection(ctx, &ui.primary.sim, self, true);
                }
            }
            _ => {}
        }

        if let Some(ID::Car(id)) = ui.primary.current_selection {
            if ui
                .per_obj
                .action(ctx, Key::Backspace, "forcibly kill this car")
            {
                ui.primary.sim.kill_stuck_car(id, &ui.primary.map);
                ui.primary.sim.step(&ui.primary.map, Duration::seconds(0.1));
                ui.primary.current_selection = None;
            } else if ui.per_obj.action(ctx, Key::G, "find front of blockage") {
                return Transition::Push(msg(
                    "Blockage results",
                    vec![format!(
                        "{} is ultimately blocked by {}",
                        id,
                        ui.primary.sim.find_blockage_front(id, &ui.primary.map)
                    )],
                ));
            }
        }
        self.connected_roads.event(ctx, ui);
        self.objects.event(ctx, ui);

        if let Some(debugger) = polygons::PolygonDebugger::new(ctx, ui) {
            return Transition::Push(Box::new(debugger));
        }

        {
            let mut changed = false;

            for (label, value) in vec![
                ("buildings", &mut self.layers.show_buildings),
                ("intersections", &mut self.layers.show_intersections),
                ("lanes", &mut self.layers.show_lanes),
                ("areas", &mut self.layers.show_areas),
                ("extra shapes", &mut self.layers.show_extra_shapes),
                ("labels", &mut self.layers.show_labels),
            ] {
                let show = format!("show {}", label);
                let hide = format!("hide {}", label);

                if *value && self.menu.swap_action(&hide, &show, ctx) {
                    *value = false;
                    changed = true;
                } else if !*value && self.menu.swap_action(&show, &hide, ctx) {
                    *value = true;
                    changed = true;
                }
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
            if self
                .menu
                .swap_action("clear OSM search results", "search OSM metadata", ctx)
            {
                self.search_results = None;
            }
        } else if self
            .menu
            .swap_action("search OSM metadata", "clear OSM search results", ctx)
        {
            // TODO If the wizard aborts (pressing escape), this crashes.
            return Transition::Push(WizardState::new(Box::new(search_osm)));
        }

        if let Some(floodfiller) = floodfill::Floodfiller::new(ctx, ui) {
            return Transition::Push(floodfiller);
        }

        if let Some(t) = self.common.event(ctx, ui, None) {
            return t;
        }
        match self.tool_panel.event(ctx, ui) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "back" => Transition::Pop,
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw_default_ui(&self) -> bool {
        false
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let mut opts = self.common.draw_options(ui);
        opts.label_buildings = self.layers.show_labels;
        opts.label_roads = self.layers.show_labels;
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
        self.associated
            .override_colors(&mut opts.override_colors, ui);

        ui.draw(g, opts, &ui.primary.sim, self);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            if let Some(ref results) = self.search_results {
                g.redraw(&results.unzoomed);
            }
        }

        self.objects.draw(g, ui);
        self.all_routes.draw(g, ui);

        if !g.is_screencap() {
            self.menu.draw(g);
            self.common.draw(g, ui);
            self.tool_panel.draw(g);
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
    for a in map.all_areas() {
        if a.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
        {
            ids.insert(ID::Area(a.id));
            batch.push(color, a.polygon.clone());
        }
    }

    let results = SearchResults {
        query: filter,
        ids,
        unzoomed: batch.upload(ctx),
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

fn load_savestate(wiz: &mut Wizard, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
    let ss = wiz.wrap(ctx).choose_string("Load which savestate?", || {
        abstutil::list_all_objects(ui.primary.sim.save_dir())
    })?;
    // TODO Oh no, we have to do path construction here :(
    let ss_path = format!("{}/{}.bin", ui.primary.sim.save_dir(), ss);

    ctx.loading_screen("load savestate", |ctx, mut timer| {
        ui.primary.sim = Sim::load_savestate(ss_path, &ui.primary.map, &mut timer)
            .expect("Can't load savestate");
        ui.recalculate_current_selection(ctx);
    });
    Some(Transition::Pop)
}
