use crate::app::App;
use crate::common::{navigate, shortcuts, Overlays, Warping};
use crate::game::Transition;
use crate::render::MIN_ZOOM_FOR_DETAIL;
use abstutil::clamp;
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, Filler, GeomBatch, GfxCtx, HorizontalAlignment, Key,
    Line, Outcome, RewriteColor, ScreenDims, ScreenPt, VerticalAlignment, Widget,
};
use geom::{Distance, Polygon, Pt2D, Ring};

// TODO Some of the math in here might assume map bound minimums start at (0, 0).
pub struct Minimap {
    dragging: bool,
    pub(crate) composite: Composite,
    // Update panel when other things change
    zoomed: bool,
    overlay: bool,

    // [0, 3], with 0 meaning the most unzoomed
    zoom_lvl: usize,
    base_zoom: f64,
    zoom: f64,
    offset_x: f64,
    offset_y: f64,
}

impl Minimap {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Minimap {
        // Initially pick a zoom to fit the entire map's width in the minimap. Arbitrary and
        // probably pretty weird.
        let bounds = app.primary.map.get_bounds();
        let base_zoom = 0.15 * ctx.canvas.window_width / bounds.width();
        Minimap {
            dragging: false,
            composite: make_minimap_panel(ctx, app, 0),
            zoomed: ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL,
            overlay: app.overlay.is_empty(),

            zoom_lvl: 0,
            base_zoom,
            zoom: base_zoom,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    fn set_zoom(&mut self, ctx: &mut EventCtx, app: &App, zoom_lvl: usize) {
        let zoom_speed: f64 = 2.0;
        self.zoom_lvl = zoom_lvl;
        self.zoom = self.base_zoom * zoom_speed.powi(self.zoom_lvl as i32);
        self.composite = make_minimap_panel(ctx, app, self.zoom_lvl);
    }

    pub fn event(&mut self, app: &mut App, ctx: &mut EventCtx) -> Option<Transition> {
        let zoomed = ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL;
        let overlay = app.overlay.is_empty();
        if zoomed != self.zoomed || overlay != self.overlay {
            self.zoomed = zoomed;
            self.overlay = overlay;
            self.composite = make_minimap_panel(ctx, app, self.zoom_lvl);
        }

        let pan_speed = 100.0;
        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x {
                x if x == "pan up" => self.offset_y -= pan_speed * self.zoom,
                x if x == "pan down" => self.offset_y += pan_speed * self.zoom,
                x if x == "pan left" => self.offset_x -= pan_speed * self.zoom,
                x if x == "pan right" => self.offset_x += pan_speed * self.zoom,
                // TODO Make the center of the cursor still point to the same thing. Same math as
                // Canvas.
                x if x == "zoom in" => {
                    if self.zoom_lvl != 3 {
                        self.set_zoom(ctx, app, self.zoom_lvl + 1);
                    }
                }
                x if x == "zoom out" => {
                    if self.zoom_lvl != 0 {
                        self.set_zoom(ctx, app, self.zoom_lvl - 1);
                    }
                }
                x if x == "zoom to level 1" => {
                    self.set_zoom(ctx, app, 0);
                }
                x if x == "zoom to level 2" => {
                    self.set_zoom(ctx, app, 1);
                }
                x if x == "zoom to level 3" => {
                    self.set_zoom(ctx, app, 2);
                }
                x if x == "zoom to level 4" => {
                    self.set_zoom(ctx, app, 3);
                }
                x if x == "search" => {
                    return Some(Transition::Push(navigate::Navigator::new(ctx, app)));
                }
                x if x == "shortcuts" => {
                    return Some(Transition::Push(shortcuts::ChoosingShortcut::new()));
                }
                x if x == "zoom out fully" => {
                    return Some(Transition::Push(Warping::new(
                        ctx,
                        app.primary.map.get_bounds().get_rectangle().center(),
                        Some(ctx.canvas.min_zoom()),
                        None,
                        &mut app.primary,
                    )));
                }
                x if x == "zoom in fully" => {
                    return Some(Transition::Push(Warping::new(
                        ctx,
                        ctx.canvas.center_to_map_pt(),
                        Some(10.0),
                        None,
                        &mut app.primary,
                    )));
                }
                x if x == "change overlay" => {
                    return Overlays::change_overlays(ctx, app);
                }
                x => {
                    // Handles both "show {}" and "hide {}"
                    let key = x[5..].to_string();
                    app.agent_cs.toggle(key);
                    self.composite = make_minimap_panel(ctx, app, self.zoom_lvl);
                }
            },
            None => {}
        }

        if self.zoomed {
            let inner_rect = self.composite.filler_rect("minimap");

            // TODO Not happy about reaching in like this. The minimap logic should be an ezgui
            // Widget eventually, a generalization of Canvas.
            let mut pt = ctx.canvas.get_cursor();
            if self.dragging {
                if ctx.input.left_mouse_button_released() {
                    self.dragging = false;
                }
                // Don't drag out of inner_rect
                pt.x = clamp(pt.x, inner_rect.x1, inner_rect.x2);
                pt.y = clamp(pt.y, inner_rect.y1, inner_rect.y2);
            } else if inner_rect.contains(pt) && ctx.input.left_mouse_button_pressed() {
                self.dragging = true;
            } else {
                return None;
            }

            let percent_x = (pt.x - inner_rect.x1) / inner_rect.width();
            let percent_y = (pt.y - inner_rect.y1) / inner_rect.height();

            let map_pt = Pt2D::new(
                (self.offset_x + percent_x * inner_rect.width()) / self.zoom,
                (self.offset_y + percent_y * inner_rect.height()) / self.zoom,
            );
            ctx.canvas.center_on_map_pt(map_pt);
        }

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.composite.draw(g);
        if !self.zoomed {
            return;
        }

        let inner_rect = self.composite.filler_rect("minimap");

        let mut map_bounds = app.primary.map.get_bounds().clone();
        // Adjust bounds to account for the current pan and zoom
        map_bounds.min_x = (map_bounds.min_x + self.offset_x) / self.zoom;
        map_bounds.min_y = (map_bounds.min_y + self.offset_y) / self.zoom;
        map_bounds.max_x = map_bounds.min_x + inner_rect.width() / self.zoom;
        map_bounds.max_y = map_bounds.min_y + inner_rect.height() / self.zoom;

        g.fork(
            Pt2D::new(map_bounds.min_x, map_bounds.min_y),
            ScreenPt::new(inner_rect.x1, inner_rect.y1),
            self.zoom,
            None,
        );
        g.enable_clipping(inner_rect);
        g.redraw(&app.primary.draw_map.boundary_polygon);
        g.redraw(&app.primary.draw_map.draw_all_areas);
        g.redraw(&app.primary.draw_map.draw_all_thick_roads);
        g.redraw(&app.primary.draw_map.draw_all_unzoomed_intersections);
        g.redraw(&app.primary.draw_map.draw_all_buildings);
        // Not the building paths
        app.overlay.draw_minimap(g);

        let mut cache = app.primary.draw_map.agents.borrow_mut();
        cache.draw_unzoomed_agents(
            &app.primary.sim,
            &app.primary.map,
            &app.agent_cs,
            g,
            Distance::meters(2.0 + (self.zoom_lvl as f64)) / self.zoom,
        );

        // The cursor
        let (x1, y1) = {
            let pt = g.canvas.screen_to_map(ScreenPt::new(0.0, 0.0));
            (
                clamp(pt.x(), map_bounds.min_x, map_bounds.max_x),
                clamp(pt.y(), map_bounds.min_y, map_bounds.max_y),
            )
        };
        let (x2, y2) = {
            let pt = g
                .canvas
                .screen_to_map(ScreenPt::new(g.canvas.window_width, g.canvas.window_height));
            (
                clamp(pt.x(), map_bounds.min_x, map_bounds.max_x),
                clamp(pt.y(), map_bounds.min_y, map_bounds.max_y),
            )
        };
        if x1 != x2 && y1 != y2 {
            g.draw_polygon(
                Color::BLACK,
                &Ring::new(vec![
                    Pt2D::new(x1, y1),
                    Pt2D::new(x2, y1),
                    Pt2D::new(x2, y2),
                    Pt2D::new(x1, y2),
                    Pt2D::new(x1, y1),
                ])
                .make_polygons(Distance::meters(20.0)),
            );
        } else {
            // TODO Happens when we're quite out-of-bounds. Maybe stop allowing this at all?
        }
        g.disable_clipping();
        g.unfork();
    }
}

fn make_minimap_panel(ctx: &mut EventCtx, app: &App, zoom_lvl: usize) -> Composite {
    if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
        return Composite::new(Widget::row(vec![
            make_tool_panel(ctx, app).align_right().margin_right(16),
            make_vert_viz_panel(ctx, app).bg(app.cs.panel_bg).padding(7),
        ]))
        .aligned(
            HorizontalAlignment::Right,
            VerticalAlignment::BottomAboveOSD,
        )
        .build(ctx);
    }

    let zoom_col = {
        let mut col = vec![Btn::svg_def("../data/system/assets/speed/speed_up.svg")
            .build(ctx, "zoom in", None)
            .margin(12)];
        for i in (0..=3).rev() {
            let color = if zoom_lvl < i {
                Color::WHITE.alpha(0.2)
            } else {
                Color::WHITE
            };
            let rect = Polygon::rectangle(20.0, 8.0);
            col.push(
                Btn::custom(
                    GeomBatch::from(vec![(color, rect.clone())]),
                    GeomBatch::from(vec![(app.cs.hovering, rect.clone())]),
                    rect,
                )
                .build(ctx, format!("zoom to level {}", i + 1), None)
                .margin(12),
            );
        }
        col.push(
            Btn::svg_def("../data/system/assets/speed/slow_down.svg")
                .build(ctx, "zoom out", None)
                .margin(12),
        );
        // The zoom column should start below the "pan up" arrow. But if we put it on the row with
        // <, minimap, and > then it messes up the horizontal alignment of the pan up arrow.
        // Also, double column to avoid the background color stretching to the bottom of the row.
        Widget::col(vec![Widget::col(col).bg(app.cs.inner_panel)]).margin_above(26)
    };

    let square_len = 0.15 * ctx.canvas.window_width;
    let minimap_controls = Widget::col(vec![
        Btn::svg_def("../data/system/assets/minimap/up.svg")
            .build(ctx, "pan up", None)
            .margin(5)
            .centered_horiz(),
        Widget::row(vec![
            Btn::svg_def("../data/system/assets/minimap/left.svg")
                .build(ctx, "pan left", None)
                .margin(5)
                .centered_vert(),
            Filler::new(ScreenDims::new(square_len, square_len)).named("minimap"),
            Btn::svg_def("../data/system/assets/minimap/right.svg")
                .build(ctx, "pan right", None)
                .margin(5)
                .centered_vert(),
        ]),
        Btn::svg_def("../data/system/assets/minimap/down.svg")
            .build(ctx, "pan down", None)
            .margin(5)
            .centered_horiz(),
    ]);

    Composite::new(Widget::row(vec![
        make_tool_panel(ctx, app).margin_right(16),
        Widget::col(vec![
            Widget::row(vec![minimap_controls, zoom_col]),
            make_horiz_viz_panel(ctx, app),
        ])
        .padding(7)
        .bg(app.cs.panel_bg),
    ]))
    .aligned(
        HorizontalAlignment::Right,
        VerticalAlignment::BottomAboveOSD,
    )
    .build(ctx)
}

fn make_tool_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    // TODO Apply something to everything in the column
    Widget::col(vec![
        (if ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL {
            Btn::svg_def("../data/system/assets/minimap/zoom_out_fully.svg").build(
                ctx,
                "zoom out fully",
                None,
            )
        } else {
            Btn::svg_def("../data/system/assets/minimap/zoom_in_fully.svg").build(
                ctx,
                "zoom in fully",
                None,
            )
        })
        .bg(app.cs.inner_panel)
        .margin_below(16),
        Btn::svg_def("../data/system/assets/tools/layers.svg")
            .build(ctx, "change overlay", hotkey(Key::L))
            .bg(app.cs.inner_panel)
            .margin_below(16),
        Btn::svg_def("../data/system/assets/tools/search.svg")
            .build(ctx, "search", hotkey(Key::K))
            .bg(app.cs.inner_panel)
            .margin_below(16),
        Btn::svg_def("../data/system/assets/tools/shortcuts.svg")
            .build(ctx, "shortcuts", hotkey(Key::SingleQuote))
            .bg(app.cs.inner_panel),
    ])
}

fn make_horiz_viz_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    let mut row = Vec::new();
    for (label, color, enabled) in &app.agent_cs.rows {
        row.push(colored_checkbox(ctx, app, label, *color, *enabled).margin_right(8));
        row.push(Line(label).draw(ctx).margin_right(24));
    }
    let last = row.pop().unwrap();
    row.push(last.margin_right(0));
    Widget::row(row)
}

fn make_vert_viz_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    let mut col = Vec::new();

    for (label, color, enabled) in &app.agent_cs.rows {
        let mut row = Vec::new();
        row.push(colored_checkbox(ctx, app, label, *color, *enabled).margin_right(8));
        row.push(Line(label).draw(ctx));
        col.push(Widget::row(row).margin_below(7));
    }
    let last = col.pop().unwrap();
    col.push(last.margin_below(0));

    Widget::col(col)
}

fn colored_checkbox(ctx: &EventCtx, app: &App, label: &str, color: Color, enabled: bool) -> Widget {
    if enabled {
        Btn::svg(
            "../data/system/assets/tools/checkmark.svg",
            RewriteColor::ChangeMore(vec![(Color::BLACK, color), (Color::WHITE, app.cs.hovering)]),
        )
        .normal_color(RewriteColor::Change(Color::BLACK, color))
        .build(ctx, format!("hide {}", label), None)
    } else {
        // Fancy way of saying a circle ;)
        Btn::svg(
            "../data/system/assets/tools/checkmark.svg",
            RewriteColor::ChangeMore(vec![
                (Color::BLACK, color),
                (Color::WHITE, Color::INVISIBLE),
            ]),
        )
        .normal_color(RewriteColor::ChangeMore(vec![
            (Color::BLACK, color.alpha(0.3)),
            (Color::WHITE, Color::INVISIBLE),
        ]))
        .build(ctx, format!("show {}", label), None)
    }
}
