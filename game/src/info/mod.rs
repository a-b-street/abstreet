mod building;
mod bus;
mod debug;
mod intersection;
mod lane;
mod parking_lot;
mod person;
mod trip;

use crate::app::App;
use crate::common::Warping;
use crate::game::Transition;
use crate::helpers::{color_for_mode, hotkey_btn, ID};
use crate::sandbox::{SandboxMode, TimeWarpScreen};
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, LinePlot, Outcome, PlotOptions, Series, TextExt,
    VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Time};
use map_model::{AreaID, BuildingID, BusStopID, IntersectionID, LaneID, ParkingLotID};
use sim::{
    AgentID, Analytics, CarID, ParkingSpot, PedestrianID, PersonID, PersonState, TripID, TripMode,
    VehicleType,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
pub use trip::OpenTrip;

pub struct InfoPanel {
    tab: Tab,
    time: Time,
    is_paused: bool,
    composite: Composite,

    unzoomed: Drawable,
    zoomed: Drawable,

    hyperlinks: HashMap<String, Tab>,
    warpers: HashMap<String, ID>,
    time_warpers: HashMap<String, (TripID, Time)>,

    // For drawing the OSD only
    cached_actions: Vec<Key>,
}

// TODO We need a separate, weaker form of PartialEq for this to detect when we're on the "current"
// tab.
#[derive(Clone, PartialEq)]
pub enum Tab {
    // What trips are open? For finished trips, show the timeline in the current simulation if
    // true, prebaked if false.
    PersonTrips(PersonID, BTreeMap<TripID, OpenTrip>),
    PersonBio(PersonID),
    PersonSchedule(PersonID),

    BusStatus(CarID),
    BusDelays(CarID),
    BusStop(BusStopID),

    ParkedCar(CarID),

    BldgInfo(BuildingID),
    BldgPeople(BuildingID),

    ParkingLot(ParkingLotID),

    Crowd(Vec<PedestrianID>),

    Area(AreaID),

    IntersectionInfo(IntersectionID),
    IntersectionTraffic(IntersectionID, DataOptions),
    IntersectionDelay(IntersectionID, DataOptions),
    IntersectionDemand(IntersectionID),

    LaneInfo(LaneID),
    LaneDebug(LaneID),
    LaneTraffic(LaneID, DataOptions),
}

impl Tab {
    pub fn from_id(app: &App, id: ID) -> Tab {
        match id {
            ID::Road(_) => unreachable!(),
            ID::Lane(l) => Tab::LaneInfo(l),
            ID::Intersection(i) => Tab::IntersectionInfo(i),
            ID::Building(b) => Tab::BldgInfo(b),
            ID::ParkingLot(b) => Tab::ParkingLot(b),
            ID::Car(c) => {
                if let Some(p) = app.primary.sim.agent_to_person(AgentID::Car(c)) {
                    Tab::PersonTrips(
                        p,
                        OpenTrip::single(app.primary.sim.agent_to_trip(AgentID::Car(c)).unwrap()),
                    )
                } else if c.1 == VehicleType::Bus {
                    Tab::BusStatus(c)
                } else {
                    Tab::ParkedCar(c)
                }
            }
            ID::Pedestrian(p) => Tab::PersonTrips(
                app.primary
                    .sim
                    .agent_to_person(AgentID::Pedestrian(p))
                    .unwrap(),
                OpenTrip::single(
                    app.primary
                        .sim
                        .agent_to_trip(AgentID::Pedestrian(p))
                        .unwrap(),
                ),
            ),
            ID::PedCrowd(members) => Tab::Crowd(members),
            ID::BusStop(bs) => Tab::BusStop(bs),
            ID::Area(a) => Tab::Area(a),
        }
    }

    // TODO Temporary hack until object actions go away.
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
            Tab::BusStatus(c) | Tab::BusDelays(c) => Some(ID::Car(*c)),
            Tab::BusStop(bs) => Some(ID::BusStop(*bs)),
            // TODO If a parked car becomes in use while the panel is open, should update the panel
            // better.
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
            | Tab::IntersectionDelay(i, _)
            | Tab::IntersectionDemand(i) => Some(ID::Intersection(*i)),
            Tab::LaneInfo(l) | Tab::LaneDebug(l) | Tab::LaneTraffic(l, _) => Some(ID::Lane(*l)),
        }
    }

    fn changed_settings(&self, c: &Composite) -> Option<Tab> {
        // Avoid an occasionally expensive clone.
        match self {
            Tab::IntersectionTraffic(_, _)
            | Tab::IntersectionDelay(_, _)
            | Tab::LaneTraffic(_, _) => {}
            _ => {
                return None;
            }
        }

        let mut new_tab = self.clone();
        match new_tab {
            Tab::IntersectionTraffic(_, ref mut opts)
            | Tab::IntersectionDelay(_, ref mut opts)
            | Tab::LaneTraffic(_, ref mut opts) => {
                *opts = DataOptions::from_controls(c);
            }
            _ => unreachable!(),
        }
        if &new_tab == self {
            None
        } else {
            Some(new_tab)
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
}

impl InfoPanel {
    pub fn new(
        ctx: &mut EventCtx,
        app: &App,
        mut tab: Tab,
        ctx_actions: &mut dyn ContextualActions,
    ) -> InfoPanel {
        let mut details = Details {
            unzoomed: GeomBatch::new(),
            zoomed: GeomBatch::new(),
            hyperlinks: HashMap::new(),
            warpers: HashMap::new(),
            time_warpers: HashMap::new(),
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
            Tab::BusDelays(c) => (bus::bus_delays(ctx, app, &mut details, c), true),
            Tab::BusStop(bs) => (bus::stop(ctx, app, &mut details, bs), true),
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
            Tab::IntersectionDelay(i, ref opts) => {
                (intersection::delay(ctx, app, &mut details, i, opts), false)
            }
            Tab::IntersectionDemand(i) => (
                intersection::current_demand(ctx, app, &mut details, i),
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
                    col.push(hotkey_btn(ctx, app, label, key).margin(5));
                }
            }
        }

        // Highlight something?
        if let Some((id, outline)) = maybe_id.clone().and_then(|id| {
            app.primary
                .draw_map
                .get_obj(
                    id.clone(),
                    app,
                    &mut app.primary.draw_map.agents.borrow_mut(),
                    ctx.prerender,
                )
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
                    details.unzoomed.push(
                        app.cs.current_object,
                        Circle::outline(bounds.center(), radius, Distance::meters(0.3)),
                    );
                    details.zoomed.push(
                        app.cs.current_object,
                        Circle::outline(bounds.center(), radius, Distance::meters(0.3)),
                    );

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
            composite: Composite::new(Widget::col(col).bg(Color::hex("#5B5B5B")).padding(16))
                .aligned(
                    HorizontalAlignment::Percent(0.02),
                    VerticalAlignment::Percent(0.2),
                )
                // TODO Some headings are too wide.. Intersection #xyz (Traffic signals)
                .exact_size_percent(30, 60)
                .build(ctx),
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
            new.composite.restore(ctx, &self.composite);
            *self = new;
            return (false, None);
        }

        let maybe_id = self.tab.to_id(app);
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(action)) => {
                if let Some(new_tab) = self.hyperlinks.get(&action).cloned() {
                    let mut new = InfoPanel::new(ctx, app, new_tab, ctx_actions);
                    // TODO Most cases use changed_settings, but one doesn't. Detect that
                    // "sameness" here.
                    if let (Tab::PersonTrips(p1, _), Tab::PersonTrips(p2, _)) =
                        (&self.tab, &new.tab)
                    {
                        if p1 == p2 {
                            new.composite.restore(ctx, &self.composite);
                        }
                    }
                    *self = new;
                    return (false, None);
                } else if action == "close info" {
                    (true, None)
                } else if action == "jump to object" {
                    // TODO Messy way of doing this
                    if let Some(id) = self.tab.to_id(app) {
                        return (
                            false,
                            Some(Transition::Push(Warping::new(
                                ctx,
                                id.canonical_point(&app.primary).unwrap(),
                                Some(10.0),
                                Some(id),
                                &mut app.primary,
                            ))),
                        );
                    } else {
                        return (false, None);
                    }
                } else if action.starts_with("examine trip phase") {
                    // Don't do anything! Just using buttons for convenient tooltips.
                    (false, None)
                } else if let Some(id) = self.warpers.get(&action) {
                    (
                        false,
                        Some(Transition::Push(Warping::new(
                            ctx,
                            id.canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            None,
                            &mut app.primary,
                        ))),
                    )
                } else if let Some((trip, time)) = self.time_warpers.get(&action) {
                    let trip = *trip;
                    let time = *time;
                    let person = app.primary.sim.trip_to_person(trip);

                    (
                        false,
                        Some(Transition::ReplaceWithData(Box::new(
                            move |state, ctx, app| {
                                let mut sandbox = state.downcast::<SandboxMode>().ok().unwrap();

                                if time < app.primary.sim.time() {
                                    sandbox = ctx.loading_screen("rewind simulation", |ctx, _| {
                                        Box::new(SandboxMode::new(ctx, app, sandbox.gameplay_mode))
                                    });
                                }

                                let mut actions = sandbox.contextual_actions();
                                sandbox.controls.common.as_mut().unwrap().launch_info_panel(
                                    ctx,
                                    app,
                                    Tab::PersonTrips(person, OpenTrip::single(trip)),
                                    &mut actions,
                                );

                                vec![sandbox, TimeWarpScreen::new(ctx, app, time, false)]
                            },
                        ))),
                    )
                } else if action == "copy OriginalLane" {
                    // TODO Not happy about this :(
                    lane::copy_orig_lane(
                        app,
                        match maybe_id {
                            Some(ID::Lane(l)) => l,
                            _ => unreachable!(),
                        },
                    );
                    return (false, None);
                } else {
                    let mut close_panel = true;
                    let t =
                        ctx_actions.execute(ctx, app, maybe_id.unwrap(), action, &mut close_panel);
                    (close_panel, Some(t))
                }
            }
            None => {
                // Maybe a non-click action should change the tab. Aka, checkboxes/dropdowns/etc on
                // a tab.
                if let Some(new_tab) = self.tab.changed_settings(&self.composite) {
                    let mut new = InfoPanel::new(ctx, app, new_tab, ctx_actions);
                    new.composite.restore(ctx, &self.composite);
                    *self = new;
                }

                (false, None)
            }
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
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

fn make_table<I: Into<String>>(ctx: &EventCtx, rows: impl Iterator<Item=(I, String)>) -> Vec<Widget> {
    rows
        .map(|(k, v)| {
            Widget::row(vec![
                Line(k).secondary().draw(ctx),
                // TODO not quite...
                v.draw_text(ctx).centered_vert().align_right(),
            ])
        })
        .collect()

    // Attempt two
    /*let mut keys = Text::new();
    let mut values = Text::new();
    for (k, v) in rows {
        keys.add(Line(k));
        values.add(Line(v));
    }
    vec![Widget::row(vec![
        keys.draw(ctx),
        values.draw(ctx).centered_vert().bg(Color::GREEN),
    ])]*/
}

fn throughput<F: Fn(&Analytics) -> Vec<(TripMode, Vec<(Time, usize)>)>>(
    ctx: &EventCtx,
    app: &App,
    get_data: F,
    opts: &DataOptions,
) -> Widget {
    let mut series = get_data(app.primary.sim.get_analytics())
        .into_iter()
        .map(|(m, pts)| Series {
            label: m.noun().to_string(),
            color: color_for_mode(app, m),
            pts,
        })
        .collect::<Vec<_>>();
    if opts.show_before {
        // TODO Ahh these colors don't show up differently at all.
        for (m, pts) in get_data(app.prebaked()) {
            series.push(Series {
                label: m.noun().to_string(),
                color: color_for_mode(app, m).alpha(0.3),
                pts,
            });
        }
    }

    let mut plot_opts = PlotOptions::filterable();
    plot_opts.disabled = opts.disabled_series();
    Widget::col(vec![
        Line("Number of crossing agents per hour")
            .small_heading()
            .draw(ctx)
            .margin_below(10),
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
        if current_tab == link {
            row.push(Btn::text_bg2(name).inactive(ctx));
        } else {
            hyperlinks.insert(name.to_string(), link);
            row.push(Btn::text_bg2(name).build_def(ctx, None));
        }
    }
    // TODO Centered, but actually, we need to set the padding of each button to divide the
    // available space evenly. Fancy fill rules... hmmm.
    Widget::row(row).bg(Color::WHITE).margin_vert(16)
}

fn header_btns(ctx: &EventCtx) -> Widget {
    Widget::row(vec![
        Btn::svg_def("../data/system/assets/tools/location.svg")
            .build(ctx, "jump to object", hotkey(Key::J))
            .margin(5),
        Btn::plaintext("X").build(ctx, "close info", hotkey(Key::Escape)),
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
}

#[derive(Clone, PartialEq)]
pub struct DataOptions {
    pub show_before: bool,
    pub show_end_of_day: bool,
    disabled_modes: BTreeSet<TripMode>,
}

impl DataOptions {
    pub fn new() -> DataOptions {
        DataOptions {
            show_before: false,
            show_end_of_day: false,
            disabled_modes: BTreeSet::new(),
        }
    }

    pub fn to_controls(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        if app.has_prebaked().is_none() {
            return Widget::nothing();
        }
        Widget::row(vec![
            Checkbox::custom_text(
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
                Checkbox::text(ctx, "Show full day", None, self.show_end_of_day)
            } else {
                Widget::nothing()
            },
        ])
        .evenly_spaced()
    }

    pub fn from_controls(c: &Composite) -> DataOptions {
        let show_before =
            c.has_widget("Show before changes") && c.is_checked("Show before changes");
        let mut disabled_modes = BTreeSet::new();
        for m in TripMode::all() {
            let label = m.noun();
            if c.has_widget(label) && !c.is_checked(label) {
                disabled_modes.insert(m);
            }
        }
        DataOptions {
            show_before,
            show_end_of_day: show_before
                && c.has_widget("Show full day")
                && c.is_checked("Show full day"),
            disabled_modes,
        }
    }

    pub fn disabled_series(&self) -> HashSet<String> {
        self.disabled_modes
            .iter()
            .map(|m| m.noun().to_string())
            .collect()
    }
}
