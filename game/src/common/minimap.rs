use crate::colors;
use crate::common::{navigate, shortcuts, Overlays, Warping};
use crate::game::{Transition, WizardState};
use crate::managed::WrappedComposite;
use crate::render::{AgentColorScheme, MIN_ZOOM_FOR_DETAIL};
use crate::ui::UI;
use abstutil::clamp;
use ezgui::{
    hotkey, Button, Choice, Color, Composite, EventCtx, Filler, GeomBatch, GfxCtx,
    HorizontalAlignment, Key, Line, ManagedWidget, Outcome, RewriteColor, ScreenDims, ScreenPt,
    Text, VerticalAlignment,
};
use geom::{Circle, Distance, Polygon, Pt2D, Ring};

// TODO Some of the math in here might assume map bound minimums start at (0, 0).
pub struct Minimap {
    dragging: bool,
    pub(crate) composite: Composite,
    acs: AgentColorScheme,
    zoomed: bool,

    // [0, 3], with 0 meaning the most unzoomed
    zoom_lvl: usize,
    base_zoom: f64,
    zoom: f64,
    offset_x: f64,
    offset_y: f64,
}

impl Minimap {
    pub fn new(ctx: &mut EventCtx, ui: &UI) -> Minimap {
        // Initially pick a zoom to fit the entire map's width in the minimap. Arbitrary and
        // probably pretty weird.
        let bounds = ui.primary.map.get_bounds();
        let base_zoom = 0.15 * ctx.canvas.window_width / bounds.width();
        Minimap {
            dragging: false,
            composite: make_minimap_panel(ctx, &ui.agent_cs, 0),
            acs: ui.agent_cs.clone(),
            zoomed: ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL,

            zoom_lvl: 0,
            base_zoom,
            zoom: base_zoom,
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }

    fn set_zoom(&mut self, ctx: &mut EventCtx, zoom_lvl: usize) {
        let zoom_speed: f64 = 2.0;
        self.zoom_lvl = zoom_lvl;
        self.zoom = self.base_zoom * zoom_speed.powi(self.zoom_lvl as i32);
        self.composite = make_minimap_panel(ctx, &self.acs, self.zoom_lvl);
    }

    pub fn event(&mut self, ui: &mut UI, ctx: &mut EventCtx) -> Option<Transition> {
        // Happens when we changed the colorscheme in WizardState
        if ui.agent_cs != self.acs {
            self.acs = ui.agent_cs.clone();
            self.composite = make_minimap_panel(ctx, &self.acs, self.zoom_lvl);
        }
        let zoomed = ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL;
        if zoomed != self.zoomed {
            self.zoomed = zoomed;
            self.composite = make_minimap_panel(ctx, &self.acs, self.zoom_lvl);
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
                        self.set_zoom(ctx, self.zoom_lvl + 1);
                    }
                }
                x if x == "zoom out" => {
                    if self.zoom_lvl != 0 {
                        self.set_zoom(ctx, self.zoom_lvl - 1);
                    }
                }
                x if x == "zoom to level 1" => {
                    self.set_zoom(ctx, 0);
                }
                x if x == "zoom to level 2" => {
                    self.set_zoom(ctx, 1);
                }
                x if x == "zoom to level 3" => {
                    self.set_zoom(ctx, 2);
                }
                x if x == "zoom to level 4" => {
                    self.set_zoom(ctx, 3);
                }
                x if x == "change agent colorscheme" => {
                    let btn = self.composite.rect_of("change agent colorscheme").clone();
                    return Some(Transition::Push(WizardState::new(Box::new(
                        move |wiz, ctx, ui| {
                            let (_, acs) = wiz.wrap(ctx).choose_exact(
                                (
                                    HorizontalAlignment::Centered(btn.center().x),
                                    VerticalAlignment::Below(btn.y2 + 15.0),
                                ),
                                None,
                                || {
                                    let mut choices = Vec::new();
                                    for acs in AgentColorScheme::all(&ui.cs) {
                                        if ui.agent_cs.acs != acs.acs {
                                            choices.push(Choice::new(acs.long_name.clone(), acs));
                                        }
                                    }
                                    choices
                                },
                            )?;
                            ui.agent_cs = acs;
                            // TODO It'd be great to replace self here, but the lifetimes don't
                            // work out.
                            Some(Transition::Pop)
                        },
                    ))));
                }
                x if x == "search" => {
                    return Some(Transition::Push(Box::new(navigate::Navigator::new(ui))));
                }
                x if x == "shortcuts" => {
                    return Some(Transition::Push(shortcuts::ChoosingShortcut::new()));
                }
                x if x == "zoom out fully" => {
                    return Some(Transition::Push(Warping::new(
                        ctx,
                        ui.primary.map.get_bounds().get_rectangle().center(),
                        Some(ctx.canvas.min_zoom()),
                        None,
                        &mut ui.primary,
                    )));
                }
                x if x == "zoom in fully" => {
                    return Some(Transition::Push(Warping::new(
                        ctx,
                        ctx.canvas.center_to_map_pt(),
                        Some(10.0),
                        None,
                        &mut ui.primary,
                    )));
                }
                x if x == "change overlay" => {
                    return Overlays::change_overlays(ctx, ui);
                }
                x => {
                    let key = x["show/hide ".len()..].to_string();
                    ui.agent_cs.toggle(key);
                    self.acs = ui.agent_cs.clone();
                    self.composite = make_minimap_panel(ctx, &self.acs, self.zoom_lvl);
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

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.composite.draw(g);
        if !self.zoomed {
            return;
        }

        let inner_rect = self.composite.filler_rect("minimap");

        let mut map_bounds = ui.primary.map.get_bounds().clone();
        // Adjust bounds to account for the current pan and zoom
        map_bounds.min_x = (map_bounds.min_x + self.offset_x) / self.zoom;
        map_bounds.min_y = (map_bounds.min_y + self.offset_y) / self.zoom;
        map_bounds.max_x = map_bounds.min_x + inner_rect.width() / self.zoom;
        map_bounds.max_y = map_bounds.min_y + inner_rect.height() / self.zoom;

        g.fork(
            Pt2D::new(map_bounds.min_x, map_bounds.min_y),
            ScreenPt::new(inner_rect.x1, inner_rect.y1),
            self.zoom,
        );
        g.enable_clipping(inner_rect);
        g.redraw(&ui.primary.draw_map.boundary_polygon);
        g.redraw(&ui.primary.draw_map.draw_all_areas);
        g.redraw(&ui.primary.draw_map.draw_all_thick_roads);
        g.redraw(&ui.primary.draw_map.draw_all_unzoomed_intersections);
        g.redraw(&ui.primary.draw_map.draw_all_buildings);
        if let Some(ref c) = ui.overlay.maybe_colorer() {
            g.redraw(&c.unzoomed);
        }

        let mut cache = ui.primary.draw_map.agents.borrow_mut();
        cache.draw_unzoomed_agents(
            &ui.primary.sim,
            &ui.primary.map,
            &ui.agent_cs,
            g,
            self.zoom,
            Distance::meters(5.0),
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

fn make_minimap_panel(ctx: &mut EventCtx, acs: &AgentColorScheme, zoom_lvl: usize) -> Composite {
    if ctx.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
        return Composite::new(make_viz_panel(ctx, acs))
            .aligned(
                HorizontalAlignment::Right,
                VerticalAlignment::BottomAboveOSD,
            )
            .build(ctx);
    }

    let square_len = 0.15 * ctx.canvas.window_width;
    let mut zoom_col = vec![ManagedWidget::btn(Button::rectangle_svg(
        "../data/system/assets/speed/speed_up.svg",
        "zoom in",
        None,
        RewriteColor::ChangeAll(colors::HOVERING),
        ctx,
    ))];
    for i in (0..=3).rev() {
        let color = if zoom_lvl < i {
            Color::grey(0.2)
        } else {
            Color::WHITE
        };
        let rect = Polygon::rectangle(20.0, 8.0);
        zoom_col.push(ManagedWidget::btn(Button::new(
            ctx,
            GeomBatch::from(vec![(color, rect.clone())]),
            GeomBatch::from(vec![(colors::HOVERING, rect.clone())]),
            None,
            &format!("zoom to level {}", i + 1),
            rect,
        )));
    }
    zoom_col.push(ManagedWidget::btn(Button::rectangle_svg(
        "../data/system/assets/speed/slow_down.svg",
        "zoom out",
        None,
        RewriteColor::ChangeAll(colors::HOVERING),
        ctx,
    )));

    Composite::new(
        ManagedWidget::row(vec![
            make_viz_panel(ctx, acs),
            ManagedWidget::col(zoom_col).margin(5).centered(),
            ManagedWidget::col(vec![
                WrappedComposite::svg_button(
                    ctx,
                    "../data/system/assets/minimap/up.svg",
                    "pan up",
                    None,
                )
                .margin(5)
                .centered_horiz(),
                ManagedWidget::row(vec![
                    WrappedComposite::svg_button(
                        ctx,
                        "../data/system/assets/minimap/left.svg",
                        "pan left",
                        None,
                    )
                    .margin(5)
                    .centered_vert(),
                    ManagedWidget::filler("minimap"),
                    WrappedComposite::svg_button(
                        ctx,
                        "../data/system/assets/minimap/right.svg",
                        "pan right",
                        None,
                    )
                    .margin(5)
                    .centered_vert(),
                ]),
                WrappedComposite::svg_button(
                    ctx,
                    "../data/system/assets/minimap/down.svg",
                    "pan down",
                    None,
                )
                .margin(5)
                .centered_horiz(),
            ])
            .centered(),
        ])
        .bg(colors::PANEL_BG),
    )
    .aligned(
        HorizontalAlignment::Right,
        VerticalAlignment::BottomAboveOSD,
    )
    .filler(
        "minimap",
        Filler::new(ScreenDims::new(square_len, square_len)),
    )
    .build(ctx)
}

fn make_viz_panel(ctx: &mut EventCtx, acs: &AgentColorScheme) -> ManagedWidget {
    let radius = 10.0;
    let mut col = vec![
        ManagedWidget::row(vec![
            WrappedComposite::svg_button(
                ctx,
                "../data/system/assets/tools/search.svg",
                "search",
                hotkey(Key::K),
            )
            .margin(10),
            WrappedComposite::svg_button(
                ctx,
                "../data/system/assets/tools/shortcuts.svg",
                "shortcuts",
                hotkey(Key::SingleQuote),
            )
            .margin(10),
            if ctx.canvas.cam_zoom >= MIN_ZOOM_FOR_DETAIL {
                WrappedComposite::svg_button(
                    ctx,
                    "../data/system/assets/minimap/zoom_out_fully.svg",
                    "zoom out fully",
                    None,
                )
                .margin(10)
            } else {
                WrappedComposite::svg_button(
                    ctx,
                    "../data/system/assets/minimap/zoom_in_fully.svg",
                    "zoom in fully",
                    None,
                )
                .margin(10)
            },
            WrappedComposite::svg_button(
                ctx,
                "../data/system/assets/tools/layers.svg",
                "change overlay",
                hotkey(Key::L),
            )
            .margin(10),
        ])
        .centered(),
        WrappedComposite::nice_text_button(
            ctx,
            Text::from(Line(format!("{} ▼", acs.short_name))),
            hotkey(Key::Semicolon),
            "change agent colorscheme",
        )
        .centered_horiz(),
    ];
    for (label, color, enabled) in &acs.rows {
        col.push(
            ManagedWidget::row(vec![
                ManagedWidget::btn(Button::rectangle_svg_rewrite(
                    "../data/system/assets/tools/visibility.svg",
                    &format!("show/hide {}", label),
                    None,
                    if *enabled {
                        RewriteColor::NoOp
                    } else {
                        RewriteColor::ChangeAll(Color::WHITE.alpha(0.5))
                    },
                    RewriteColor::ChangeAll(colors::HOVERING),
                    ctx,
                ))
                .margin(3),
                ManagedWidget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(
                        Color::WHITE.alpha(0.5),
                        Polygon::rectangle(2.0, 1.5 * radius),
                    )]),
                )
                .margin(3),
                ManagedWidget::draw_batch(
                    ctx,
                    GeomBatch::from(vec![(
                        color.alpha(if *enabled { 1.0 } else { 0.5 }),
                        Circle::new(Pt2D::new(radius, radius), Distance::meters(radius))
                            .to_polygon(),
                    )]),
                )
                .margin(3),
                ManagedWidget::draw_text(
                    ctx,
                    Text::from(if *enabled {
                        Line(label)
                    } else {
                        Line(label).fg(Color::WHITE.alpha(0.5))
                    }),
                )
                .margin(3),
            ])
            .centered_cross(),
        );
    }
    ManagedWidget::col(col).bg(colors::PANEL_BG).padding(5)
}
