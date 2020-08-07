use crate::app::App;
use crate::common::{navigate, Warping};
use crate::game::Transition;
use crate::layer::PickLayer;
use abstutil::clamp;
use ezgui::{
    hotkey, Btn, Checkbox, Color, Composite, EventCtx, Filler, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Outcome, ScreenDims, ScreenPt, Spinner, VerticalAlignment, Widget,
};
use geom::{Distance, Polygon, Pt2D, Ring};

// TODO Some of the math in here might assume map bound minimums start at (0, 0).
pub struct Minimap {
    dragging: bool,
    pub(crate) composite: Composite,
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
            composite: make_minimap_panel(ctx, app, 0),
            zoomed: ctx.canvas.cam_zoom >= app.opts.min_zoom_for_detail,
            layer: app.layer.is_none(),

            zoom_lvl: 0,
            base_zoom,
            zoom: base_zoom,
            offset_x: 0.0,
            offset_y: 0.0,
        };
        if m.zoomed {
            m.recenter(ctx, app);
        }
        m
    }

    fn map_to_minimap_pct(&self, pt: Pt2D) -> (f64, f64) {
        let inner_rect = self.composite.rect_of("minimap");
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
        self.composite = make_minimap_panel(ctx, app, self.zoom_lvl);

        // Find the new offset
        let map_center = ctx.canvas.center_to_map_pt();
        let inner_rect = self.composite.rect_of("minimap");
        self.offset_x = map_center.x() * self.zoom - pct_x * inner_rect.width();
        self.offset_y = map_center.y() * self.zoom - pct_y * inner_rect.height();
    }

    fn recenter(&mut self, ctx: &EventCtx, app: &App) {
        // Recenter the minimap on the screen bounds
        let map_center = ctx.canvas.center_to_map_pt();
        let rect = self.composite.rect_of("minimap");
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
        let layer = app.layer.is_none();
        if zoomed != self.zoomed || layer != self.layer {
            let just_zoomed_in = zoomed && !self.zoomed;

            self.zoomed = zoomed;
            self.layer = layer;
            self.composite = make_minimap_panel(ctx, app, self.zoom_lvl);

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

        let pan_speed = 100.0;
        match self.composite.event(ctx) {
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
                    return Some(Transition::Push(navigate::Navigator::new(ctx, app)));
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
                app.unzoomed_agents.cars = self.composite.is_checked("Car");
                app.unzoomed_agents.bikes = self.composite.is_checked("Bike");
                app.unzoomed_agents.buses_and_trains = self.composite.is_checked("Bus");
                app.unzoomed_agents.peds = self.composite.is_checked("Pedestrian");
                if self.composite.has_widget("zorder") {
                    app.primary.show_zorder = self.composite.spinner("zorder");
                }
                self.composite = make_minimap_panel(ctx, app, self.zoom_lvl);
            }
            _ => {}
        }

        if self.zoomed {
            let inner_rect = self.composite.rect_of("minimap");

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

        let inner_rect = self.composite.rect_of("minimap").clone();

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
        if let Some(ref l) = app.layer {
            l.draw_minimap(g);
        }

        let mut cache = app.primary.draw_map.agents.borrow_mut();
        cache.draw_unzoomed_agents(
            &app.primary.sim,
            &app.primary.map,
            &app.unzoomed_agents,
            g,
            if app.opts.large_unzoomed_agents {
                Some(Distance::meters(2.0 + (self.zoom_lvl as f64)) / self.zoom)
            } else {
                None
            },
            app.opts.debug_all_agents,
            &app.cs,
        );

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

fn make_minimap_panel(ctx: &mut EventCtx, app: &App, zoom_lvl: usize) -> Composite {
    if ctx.canvas.cam_zoom < app.opts.min_zoom_for_detail {
        return Composite::new(Widget::row(vec![
            make_tool_panel(ctx, app).align_right(),
            make_vert_viz_panel(ctx, app)
                .bg(app.cs.panel_bg)
                .padding(16),
        ]))
        .aligned(
            HorizontalAlignment::Right,
            VerticalAlignment::BottomAboveOSD,
        )
        .build_custom(ctx);
    }

    let zoom_col = {
        let mut col = vec![Btn::svg_def("system/assets/speed/speed_up.svg")
            .build(ctx, "zoom in", None)
            .margin_below(20)];
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
                .margin_below(20),
            );
        }
        col.push(Btn::svg_def("system/assets/speed/slow_down.svg").build(ctx, "zoom out", None));
        // The zoom column should start below the "pan up" arrow. But if we put it on the row with
        // <, minimap, and > then it messes up the horizontal alignment of the pan up arrow.
        // Also, double column to avoid the background color stretching to the bottom of the row.
        Widget::custom_col(vec![
            Widget::custom_col(col).padding(10).bg(app.cs.inner_panel),
            if app.opts.dev {
                Spinner::new(ctx, app.primary.zorder_range, app.primary.show_zorder)
                    .named("zorder")
                    .margin_above(10)
            } else {
                Widget::nothing()
            },
        ])
        .margin_above(26)
    };

    let square_len = 0.15 * ctx.canvas.window_width;
    let minimap_controls = Widget::col(vec![
        Btn::svg_def("system/assets/minimap/up.svg")
            .build(ctx, "pan up", None)
            .centered_horiz(),
        Widget::row(vec![
            Btn::svg_def("system/assets/minimap/left.svg")
                .build(ctx, "pan left", None)
                .centered_vert(),
            Filler::new(ScreenDims::new(square_len, square_len)).named("minimap"),
            Btn::svg_def("system/assets/minimap/right.svg")
                .build(ctx, "pan right", None)
                .centered_vert(),
        ]),
        Btn::svg_def("system/assets/minimap/down.svg")
            .build(ctx, "pan down", None)
            .centered_horiz(),
    ]);

    Composite::new(Widget::row(vec![
        make_tool_panel(ctx, app),
        Widget::col(vec![
            Widget::row(vec![minimap_controls, zoom_col]),
            make_horiz_viz_panel(ctx, app),
        ])
        .padding(16)
        .bg(app.cs.panel_bg),
    ]))
    .aligned(
        HorizontalAlignment::Right,
        VerticalAlignment::BottomAboveOSD,
    )
    .build_custom(ctx)
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
            .build(ctx, "change layers", hotkey(Key::L))
            .bg(app.cs.inner_panel),
        Btn::svg_def("system/assets/tools/search.svg")
            .build(ctx, "search", hotkey(Key::K))
            .bg(app.cs.inner_panel),
    ])
}

fn make_horiz_viz_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    let a = &app.unzoomed_agents;
    Widget::custom_row(vec![
        Checkbox::colored(ctx, "Car", a.car_color, a.cars).margin_right(8),
        Widget::draw_svg(ctx, "system/assets/timeline/parking.svg").margin_right(24),
        Checkbox::colored(ctx, "Bike", a.bike_color, a.bikes).margin_right(24),
        Checkbox::colored(ctx, "Bus", a.bus_color, a.buses_and_trains).margin_right(24),
        Checkbox::colored(ctx, "Pedestrian", a.ped_color, a.peds).margin_right(8),
    ])
}

fn make_vert_viz_panel(ctx: &mut EventCtx, app: &App) -> Widget {
    let a = &app.unzoomed_agents;
    Widget::col(vec![
        Widget::custom_row(vec![
            Checkbox::colored(ctx, "Car", a.car_color, a.cars).margin_right(8),
            Widget::draw_svg(ctx, "system/assets/timeline/parking.svg"),
        ]),
        Checkbox::colored(ctx, "Bike", a.bike_color, a.bikes),
        Checkbox::colored(ctx, "Bus", a.bus_color, a.buses_and_trains),
        Checkbox::colored(ctx, "Pedestrian", a.ped_color, a.peds),
    ])
}
