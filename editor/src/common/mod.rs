mod associated;
mod navigate;
mod turn_cycler;
mod warp;

use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{
    hotkey, Color, EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Key, ModalMenu, MultiKey,
    ScreenPt, Slider, Text, VerticalAlignment,
};
use geom::{Duration, Line, Pt2D};
use std::collections::BTreeSet;
use std::time::Instant;

pub struct CommonState {
    associated: associated::ShowAssociatedState,
    turn_cycler: turn_cycler::TurnCyclerState,
    warp: Option<warp::WarpState>,
    navigate: Option<navigate::Navigator>,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            associated: associated::ShowAssociatedState::Inactive,
            turn_cycler: turn_cycler::TurnCyclerState::new(),
            warp: None,
            navigate: None,
        }
    }

    pub fn modal_menu_entries() -> Vec<(Option<MultiKey>, &'static str)> {
        vec![
            (hotkey(Key::J), "warp"),
            // TODO This definitely conflicts with some modes.
            (hotkey(Key::K), "navigate"),
            (hotkey(Key::F1), "take a screenshot"),
        ]
    }

    // If this returns something, then this common state should prevent other things from
    // happening.
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        menu: &mut ModalMenu,
    ) -> Option<EventLoopMode> {
        if let Some(ref mut warp) = self.warp {
            if let Some(evmode) = warp.event(ctx, ui) {
                return Some(evmode);
            }
            self.warp = None;
        }
        if menu.action("warp") {
            self.warp = Some(warp::WarpState::new());
        }
        if let Some(ref mut navigate) = self.navigate {
            if let Some(evmode) = navigate.event(ctx, ui) {
                return Some(evmode);
            }
            self.navigate = None;
        }
        if menu.action("navigate") {
            self.navigate = Some(navigate::Navigator::new(ui));
        }

        self.associated.event(ui);
        self.turn_cycler.event(ctx, ui);
        if menu.action("take a screenshot") {
            return Some(EventLoopMode::ScreenCaptureCurrentShot);
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        if let Some(ref warp) = self.warp {
            warp.draw(g);
        }
        if let Some(ref navigate) = self.navigate {
            navigate.draw(g);
        }
        self.turn_cycler.draw(g, ui);

        CommonState::draw_osd(g, ui, ui.primary.current_selection);
    }

    pub fn draw_osd(g: &mut GfxCtx, ui: &UI, id: Option<ID>) {
        let map = &ui.primary.map;
        let id_color = ui.cs.get_def("OSD ID color", Color::RED);
        let name_color = ui.cs.get_def("OSD name color", Color::CYAN);
        let mut osd = Text::new();
        match id {
            None => {
                osd.append("...".to_string(), None);
            }
            Some(ID::Lane(l)) => {
                osd.append(format!("{}", l), Some(id_color));
                osd.append(" is ".to_string(), None);
                osd.append(map.get_parent(l).get_name(), Some(name_color));
            }
            Some(ID::Building(b)) => {
                osd.append(format!("{}", b), Some(id_color));
                osd.append(" is ".to_string(), None);
                osd.append(map.get_b(b).get_name(), Some(name_color));
            }
            Some(ID::Turn(t)) => {
                osd.append(
                    format!("TurnID({})", map.get_t(t).lookup_idx),
                    Some(id_color),
                );
                osd.append(" between ".to_string(), None);
                osd.append(map.get_parent(t.src).get_name(), Some(name_color));
                osd.append(" and ".to_string(), None);
                osd.append(map.get_parent(t.dst).get_name(), Some(name_color));
            }
            Some(ID::Intersection(i)) => {
                osd.append(format!("{}", i), Some(id_color));
                osd.append(" of ".to_string(), None);

                let mut road_names = BTreeSet::new();
                for r in &map.get_i(i).roads {
                    road_names.insert(map.get_r(*r).get_name());
                }
                let len = road_names.len();
                for (idx, n) in road_names.into_iter().enumerate() {
                    osd.append(n, Some(name_color));
                    if idx != len - 1 {
                        osd.append(", ".to_string(), None);
                    }
                }
            }
            Some(ID::Car(c)) => {
                osd.append(format!("{}", c), Some(id_color));
                if let Some(r) = ui.primary.sim.bus_route_name(c) {
                    osd.append(" serving ".to_string(), None);
                    osd.append(format!("{}", map.get_br(r).name), Some(name_color));
                }
            }
            Some(ID::BusStop(bs)) => {
                osd.append(format!("{}", bs), Some(id_color));
                osd.append(" serving ".to_string(), None);

                let routes = map.get_routes_serving_stop(bs);
                let len = routes.len();
                for (idx, n) in routes.into_iter().enumerate() {
                    osd.append(n.name.clone(), Some(name_color));
                    if idx != len - 1 {
                        osd.append(", ".to_string(), None);
                    }
                }
            }
            Some(id) => {
                osd.append(format!("{:?}", id), Some(id_color));
            }
        }
        CommonState::draw_custom_osd(g, osd);
    }

    pub fn draw_custom_osd(g: &mut GfxCtx, mut osd: Text) {
        let keys = g.get_active_context_menu_keys();
        if !keys.is_empty() {
            osd.append("   Hotkeys: ".to_string(), None);
            for (idx, key) in keys.into_iter().enumerate() {
                if idx != 0 {
                    osd.append(", ".to_string(), None);
                }
                osd.append(key.describe(), Some(ezgui::HOTKEY_COLOR));
            }
        }

        g.draw_blocking_text(
            &osd,
            (HorizontalAlignment::FillScreen, VerticalAlignment::Bottom),
        );
    }

    pub fn draw_options(&self, ui: &UI) -> DrawOptions {
        let mut opts = DrawOptions::new();
        self.associated
            .override_colors(&mut opts.override_colors, ui);
        opts.suppress_traffic_signal_details = self
            .turn_cycler
            .suppress_traffic_signal_details(&ui.primary.map);
        opts
    }
}

const ANIMATION_TIME_S: f64 = 0.5;
// TODO Should factor in zoom too
const MIN_ANIMATION_SPEED: f64 = 200.0;

pub struct Warper {
    started: Instant,
    line: Option<Line>,
    id: ID,
}

impl Warper {
    pub fn new(ctx: &EventCtx, pt: Pt2D, id: ID) -> Warper {
        Warper {
            started: Instant::now(),
            line: Line::maybe_new(ctx.canvas.center_to_map_pt(), pt),
            id,
        }
    }

    pub fn event(&self, ctx: &mut EventCtx, ui: &mut UI) -> Option<EventLoopMode> {
        let line = self.line.as_ref()?;

        // Weird to do stuff for any event?
        if ctx.input.nonblocking_is_update_event() {
            ctx.input.use_update_event();
        }

        let speed = line.length().inner_meters() / ANIMATION_TIME_S;
        let total_time = if speed >= MIN_ANIMATION_SPEED {
            ANIMATION_TIME_S
        } else {
            line.length().inner_meters() / MIN_ANIMATION_SPEED
        };
        let percent = elapsed_seconds(self.started) / total_time;

        if percent >= 1.0 || ctx.input.nonblocking_is_keypress_event() {
            ctx.canvas.center_on_map_pt(line.pt2());
            ui.primary.current_selection = Some(self.id);
            None
        } else {
            ctx.canvas
                .center_on_map_pt(line.dist_along(line.length() * percent));
            Some(EventLoopMode::Animation)
        }
    }
}

const ADJUST_SPEED: f64 = 0.1;
// TODO hardcoded cap for now...
const SPEED_CAP: f64 = 10.0 * 60.0;

pub struct SpeedControls {
    slider: Slider,
    state: State,
}

enum State {
    Paused,
    Running {
        last_step: Instant,
        speed_description: String,
        last_measurement: Instant,
        last_measurement_sim: Duration,
    },
}

impl SpeedControls {
    pub fn new(ctx: &mut EventCtx, top_left_at: Option<ScreenPt>) -> SpeedControls {
        let mut slider = Slider::new(top_left_at);
        slider.set_percent(ctx, 1.0 / SPEED_CAP);
        SpeedControls {
            slider,
            state: State::Paused,
        }
    }

    pub fn modal_status_line(&self) -> String {
        if let State::Running {
            ref speed_description,
            ..
        } = self.state
        {
            format!(
                "Speed: {} / desired {:.2}x",
                speed_description,
                self.desired_speed()
            )
        } else {
            format!("Speed: paused / desired {:.2}x", self.desired_speed())
        }
    }

    // Returns the amount of simulation time to step, if running.
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        menu: &mut ModalMenu,
        current_sim_time: Duration,
    ) -> Option<Duration> {
        let desired_speed = self.desired_speed();
        if desired_speed != SPEED_CAP && menu.action("speed up") {
            self.slider
                .set_percent(ctx, ((desired_speed + ADJUST_SPEED) / SPEED_CAP).min(1.0));
        } else if desired_speed != 0.0 && menu.action("slow down") {
            self.slider
                .set_percent(ctx, ((desired_speed - ADJUST_SPEED) / SPEED_CAP).max(0.0));
        } else if self.slider.event(ctx) {
            // Keep going
        }

        match self.state {
            State::Paused => {
                if menu.action("pause/resume") {
                    let now = Instant::now();
                    self.state = State::Running {
                        last_step: now,
                        speed_description: "...".to_string(),
                        last_measurement: now,
                        last_measurement_sim: current_sim_time,
                    };
                    // Sorta hack to trigger EventLoopMode::Animation.
                    return Some(Duration::ZERO);
                }
            }
            State::Running {
                ref mut last_step,
                ref mut speed_description,
                ref mut last_measurement,
                ref mut last_measurement_sim,
            } => {
                if menu.action("pause/resume") {
                    self.state = State::Paused;
                } else if ctx.input.nonblocking_is_update_event() {
                    ctx.input.use_update_event();
                    let dt = Duration::seconds(elapsed_seconds(*last_step)) * desired_speed;
                    *last_step = Instant::now();

                    let dt_descr = Duration::seconds(elapsed_seconds(*last_measurement));
                    if dt_descr >= Duration::seconds(1.0) {
                        *speed_description = format!(
                            "{:.2}x",
                            (current_sim_time - *last_measurement_sim) / dt_descr
                        );
                        *last_measurement = *last_step;
                        *last_measurement_sim = current_sim_time;
                    }
                    return Some(dt);
                }
            }
        }
        None
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.slider.draw(g);
    }

    pub fn pause(&mut self) {
        self.state = State::Paused;
    }

    pub fn is_paused(&self) -> bool {
        match self.state {
            State::Paused => true,
            State::Running { .. } => false,
        }
    }

    fn desired_speed(&self) -> f64 {
        self.slider.get_percent() * SPEED_CAP
    }
}
