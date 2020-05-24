mod colors;
mod heatmap;
mod minimap;
mod navigate;
mod panels;
mod warp;

pub use self::colors::{ColorLegend, Colorer};
pub use self::heatmap::{make_heatmap, HeatmapOptions};
pub use self::minimap::Minimap;
pub use self::panels::tool_panel;
pub use self::warp::Warping;
use crate::app::App;
use crate::game::Transition;
use crate::helpers::{list_names, ID};
use crate::info::InfoPanel;
pub use crate::info::{ContextualActions, Tab};
use ezgui::{
    hotkey, lctrl, Color, EventCtx, GeomBatch, GfxCtx, Key, Line, ScreenDims, ScreenPt,
    ScreenRectangle, Text,
};
use geom::Polygon;
use std::collections::BTreeSet;

// TODO Weird name.
pub struct CommonState {
    // TODO Better to express these as mutex
    info_panel: Option<InfoPanel>,
    // Just for drawing the OSD
    cached_actions: Vec<Key>,
}

impl CommonState {
    pub fn new() -> CommonState {
        CommonState {
            info_panel: None,
            cached_actions: Vec::new(),
        }
    }

    pub fn event(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        ctx_actions: &mut dyn ContextualActions,
    ) -> Option<Transition> {
        if ctx.input.new_was_pressed(&lctrl(Key::S).unwrap()) {
            app.opts.dev = !app.opts.dev;
        }
        if app.opts.dev && ctx.input.new_was_pressed(&lctrl(Key::J).unwrap()) {
            return Some(Transition::Push(warp::EnteringWarp::new()));
        }

        if let Some(ref id) = app.primary.current_selection {
            // TODO Also have a hotkey binding for this?
            if app.per_obj.left_click(ctx, "show info") {
                self.info_panel = Some(InfoPanel::new(
                    ctx,
                    app,
                    Tab::from_id(app, id.clone()),
                    ctx_actions,
                ));
                return None;
            }
        }

        if let Some(ref mut info) = self.info_panel {
            let (closed, maybe_t) = info.event(ctx, app, ctx_actions);
            if closed {
                self.info_panel = None;
            }
            if let Some(t) = maybe_t {
                return Some(t);
            }
        }

        if self.info_panel.is_none() {
            self.cached_actions.clear();
            if let Some(id) = app.primary.current_selection.clone() {
                // Allow hotkeys to work without opening the panel.
                for (k, action) in ctx_actions.actions(app, id.clone()) {
                    if ctx.input.new_was_pressed(&hotkey(k).unwrap()) {
                        return Some(ctx_actions.execute(ctx, app, id, action, &mut false));
                    }
                    self.cached_actions.push(k);
                }
            }
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        let keys = if let Some(ref info) = self.info_panel {
            info.draw(g, app);
            info.active_keys()
        } else {
            &self.cached_actions
        };
        let mut osd = if let Some(id) = &app.primary.current_selection {
            CommonState::osd_for(app, id.clone())
        } else {
            Text::from(Line("..."))
        };
        if !keys.is_empty() {
            osd.append(Line("   Hotkeys: "));
            for (idx, key) in keys.into_iter().enumerate() {
                if idx != 0 {
                    osd.append(Line(", "));
                }
                osd.append(Line(key.describe()).fg(g.style().hotkey_color));
            }
        }

        CommonState::draw_custom_osd(g, app, osd);
    }

    fn osd_for(app: &App, id: ID) -> Text {
        let map = &app.primary.map;
        let id_color = app.cs.bottom_bar_id;
        let name_color = app.cs.bottom_bar_name;
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
                osd.append(Line(&bldg.address).fg(name_color));
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
            CommonState::osd_for(app, id.clone())
        } else {
            Text::from(Line("..."))
        };
        CommonState::draw_custom_osd(g, app, osd);
    }

    pub fn draw_custom_osd(g: &mut GfxCtx, app: &App, mut osd: Text) {
        if let Some(ref action) = app.per_obj.click_action {
            osd.append_all(vec![
                Line("; "),
                Line("click").fg(g.style().hotkey_color),
                Line(format!(" to {}", action)),
            ]);
        }

        // TODO Rendering the OSD is actually a bit hacky.

        // First the constant background
        let mut batch = GeomBatch::from(vec![(
            app.cs.panel_bg,
            Polygon::rectangle(g.canvas.window_width, 1.5 * g.default_line_height()),
        )]);
        batch.add_translated(osd.render_g(g), 10.0, 0.25 * g.default_line_height());

        if app.opts.dev && !g.is_screencap() {
            let dev_batch = Text::from(Line("DEV")).bg(Color::RED).render_g(g);
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
    pub fn launch_info_panel(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        tab: Tab,
        ctx_actions: &mut dyn ContextualActions,
    ) {
        self.info_panel = Some(InfoPanel::new(ctx, app, tab, ctx_actions));
    }

    pub fn info_panel_open(&self, app: &App) -> Option<ID> {
        self.info_panel.as_ref().and_then(|i| i.active_id(app))
    }
}
