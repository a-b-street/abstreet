mod floodfill;
mod objects;
mod polygons;
mod routes;

use crate::app::{App, ShowLayers, ShowObject};
use crate::colors;
use crate::common::{tool_panel, CommonState};
use crate::game::{msg, DrawBaselayer, State, Transition, WizardState};
use crate::helpers::ID;
use crate::managed::{WrappedComposite, WrappedOutcome};
use crate::render::DrawOptions;
use ezgui::{
    hotkey, lctrl, Color, Composite, Drawable, EventCtx, EventLoopMode, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, Text, VerticalAlignment, Wizard,
};
use geom::Duration;
use map_model::IntersectionID;
use sim::Sim;
use std::collections::HashSet;

pub struct DebugMode {
    composite: Composite,
    common: CommonState,
    tool_panel: WrappedComposite,
    objects: objects::ObjectDebugger,
    hidden: HashSet<ID>,
    layers: ShowLayers,
    search_results: Option<SearchResults>,
    all_routes: routes::AllRoutesViewer,

    highlighted_agents: Option<(IntersectionID, Drawable)>,
}

impl DebugMode {
    pub fn new(ctx: &mut EventCtx) -> DebugMode {
        DebugMode {
            composite: Composite::new(
                ManagedWidget::col(vec![
                    ManagedWidget::row(vec![
                        Line("Debug Mode").roboto_bold().draw(ctx),
                        WrappedComposite::text_button(ctx, "X", hotkey(Key::Escape)).align_right(),
                    ]),
                    Text::new().draw(ctx).named("current info"),
                    ManagedWidget::row(
                        vec![
                            (hotkey(Key::Num1), "toggle buildings"),
                            (hotkey(Key::Num2), "toggle intersections"),
                            (hotkey(Key::Num3), "toggle lanes"),
                            (hotkey(Key::Num4), "toggle areas"),
                            (hotkey(Key::Num5), "toggle extra shapes"),
                            (hotkey(Key::Num6), "toggle labels"),
                            (lctrl(Key::H), "unhide everything"),
                            (hotkey(Key::R), "toggle route for all agents"),
                            (None, "screenshot everything"),
                            (hotkey(Key::Slash), "search OSM metadata"),
                            (lctrl(Key::Slash), "clear OSM search results"),
                            (hotkey(Key::O), "save sim state"),
                            (hotkey(Key::Y), "load previous sim state"),
                            (hotkey(Key::U), "load next sim state"),
                            (None, "pick a savestate to load"),
                        ]
                        .into_iter()
                        .map(|(key, action)| WrappedComposite::text_button(ctx, action, key))
                        .collect(),
                    )
                    .flex_wrap(ctx, 80),
                ])
                .padding(10)
                .bg(colors::PANEL_BG),
            )
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            common: CommonState::new(),
            tool_panel: tool_panel(ctx),
            objects: objects::ObjectDebugger::new(),
            hidden: HashSet::new(),
            layers: ShowLayers::new(),
            search_results: None,
            all_routes: routes::AllRoutesViewer::Inactive,
            highlighted_agents: None,
        }
    }

    fn reset_info(&mut self, ctx: &mut EventCtx) {
        let mut txt = Text::new();
        if !self.hidden.is_empty() {
            txt.add(Line(format!("Hiding {} things", self.hidden.len())));
        }
        if let Some(ref results) = self.search_results {
            txt.add(Line(format!(
                "Search for {} has {} results",
                results.query, results.num_matches
            )));
        }
        if let routes::AllRoutesViewer::Active(ref traces) = self.all_routes {
            txt.add(Line(format!("Showing {} routes", traces.len())));
        }
        self.composite
            .replace(ctx, "current info", txt.draw(ctx).named("current info"));
    }
}

impl State for DebugMode {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if ctx.redo_mouseover() {
            app.primary.current_selection =
                app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "save sim state" => {
                    ctx.loading_screen("savestate", |_, timer| {
                        timer.start("save sim state");
                        app.primary.sim.save();
                        timer.stop("save sim state");
                    });
                }
                "load previous sim state" => {
                    if let Some(t) =
                        ctx.loading_screen("load previous savestate", |ctx, mut timer| {
                            let prev_state = app
                                .primary
                                .sim
                                .find_previous_savestate(app.primary.sim.time());
                            match prev_state.clone().and_then(|path| {
                                Sim::load_savestate(path, &app.primary.map, &mut timer).ok()
                            }) {
                                Some(new_sim) => {
                                    app.primary.sim = new_sim;
                                    app.recalculate_current_selection(ctx);
                                    None
                                }
                                None => Some(Transition::Push(msg(
                                    "Error",
                                    vec![format!(
                                        "Couldn't load previous savestate {:?}",
                                        prev_state
                                    )],
                                ))),
                            }
                        })
                    {
                        return t;
                    }
                }
                "load next sim state" => {
                    if let Some(t) = ctx.loading_screen("load next savestate", |ctx, mut timer| {
                        let next_state =
                            app.primary.sim.find_next_savestate(app.primary.sim.time());
                        match next_state.clone().and_then(|path| {
                            Sim::load_savestate(path, &app.primary.map, &mut timer).ok()
                        }) {
                            Some(new_sim) => {
                                app.primary.sim = new_sim;
                                app.recalculate_current_selection(ctx);
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
                "pick a savestate to load" => {
                    return Transition::Push(WizardState::new(Box::new(load_savestate)));
                }
                "unhide everything" => {
                    self.hidden.clear();
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                    self.reset_info(ctx);
                }
                "toggle route for all agents" => {
                    self.all_routes.toggle(app);
                    self.reset_info(ctx);
                }
                "search OSM metadata" => {
                    return Transition::Push(WizardState::new(Box::new(search_osm)));
                }
                "clear OSM search results" => {
                    self.search_results = None;
                    self.reset_info(ctx);
                }
                "screenshot everything" => {
                    let bounds = app.primary.map.get_bounds();
                    assert!(bounds.min_x == 0.0 && bounds.min_y == 0.0);
                    return Transition::KeepWithMode(EventLoopMode::ScreenCaptureEverything {
                        dir: abstutil::path_pending_screenshots(app.primary.map.get_name()),
                        zoom: 3.0,
                        max_x: bounds.max_x,
                        max_y: bounds.max_y,
                    });
                }
                "toggle buildings" => {
                    self.layers.show_buildings = !self.layers.show_buildings;
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                }
                "toggle intersections" => {
                    self.layers.show_intersections = !self.layers.show_intersections;
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                }
                "toggle lanes" => {
                    self.layers.show_lanes = !self.layers.show_lanes;
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                }
                "toggle areas" => {
                    self.layers.show_areas = !self.layers.show_areas;
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                }
                "toggle extra shapes" => {
                    self.layers.show_extra_shapes = !self.layers.show_extra_shapes;
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                }
                "toggle labels" => {
                    self.layers.show_labels = !self.layers.show_labels;
                    app.primary.current_selection =
                        app.calculate_current_selection(ctx, &app.primary.sim, self, true, false);
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if let Some(ID::Lane(_)) | Some(ID::Intersection(_)) | Some(ID::ExtraShape(_)) =
            app.primary.current_selection
        {
            let id = app.primary.current_selection.clone().unwrap();
            if app.per_obj.action(ctx, Key::H, format!("hide {:?}", id)) {
                println!("Hiding {:?}", id);
                app.primary.current_selection = None;
                self.hidden.insert(id);
                self.reset_info(ctx);
            }
        }

        if let Some(ID::Car(id)) = app.primary.current_selection {
            if app
                .per_obj
                .action(ctx, Key::Backspace, "forcibly kill this car")
            {
                app.primary.sim.kill_stuck_car(id, &app.primary.map);
                app.primary
                    .sim
                    .normal_step(&app.primary.map, Duration::seconds(0.1));
                app.primary.current_selection = None;
            } else if app.per_obj.action(ctx, Key::G, "find front of blockage") {
                return Transition::Push(msg(
                    "Blockage results",
                    vec![format!(
                        "{} is ultimately blocked by {}",
                        id,
                        app.primary.sim.find_blockage_front(id, &app.primary.map)
                    )],
                ));
            }
        }
        if let Some(ID::Intersection(id)) = app.primary.current_selection {
            if self
                .highlighted_agents
                .as_ref()
                .map(|(i, _)| id != *i)
                .unwrap_or(true)
            {
                let mut batch = GeomBatch::new();
                for a in app.primary.sim.get_accepted_agents(id) {
                    batch.push(
                        app.cs.get("something associated with something else"),
                        app.primary
                            .draw_map
                            .get_obj(
                                ID::from_agent(a),
                                app,
                                &mut app.primary.draw_map.agents.borrow_mut(),
                                ctx.prerender,
                            )
                            .unwrap()
                            .get_outline(&app.primary.map),
                    );
                }
                self.highlighted_agents = Some((id, ctx.upload(batch)));
            }
        } else {
            self.highlighted_agents = None;
        }

        self.objects.event(ctx, app);

        if let Some(debugger) = polygons::PolygonDebugger::new(ctx, app) {
            return Transition::Push(Box::new(debugger));
        }

        if let Some(floodfiller) = floodfill::Floodfiller::new(ctx, app) {
            return Transition::Push(floodfiller);
        }

        if let Some(t) = self.common.event(ctx, app, None) {
            return t;
        }
        match self.tool_panel.event(ctx, app) {
            Some(WrappedOutcome::Transition(t)) => t,
            Some(WrappedOutcome::Clicked(x)) => match x.as_ref() {
                "back" => Transition::Pop,
                _ => unreachable!(),
            },
            None => Transition::Keep,
        }
    }

    fn draw_baselayer(&self) -> DrawBaselayer {
        DrawBaselayer::Custom
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let mut opts = DrawOptions::new();
        opts.label_buildings = self.layers.show_labels;
        opts.label_roads = self.layers.show_labels;
        app.draw(g, opts, &app.primary.sim, self);

        if let Some(ref results) = self.search_results {
            g.redraw(&results.draw);
        }
        if let Some((_, ref draw)) = self.highlighted_agents {
            g.redraw(draw);
        }

        self.objects.draw(g, app);
        self.all_routes.draw(g, app);

        if !g.is_screencap() {
            self.composite.draw(g);
            self.common.draw(g, app);
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

fn search_osm(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let filter = wiz.wrap(ctx).input_string("Search for what?")?;
    let mut num_matches = 0;
    let mut batch = GeomBatch::new();

    // TODO Case insensitive
    let map = &app.primary.map;
    let color = app.cs.get_def("search result", Color::RED);
    for r in map.all_roads() {
        if r.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
        {
            num_matches += 1;
            batch.push(color, r.get_thick_polygon(map).unwrap());
        }
    }
    for b in map.all_buildings() {
        if b.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
            || b.amenities
                .iter()
                .any(|(n, a)| n.contains(&filter) || a.contains(&filter))
        {
            num_matches += 1;
            batch.push(color, b.polygon.clone());
        }
    }
    for a in map.all_areas() {
        if a.osm_tags
            .iter()
            .any(|(k, v)| format!("{} = {}", k, v).contains(&filter))
        {
            num_matches += 1;
            batch.push(color, a.polygon.clone());
        }
    }

    let results = SearchResults {
        query: filter,
        num_matches,
        draw: batch.upload(ctx),
    };

    Some(Transition::PopWithData(Box::new(|state, _, ctx| {
        let mut mode = state.downcast_mut::<DebugMode>().unwrap();
        mode.search_results = Some(results);
        mode.reset_info(ctx);
    })))
}

struct SearchResults {
    query: String,
    num_matches: usize,
    draw: Drawable,
}

fn load_savestate(wiz: &mut Wizard, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
    let ss = wiz.wrap(ctx).choose_string("Load which savestate?", || {
        abstutil::list_all_objects(app.primary.sim.save_dir())
    })?;
    // TODO Oh no, we have to do path construction here :(
    let ss_path = format!("{}/{}.bin", app.primary.sim.save_dir(), ss);

    ctx.loading_screen("load savestate", |ctx, mut timer| {
        app.primary.sim = Sim::load_savestate(ss_path, &app.primary.map, &mut timer)
            .expect("Can't load savestate");
        app.recalculate_current_selection(ctx);
    });
    Some(Transition::Pop)
}
