use std::collections::BTreeSet;

use geom::Polygon;
use widgetry::{
    lctrl, Btn, Color, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Panel,
    ScreenDims, ScreenPt, ScreenRectangle, Text, VerticalAlignment, Widget,
};

pub use self::city_picker::CityPicker;
pub use self::colors::{ColorDiscrete, ColorLegend, ColorNetwork, DivergingScale};
pub use self::heatmap::{make_heatmap, Grid, HeatmapOptions};
pub use self::minimap::Minimap;
pub use self::navigate::Navigator;
pub use self::warp::Warping;
use crate::app::App;
use crate::game::Transition;
use crate::helpers::{list_names, ID};
use crate::info::InfoPanel;
pub use crate::info::{ContextualActions, Tab};

mod city_picker;
mod colors;
mod heatmap;
mod minimap;
mod navigate;
#[cfg(not(target_arch = "wasm32"))]
mod updater;
mod warp;

// TODO This is now just used in two modes...
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
        if ctx.input.pressed(lctrl(Key::S)) {
            app.opts.dev = !app.opts.dev;
        }
        if ctx.input.pressed(lctrl(Key::J)) {
            return Some(Transition::Push(warp::DebugWarp::new(ctx)));
        }

        if let Some(id) = app.primary.current_selection.clone() {
            // TODO Also have a hotkey binding for this?
            if app.per_obj.left_click(ctx, "show info") {
                self.info_panel =
                    Some(InfoPanel::new(ctx, app, Tab::from_id(app, id), ctx_actions));
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
                    if ctx.input.pressed(k) {
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
        let mut osd = if let Some(ref id) = app.primary.current_selection {
            CommonState::osd_for(app, id.clone())
        } else if app.opts.dev {
            Text::from_all(vec![
                Line("Nothing selected. Hint: "),
                Line("Ctrl+J").fg(g.style().hotkey_color),
                Line(" to warp"),
            ])
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
                let r = map.get_parent(l);
                osd.append_all(vec![
                    Line(format!("{} of ", map.get_l(l).lane_type.describe())),
                    Line(r.get_name(app.opts.language.as_ref())).fg(name_color),
                ]);
                if app.opts.dev {
                    osd.append(Line(" ("));
                    osd.append(Line(r.id.to_string()).fg(id_color));
                    osd.append(Line(")"));
                }
            }
            ID::Building(b) => {
                if app.opts.dev {
                    osd.append(Line(b.to_string()).fg(id_color));
                    osd.append(Line(" is "));
                }
                let bldg = map.get_b(b);
                osd.append(Line(&bldg.address).fg(name_color));
            }
            ID::ParkingLot(pl) => {
                osd.append(Line(pl.to_string()).fg(id_color));
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
                    road_names.insert(map.get_r(*r).get_name(app.opts.language.as_ref()));
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
                        Line(&map.get_br(r).full_name).fg(name_color),
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
                    osd.append(Line("transit stop "));
                    osd.append(Line(&map.get_bs(bs).name).fg(name_color));
                }
                osd.append(Line(" served by "));

                let routes: BTreeSet<String> = map
                    .get_routes_serving_stop(bs)
                    .into_iter()
                    .map(|r| r.short_name.clone())
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
                osd.append(Line(map.get_r(r).get_name(app.opts.language.as_ref())).fg(name_color));
            }
        }
        osd
    }

    pub fn draw_osd(g: &mut GfxCtx, app: &App) {
        let osd = if let Some(ref id) = app.primary.current_selection {
            CommonState::osd_for(app, id.clone())
        } else if app.opts.dev {
            Text::from_all(vec![
                Line("Nothing selected. Hint: "),
                Line("Ctrl+J").fg(g.style().hotkey_color),
                Line(" to warp"),
            ])
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
        batch.append(
            osd.render(g)
                .translate(10.0, 0.25 * g.default_line_height()),
        );

        if app.opts.dev && !g.is_screencap() {
            let dev_batch = Text::from(Line("DEV")).bg(Color::RED).render(g);
            let dims = dev_batch.get_dims();
            batch.append(dev_batch.translate(
                g.canvas.window_width - dims.width - 10.0,
                0.25 * g.default_line_height(),
            ));
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

// TODO Kinda misnomer
pub fn tool_panel(ctx: &mut EventCtx) -> Panel {
    Panel::new(Widget::row(vec![
        Btn::svg_def("system/assets/tools/home.svg").build(ctx, "back", Key::Escape),
        Btn::svg_def("system/assets/tools/settings.svg").build(ctx, "settings", None),
    ]))
    .aligned(HorizontalAlignment::Left, VerticalAlignment::BottomAboveOSD)
    .build(ctx)
}
