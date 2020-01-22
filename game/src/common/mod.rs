mod bus_explorer;
mod colors;
mod info;
mod minimap;
mod navigate;
mod overlays;
mod panels;
mod shortcuts;
mod turn_cycler;
mod warp;

pub use self::bus_explorer::ShowBusRoute;
pub use self::colors::{ColorLegend, Colorer, ColorerBuilder};
pub use self::minimap::Minimap;
pub use self::overlays::Overlays;
pub use self::panels::tool_panel;
pub use self::warp::Warping;
use crate::game::Transition;
use crate::helpers::{list_names, ID};
use crate::render::DrawOptions;
use crate::sandbox::SpeedControls;
use crate::ui::UI;
use ezgui::{
    lctrl, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Text, VerticalAlignment,
};
use std::collections::BTreeSet;

pub struct CommonState {
    turn_cycler: turn_cycler::TurnCyclerState,
    info_panel: Option<info::InfoPanel>,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            turn_cycler: turn_cycler::TurnCyclerState::Inactive,
            info_panel: None,
        }
    }

    // This has to be called after anything that calls ui.per_obj.action(). Oof.
    // TODO This'll be really clear once we consume. Hah!
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        ui: &mut UI,
        maybe_speed: Option<&mut SpeedControls>,
    ) -> Option<Transition> {
        if ctx.input.new_was_pressed(lctrl(Key::S).unwrap()) {
            ui.opts.dev = !ui.opts.dev;
        }
        if ui.opts.dev && ctx.input.new_was_pressed(lctrl(Key::J).unwrap()) {
            return Some(Transition::Push(warp::EnteringWarp::new()));
        }

        if let Some(t) = self.turn_cycler.event(ctx, ui) {
            return Some(t);
        }

        if let Some(ref id) = ui.primary.current_selection {
            if ui.per_obj.action(ctx, Key::I, "show info")
                || ui.per_obj.left_click(ctx, "show info")
            {
                ui.per_obj.info_panel_open = true;
                let actions = ui.per_obj.consume();
                self.info_panel = Some(info::InfoPanel::new(
                    id.clone(),
                    ctx,
                    ui,
                    actions,
                    maybe_speed,
                ));
            }
            return None;
        }

        if let Some(ref mut info) = self.info_panel {
            let (closed, maybe_t) = info.event(ctx, ui, maybe_speed);
            if closed {
                self.info_panel = None;
                assert!(ui.per_obj.info_panel_open);
                ui.per_obj.info_panel_open = false;
            }
            if let Some(t) = maybe_t {
                return Some(t);
            }
        }

        None
    }

    pub fn draw_no_osd(&self, g: &mut GfxCtx, ui: &UI) {
        self.turn_cycler.draw(g, ui);
        if let Some(ref info) = self.info_panel {
            info.draw(g);
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.draw_no_osd(g, ui);
        CommonState::draw_osd(g, ui, &ui.primary.current_selection);
    }

    pub fn default_osd(id: ID, ui: &UI) -> Text {
        let map = &ui.primary.map;
        let id_color = ui.cs.get_def("OSD ID color", Color::RED);
        let name_color = ui.cs.get_def("OSD name color", Color::CYAN);
        let mut osd = Text::new().with_bg();
        match id {
            ID::Lane(l) => {
                if ui.opts.dev {
                    osd.append(Line(l.to_string()).fg(id_color));
                    osd.append(Line(" is "));
                }
                osd.append_all(vec![
                    Line(format!("{} of ", map.get_l(l).lane_type.describe())),
                    Line(map.get_parent(l).get_name()).fg(name_color),
                ]);
            }
            ID::Building(b) => {
                if ui.opts.dev {
                    osd.append(Line(b.to_string()).fg(id_color));
                    osd.append(Line(" is "));
                }
                let bldg = map.get_b(b);
                osd.append(Line(bldg.get_name(map)).fg(name_color));
                if let Some(ref p) = bldg.parking {
                    osd.append(Line(format!(
                        " ({} parking spots via {})",
                        p.num_stalls, p.name
                    )));
                }
            }
            ID::Turn(t) => {
                // Only selectable in dev mode anyway
                osd.append_all(vec![
                    Line(format!("TurnID({})", map.get_t(t).lookup_idx)).fg(id_color),
                    Line(" between "),
                    Line(map.get_parent(t.src).get_name()).fg(name_color),
                    Line(" and "),
                    Line(map.get_parent(t.dst).get_name()).fg(name_color),
                ]);
            }
            ID::Intersection(i) => {
                if map.get_i(i).is_border() {
                    osd.append(Line("Border "));
                }

                if ui.opts.dev {
                    osd.append(Line(i.to_string()).fg(id_color));
                } else {
                    osd.append(Line("Intersection"));
                }
                osd.append(Line(" of "));

                let mut road_names = BTreeSet::new();
                for r in &map.get_i(i).roads {
                    road_names.insert(map.get_r(*r).get_name());
                }
                list_names(&mut osd, |l| l.fg(name_color), road_names);
            }
            ID::Car(c) => {
                if ui.opts.dev {
                    osd.append(Line(c.to_string()).fg(id_color));
                } else {
                    osd.append(Line(format!("a {}", c.1)));
                }
                if let Some(r) = ui.primary.sim.bus_route_id(c) {
                    osd.append_all(vec![
                        Line(" serving "),
                        Line(&map.get_br(r).name).fg(name_color),
                    ]);
                }
            }
            ID::Pedestrian(p) => {
                if ui.opts.dev {
                    osd.append(Line(p.to_string()).fg(id_color));
                } else {
                    osd.append(Line("a pedestrian"));
                }
            }
            ID::PedCrowd(list) => {
                osd.append(Line(format!("a crowd of {} pedestrians", list.len())));
            }
            ID::BusStop(bs) => {
                if ui.opts.dev {
                    osd.append(Line(bs.to_string()).fg(id_color));
                } else {
                    osd.append(Line("a bus stop"));
                }
                osd.append(Line(" served by "));

                let routes: BTreeSet<String> = map
                    .get_routes_serving_stop(bs)
                    .into_iter()
                    .map(|r| r.name.clone())
                    .collect();
                list_names(&mut osd, |l| l.fg(name_color), routes);
            }
            ID::Trip(t) => {
                osd.append(Line(t.to_string()).fg(id_color));
            }
            ID::ExtraShape(es) => {
                // Only selectable in dev mode anyway
                osd.append(Line(es.to_string()).fg(id_color));
            }
            ID::Area(a) => {
                // Only selectable in dev mode anyway
                osd.append(Line(a.to_string()).fg(id_color));
            }
            ID::Road(_) => unreachable!(),
        }
        osd
    }

    pub fn draw_osd(g: &mut GfxCtx, ui: &UI, id: &Option<ID>) {
        let osd = if let Some(id) = id {
            CommonState::default_osd(id.clone(), ui)
        } else if let Some(button) = g.button_tooltip() {
            button
        } else {
            Text::from(Line("...")).with_bg()
        };
        CommonState::draw_custom_osd(ui, g, osd);
    }

    pub fn draw_custom_osd(ui: &UI, g: &mut GfxCtx, mut osd: Text) {
        let (keys, click_action) = ui.per_obj.get_active_keys();
        if !keys.is_empty() {
            osd.append(Line("   Hotkeys: "));
            for (idx, key) in keys.into_iter().enumerate() {
                if idx != 0 {
                    osd.append(Line(", "));
                }
                osd.append(Line(key.describe()).fg(ezgui::HOTKEY_COLOR));
            }
        }
        if let Some(action) = click_action {
            osd.append_all(vec![
                Line("; "),
                Line("click").fg(ezgui::HOTKEY_COLOR),
                Line(format!(" to {}", action)),
            ]);
        }

        g.draw_blocking_text(
            &osd,
            (HorizontalAlignment::FillScreen, VerticalAlignment::Bottom),
        );
    }

    pub fn draw_options(&self, ui: &UI) -> DrawOptions {
        let mut opts = DrawOptions::new();
        opts.suppress_traffic_signal_details = self
            .turn_cycler
            .suppress_traffic_signal_details(&ui.primary.map);
        opts
    }

    // Meant to be used for launching from other states
    pub fn launch_info_panel(&mut self, id: ID, ctx: &mut EventCtx, ui: &mut UI) {
        self.info_panel = Some(info::InfoPanel::new(id, ctx, ui, Vec::new(), None));
        ui.per_obj.info_panel_open = true;
    }
}
