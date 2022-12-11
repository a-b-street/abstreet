use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub use trip::OpenTrip;

use crate::ID;
use geom::{Circle, Distance, Polygon, Time};
use map_model::{
    AreaID, BuildingID, IntersectionID, LaneID, ParkingLotID, TransitRouteID, TransitStopID,
};
use sim::{
    AgentID, AgentType, Analytics, CarID, ParkingSpot, PedestrianID, PersonID, PersonState,
    ProblemType, TripID, VehicleType,
};
use widgetry::mapspace::{ToggleZoomed, ToggleZoomedBuilder};
use widgetry::tools::open_browser;
use widgetry::{
    Color, EventCtx, GfxCtx, Key, Line, LinePlot, Outcome, Panel, PlotOptions, Series, Text,
    TextExt, Toggle, Widget,
};

use crate::app::{App, Transition};
use crate::common::{color_for_agent_type, Warping};
use crate::debug::path_counter::PathCounter;
use crate::edit::{EditMode, RouteEditor};
use crate::layer::PANEL_PLACEMENT;
use crate::sandbox::{dashboards, GameplayMode, SandboxMode, TimeWarpScreen};

mod building;
mod debug;
mod intersection;
mod lane;
mod parking_lot;
mod person;
mod transit;
mod trip;

pub struct InfoPanel {
    tab: Tab,
    time: Time,
    is_paused: bool,
    panel: Panel,

    draw_extra: ToggleZoomed,
    tooltips: Vec<(Polygon, Text, (TripID, Time))>,

    hyperlinks: HashMap<String, Tab>,
    warpers: HashMap<String, ID>,
    time_warpers: HashMap<String, (TripID, Time)>,

    // For drawing the OSD only
    cached_actions: Vec<Key>,
}

#[derive(Clone)]
pub enum Tab {
    // What trips are open? For finished trips, show the timeline in the current simulation if
    // true, prebaked if false.
    PersonTrips(PersonID, BTreeMap<TripID, OpenTrip>),
    PersonBio(PersonID),
    PersonSchedule(PersonID),

    TransitVehicleStatus(CarID),
    TransitStop(TransitStopID),
    TransitRoute(TransitRouteID),

    ParkedCar(CarID),

    BldgInfo(BuildingID),
    BldgPeople(BuildingID),

    ParkingLot(ParkingLotID),

    Crowd(Vec<PedestrianID>),

    Area(AreaID),

    IntersectionInfo(IntersectionID),
    IntersectionTraffic(IntersectionID, DataOptions),
    // The extra bool is for fan chart. TODO Probably scatter plot should own the job of switching
    // between these?
    IntersectionDelay(IntersectionID, DataOptions, bool),
    IntersectionDemand(IntersectionID),
    IntersectionArrivals(IntersectionID, DataOptions),
    IntersectionTrafficSignal(IntersectionID),
    IntersectionProblems(IntersectionID, ProblemOptions),

    LaneInfo(LaneID),
    LaneDebug(LaneID),
    LaneTraffic(LaneID, DataOptions),
    LaneProblems(LaneID, ProblemOptions),
}

impl Tab {
    pub fn from_id(app: &App, id: ID) -> Tab {
        match id {
            ID::Road(_) => unreachable!(),
            ID::Lane(l) => match app.session.info_panel_tab["lane"] {
                "info" => Tab::LaneInfo(l),
                "debug" => Tab::LaneDebug(l),
                "traffic" => Tab::LaneTraffic(l, DataOptions::new()),
                "problems" => Tab::LaneProblems(l, ProblemOptions::new()),
                _ => unreachable!(),
            },
            ID::Intersection(i) => match app.session.info_panel_tab["intersection"] {
                "info" => Tab::IntersectionInfo(i),
                "traffic" => Tab::IntersectionTraffic(i, DataOptions::new()),
                "delay" => {
                    if app.primary.map.get_i(i).is_traffic_signal() {
                        Tab::IntersectionDelay(i, DataOptions::new(), false)
                    } else {
                        Tab::IntersectionInfo(i)
                    }
                }
                "demand" => {
                    if app.primary.map.get_i(i).is_traffic_signal() {
                        Tab::IntersectionDemand(i)
                    } else {
                        Tab::IntersectionInfo(i)
                    }
                }
                "arrivals" => {
                    if app.primary.map.get_i(i).is_incoming_border() {
                        Tab::IntersectionArrivals(i, DataOptions::new())
                    } else {
                        Tab::IntersectionInfo(i)
                    }
                }
                "traffic signal" => {
                    if app.primary.map.get_i(i).is_traffic_signal() {
                        Tab::IntersectionTrafficSignal(i)
                    } else {
                        Tab::IntersectionInfo(i)
                    }
                }
                "problems" => Tab::IntersectionProblems(i, ProblemOptions::new()),
                _ => unreachable!(),
            },
            ID::Building(b) => match app.session.info_panel_tab["bldg"] {
                "info" => Tab::BldgInfo(b),
                "people" => Tab::BldgPeople(b),
                _ => unreachable!(),
            },
            ID::ParkingLot(b) => Tab::ParkingLot(b),
            ID::Car(c) => {
                if let Some(p) = app.primary.sim.agent_to_person(AgentID::Car(c)) {
                    match app.session.info_panel_tab["person"] {
                        "trips" => Tab::PersonTrips(
                            p,
                            OpenTrip::single(
                                app.primary.sim.agent_to_trip(AgentID::Car(c)).unwrap(),
                            ),
                        ),
                        "bio" => Tab::PersonBio(p),
                        "schedule" => Tab::PersonSchedule(p),
                        _ => unreachable!(),
                    }
                } else if c.vehicle_type == VehicleType::Bus || c.vehicle_type == VehicleType::Train
                {
                    match app.session.info_panel_tab["bus"] {
                        "status" => Tab::TransitVehicleStatus(c),
                        _ => unreachable!(),
                    }
                } else {
                    Tab::ParkedCar(c)
                }
            }
            ID::Pedestrian(p) => {
                let person = app
                    .primary
                    .sim
                    .agent_to_person(AgentID::Pedestrian(p))
                    .unwrap();
                match app.session.info_panel_tab["person"] {
                    "trips" => Tab::PersonTrips(
                        person,
                        OpenTrip::single(
                            app.primary
                                .sim
                                .agent_to_trip(AgentID::Pedestrian(p))
                                .unwrap(),
                        ),
                    ),
                    "bio" => Tab::PersonBio(person),
                    "schedule" => Tab::PersonSchedule(person),
                    _ => unreachable!(),
                }
            }
            ID::PedCrowd(members) => Tab::Crowd(members),
            ID::TransitStop(bs) => Tab::TransitStop(bs),
            ID::Area(a) => Tab::Area(a),
        }
    }

    fn to_id(&self, app: &App) -> Option<ID> {
        match self {
            Tab::PersonTrips(p, _) | Tab::PersonBio(p) | Tab::PersonSchedule(p) => {
                match app.primary.sim.get_person(*p).state {
                    PersonState::Inside(b) => Some(ID::Building(b)),
                    PersonState::Trip(t) => {
                        app.primary.sim.trip_to_agent(t).ok().map(ID::from_agent)
                    }
                    _ => None,
                }
            }
            Tab::TransitVehicleStatus(c) => Some(ID::Car(*c)),
            Tab::TransitStop(bs) => Some(ID::TransitStop(*bs)),
            Tab::TransitRoute(_) => None,
            // TODO If a parked car becomes in use while the panel is open, should update the
            // panel better.
            Tab::ParkedCar(c) => match app.primary.sim.lookup_parked_car(*c)?.spot {
                ParkingSpot::Onstreet(_, _) => Some(ID::Car(*c)),
                ParkingSpot::Offstreet(b, _) => Some(ID::Building(b)),
                ParkingSpot::Lot(_, _) => Some(ID::Car(*c)),
            },
            Tab::BldgInfo(b) | Tab::BldgPeople(b) => Some(ID::Building(*b)),
            Tab::ParkingLot(pl) => Some(ID::ParkingLot(*pl)),
            Tab::Crowd(members) => Some(ID::PedCrowd(members.clone())),
            Tab::Area(a) => Some(ID::Area(*a)),
            Tab::IntersectionInfo(i)
            | Tab::IntersectionTraffic(i, _)
            | Tab::IntersectionDelay(i, _, _)
            | Tab::IntersectionDemand(i)
            | Tab::IntersectionArrivals(i, _)
            | Tab::IntersectionTrafficSignal(i)
            | Tab::IntersectionProblems(i, _) => Some(ID::Intersection(*i)),
            Tab::LaneInfo(l)
            | Tab::LaneDebug(l)
            | Tab::LaneTraffic(l, _)
            | Tab::LaneProblems(l, _) => Some(ID::Lane(*l)),
        }
    }

    fn changed_settings(&self, c: &Panel) -> Option<Tab> {
        // Avoid an occasionally expensive clone.
        match self {
            Tab::IntersectionTraffic(_, _)
            | Tab::IntersectionDelay(_, _, _)
            | Tab::IntersectionArrivals(_, _)
            | Tab::IntersectionProblems(_, _)
            | Tab::LaneTraffic(_, _) => {}
            Tab::LaneProblems(_, _) => {}
            _ => {
                return None;
            }
        }

        let mut new_tab = self.clone();
        match new_tab {
            Tab::IntersectionTraffic(_, ref mut opts)
            | Tab::IntersectionArrivals(_, ref mut opts)
            | Tab::LaneTraffic(_, ref mut opts) => {
                let new_opts = DataOptions::from_controls(c);
                if *opts == new_opts {
                    return None;
                }
                *opts = new_opts;
            }
            Tab::IntersectionDelay(_, ref mut opts, ref mut fan_chart) => {
                let new_opts = DataOptions::from_controls(c);
                let new_fan_chart = c.is_checked("fan chart / scatter plot");
                if *opts == new_opts && *fan_chart == new_fan_chart {
                    return None;
                }
                *opts = new_opts;
                *fan_chart = new_fan_chart;
            }
            Tab::IntersectionProblems(_, ref mut opts) | Tab::LaneProblems(_, ref mut opts) => {
                let new_opts = ProblemOptions::from_controls(c);
                if *opts == new_opts {
                    return None;
                }
                *opts = new_opts;
            }
            _ => unreachable!(),
        }
        Some(new_tab)
    }

    fn variant(&self) -> (&'static str, &'static str) {
        match self {
            Tab::PersonTrips(_, _) => ("person", "trips"),
            Tab::PersonBio(_) => ("person", "bio"),
            Tab::PersonSchedule(_) => ("person", "schedule"),
            Tab::TransitVehicleStatus(_) => ("bus", "status"),
            Tab::TransitStop(_) => ("bus stop", "info"),
            Tab::TransitRoute(_) => ("bus route", "info"),
            Tab::ParkedCar(_) => ("parked car", "info"),
            Tab::BldgInfo(_) => ("bldg", "info"),
            Tab::BldgPeople(_) => ("bldg", "people"),
            Tab::ParkingLot(_) => ("parking lot", "info"),
            Tab::Crowd(_) => ("crowd", "info"),
            Tab::Area(_) => ("area", "info"),
            Tab::IntersectionInfo(_) => ("intersection", "info"),
            Tab::IntersectionTraffic(_, _) => ("intersection", "traffic"),
            Tab::IntersectionDelay(_, _, _) => ("intersection", "delay"),
            Tab::IntersectionDemand(_) => ("intersection", "demand"),
            Tab::IntersectionArrivals(_, _) => ("intersection", "arrivals"),
            Tab::IntersectionTrafficSignal(_) => ("intersection", "traffic signal"),
            Tab::IntersectionProblems(_, _) => ("intersection", "problems"),
            Tab::LaneInfo(_) => ("lane", "info"),
            Tab::LaneDebug(_) => ("lane", "debug"),
            Tab::LaneTraffic(_, _) => ("lane", "traffic"),
            Tab::LaneProblems(_, _) => ("lane", "problems"),
        }
    }
}

// TODO Name sucks
pub struct Details {
    /// Draw extra things when unzoomed or zoomed.
    pub draw_extra: ToggleZoomedBuilder,
    /// Show these tooltips over the map. If the tooltip is clicked, time-warp and open the info
    /// panel.
    pub tooltips: Vec<(Polygon, Text, (TripID, Time))>,
    /// When a button with this label is clicked, open this info panel tab instead.
    pub hyperlinks: HashMap<String, Tab>,
    /// When a button with this label is clicked, warp to this ID.
    pub warpers: HashMap<String, ID>,
    /// When a button with this label is clicked, time-warp and open the info panel for this trip.
    pub time_warpers: HashMap<String, (TripID, Time)>,
    // It's just convenient to plumb this here
    pub can_jump_to_time: bool,
}

impl InfoPanel {
    pub fn new(
        ctx: &mut EventCtx,
        app: &mut App,
        mut tab: Tab,
        ctx_actions: &mut dyn ContextualActions,
    ) -> InfoPanel {
        let (k, v) = tab.variant();
        app.session.info_panel_tab.insert(k, v);

        let mut details = Details {
            draw_extra: ToggleZoomed::builder(),
            tooltips: Vec::new(),
            hyperlinks: HashMap::new(),
            warpers: HashMap::new(),
            time_warpers: HashMap::new(),
            can_jump_to_time: ctx_actions.gameplay_mode().can_jump_to_time(),
        };

        let (header_and_tabs, main_tab) = match tab {
            Tab::PersonTrips(p, ref mut open) => (
                person::trips(ctx, app, &mut details, p, open, ctx_actions.is_paused()),
                true,
            ),
            Tab::PersonBio(p) => (
                person::bio(ctx, app, &mut details, p, ctx_actions.is_paused()),
                false,
            ),
            Tab::PersonSchedule(p) => (
                person::schedule(ctx, app, &mut details, p, ctx_actions.is_paused()),
                false,
            ),
            Tab::TransitVehicleStatus(c) => (transit::bus_status(ctx, app, &mut details, c), true),
            Tab::TransitStop(bs) => (transit::stop(ctx, app, &mut details, bs), true),
            Tab::TransitRoute(br) => (transit::route(ctx, app, &mut details, br), true),
            Tab::ParkedCar(c) => (
                person::parked_car(ctx, app, &mut details, c, ctx_actions.is_paused()),
                true,
            ),
            Tab::BldgInfo(b) => (building::info(ctx, app, &mut details, b), true),
            Tab::BldgPeople(b) => (building::people(ctx, app, &mut details, b), false),
            Tab::ParkingLot(pl) => (parking_lot::info(ctx, app, &mut details, pl), true),
            Tab::Crowd(ref members) => (person::crowd(ctx, app, &mut details, members), true),
            Tab::Area(a) => (debug::area(ctx, app, &mut details, a), true),
            Tab::IntersectionInfo(i) => (intersection::info(ctx, app, &mut details, i), true),
            Tab::IntersectionTraffic(i, ref opts) => (
                intersection::traffic(ctx, app, &mut details, i, opts),
                false,
            ),
            Tab::IntersectionDelay(i, ref opts, fan_chart) => (
                intersection::delay(ctx, app, &mut details, i, opts, fan_chart),
                false,
            ),
            Tab::IntersectionDemand(i) => (
                intersection::current_demand(ctx, app, &mut details, i),
                false,
            ),
            Tab::IntersectionArrivals(i, ref opts) => (
                intersection::arrivals(ctx, app, &mut details, i, opts),
                false,
            ),
            Tab::IntersectionTrafficSignal(i) => (
                intersection::traffic_signal(ctx, app, &mut details, i),
                false,
            ),
            Tab::IntersectionProblems(i, ref opts) => (
                intersection::problems(ctx, app, &mut details, i, opts),
                false,
            ),
            Tab::LaneInfo(l) => (lane::info(ctx, app, &mut details, l), true),
            Tab::LaneDebug(l) => (lane::debug(ctx, app, &mut details, l), false),
            Tab::LaneTraffic(l, ref opts) => {
                (lane::traffic(ctx, app, &mut details, l, opts), false)
            }
            Tab::LaneProblems(l, ref opts) => {
                (lane::problems(ctx, app, &mut details, l, opts), false)
            }
        };

        let mut col = vec![header_and_tabs];
        let maybe_id = tab.to_id(app);
        let mut cached_actions = Vec::new();
        if main_tab {
            if let Some(id) = maybe_id.clone() {
                for (key, label) in ctx_actions.actions(app, id) {
                    cached_actions.push(key);
                    let button = ctx
                        .style()
                        .btn_outline
                        .text(&label)
                        .hotkey(key)
                        .build_widget(ctx, label);
                    col.push(button);
                }
            }
        }

        // Highlight something?
        if let Some((id, outline)) = maybe_id.and_then(|id| {
            app.primary
                .get_obj_outline(
                    ctx,
                    id.clone(),
                    &app.cs,
                    &app.primary.map,
                    &mut app.primary.agents.borrow_mut(),
                )
                .map(|outline| (id, outline))
        }) {
            // Different selection styles for different objects.
            match id {
                ID::Car(_) | ID::Pedestrian(_) | ID::PedCrowd(_) => {
                    // Some objects are much wider/taller than others
                    let multiplier = match id {
                        ID::Car(c) => {
                            if c.vehicle_type == VehicleType::Bike {
                                3.0
                            } else {
                                0.75
                            }
                        }
                        ID::Pedestrian(_) => 3.0,
                        ID::PedCrowd(_) => 0.75,
                        _ => unreachable!(),
                    };
                    // Make a circle to cover the object.
                    let bounds = outline.get_bounds();
                    let radius = multiplier * Distance::meters(bounds.width().max(bounds.height()));
                    details.draw_extra.unzoomed.push(
                        app.cs.current_object.alpha(0.5),
                        Circle::new(bounds.center(), radius).to_polygon(),
                    );
                    match Circle::new(bounds.center(), radius).to_outline(Distance::meters(0.3)) {
                        Ok(poly) => {
                            details
                                .draw_extra
                                .unzoomed
                                .push(app.cs.current_object, poly.clone());
                            details.draw_extra.zoomed.push(app.cs.current_object, poly);
                        }
                        Err(err) => {
                            warn!("No outline for {:?}: {}", id, err);
                        }
                    }

                    // TODO And actually, don't cover up the agent. The Renderable API isn't quite
                    // conducive to doing this yet.
                }
                _ => {
                    details
                        .draw_extra
                        .unzoomed
                        .push(app.cs.perma_selected_object, outline.clone());
                    details
                        .draw_extra
                        .zoomed
                        .push(app.cs.perma_selected_object, outline);
                }
            }
        }

        InfoPanel {
            tab,
            time: app.primary.sim.time(),
            is_paused: ctx_actions.is_paused(),
            panel: Panel::new_builder(Widget::col(col).bg(app.cs.panel_bg).padding(16))
                .aligned_pair(PANEL_PLACEMENT)
                // TODO Some headings are too wide.. Intersection #xyz (Traffic signals)
                .exact_size_percent(30, 60)
                .build_custom(ctx),
            draw_extra: details.draw_extra.build(ctx),
            tooltips: details.tooltips,
            hyperlinks: details.hyperlinks,
            warpers: details.warpers,
            time_warpers: details.time_warpers,
            cached_actions,
        }
    }

    // (Are we done, optional transition)
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        ctx_actions: &mut dyn ContextualActions,
    ) -> (bool, Option<Transition>) {
        // Let the user click on the map to cancel out this info panel, or click on a tooltip to
        // time warp.
        if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            // TODO This'll fire left_click elsewhere and conflict; we can't override here
            if app.primary.current_selection.is_none() {
                let mut found_tooltip = false;
                if let Some((_, _, (trip, time))) = self
                    .tooltips
                    .iter()
                    .find(|(poly, _, _)| poly.contains_pt(pt))
                {
                    found_tooltip = true;
                    if app
                        .per_obj
                        .left_click(ctx, &format!("warp here at {}", time))
                    {
                        return do_time_warp(ctx_actions, app, *trip, *time);
                    }
                }
                if !found_tooltip && app.per_obj.left_click(ctx, "stop showing info") {
                    return (true, None);
                }
            }
        }

        // Live update?
        if app.primary.sim.time() != self.time || ctx_actions.is_paused() != self.is_paused {
            let mut new = InfoPanel::new(ctx, app, self.tab.clone(), ctx_actions);
            new.panel.restore(ctx, &self.panel);
            *self = new;
            return (false, None);
        }

        let maybe_id = self.tab.to_id(app);
        match self.panel.event(ctx) {
            Outcome::Clicked(action) => {
                if let Some(new_tab) = self.hyperlinks.get(&action).cloned() {
                    let mut new = InfoPanel::new(ctx, app, new_tab, ctx_actions);
                    // TODO Most cases use changed_settings, but one doesn't. Detect that
                    // "sameness" here.
                    if let (Tab::PersonTrips(p1, _), Tab::PersonTrips(p2, _)) =
                        (&self.tab, &new.tab)
                    {
                        if p1 == p2 {
                            new.panel.restore(ctx, &self.panel);
                        }
                    }
                    *self = new;
                    (false, None)
                } else if action == "close" {
                    (true, None)
                } else if action == "jump to object" {
                    // TODO Messy way of doing this
                    if let Some(id) = self.tab.to_id(app) {
                        (
                            false,
                            Some(Transition::Push(Warping::new_state(
                                ctx,
                                app.primary.canonical_point(id.clone()).unwrap(),
                                Some(10.0),
                                Some(id),
                                &mut app.primary,
                            ))),
                        )
                    } else {
                        (false, None)
                    }
                } else if let Some(id) = self.warpers.get(&action) {
                    (
                        false,
                        Some(Transition::Push(Warping::new_state(
                            ctx,
                            app.primary.canonical_point(id.clone()).unwrap(),
                            Some(10.0),
                            None,
                            &mut app.primary,
                        ))),
                    )
                } else if let Some((trip, time)) = self.time_warpers.get(&action) {
                    do_time_warp(ctx_actions, app, *trip, *time)
                } else if let Some(url) = action.strip_prefix("open ") {
                    open_browser(url);
                    (false, None)
                } else if let Some(x) = action.strip_prefix("edit TransitRoute #") {
                    (
                        false,
                        Some(Transition::Multi(vec![
                            Transition::Push(EditMode::new_state(
                                ctx,
                                app,
                                ctx_actions.gameplay_mode(),
                            )),
                            Transition::Push(RouteEditor::new_state(
                                ctx,
                                app,
                                TransitRouteID(x.parse::<usize>().unwrap()),
                            )),
                        ])),
                    )
                } else if action == "Explore demand across all traffic signals" {
                    (
                        false,
                        Some(Transition::Push(
                            dashboards::TrafficSignalDemand::new_state(ctx, app),
                        )),
                    )
                } else if let Some(x) = action.strip_prefix("routes across Intersection #") {
                    (
                        false,
                        Some(Transition::Push(PathCounter::demand_across_intersection(
                            ctx,
                            app,
                            IntersectionID(x.parse::<usize>().unwrap()),
                        ))),
                    )
                } else if let Some(id) = maybe_id {
                    let mut close_panel = true;
                    let t = ctx_actions.execute(ctx, app, id, action, &mut close_panel);
                    (close_panel, Some(t))
                } else {
                    // This happens when clicking the follow/unfollow button on a trip whose
                    // agent doesn't exist. Do nothing and just don't crash.
                    error!(
                        "Can't do {} on this tab, because it doesn't map to an ID",
                        action
                    );
                    (false, None)
                }
            }
            _ => {
                // Maybe a non-click action should change the tab. Aka, checkboxes/dropdowns/etc on
                // a tab.
                if let Some(new_tab) = self.tab.changed_settings(&self.panel) {
                    let mut new = InfoPanel::new(ctx, app, new_tab, ctx_actions);
                    new.panel.restore(ctx, &self.panel);
                    *self = new;
                    return (false, None);
                }

                (false, None)
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.panel.draw(g);
        self.draw_extra.draw(g);
        if let Some(pt) = g.canvas.get_cursor_in_map_space() {
            for (poly, txt, _) in &self.tooltips {
                if poly.contains_pt(pt) {
                    g.draw_mouse_tooltip(txt.clone());
                    break;
                }
            }
        }
    }

    pub fn active_keys(&self) -> &Vec<Key> {
        &self.cached_actions
    }

    pub fn active_id(&self, app: &App) -> Option<ID> {
        self.tab.to_id(app)
    }
}

// Internal helper method for InfoPanel::event
fn do_time_warp(
    ctx_actions: &mut dyn ContextualActions,
    app: &mut App,
    trip: TripID,
    time: Time,
) -> (bool, Option<Transition>) {
    let person = app.primary.sim.trip_to_person(trip).unwrap();
    // When executed, this assumes the SandboxMode is the top of the stack. It'll
    // reopen the info panel, then launch the jump-to-time UI.
    let jump_to_time = Transition::ConsumeState(Box::new(move |state, ctx, app| {
        let mut sandbox = state.downcast::<SandboxMode>().ok().unwrap();

        let mut actions = sandbox.contextual_actions();
        sandbox.controls.common.as_mut().unwrap().launch_info_panel(
            ctx,
            app,
            Tab::PersonTrips(person, OpenTrip::single(trip)),
            &mut actions,
        );

        vec![sandbox, TimeWarpScreen::new_state(ctx, app, time, None)]
    }));

    if time >= app.primary.sim.time() {
        return (false, Some(jump_to_time));
    }

    // We need to first rewind the simulation
    let rewind_sim = Transition::Replace(SandboxMode::async_new(
        app,
        ctx_actions.gameplay_mode(),
        Box::new(move |_, _| vec![jump_to_time]),
    ));

    (false, Some(rewind_sim))
}

fn make_table<I: Into<String>>(ctx: &EventCtx, rows: Vec<(I, String)>) -> Vec<Widget> {
    rows.into_iter()
        .map(|(k, v)| {
            Widget::row(vec![
                Line(k).secondary().into_widget(ctx),
                // TODO not quite...
                v.text_widget(ctx).centered_vert().align_right(),
            ])
        })
        .collect()
}

fn throughput<F: Fn(&Analytics) -> Vec<(AgentType, Vec<(Time, usize)>)>>(
    ctx: &EventCtx,
    app: &App,
    title: &str,
    get_data: F,
    opts: &DataOptions,
) -> Widget {
    let mut series = get_data(app.primary.sim.get_analytics())
        .into_iter()
        .map(|(agent_type, pts)| Series {
            label: agent_type.noun().to_string(),
            color: color_for_agent_type(app, agent_type),
            pts,
        })
        .collect::<Vec<_>>();
    if opts.show_before {
        // TODO Ahh these colors don't show up differently at all.
        for (agent_type, pts) in get_data(app.prebaked()) {
            series.push(Series {
                label: agent_type.noun().to_string(),
                color: color_for_agent_type(app, agent_type).alpha(0.3),
                pts,
            });
        }
    }

    let mut plot_opts = PlotOptions::filterable();
    plot_opts.disabled = opts.disabled_series();
    Widget::col(vec![
        Line(title).small_heading().into_widget(ctx),
        LinePlot::new_widget(ctx, title, series, plot_opts, app.opts.units),
    ])
    .padding(10)
    .bg(app.cs.inner_panel_bg)
    .outline(ctx.style().section_outline)
}

// Like above, but grouped differently...
fn problem_count<F: Fn(&Analytics) -> Vec<(ProblemType, Vec<(Time, usize)>)>>(
    ctx: &EventCtx,
    app: &App,
    title: &str,
    get_data: F,
    opts: &ProblemOptions,
) -> Widget {
    let mut series = get_data(app.primary.sim.get_analytics())
        .into_iter()
        .map(|(problem_type, pts)| Series {
            label: problem_type.name().to_string(),
            color: color_for_problem_type(app, problem_type),
            pts,
        })
        .collect::<Vec<_>>();
    if opts.show_before {
        for (problem_type, pts) in get_data(app.prebaked()) {
            series.push(Series {
                label: problem_type.name().to_string(),
                color: color_for_problem_type(app, problem_type).alpha(0.3),
                pts,
            });
        }
    }

    let mut plot_opts = PlotOptions::filterable();
    plot_opts.disabled = opts.disabled_series();
    Widget::col(vec![
        Line(title).small_heading().into_widget(ctx),
        LinePlot::new_widget(ctx, title, series, plot_opts, app.opts.units),
    ])
    .padding(10)
    .bg(app.cs.inner_panel_bg)
    .outline(ctx.style().section_outline)
}

fn make_tabs(
    ctx: &EventCtx,
    hyperlinks: &mut HashMap<String, Tab>,
    current_tab: Tab,
    tabs: Vec<(&str, Tab)>,
) -> Widget {
    use widgetry::DEFAULT_CORNER_RADIUS;
    let mut row = Vec::new();
    for (name, link) in tabs {
        row.push(
            ctx.style()
                .btn_tab
                .text(name)
                .corner_rounding(geom::CornerRadii {
                    top_left: DEFAULT_CORNER_RADIUS,
                    top_right: DEFAULT_CORNER_RADIUS,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                })
                // We abuse "disabled" to denote "currently selected"
                .disabled(current_tab.variant() == link.variant())
                .build_def(ctx),
        );
        hyperlinks.insert(name.to_string(), link);
    }

    Widget::row(row).margin_above(16)
}

fn header_btns(ctx: &EventCtx) -> Widget {
    Widget::row(vec![
        ctx.style()
            .btn_plain
            .icon("system/assets/tools/location.svg")
            .hotkey(Key::J)
            .build_widget(ctx, "jump to object"),
        ctx.style().btn_close_widget(ctx),
    ])
    .align_right()
}

pub trait ContextualActions {
    // TODO &str?
    fn actions(&self, app: &App, id: ID) -> Vec<(Key, String)>;
    fn execute(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        id: ID,
        action: String,
        close_panel: &mut bool,
    ) -> Transition;

    // Slightly weird way to plumb in extra info, but...
    fn is_paused(&self) -> bool;
    fn gameplay_mode(&self) -> GameplayMode;
}

#[derive(Clone, PartialEq)]
pub struct DataOptions {
    pub show_before: bool,
    pub show_end_of_day: bool,
    disabled_types: BTreeSet<AgentType>,
}

impl DataOptions {
    pub fn new() -> DataOptions {
        DataOptions {
            show_before: false,
            show_end_of_day: false,
            disabled_types: BTreeSet::new(),
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        if app.has_prebaked().is_none() {
            return Widget::nothing();
        }
        Widget::row(vec![
            Toggle::custom_checkbox(
                ctx,
                "Show before changes",
                vec![
                    Line("Show before "),
                    Line(&app.primary.map.get_edits().edits_name).underlined(),
                ],
                None,
                self.show_before,
            ),
            if self.show_before {
                Toggle::switch(ctx, "Show full day", None, self.show_end_of_day)
            } else {
                Widget::nothing()
            },
        ])
        .evenly_spaced()
    }

    pub fn from_controls(c: &Panel) -> DataOptions {
        let show_before = c.maybe_is_checked("Show before changes").unwrap_or(false);
        let mut disabled_types = BTreeSet::new();
        for a in AgentType::all() {
            let label = a.noun();
            if !c.maybe_is_checked(label).unwrap_or(true) {
                disabled_types.insert(a);
            }
        }
        DataOptions {
            show_before,
            show_end_of_day: show_before && c.maybe_is_checked("Show full day").unwrap_or(false),
            disabled_types,
        }
    }

    pub fn disabled_series(&self) -> HashSet<String> {
        self.disabled_types
            .iter()
            .map(|a| a.noun().to_string())
            .collect()
    }
}

#[derive(Clone, PartialEq)]
pub struct ProblemOptions {
    pub show_before: bool,
    pub show_end_of_day: bool,
    disabled_types: HashSet<ProblemType>,
}

impl ProblemOptions {
    pub fn new() -> Self {
        Self {
            show_before: false,
            show_end_of_day: false,
            disabled_types: HashSet::new(),
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        if app.has_prebaked().is_none() {
            return Widget::nothing();
        }
        Widget::row(vec![
            Toggle::custom_checkbox(
                ctx,
                "Show before changes",
                vec![
                    Line("Show before "),
                    Line(&app.primary.map.get_edits().edits_name).underlined(),
                ],
                None,
                self.show_before,
            ),
            if self.show_before {
                Toggle::switch(ctx, "Show full day", None, self.show_end_of_day)
            } else {
                Widget::nothing()
            },
        ])
        .evenly_spaced()
    }

    pub fn from_controls(c: &Panel) -> ProblemOptions {
        let show_before = c.maybe_is_checked("Show before changes").unwrap_or(false);
        let mut disabled_types = HashSet::new();
        for pt in ProblemType::all() {
            if !c.maybe_is_checked(pt.name()).unwrap_or(true) {
                disabled_types.insert(pt);
            }
        }
        ProblemOptions {
            show_before,
            show_end_of_day: show_before && c.maybe_is_checked("Show full day").unwrap_or(false),
            disabled_types,
        }
    }

    pub fn disabled_series(&self) -> HashSet<String> {
        self.disabled_types
            .iter()
            .map(|pt| pt.name().to_string())
            .collect()
    }
}

// TODO Maybe color should be optional, and we'll default to rotating through some options in the
// Series
fn color_for_problem_type(app: &App, problem_type: ProblemType) -> Color {
    for (idx, pt) in ProblemType::all().into_iter().enumerate() {
        if problem_type == pt {
            return app.cs.rotating_color_plot(idx);
        }
    }
    unreachable!()
}
