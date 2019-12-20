use crate::game::{Transition, WizardState};
use crate::render::{AgentColorScheme, MIN_ZOOM_FOR_DETAIL};
use crate::ui::UI;
use abstutil::clamp;
use ezgui::{
    hotkey, Button, Choice, Color, Composite, EventCtx, GeomBatch, GfxCtx, Key, Line,
    ManagedWidget, Outcome, RewriteColor, ScreenPt, ScreenRectangle, Text,
};
use geom::{Circle, Distance, Polygon, Pt2D, Ring};

pub struct Minimap {
    dragging: bool,

    controls: VisibilityPanel,
}

impl Minimap {
    pub fn new(ctx: &EventCtx, ui: &UI) -> Minimap {
        Minimap {
            dragging: false,
            controls: VisibilityPanel::new(ctx, ui),
        }
    }

    pub fn event(&mut self, ui: &mut UI, ctx: &mut EventCtx) -> Option<Transition> {
        if let Some(t) = self.controls.event(ctx, ui) {
            return Some(t);
        }

        // TODO duplicate some stuff for now, until we figure out what to cache
        let square_len = 0.15 * ctx.canvas.window_width;
        let top_left = ScreenPt::new(
            ctx.canvas.window_width - square_len - 50.0,
            ctx.canvas.window_height - square_len - 50.0,
        );
        let padding = 10.0;
        let inner_rect = ScreenRectangle {
            x1: top_left.x + padding,
            x2: top_left.x + square_len - padding,
            y1: top_left.y + padding,
            y2: top_left.y + square_len - padding,
        };

        let mut pt = ctx.canvas.get_cursor_in_screen_space();
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

        let percent_x = (pt.x - inner_rect.x1) / (inner_rect.x2 - inner_rect.x1);
        let percent_y = (pt.y - inner_rect.y1) / (inner_rect.y2 - inner_rect.y1);

        let bounds = ui.primary.map.get_bounds();
        let zoom = (square_len - (padding * 2.0)) / (bounds.max_x - bounds.min_x);

        // We're stretching to fit the entire width, so...
        let map_x = percent_x * (bounds.max_x - bounds.min_x);
        // The y2 on the map that we're currently displaying
        let map_y2 = bounds.min_y + (inner_rect.y2 - inner_rect.y1) / zoom;
        let map_pt = Pt2D::new(map_x, percent_y * (map_y2 - bounds.min_y));
        ctx.canvas.center_on_map_pt(map_pt);

        None
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        self.controls.draw(g);

        if g.canvas.cam_zoom < MIN_ZOOM_FOR_DETAIL {
            return;
        }

        // The background panel
        let square_len = 0.15 * g.canvas.window_width;
        let top_left = ScreenPt::new(
            g.canvas.window_width - square_len - 50.0,
            g.canvas.window_height - square_len - 50.0,
        );
        let bg = Polygon::rounded_rectangle(
            Distance::meters(square_len),
            Distance::meters(square_len),
            Distance::meters(10.0),
        )
        .translate(top_left.x, top_left.y);
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: top_left.x,
            x2: top_left.x + square_len,
            y1: top_left.y,
            y2: top_left.y + square_len,
        });
        g.fork_screenspace();
        g.draw_polygon(Color::grey(0.5), &bg);
        g.unfork();

        // The map
        let padding = 10.0;
        let inner_rect = ScreenRectangle {
            x1: top_left.x + padding,
            x2: top_left.x + square_len - padding,
            y1: top_left.y + padding,
            y2: top_left.y + square_len - padding,
        };
        let bounds = ui.primary.map.get_bounds();
        // Fit the entire width of the map in the box, to start
        let zoom = (square_len - (padding * 2.0)) / (bounds.max_x - bounds.min_x);

        g.fork(
            Pt2D::new(0.0, 0.0),
            ScreenPt::new(inner_rect.x1, inner_rect.y1),
            zoom,
        );
        g.redraw_clipped(&ui.primary.draw_map.boundary_polygon, &inner_rect);
        g.redraw_clipped(&ui.primary.draw_map.draw_all_areas, &inner_rect);
        g.redraw_clipped(&ui.primary.draw_map.draw_all_thick_roads, &inner_rect);
        g.redraw_clipped(
            &ui.primary.draw_map.draw_all_unzoomed_intersections,
            &inner_rect,
        );
        g.redraw_clipped(&ui.primary.draw_map.draw_all_buildings, &inner_rect);

        let mut cache = ui.primary.draw_map.agents.borrow_mut();
        cache.draw_unzoomed_agents(
            &ui.primary.sim,
            &ui.primary.map,
            &ui.agent_cs,
            g,
            Some(&inner_rect),
            zoom,
            Distance::meters(5.0),
        );

        // The cursor
        let (x1, y1) = {
            let pt = g.canvas.screen_to_map(ScreenPt::new(0.0, 0.0));
            (
                clamp(pt.x(), 0.0, bounds.max_x),
                clamp(pt.y(), 0.0, bounds.max_y),
            )
        };
        let (x2, y2) = {
            let pt = g
                .canvas
                .screen_to_map(ScreenPt::new(g.canvas.window_width, g.canvas.window_height));
            (
                clamp(pt.x(), 0.0, bounds.max_x),
                clamp(pt.y(), 0.0, bounds.max_y),
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
            println!("Warning: Minimap cursor is just a point right now");
        }
        g.unfork();
    }
}

pub struct VisibilityPanel {
    acs: AgentColorScheme,
    composite: Composite,
}

impl VisibilityPanel {
    fn make_panel(ctx: &EventCtx, acs: &AgentColorScheme) -> Composite {
        let radius = 15.0;
        let mut col = vec![
            // TODO Too wide most of the time...
            ManagedWidget::draw_text(ctx, Text::prompt(&acs.title)),
            ManagedWidget::btn(Button::text(
                Text::from(Line("change")),
                Color::grey(0.6),
                Color::ORANGE,
                hotkey(Key::Semicolon),
                "change agent colorscheme",
                ctx,
            )),
        ];
        for (label, color, enabled) in &acs.rows {
            col.push(
                ManagedWidget::row(vec![
                    ManagedWidget::btn(Button::rectangle_svg(
                        "assets/tools/visibility.svg",
                        &format!("show/hide {}", label),
                        None,
                        RewriteColor::Change(Color::WHITE, Color::ORANGE),
                        ctx,
                    )),
                    ManagedWidget::draw_batch(
                        ctx,
                        GeomBatch::from(vec![(
                            if *enabled {
                                color.alpha(1.0)
                            } else {
                                color.alpha(0.5)
                            },
                            Circle::new(Pt2D::new(radius, radius), Distance::meters(radius))
                                .to_polygon(),
                        )]),
                    ),
                    ManagedWidget::draw_text(
                        ctx,
                        Text::from(if *enabled {
                            Line(label)
                        } else {
                            Line(label).fg(Color::WHITE.alpha(0.5))
                        }),
                    ),
                ])
                .centered_cross(),
            );
        }
        Composite::minimal_size(
            ctx,
            ManagedWidget::col(col).bg(Color::grey(0.4)),
            ScreenPt::new(
                ctx.canvas.window_width - 550.0,
                ctx.canvas.window_height - 300.0,
            ),
        )
    }

    fn new(ctx: &EventCtx, ui: &UI) -> VisibilityPanel {
        VisibilityPanel {
            acs: ui.agent_cs.clone(),
            composite: VisibilityPanel::make_panel(ctx, &ui.agent_cs),
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Option<Transition> {
        // Happens when we changed the colorscheme in WizardState
        if ui.agent_cs != self.acs {
            *self = VisibilityPanel::new(ctx, ui);
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "change agent colorscheme" => {
                    return Some(Transition::Push(WizardState::new(Box::new(
                        |wiz, ctx, ui| {
                            let (_, acs) =
                                wiz.wrap(ctx).choose("Which colorscheme for agents?", || {
                                    let mut choices = Vec::new();
                                    for (acs, name) in AgentColorScheme::all(&ui.cs) {
                                        if ui.agent_cs.acs != acs.acs {
                                            choices.push(Choice::new(name, acs));
                                        }
                                    }
                                    choices
                                })?;
                            ui.agent_cs = acs;
                            // TODO It'd be great to replace self here, but the lifetimes don't
                            // work out.
                            Some(Transition::Pop)
                        },
                    ))));
                }
                x => {
                    let key = x["show/hide ".len()..].to_string();
                    ui.agent_cs.toggle(key);
                    *self = VisibilityPanel::new(ctx, ui);
                }
            },
            None => {}
        }

        None
    }

    fn draw(&self, g: &mut GfxCtx) {
        self.composite.draw(g);
    }
}
