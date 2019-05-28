use crate::screen_geom::ScreenRectangle;
use crate::{Color, EventCtx, GfxCtx};
use geom::{Distance, Polygon, Pt2D};

// Pixels
const BAR_WIDTH: f64 = 300.0;
const BAR_HEIGHT: f64 = 100.0;
const SLIDER_WIDTH: f64 = 50.0;
const SLIDER_HEIGHT: f64 = 120.0;

const HORIZ_PADDING: f64 = 60.0;
const VERT_PADDING: f64 = 20.0;

pub struct Slider {
    current_percent: f64,
    mouse_on_slider: bool,
    dragging: bool,
}

impl Slider {
    pub fn new() -> Slider {
        Slider {
            current_percent: 0.0,
            mouse_on_slider: false,
            dragging: false,
        }
    }

    pub fn get_percent(&self) -> f64 {
        self.current_percent
    }

    pub fn get_value(&self, num_items: usize) -> usize {
        (self.current_percent * (num_items as f64 - 1.0)) as usize
    }

    pub fn set_percent(&mut self, ctx: &mut EventCtx, percent: f64) {
        assert!(percent >= 0.0 && percent <= 1.0);
        self.current_percent = percent;
        // Just reset dragging, to prevent chaos
        self.dragging = false;
        let pt = ctx.canvas.get_cursor_in_screen_space();
        self.mouse_on_slider = self.slider_geom().contains_pt(Pt2D::new(pt.x, pt.y));
    }

    pub fn set_value(&mut self, ctx: &mut EventCtx, idx: usize, num_items: usize) {
        self.set_percent(ctx, (idx as f64) / (num_items as f64 - 1.0));
    }

    // Returns true if the percentage changed.
    pub fn event(&mut self, ctx: &mut EventCtx) -> bool {
        if self.dragging {
            if ctx.input.get_moved_mouse().is_some() {
                let percent =
                    (ctx.canvas.get_cursor_in_screen_space().x - HORIZ_PADDING) / BAR_WIDTH;
                self.current_percent = percent.min(1.0).max(0.0);
                return true;
            }
            if ctx.input.left_mouse_button_released() {
                self.dragging = false;
            }
        } else {
            if !ctx.canvas.is_dragging() && ctx.input.get_moved_mouse().is_some() {
                let pt = ctx.canvas.get_cursor_in_screen_space();
                self.mouse_on_slider = self.slider_geom().contains_pt(Pt2D::new(pt.x, pt.y));
            }
            if ctx.input.left_mouse_button_pressed() {
                if self.mouse_on_slider {
                    self.dragging = true;
                } else {
                    // Did we click somewhere else on the bar?
                    let pt = ctx.canvas.get_cursor_in_screen_space();
                    if Polygon::rectangle_topleft(
                        Pt2D::new(HORIZ_PADDING, VERT_PADDING),
                        Distance::meters(BAR_WIDTH),
                        Distance::meters(BAR_HEIGHT),
                    )
                    .contains_pt(Pt2D::new(pt.x, pt.y))
                    {
                        let percent = (pt.x - HORIZ_PADDING) / BAR_WIDTH;
                        self.current_percent = percent.min(1.0).max(0.0);
                        self.mouse_on_slider = true;
                        self.dragging = true;
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        g.fork_screenspace();

        // A nice background for the entire thing
        g.draw_polygon(
            Color::grey(0.3),
            &Polygon::rectangle_topleft(
                Pt2D::new(0.0, 0.0),
                Distance::meters(BAR_WIDTH + 2.0 * HORIZ_PADDING),
                Distance::meters(BAR_HEIGHT + 2.0 * VERT_PADDING),
            ),
        );
        g.canvas.mark_covered_area(ScreenRectangle {
            x1: 0.0,
            y1: 0.0,
            x2: BAR_WIDTH + 2.0 * HORIZ_PADDING,
            y2: BAR_HEIGHT + 2.0 * VERT_PADDING,
        });

        // The bar
        g.draw_polygon(
            Color::WHITE,
            &Polygon::rectangle_topleft(
                Pt2D::new(HORIZ_PADDING, VERT_PADDING),
                Distance::meters(BAR_WIDTH),
                Distance::meters(BAR_HEIGHT),
            ),
        );

        // Show the progress
        if self.current_percent != 0.0 {
            g.draw_polygon(
                Color::GREEN,
                &Polygon::rectangle_topleft(
                    Pt2D::new(HORIZ_PADDING, VERT_PADDING),
                    Distance::meters(self.current_percent * BAR_WIDTH),
                    Distance::meters(BAR_HEIGHT),
                ),
            );
        }

        // The actual slider
        g.draw_polygon(
            if self.mouse_on_slider {
                Color::YELLOW
            } else {
                Color::grey(0.7)
            },
            &self.slider_geom(),
        );
    }

    fn slider_geom(&self) -> Polygon {
        Polygon::rectangle_topleft(
            Pt2D::new(
                HORIZ_PADDING + self.current_percent * BAR_WIDTH - (SLIDER_WIDTH / 2.0),
                VERT_PADDING - (SLIDER_HEIGHT - BAR_HEIGHT) / 2.0,
            ),
            Distance::meters(SLIDER_WIDTH),
            Distance::meters(SLIDER_HEIGHT),
        )
    }
}
