mod associated;
mod navigate;
mod turn_cycler;
mod warp;

use crate::helpers::ID;
use crate::render::DrawOptions;
use crate::ui::UI;
use abstutil::elapsed_seconds;
use ezgui::{
    Color, EventCtx, EventLoopMode, GfxCtx, HorizontalAlignment, Key, ModalMenu, Text,
    VerticalAlignment,
};
use geom::{Line, Pt2D};
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

    pub fn modal_menu_entries() -> Vec<(Option<Key>, &'static str)> {
        vec![
            (Some(Key::J), "warp"),
            // TODO This definitely conflicts with some modes.
            (Some(Key::K), "navigate"),
            (Some(Key::F1), "take a screenshot"),
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
            // TODO Cars, pedestrians...
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
        // On behalf of turn_cycler, just do this directly here.
        if let Some(ID::Lane(l)) = ui.primary.current_selection {
            opts.suppress_traffic_signal_details = Some(ui.primary.map.get_l(l).dst_i);
        }
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
