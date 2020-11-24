use abstutil::clamp;
use geom::{Distance, Polygon, Pt2D, Ring};
use map_gui::tools::Navigator;
use widgetry::{
    Btn, Color, EventCtx, Filler, GeomBatch, GfxCtx, HorizontalAlignment, Key, Line, Outcome,
    Panel, ScreenPt, Spinner, VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::Warping;
use crate::layer::PickLayer;

// TODO Some of the math in here might assume map bound minimums start at (0, 0).
pub struct Minimap {
    dragging: bool,
    pub(crate) panel: Panel,
    // Update panel when other things change
    zoomed: bool,
    layer: bool,

    // [0, 3], with 0 meaning the most unzoomed
    zoom_lvl: usize,
    base_zoom: f64,
    zoom: f64,
    offset_x: f64,
    offset_y: f64,
}

impl Minimap {
    pub fn new(ctx: &mut EventCtx, app: &App) -> Minimap {
        // Initially pick a zoom to fit the smaller of the entire map's width or height in the
        // minimap. Arbitrary and probably pretty weird.
        let bounds = app.primary.map.get_bounds();
        let base_zoom = 0.15 * ctx.canvas.window_width / bounds.width().min(bounds.height());
        let mut m = Minimap {
            dragging: false,
            panel: Panel::empty(ctx),
            zoomed: ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail,
            layer: app.primary.layer.is_none(),

            zoom_lvl: 0,
            base_zoom,
            zoom: base_zoom,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        m.recreate_panel(ctx, app);
        if m.zoomed {
            m.recenter(ctx, app);
        }
        m
    }

    pub fn recreate_panel(&mut self, ctx: &mut EventCtx, app: &App) {
        if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
            self.panel = Panel::new(Widget::row(vec![
                make_tool_panel(ctx, app).align_right(),
                app.primary
                    .agents
                    .borrow()
                    .unzoomed_agents
                    .make_vert_viz_panel(ctx)
                    .bg(app.cs.panel_bg)
                    .padding(16),
            ]))
            .aligned(
                HorizontalAlignment::Right,
                VerticalAlignment::BottomAboveOSD,
            )
            .build_custom(ctx);
            return;
        }

        let zoom_col = {
            let mut col = vec![Btn::svg_def("system/assets/speed/speed_up.svg")
                .build(ctx, "zoom in", None)
                .margin_below(20)];
            for i in (0..=3).rev() {
                let color = if self.zoom_lvl < i {
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
                        None,
                    )
                    .build(ctx, format!("zoom to level {}", i + 1), None)
                    .margin_below(20),
                );
            }
            col.push(
                Btn::svg_def("system/assets/speed/slow_down.svg").build(ctx, "zoom out", None),
            );
            // The zoom column should start below the "pan up" arrow. But if we put it on the row
            // with <, minimap, and > then it messes up the horizontal alignment of the
            // pan up arrow. Also, double column to avoid the background color
            // stretching to the bottom of the row.
            Widget::custom_col(vec![
                Widget::custom_col(col).padding(10).bg(app.cs.inner_panel),
                if app.opts.dev {
                    Widget::col(vec![
                        Line("Z-order:").small().draw(ctx),
                        Spinner::new(
                            ctx,
                            app.primary.draw_map.zorder_range,
                            app.primary.draw_map.show_zorder,
                        )
                        .named("zorder"),
                    ])
                    .margin_above(10)
                } else {
                    Widget::nothing()
                },
            ])
            .margin_above(26)
        };

        let minimap_controls = Widget::col(vec![
            Btn::svg_def("system/assets/minimap/up.svg")
                .build(ctx, "pan up", None)
                .centered_horiz(),
            Widget::row(vec![
                Btn::svg_def("system/assets/minimap/left.svg")
                    .build(ctx, "pan left", None)
                    .centered_vert(),
                Filler::square_width(ctx, 0.15).named("minimap"),
                Btn::svg_def("system/assets/minimap/right.svg")
                    .build(ctx, "pan right", None)
                    .centered_vert(),
            ]),
            Btn::svg_def("system/assets/minimap/down.svg")
                .build(ctx, "pan down", None)
                .centered_horiz(),
        ]);

        self.panel = Panel::new(Widget::row(vec![
            make_tool_panel(ctx, app),
            Widget::col(vec![
                Widget::row(vec![minimap_controls, zoom_col]),
                app.primary
                    .agents
                    .borrow()
                    .unzoomed_agents
                    .make_horiz_viz_panel(ctx),
            ])
            .padding(16)
            .bg(app.cs.panel_bg),
        ]))
        .aligned(
            HorizontalAlignment::Right,
            VerticalAlignment::BottomAboveOSD,
        )
        .build_custom(ctx);
    }

    fn map_to_minimap_pct(&self, pt: Pt2D) -> (f64, f64) {
        let inner_rect = self.panel.rect_of("minimap");
        let pct_x = (pt.x() * self.zoom - self.offset_x) / inner_rect.width();
        let pct_y = (pt.y() * self.zoom - self.offset_y) / inner_rect.height();
        (pct_x, pct_y)
    }

    fn set_zoom(&mut self, ctx: &mut EventCtx, app: &App, zoom_lvl: usize) {
        // Make the frame wind up in the same relative position on the minimap
        let (pct_x, pct_y) = self.map_to_minimap_pct(ctx.canvas.center_to_map_pt());

        let zoom_speed: f64 = 2.0;
        self.zoom_lvl = zoom_lvl;
        self.zoom = self.base_zoom * zoom_speed.powi(self.zoom_lvl as i32);
        self.recreate_panel(ctx, app);

        // Find the new offset
        let map_center = ctx.canvas.center_to_map_pt();
        let inner_rect = self.panel.rect_of("minimap");
        self.offset_x = map_center.x() * self.zoom - pct_x * inner_rect.width();
        self.offset_y = map_center.y() * self.zoom - pct_y * inner_rect.height();
    }

    fn recenter(&mut self, ctx: &EventCtx, app: &App) {
        // Recenter the minimap on the screen bounds
        let map_center = ctx.canvas.center_to_map_pt();
        let rect = self.panel.rect_of("minimap");
        let off_x = map_center.x() * self.zoom - rect.width() / 2.0;
        let off_y = map_center.y() * self.zoom - rect.height() / 2.0;

        // Don't go out of bounds.
        let bounds = app.primary.map.get_bounds();
        // TODO For boundaries without rectangular shapes, it'd be even nicer to clamp to the
        // boundary.
        self.offset_x = off_x.max(0.0).min(bounds.max_x * self.zoom - rect.width());
        self.offset_y = off_y.max(0.0).min(bounds.max_y * self.zoom - rect.height());
    }

    pub fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Option<Transition> {
        let zoomed = ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail;
        let layer = app.primary.layer.is_none();
        if zoomed != self.zoomed || layer != self.layer {
            let just_zoomed_in = zoomed && !self.zoomed;

            self.zoomed = zoomed;
            self.layer = layer;
            self.recreate_panel(ctx, app);

            if just_zoomed_in {
                self.recenter(ctx, app);
            }
        } else if self.zoomed && !self.dragging {
            // If either corner of the cursor is out of bounds on the minimap, recenter.
            // TODO This means clicking the pan buttons while along the boundary won't work.
            let mut ok = true;
            for pt in vec![
                ScreenPt::new(0.0, 0.0),
                ScreenPt::new(ctx.canvas.window_width, ctx.canvas.window_height),
            ] {
                let (pct_x, pct_y) = self.map_to_minimap_pct(ctx.canvas.screen_to_map(pt));
                if pct_x < 0.0 || pct_x > 1.0 || pct_y < 0.0 || pct_y > 1.0 {
                    ok = false;
                    break;
                }
            }
            if !ok {
                self.recenter(ctx, app);
            }
        }
        if ctx.input.is_window_resized() {
            // When the window is resized, just reset completely. This is important when the window
            // size at startup is incorrect and immediately corrected by the window manager after
            // Minimap::new happens.
            let bounds = app.primary.map.get_bounds();
            // On Windows, apparently minimizing can cause some resize events with 0, 0 dimensions!
            self.base_zoom =
                (0.15 * ctx.canvas.window_width / bounds.width().min(bounds.height())).max(0.0001);
            self.zoom = self.base_zoom;
            if self.zoomed {
                self.recenter(ctx, app);
            }
        }

        let pan_speed = 100.0;
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x {
                x if x == "pan up" => {
                    self.offset_y -= pan_speed * self.zoom;
                    return Some(Transition::KeepWithMouseover);
                }
                x if x == "pan down" => {
                    self.offset_y += pan_speed * self.zoom;
                    return Some(Transition::KeepWithMouseover);
                }
                x if x == "pan left" => {
                    self.offset_x -= pan_speed * self.zoom;
                    return Some(Transition::KeepWithMouseover);
                }
                x if x == "pan right" => {
                    self.offset_x += pan_speed * self.zoom;
                    return Some(Transition::KeepWithMouseover);
                }
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
                    return Some(Transition::Push(Navigator::new(ctx, app)));
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
                x if x == "change layers" => {
                    return Some(Transition::Push(PickLayer::pick(ctx, app)));
                }
                _ => unreachable!(),
            },
            Outcome::Changed => {
                if self.panel.has_widget("Car") {
                    app.primary
                        .agents
                        .borrow_mut()
                        .unzoomed_agents
                        .update(&self.panel);
                }
                if self.panel.has_widget("zorder") {
                    app.primary.draw_map.show_zorder = self.panel.spinner("zorder");
                }
                self.recreate_panel(ctx, app);
            }
            _ => {}
        }

        if self.zoomed {
            let inner_rect = self.panel.rect_of("minimap");

            // TODO Not happy about reaching in like this. The minimap logic should be an widgetry
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
        self.panel.draw(g);
        if !self.zoomed {
            return;
        }

        let inner_rect = self.panel.rect_of("minimap").clone();

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
        g.redraw(&app.primary.draw_map.draw_all_unzoomed_parking_lots);
        g.redraw(
            &app.primary
                .draw_map
                .draw_all_unzoomed_roads_and_intersections,
        );
        g.redraw(&app.primary.draw_map.draw_all_buildings);
        // Not the building or parking lot paths
        if let Some(ref l) = app.primary.layer {
            l.draw_minimap(g);
        }

        let mut cache = app.primary.agents.borrow_mut();
        cache.draw_unzoomed_agents(g, app);

        // The cursor
        let (x1, y1) = {
            let pt = g.canvas.screen_to_map(ScreenPt::new(0.0, 0.0));
            (pt.x(), pt.y())
        };
        let (x2, y2) = {
            let pt = g
                .canvas
                .screen_to_map(ScreenPt::new(g.canvas.window_width, g.canvas.window_height));
            (pt.x(), pt.y())
        };
        g.draw_polygon(
            Color::BLACK,
            Ring::must_new(vec![
                Pt2D::new(x1, y1),
                Pt2D::new(x2, y1),
                Pt2D::new(x2, y2),
                Pt2D::new(x1, y2),
                Pt2D::new(x1, y1),
            ])
            .to_outline(Distance::meters(20.0)),
        );
        g.disable_clipping();
        g.unfork();
    }
}

fn make_tool_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    Widget::col(vec![
        (if ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail {
            Btn::svg_def("system/assets/minimap/zoom_out_fully.svg").build(
                ctx,
                "zoom out fully",
                None,
            )
        } else {
            Btn::svg_def("system/assets/minimap/zoom_in_fully.svg").build(
                ctx,
                "zoom in fully",
                None,
            )
        })
        .bg(app.cs.inner_panel),
        Btn::svg_def("system/assets/tools/layers.svg")
            .build(ctx, "change layers", Key::L)
            .bg(app.cs.inner_panel),
        Btn::svg_def("system/assets/tools/search.svg")
            .build(ctx, "search", Key::K)
            .bg(app.cs.inner_panel),
    ])
}
