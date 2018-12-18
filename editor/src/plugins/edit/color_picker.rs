// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use crate::objects::Ctx;
use crate::plugins::{Plugin, PluginCtx};
use ezgui::ScreenPt;
use ezgui::{Canvas, Color, GfxCtx, InputResult, ScrollingMenu};
use geom::Polygon;

// TODO assumes minimum screen size
const WIDTH: u32 = 255;
const HEIGHT: u32 = 255;
const TILE_DIMS: u32 = 2;

// TODO parts of this should be in ezgui
pub enum ColorPicker {
    Choosing(ScrollingMenu<()>),
    // Remember the original modified color in case we revert.
    ChangingColor(String, Option<Color>),
}

impl ColorPicker {
    pub fn new(ctx: &mut PluginCtx) -> Option<ColorPicker> {
        if ctx.input.action_chosen("configure colors") {
            return Some(ColorPicker::Choosing(ScrollingMenu::new(
                "Pick a color to change",
                ctx.cs.color_names(),
            )));
        }
        None
    }
}

impl Plugin for ColorPicker {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        match self {
            ColorPicker::Choosing(ref mut menu) => {
                match menu.event(&mut ctx.input) {
                    InputResult::Canceled => {
                        return false;
                    }
                    InputResult::StillActive => {}
                    InputResult::Done(name, _) => {
                        *self =
                            ColorPicker::ChangingColor(name.clone(), ctx.cs.get_modified(&name));
                    }
                };
            }
            ColorPicker::ChangingColor(name, orig) => {
                ctx.input.set_mode_with_prompt(
                    "Color Picker",
                    format!("Color Picker for {}", name),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("revert") {
                    ctx.cs.reset_modified(name, *orig);
                    return false;
                } else if ctx.input.modal_action("finalize") {
                    info!("Setting color for {}", name);
                    return false;
                }

                if let Some(pt) = ctx.input.get_moved_mouse() {
                    // TODO argh too much casting
                    let (start_x, start_y) = get_screen_offset(&ctx.canvas);
                    let x = (pt.x - f64::from(start_x)) / f64::from(TILE_DIMS) / 255.0;
                    let y = (pt.y - f64::from(start_y)) / f64::from(TILE_DIMS) / 255.0;
                    if x >= 0.0 && x <= 1.0 && y >= 0.0 && y <= 1.0 {
                        ctx.cs.override_color(name, get_color(x as f32, y as f32));
                    }
                }
            }
        };
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &Ctx) {
        match self {
            ColorPicker::Choosing(menu) => {
                menu.draw(g, ctx.canvas);
            }
            ColorPicker::ChangingColor(_, _) => {
                let (start_x, start_y) = get_screen_offset(ctx.canvas);

                for x in 0..WIDTH {
                    for y in 0..HEIGHT {
                        let color = get_color((x as f32) / 255.0, (y as f32) / 255.0);
                        let corner = ctx.canvas.screen_to_map(ScreenPt::new(
                            f64::from(x * TILE_DIMS + start_x),
                            f64::from(y * TILE_DIMS + start_y),
                        ));
                        g.draw_polygon(
                            color,
                            &Polygon::rectangle_topleft(
                                corner,
                                f64::from(TILE_DIMS),
                                f64::from(TILE_DIMS),
                            ),
                        );
                    }
                }
            }
        }
    }
}

fn get_screen_offset(canvas: &Canvas) -> (u32, u32) {
    let total_width = TILE_DIMS * WIDTH;
    let total_height = TILE_DIMS * HEIGHT;
    let start_x = (canvas.window_size.width - total_width) / 2;
    let start_y = (canvas.window_size.height - total_height) / 2;
    (start_x, start_y)
}

fn get_color(x: f32, y: f32) -> Color {
    assert!(x >= 0.0 && x <= 1.0);
    assert!(y >= 0.0 && y <= 1.0);
    Color::rgb_f(x, y, (x + y) / 2.0)
}
