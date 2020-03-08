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
pub use self::colors::{ColorLegend, Colorer};
pub use self::minimap::Minimap;
pub use self::overlays::Overlays;
pub use self::panels::tool_panel;
pub use self::warp::Warping;
use crate::app::App;
use crate::game::Transition;
use crate::helpers::{list_names, ID};
use crate::sandbox::SpeedControls;
use ezgui::{
    lctrl, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, ScreenDims, ScreenPt, ScreenRectangle,
    Text,
};
use geom::Polygon;
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

    // This has to be called after anything that calls app.per_obj.action(). Oof.
    // TODO This'll be really clear once we consume. Hah!
    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        maybe_speed: Option<&mut SpeedControls>,
    ) -> Option<Transition> {
        if ctx.input.new_was_pressed(&lctrl(Key::S).unwrap()) {
            app.opts.dev = !app.opts.dev;
        }
        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::J).unwrap()) {
            return Some(Transition::Push(warp::EnteringWarp::new()));
        }

        // TODO Disable unless gameplay.can_examine_objects. Not going to worry about this right
        // now, since these controls should change anyway.
        if let Some(t) = self.turn_cycler.event(ctx, app) {
            return Some(t);
        }

        if let Some(ref id) = app.primary.current_selection {
            if app.per_obj.action(ctx, Key::I, "show info")
                || app.per_obj.left_click(ctx, "show info")
            {
                app.per_obj.info_panel_open = true;
                let actions = app.per_obj.consume();
                self.info_panel = Some(info::InfoPanel::new(
                    id.clone(),
                    ctx,
                    app,
                    actions,
                    maybe_speed,
                ));
                return None;
            }
        }

        if let Some(ref mut info) = self.info_panel {
            let (closed, maybe_t) = info.event(ctx, app, maybe_speed);
            if closed {
                self.info_panel = None;
                assert!(app.per_obj.info_panel_open);
                app.per_obj.info_panel_open = false;
            }
            if let Some(t) = maybe_t {
                return Some(t);
            }
        }

        None
    }

    pub fn draw_no_osd(&self, g: &mut GfxCtx, app: &App) {
        self.turn_cycler.draw(g, app);
        if let Some(ref info) = self.info_panel {
            info.draw(g);
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.draw_no_osd(g, app);
        CommonState::draw_osd(g, app, &app.primary.current_selection);
    }

    pub fn default_osd(id: ID, app: &App) -> Text {
        let map = &app.primary.map;
        let id_color = app.cs.get_def("OSD ID color", Color::RED);
        let name_color = app.cs.get_def("OSD name color", Color::CYAN);
        let mut osd = Text::new();
        match id {
            ID::Lane(l) => {
                if app.opts.dev {
                    osd.append(Line(l.to_string()).fg(id_color));
                    osd.append(Line(" is "));
                }
                osd.append_all(vec![
                    Line(format!("{} of ", map.get_l(l).lane_type.describe())),
                    Line(map.get_parent(l).get_name()).fg(name_color),
                ]);
            }
            ID::Building(b) => {
                if app.opts.dev {
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

                if app.opts.dev {
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
                if app.opts.dev {
                    osd.append(Line(c.to_string()).fg(id_color));
                } else {
                    osd.append(Line(format!("a {}", c.1)));
                }
                if let Some(r) = app.primary.sim.bus_route_id(c) {
                    osd.append_all(vec![
                        Line(" serving "),
                        Line(&map.get_br(r).name).fg(name_color),
                    ]);
                }
            }
            ID::Pedestrian(p) => {
                if app.opts.dev {
                    osd.append(Line(p.to_string()).fg(id_color));
                } else {
                    osd.append(Line("a pedestrian"));
                }
            }
            ID::PedCrowd(list) => {
                osd.append(Line(format!("a crowd of {} pedestrians", list.len())));
            }
            ID::BusStop(bs) => {
                if app.opts.dev {
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
            ID::Road(r) => {
                if app.opts.dev {
                    osd.append(Line(r.to_string()).fg(id_color));
                    osd.append(Line(" is "));
                }
                osd.append(Line(map.get_r(r).get_name()).fg(name_color));
            }
        }
        osd
    }

    pub fn draw_osd(g: &mut GfxCtx, app: &App, id: &Option<ID>) {
        let osd = if let Some(id) = id {
            CommonState::default_osd(id.clone(), app)
        } else {
            Text::from(Line("..."))
        };
        CommonState::draw_custom_osd(g, app, osd);
    }

    pub fn draw_custom_osd(g: &mut GfxCtx, app: &App, mut osd: Text) {
        let (keys, click_action) = app.per_obj.get_active_keys();
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

        // TODO Rendering the OSD is actually a bit hacky.

        // First the constant background
        let mut batch = GeomBatch::from(vec![(
            crate::colors::PANEL_BG,
            Polygon::rectangle(g.canvas.window_width, 1.5 * g.default_line_height()),
        )]);
        batch.add_translated(osd.render_g(g), 10.0, 0.25 * g.default_line_height());

        if app.opts.dev && !g.is_screencap() {
            let mut txt = Text::from(Line("DEV"));
            txt.highlight_last_line(Color::RED);
            let dev_batch = txt.render_g(g);
            let dims = dev_batch.get_dims();
            batch.add_translated(
                dev_batch,
                g.canvas.window_width - dims.width - 10.0,
                0.25 * g.default_line_height(),
            );
        }
        let draw = g.upload(batch);
        let top_left = ScreenPt::new(0.0, g.canvas.window_height - 1.5 * g.default_line_height());
        g.redraw_at(top_left, &draw);
        g.canvas.mark_covered_area(ScreenRectangle::top_left(
            top_left,
            ScreenDims::new(g.canvas.window_width, 1.5 * g.default_line_height()),
        ));
    }

    // Meant to be used for launching from other states
    pub fn launch_info_panel(&mut self, id: ID, ctx: &mut EventCtx, app: &mut App) {
        self.info_panel = Some(info::InfoPanel::new(id, ctx, app, Vec::new(), None));
        app.per_obj.info_panel_open = true;
    }

    pub fn info_panel_open(&self) -> Option<ID> {
        self.info_panel.as_ref().map(|i| i.id.clone())
    }
}
