use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub use trip::OpenTrip;

use geom::{Circle, Distance, Time};
use map_gui::tools::open_browser;
use map_gui::ID;
use map_model::{AreaID, BuildingID, BusRouteID, BusStopID, IntersectionID, LaneID, ParkingLotID};
use sim::{
    AgentID, AgentType, Analytics, CarID, ParkingSpot, PedestrianID, PersonID, PersonState, TripID,
    VehicleType,
};
use widgetry::{
    Btn, Checkbox, Color, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line,
    LinePlot, Outcome, Panel, PlotOptions, Series, TextExt, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::{color_for_agent_type, hotkey_btn, Warping};
use crate::debug::path_counter::PathCounter;
use crate::edit::{EditMode, RouteEditor};
use crate::sandbox::{dashboards, GameplayMode, SandboxMode, TimeWarpScreen};

mod building;
mod bus;
mod debug;
mod intersection;
mod lane;
mod parking_lot;
mod person;
mod trip;

pub struct InfoPanel {
    tab: Tab,
    time: Time,
    is_paused: bool,
    panel: Panel,

    unzoomed: Drawable,
    zoomed: Drawable,

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

    BusStatus(CarID),
    BusStop(BusStopID),
    BusRoute(BusRouteID),

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

    LaneInfo(LaneID),
    LaneDebug(LaneID),
    LaneTraffic(LaneID, DataOptions),
}

impl Tab {
    pub fn from_id(app: &App, id: ID) -> Tab {
        match id {
            ID::Road(_) => unreachable!(),
            ID::Lane(l) => match app.session.info_panel_tab["lane"] {
                "info" => Tab::LaneInfo(l),
                "debug" => Tab::LaneDebug(l),
                "traffic" => Tab::LaneTraffic(l, DataOptions::new()),
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
                } else if c.1 == VehicleType::Bus || c.1 == VehicleType::Train {
                    match app.session.info_panel_tab["bus"] {
                        "status" => Tab::BusStatus(c),
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
            ID::BusStop(bs) => Tab::BusStop(bs),
            ID::Area(a) => Tab::Area(a),
        }
    }

    fn to_id(&self, app: &App) -> Option<ID> {
        match self {
            Tab::PersonTrips(p, _) | Tab::PersonBio(p) | Tab::PersonSchedule(p) => {
                match app.primary.sim.get_person(*p).state {
                    PersonState::Inside(b) => Some(ID::Building(b)),
                    PersonState::Trip(t) => app
                        .primary
                        .sim
                        .trip_to_agent(t)
                        .ok()
                        .map(|a| ID::from_agent(a)),
                    _ => None,
                }
            }
            Tab::BusStatus(c) => Some(ID::Car(*c)),
            Tab::BusStop(bs) => Some(ID::BusStop(*bs)),
            Tab::BusRoute(_) => None,
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
            | Tab::IntersectionTrafficSignal(i) => Some(ID::Intersection(*i)),
            Tab::LaneInfo(l) | Tab::LaneDebug(l) | Tab::LaneTraffic(l, _) => Some(ID::Lane(*l)),
        }
    }

    fn changed_settings(&self, c: &Panel) -> Option<Tab> {
        // Avoid an occasionally expensive clone.
        match self {
            Tab::IntersectionTraffic(_, _)
            | Tab::IntersectionDelay(_, _, _)
            | Tab::IntersectionArrivals(_, _)
            | Tab::LaneTraffic(_, _) => {}
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
            _ => unreachable!(),
        }
        Some(new_tab)
    }

    fn variant(&self) -> (&'static str, &'static str) {
        match self {
            Tab::PersonTrips(_, _) => ("person", "trips"),
            Tab::PersonBio(_) => ("person", "bio"),
            Tab::PersonSchedule(_) => ("person", "schedule"),
            Tab::BusStatus(_) => ("bus", "status"),
            Tab::BusStop(_) => ("bus stop", "info"),
            Tab::BusRoute(_) => ("bus route", "info"),
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
            Tab::LaneInfo(_) => ("lane", "info"),
            Tab::LaneDebug(_) => ("lane", "debug"),
            Tab::LaneTraffic(_, _) => ("lane", "traffic"),
        }
    }
}

// TODO Name sucks
pub struct Details {
    pub unzoomed: GeomBatch,
    pub zoomed: GeomBatch,
    pub hyperlinks: HashMap<String, Tab>,
    pub warpers: HashMap<String, ID>,
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
            unzoomed: GeomBatch::new(),
            zoomed: GeomBatch::new(),
            hyperlinks: HashMap::new(),
            warpers: HashMap::new(),
            time_warpers: HashMap::new(),
            can_jump_to_time: ctx_actions.gameplay_mode().can_jump_to_time(),
        };

        let (mut col, main_tab) = match tab {
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
            Tab::BusStatus(c) => (bus::bus_status(ctx, app, &mut details, c), true),
            Tab::BusStop(bs) => (bus::stop(ctx, app, &mut details, bs), true),
            Tab::BusRoute(br) => (bus::route(ctx, app, &mut details, br), true),
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
            Tab::LaneInfo(l) => (lane::info(ctx, app, &mut details, l), true),
            Tab::LaneDebug(l) => (lane::debug(ctx, app, &mut details, l), false),
            Tab::LaneTraffic(l, ref opts) => {
                (lane::traffic(ctx, app, &mut details, l, opts), false)
            }
        };
        let maybe_id = tab.to_id(app);
        let mut cached_actions = Vec::new();
        if main_tab {
            if let Some(id) = maybe_id.clone() {
                for (key, label) in ctx_actions.actions(app, id) {
                    cached_actions.push(key);
                    col.push(hotkey_btn(ctx, app, label, key));
                }
            }
        }

        // Highlight something?
        if let Some((id, outline)) = maybe_id.clone().and_then(|id| {
            app.primary
                .draw_map
                .get_obj(ctx, id.clone(), app, &mut app.primary.agents.borrow_mut())
                .map(|obj| (id, obj.get_outline(&app.primary.map)))
        }) {
            // Different selection styles for different objects.
            match id {
                ID::Car(_) | ID::Pedestrian(_) | ID::PedCrowd(_) => {
                    // Some objects are much wider/taller than others
                    let multiplier = match id {
                        ID::Car(c) => {
                            if c.1 == VehicleType::Bike {
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
                    details.unzoomed.push(
                        app.cs.current_object.alpha(0.5),
                        Circle::new(bounds.center(), radius).to_polygon(),
                    );
                    match Circle::new(bounds.center(), radius).to_outline(Distance::meters(0.3)) {
                        Ok(poly) => {
                            details.unzoomed.push(app.cs.current_object, poly.clone());
                            details.zoomed.push(app.cs.current_object, poly.clone());
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
                        .unzoomed
                        .push(app.cs.perma_selected_object, outline.clone());
                    details.zoomed.push(app.cs.perma_selected_object, outline);
                }
            }
        }

        InfoPanel {
            tab,
            time: app.primary.sim.time(),
            is_paused: ctx_actions.is_paused(),
            panel: Panel::new(Widget::col(col).bg(Color::hex("#5B5B5B")).padding(16))
                .aligned(
                    HorizontalAlignment::Percent(0.02),
                    VerticalAlignment::Percent(0.2),
                )
                // TODO Some headings are too wide.. Intersection #xyz (Traffic signals)
                .exact_size_percent(30, 60)
                .build_custom(ctx),
            unzoomed: details.unzoomed.upload(ctx),
            zoomed: details.zoomed.upload(ctx),
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
        // Can click on the map to cancel
        if ctx.canvas.get_cursor_in_map_space().is_some()
            && app.primary.current_selection.is_none()
            && app.per_obj.left_click(ctx, "stop showing info")
        {
            return (true, None);
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
                            Some(Transition::Push(Warping::new(
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
                        Some(Transition::Push(Warping::new(
                            ctx,
                            app.primary.canonical_point(id.clone()).unwrap(),
                            Some(10.0),
                            None,
                            &mut app.primary,
                        ))),
                    )
                } else if let Some((trip, time)) = self.time_warpers.get(&action) {
                    let trip = *trip;
                    let time = *time;
                    let person = app.primary.sim.trip_to_person(trip).unwrap();
                    // When executed, this assumes the SandboxMode is the top of the stack. It'll
                    // reopen the info panel, then launch the jump-to-time UI.
                    let jump_to_time =
                        Transition::ReplaceWithData(Box::new(move |state, ctx, app| {
                            let mut sandbox = state.downcast::<SandboxMode>().ok().unwrap();

                            let mut actions = sandbox.contextual_actions();
                            sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                ctx,
                                app,
                                Tab::PersonTrips(person, OpenTrip::single(trip)),
                                &mut actions,
                            );

                            vec![sandbox, TimeWarpScreen::new(ctx, app, time, None)]
                        }));

                    if time >= app.primary.sim.time() {
                        return (false, Some(jump_to_time));
                    }

                    // We need to first rewind the simulation
                    (
                        false,
                        Some(Transition::Replace(SandboxMode::async_new(
                            ctx,
                            app,
                            ctx_actions.gameplay_mode(),
                            Box::new(move |_, _| vec![jump_to_time]),
                        ))),
                    )
                } else if let Some(url) = action.strip_prefix("open ") {
                    open_browser(url.to_string());
                    (false, None)
                } else if let Some(x) = action.strip_prefix("edit BusRoute #") {
                    (
                        false,
                        Some(Transition::Multi(vec![
                            Transition::Push(EditMode::new(ctx, app, ctx_actions.gameplay_mode())),
                            Transition::Push(RouteEditor::new(
                                ctx,
                                app,
                                BusRouteID(x.parse::<usize>().unwrap()),
                            )),
                        ])),
                    )
                } else if action == "Explore demand across all traffic signals" {
                    (
                        false,
                        Some(Transition::Push(dashboards::TrafficSignalDemand::new(
                            ctx, app,
                        ))),
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
                } else {
                    if let Some(id) = maybe_id {
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
            }
            _ => {
                // Maybe a non-click action should change the tab. Aka, checkboxes/dropdowns/etc on
                // a tab.
                if let Some(new_tab) = self.tab.changed_settings(&self.panel) {
                    let mut new = InfoPanel::new(ctx, app, new_tab, ctx_actions);
                    new.panel.restore(ctx, &self.panel);
                    *self = new;
                }

                (false, None)
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        if g.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            g.redraw(&self.unzoomed);
        } else {
            g.redraw(&self.zoomed);
        }
    }

    pub fn active_keys(&self) -> &Vec<Key> {
        &self.cached_actions
    }

    pub fn active_id(&self, app: &App) -> Option<ID> {
        self.tab.to_id(app)
    }
}

fn make_table<I: Into<String>>(ctx: &EventCtx, rows: Vec<(I, String)>) -> Vec<Widget> {
    rows.into_iter()
        .map(|(k, v)| {
            Widget::row(vec![
                Line(k).secondary().draw(ctx),
                // TODO not quite...
                v.draw_text(ctx).centered_vert().align_right(),
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
        Line(title).small_heading().draw(ctx),
        LinePlot::new(ctx, series, plot_opts),
    ])
    .padding(10)
    .bg(app.cs.inner_panel)
    .outline(2.0, Color::WHITE)
}

fn make_tabs(
    ctx: &EventCtx,
    hyperlinks: &mut HashMap<String, Tab>,
    current_tab: Tab,
    tabs: Vec<(&str, Tab)>,
) -> Widget {
    let mut row = Vec::new();
    for (name, link) in tabs {
        if current_tab.variant() == link.variant() {
            row.push(Btn::text_bg2(name).inactive(ctx).centered_vert());
        } else {
            hyperlinks.insert(name.to_string(), link);
            row.push(Btn::text_bg2(name).build_def(ctx, None).centered_vert());
        }
    }
    // TODO Centered, but actually, we need to set the padding of each button to divide the
    // available space evenly. Fancy fill rules... hmmm.
    Widget::custom_row(row).bg(Color::WHITE).margin_vert(16)
}

fn header_btns(ctx: &EventCtx) -> Widget {
    Widget::row(vec![
        Btn::svg_def("system/assets/tools/location.svg").build(ctx, "jump to object", Key::J),
        Btn::close(ctx),
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
            Checkbox::custom_checkbox(
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
                Checkbox::switch(ctx, "Show full day", None, self.show_end_of_day)
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
