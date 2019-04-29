use crate::ui::UI;
use ezgui::{Canvas, Color, EventCtx, GfxCtx, InputResult, ScreenPt, ScrollingMenu};
use geom::Polygon;

// TODO assumes minimum screen size
const WIDTH: u32 = 255;
const HEIGHT: u32 = 255;
const TILE_DIMS: f64 = 2.0;

// TODO parts of this should be in ezgui
pub enum ColorPicker {
    Choosing(ScrollingMenu<()>),
    // Remember the original modified color in case we revert.
    ChangingColor(String, Option<Color>),
}

impl ColorPicker {
    // True when done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> bool {
        match self {
            ColorPicker::Choosing(ref mut menu) => match menu.event(&mut ctx.input) {
                InputResult::Canceled => {
                    return true;
                }
                InputResult::StillActive => {}
                InputResult::Done(name, _) => {
                    *self = ColorPicker::ChangingColor(name.clone(), ui.cs.get_modified(&name));
                }
            },
            ColorPicker::ChangingColor(name, orig) => {
                ctx.input.set_mode_with_prompt(
                    "Color Picker",
                    format!("Color Picker for {}", name),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("revert") {
                    ui.cs.reset_modified(name, *orig);
                    return true;
                } else if ctx.input.modal_action("finalize") {
                    println!("Setting color for {}", name);
                    return true;
                }

                if let Some(pt) = ctx.input.get_moved_mouse() {
                    // TODO argh too much casting
                    let (start_x, start_y) = get_screen_offset(&ctx.canvas);
                    let x = (pt.x - start_x) / TILE_DIMS / 255.0;
                    let y = (pt.y - start_y) / TILE_DIMS / 255.0;
                    if x >= 0.0 && x <= 1.0 && y >= 0.0 && y <= 1.0 {
                        ui.cs.override_color(name, get_color(x as f32, y as f32));
                    }
                }
            }
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        match self {
            ColorPicker::Choosing(menu) => {
                menu.draw(g);
            }
            ColorPicker::ChangingColor(_, _) => {
                let (start_x, start_y) = get_screen_offset(g.canvas);

                for x in 0..WIDTH {
                    for y in 0..HEIGHT {
                        let color = get_color((x as f32) / 255.0, (y as f32) / 255.0);
                        let corner = g.screen_to_map(ScreenPt::new(
                            f64::from(x) * TILE_DIMS + start_x,
                            f64::from(y) * TILE_DIMS + start_y,
                        ));
                        g.draw_polygon(
                            color,
                            &Polygon::rectangle_topleft(corner, TILE_DIMS, TILE_DIMS),
                        );
                    }
                }
            }
        }
    }
}

fn get_screen_offset(canvas: &Canvas) -> (f64, f64) {
    let total_width = TILE_DIMS * f64::from(WIDTH);
    let total_height = TILE_DIMS * f64::from(HEIGHT);
    let start_x = (canvas.window_width - total_width) / 2.0;
    let start_y = (canvas.window_height - total_height) / 2.0;
    (start_x, start_y)
}

fn get_color(x: f32, y: f32) -> Color {
    assert!(x >= 0.0 && x <= 1.0);
    assert!(y >= 0.0 && y <= 1.0);
    Color::rgb_f(x, y, (x + y) / 2.0)
}
