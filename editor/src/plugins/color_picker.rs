// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::{ColorScheme, Colors};
use ezgui::{Canvas, GfxCtx, InputResult, Menu, UserInput};
use graphics;
use piston::input::{Key, MouseCursorEvent};
use plugins::Colorizer;
use std::str::FromStr;
use std::string::ToString;
use strum::IntoEnumIterator;

// TODO assumes minimum screen size
const WIDTH: u32 = 255;
const HEIGHT: u32 = 255;
const TILE_DIMS: u32 = 2;

// TODO parts of this should be in ezgui
pub enum ColorPicker {
    Inactive,
    Choosing(Menu),
    // Remember the original color, in case we revert
    PickingColor(Colors, graphics::types::Color),
}

impl ColorPicker {
    pub fn new() -> ColorPicker {
        ColorPicker::Inactive
    }

    pub fn event(&mut self, input: &mut UserInput, canvas: &Canvas, cs: &mut ColorScheme) -> bool {
        let mut new_state: Option<ColorPicker> = None;
        match self {
            ColorPicker::Inactive => {
                if input.unimportant_key_pressed(Key::D8, "configure colors") {
                    new_state = Some(ColorPicker::Choosing(Menu::new(
                        "Pick a color to change",
                        Colors::iter().map(|c| c.to_string()).collect(),
                    )));
                }
            }
            ColorPicker::Choosing(ref mut menu) => {
                match menu.event(input) {
                    InputResult::Canceled => {
                        new_state = Some(ColorPicker::Inactive);
                    }
                    InputResult::StillActive => {}
                    InputResult::Done(choice) => {
                        let c = Colors::from_str(&choice).unwrap();
                        new_state = Some(ColorPicker::PickingColor(c, cs.get(c)));
                    }
                };
            }
            ColorPicker::PickingColor(c, orig_color) => {
                if input.key_pressed(
                    Key::Escape,
                    &format!("stop configuring color for {:?} and revert", c),
                ) {
                    cs.set(*c, *orig_color);
                    new_state = Some(ColorPicker::Inactive);
                } else if input.key_pressed(Key::Return, &format!("finalize new color for {:?}", c))
                {
                    println!("Setting color for {:?}", c);
                    new_state = Some(ColorPicker::Inactive);
                }

                if let Some(pos) = input.use_event_directly().mouse_cursor_args() {
                    // TODO argh too much casting
                    let (start_x, start_y) = get_screen_offset(canvas);
                    let x = (pos[0] - (start_x as f64)) / (TILE_DIMS as f64) / 255.0;
                    let y = (pos[1] - (start_y as f64)) / (TILE_DIMS as f64) / 255.0;
                    if x >= 0.0 && x <= 1.0 && y >= 0.0 && y <= 1.0 {
                        cs.set(*c, get_color(x as f32, y as f32));
                    }
                }
            }
        };
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            ColorPicker::Inactive => false,
            _ => true,
        }
    }

    pub fn draw(&self, canvas: &Canvas, g: &mut GfxCtx) {
        match self {
            ColorPicker::Inactive => {}
            ColorPicker::Choosing(menu) => {
                canvas.draw_centered_text(g, menu.get_osd());
            }
            ColorPicker::PickingColor(_, _) => {
                let (start_x, start_y) = get_screen_offset(canvas);

                for x in 0..WIDTH {
                    for y in 0..HEIGHT {
                        let color = get_color((x as f32) / 255.0, (y as f32) / 255.0);
                        let corner = canvas.screen_to_map((
                            (x * TILE_DIMS + start_x) as f64,
                            (y * TILE_DIMS + start_y) as f64,
                        ));
                        g.draw_rectangle(
                            color,
                            [corner.x(), corner.y(), TILE_DIMS as f64, TILE_DIMS as f64],
                        );
                    }
                }
            }
        }
    }
}

impl Colorizer for ColorPicker {}

fn get_screen_offset(canvas: &Canvas) -> (u32, u32) {
    let total_width = TILE_DIMS * WIDTH;
    let total_height = TILE_DIMS * HEIGHT;
    let start_x = (canvas.window_size.width - total_width) / 2;
    let start_y = (canvas.window_size.height - total_height) / 2;
    (start_x, start_y)
}

fn get_color(x: f32, y: f32) -> graphics::types::Color {
    assert!(x >= 0.0 && x <= 1.0);
    assert!(y >= 0.0 && y <= 1.0);
    [x, y, (x + y) / 2.0, 1.0]
}
