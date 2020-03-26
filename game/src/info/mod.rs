mod agents;
mod building;
mod bus_stop;
mod debug;
mod intersection;
mod lane;
mod person;
mod trip;

use crate::app::App;
use crate::colors;
use crate::common::Warping;
use crate::game::{msg, State, Transition, WizardState};
use crate::helpers::ID;
use crate::render::MIN_ZOOM_FOR_DETAIL;
use crate::sandbox::{SandboxMode, SpeedControls};
use ezgui::{
    hotkey, Btn, Color, Composite, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, Plot, PlotOptions, Series, Text, TextExt, VerticalAlignment, Widget,
};
use geom::{Circle, Distance, Time};
use sim::{AgentID, Analytics, TripID, TripMode, TripResult, VehicleType};
use std::collections::{BTreeMap, HashMap};

pub struct InfoPanel {
    pub id: ID,
    tab: InfoTab,
    time: Time,
    composite: Composite,

    also_draw: Drawable,
    trip_details: Option<TripDetails>,

    actions: Vec<(Key, String)>,
    hyperlinks: HashMap<String, (ID, InfoTab)>,
    warpers: HashMap<String, ID>,
}

// TODO Safer to expand out ID cases here
// The PartialEq is ONLY used for determining when we're on the current tab. So maybe omit data.
#[derive(Clone, PartialEq)]
pub enum InfoTab {
    Nil,
    Bldg(building::Tab),
    Lane(lane::Tab),
    Intersection(intersection::Tab),
    Agent(agents::Tab),
    Trip(trip::Tab),
}

pub struct TripDetails {
    id: TripID,
    unzoomed: Drawable,
    zoomed: Drawable,
}

impl InfoPanel {
    pub fn new(
        id: ID,
        tab: InfoTab,
        ctx: &mut EventCtx,
        app: &App,
        mut actions: Vec<(Key, String)>,
        maybe_speed: Option<&mut SpeedControls>,
    ) -> InfoPanel {
        if maybe_speed.map(|s| s.is_paused()).unwrap_or(false)
            && id.agent_id().is_some()
            && actions
                .get(0)
                .map(|(_, a)| a != "follow agent")
                .unwrap_or(true)
        {
            actions.insert(0, (Key::F, "follow agent".to_string()));
        }

        let action_btns = actions
            .iter()
            .map(|(key, label)| {
                let mut txt = Text::new();
                txt.append(Line(key.describe()).fg(ezgui::HOTKEY_COLOR));
                txt.append(Line(format!(" - {}", label)));
                Btn::text_bg(label, txt, colors::SECTION_BG, colors::HOVERING)
                    .build_def(ctx, hotkey(*key))
                    .margin(5)
            })
            .collect();

        let mut batch = GeomBatch::new();
        // TODO Handle transitions between peds and crowds better
        if let Some(obj) = app.primary.draw_map.get_obj(
            id.clone(),
            app,
            &mut app.primary.draw_map.agents.borrow_mut(),
            ctx.prerender,
        ) {
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
                    let bounds = obj.get_outline(&app.primary.map).get_bounds();
                    let radius = multiplier * Distance::meters(bounds.width().max(bounds.height()));
                    batch.push(
                        app.cs.get_def("current object", Color::WHITE).alpha(0.5),
                        Circle::new(bounds.center(), radius).to_polygon(),
                    );
                    batch.push(
                        app.cs.get("current object"),
                        Circle::outline(bounds.center(), radius, Distance::meters(0.3)),
                    );

                    // TODO And actually, don't cover up the agent. The Renderable API isn't quite
                    // conducive to doing this yet.
                }
                _ => {
                    batch.push(
                        app.cs.get_def("perma selected thing", Color::BLUE),
                        obj.get_outline(&app.primary.map),
                    );
                }
            }
        }

        let header_btns = Widget::row(vec![
            Btn::svg_def("../data/system/assets/tools/location.svg")
                .build(ctx, "jump to object", hotkey(Key::J))
                .margin(5),
            Btn::text_fg("X").build(ctx, "close info", hotkey(Key::Escape)),
        ])
        .align_right();
        let mut hyperlinks = HashMap::new();
        let mut warpers = HashMap::new();
        let (col, trip_details) = match id.clone() {
            ID::Road(_) => unreachable!(),
            ID::Lane(id) => (
                lane::info(
                    ctx,
                    app,
                    id,
                    tab.clone(),
                    header_btns,
                    action_btns,
                    &mut hyperlinks,
                ),
                None,
            ),
            ID::Intersection(id) => (
                intersection::info(
                    ctx,
                    app,
                    id,
                    tab.clone(),
                    header_btns,
                    action_btns,
                    &mut hyperlinks,
                ),
                None,
            ),
            ID::Turn(_) => unreachable!(),
            ID::Building(id) => (
                building::info(
                    ctx,
                    app,
                    id,
                    tab.clone(),
                    header_btns,
                    action_btns,
                    &mut batch,
                    &mut hyperlinks,
                ),
                None,
            ),
            ID::Car(id) => agents::car_info(
                ctx,
                app,
                id,
                tab.clone(),
                header_btns,
                action_btns,
                &mut batch,
                &mut hyperlinks,
                &mut warpers,
            ),
            ID::Pedestrian(id) => agents::ped_info(
                ctx,
                app,
                id,
                tab.clone(),
                header_btns,
                action_btns,
                &mut hyperlinks,
                &mut warpers,
            ),
            ID::PedCrowd(members) => {
                assert!(action_btns.is_empty());
                (
                    agents::crowd_info(
                        ctx,
                        app,
                        members,
                        tab.clone(),
                        header_btns,
                        &mut hyperlinks,
                        &mut warpers,
                    ),
                    None,
                )
            }
            ID::BusStop(id) => (bus_stop::info(ctx, app, id, header_btns, action_btns), None),
            ID::Area(id) => (debug::area(ctx, app, id, header_btns, action_btns), None),
            ID::ExtraShape(id) => (
                debug::extra_shape(ctx, app, id, header_btns, action_btns),
                None,
            ),
            ID::Trip(id) => match app.primary.sim.trip_to_agent(id).ok() {
                Some(AgentID::Car(c)) => agents::car_info(
                    ctx,
                    app,
                    c,
                    tab.clone(),
                    header_btns,
                    Vec::new(),
                    &mut batch,
                    &mut hyperlinks,
                    &mut warpers,
                ),
                Some(AgentID::Pedestrian(p)) => agents::ped_info(
                    ctx,
                    app,
                    p,
                    tab.clone(),
                    header_btns,
                    Vec::new(),
                    &mut hyperlinks,
                    &mut warpers,
                ),
                None => trip::inactive_info(
                    ctx,
                    app,
                    id,
                    tab.clone(),
                    action_btns,
                    &mut hyperlinks,
                    &mut warpers,
                ),
            },
            ID::Person(id) => (
                person::info(
                    ctx,
                    app,
                    id,
                    Some(header_btns),
                    action_btns,
                    &mut hyperlinks,
                    &mut warpers,
                ),
                None,
            ),
        };

        // Follow the agent. When the sim is paused, this lets the player naturally pan away,
        // because the InfoPanel isn't being updated.
        if let Some(pt) = id
            .agent_id()
            .and_then(|a| app.primary.sim.canonical_pt_for_agent(a, &app.primary.map))
        {
            ctx.canvas.center_on_map_pt(pt);
        }

        InfoPanel {
            id,
            tab,
            actions,
            trip_details,
            time: app.primary.sim.time(),
            composite: Composite::new(Widget::col(col).bg(colors::PANEL_BG).padding(10))
                .aligned(
                    HorizontalAlignment::Percent(0.02),
                    VerticalAlignment::Percent(0.2),
                )
                .max_size_percent(35, 60)
                .build(ctx),
            also_draw: batch.upload(ctx),
            hyperlinks,
            warpers,
        }
    }

    // (Are we done, optional transition)
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        maybe_speed: Option<&mut SpeedControls>,
    ) -> (bool, Option<Transition>) {
        // Can click on the map to cancel
        if ctx.canvas.get_cursor_in_map_space().is_some()
            && app.primary.current_selection.is_none()
            && app.per_obj.left_click(ctx, "stop showing info")
        {
            return (true, None);
        }

        // Live update?
        if app.primary.sim.time() != self.time {
            if let Some(a) = self.id.agent_id() {
                if let Some(ref details) = self.trip_details {
                    match app.primary.sim.trip_to_agent(details.id) {
                        TripResult::Ok(a2) => {
                            if a != a2 {
                                if !app.primary.sim.does_agent_exist(a) {
                                    *self = InfoPanel::new(
                                        ID::from_agent(a2),
                                        InfoTab::Nil,
                                        ctx,
                                        app,
                                        Vec::new(),
                                        maybe_speed,
                                    );
                                    return (
                                        false,
                                        Some(Transition::Push(msg(
                                            "The trip is transitioning to a new mode",
                                            vec![format!(
                                                "{} is now {}, following them instead",
                                                agent_name(a),
                                                agent_name(a2)
                                            )],
                                        ))),
                                    );
                                }

                                return (true, Some(Transition::Push(trip_transition(a, a2))));
                            }
                        }
                        TripResult::TripDone => {
                            *self = InfoPanel::new(
                                ID::Trip(details.id),
                                InfoTab::Nil,
                                ctx,
                                app,
                                Vec::new(),
                                maybe_speed,
                            );
                            return (
                                false,
                                Some(Transition::Push(msg(
                                    "Trip complete",
                                    vec![format!(
                                        "{} has finished their trip. Say goodbye!",
                                        agent_name(a)
                                    )],
                                ))),
                            );
                        }
                        TripResult::TripNotStarted | TripResult::TripDoesntExist => unreachable!(),
                        // Just wait a moment for trip_transition to kick in...
                        TripResult::ModeChange => {}
                    }
                }
            }
            // TODO Detect crowds changing here maybe

            let preserve_scroll = self.composite.preserve_scroll();
            *self = InfoPanel::new(
                self.id.clone(),
                self.tab.clone(),
                ctx,
                app,
                self.actions.clone(),
                maybe_speed,
            );
            self.composite.restore_scroll(ctx, preserve_scroll);
            return (false, None);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(action)) => {
                if let Some((new_id, tab)) = self.hyperlinks.get(&action).cloned() {
                    *self = InfoPanel::new(
                        new_id.clone(),
                        tab,
                        ctx,
                        app,
                        if self.id == new_id {
                            self.actions.clone()
                        } else {
                            Vec::new()
                        },
                        maybe_speed,
                    );
                    return (false, None);
                } else if action == "close info" {
                    (true, None)
                } else if action == "jump to object" {
                    (
                        false,
                        Some(Transition::Push(Warping::new(
                            ctx,
                            self.id.canonical_point(&app.primary).unwrap(),
                            Some(10.0),
                            Some(self.id.clone()),
                            &mut app.primary,
                        ))),
                    )
                } else if action == "follow agent" {
                    maybe_speed.unwrap().resume_realtime(ctx);
                    (false, None)
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
                } else if action == "Info" {
                    // Genericish
                    *self = InfoPanel::new(
                        self.id.clone(),
                        InfoTab::Nil,
                        ctx,
                        app,
                        self.actions.clone(),
                        maybe_speed,
                    );
                    return (false, None);
                } else {
                    app.primary.current_selection = Some(self.id.clone());
                    (true, Some(Transition::ApplyObjectAction(action)))
                }
            }
            None => (false, None),
        }
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
        if let Some(ref details) = self.trip_details {
            if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
                g.redraw(&details.unzoomed);
            } else {
                g.redraw(&details.zoomed);
            }
        }
        g.redraw(&self.also_draw);
    }
}

fn make_table<I: Into<String>>(ctx: &EventCtx, rows: Vec<(I, String)>) -> Vec<Widget> {
    rows.into_iter()
        .map(|(k, v)| {
            Widget::row(vec![
                Line(k).draw(ctx),
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

fn throughput<F: Fn(&Analytics, Time) -> BTreeMap<TripMode, Vec<(Time, usize)>>>(
    ctx: &EventCtx,
    app: &App,
    get_data: F,
) -> Widget {
    let mut series = get_data(app.primary.sim.get_analytics(), app.primary.sim.time())
        .into_iter()
        .map(|(m, pts)| Series {
            label: m.to_string(),
            color: color_for_mode(m, app),
            pts,
        })
        .collect::<Vec<_>>();
    if app.has_prebaked().is_some() {
        // TODO Ahh these colors don't show up differently at all.
        for (m, pts) in get_data(app.prebaked(), Time::END_OF_DAY) {
            series.push(Series {
                label: format!("{} (baseline)", m),
                color: color_for_mode(m, app).alpha(0.3),
                pts,
            });
        }
    }

    Plot::new_usize(ctx, series, PlotOptions::new())
}

fn color_for_mode(m: TripMode, app: &App) -> Color {
    match m {
        TripMode::Walk => app.cs.get("unzoomed pedestrian"),
        TripMode::Bike => app.cs.get("unzoomed bike"),
        TripMode::Transit => app.cs.get("unzoomed bus"),
        TripMode::Drive => app.cs.get("unzoomed car"),
    }
}

fn trip_transition(from: AgentID, to: AgentID) -> Box<dyn State> {
    WizardState::new(Box::new(move |wiz, ctx, _| {
        let orig = format!("keep following {}", agent_name(from));
        let change = format!("follow {} instead", agent_name(to));

        let id = if wiz
            .wrap(ctx)
            .choose_string("The trip is transitioning to a new mode", || {
                vec![orig.clone(), change.clone()]
            })?
            == orig
        {
            ID::from_agent(from)
        } else {
            ID::from_agent(to)
        };
        Some(Transition::PopWithData(Box::new(move |state, app, ctx| {
            state
                .downcast_mut::<SandboxMode>()
                .unwrap()
                .controls
                .common
                .as_mut()
                .unwrap()
                .launch_info_panel(id, ctx, app);
        })))
    }))
}

fn agent_name(a: AgentID) -> String {
    match a {
        AgentID::Car(c) => match c.1 {
            VehicleType::Car => format!("Car #{}", c.0),
            VehicleType::Bike => format!("Bike #{}", c.0),
            VehicleType::Bus => format!("Bus #{}", c.0),
        },
        AgentID::Pedestrian(p) => format!("Pedestrian #{}", p.0),
    }
}

fn make_tabs(
    ctx: &EventCtx,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
    id: ID,
    current_tab: InfoTab,
    tabs: Vec<(&str, InfoTab)>,
) -> Widget {
    let mut row = Vec::new();
    for (name, link) in tabs {
        if current_tab == link {
            row.push(Btn::text_bg2(name).inactive(ctx));
        } else {
            hyperlinks.insert(name.to_string(), (id.clone(), link));
            row.push(Btn::text_bg2(name).build_def(ctx, None));
        }
    }
    Widget::row(row)
}

fn make_browser<F: Fn(usize) -> (ID, InfoTab)>(
    ctx: &EventCtx,
    hyperlinks: &mut HashMap<String, (ID, InfoTab)>,
    noun: &str,
    total: usize,
    idx: usize,
    make_link: F,
) -> Widget {
    // TODO Keys are weird! But left/right for speed
    Widget::row(vec![
        if idx != 0 {
            hyperlinks.insert("previous".to_string(), make_link(idx - 1));
            Btn::text_fg("<").build(ctx, "previous", hotkey(Key::UpArrow))
        } else {
            Btn::text_fg("<").inactive(ctx)
        }
        .margin(5),
        format!("{} {}/{}", noun, idx + 1, total).draw_text(ctx),
        if idx != total - 1 {
            hyperlinks.insert("next".to_string(), make_link(idx + 1));
            Btn::text_fg(">").build(ctx, "next", hotkey(Key::DownArrow))
        } else {
            Btn::text_fg(">").inactive(ctx)
        }
        .margin(5),
    ])
    .centered()
}
