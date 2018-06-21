// Copyright 2018 Google LLC, licensed under http://www.apache.org/licenses/LICENSE-2.0

use colors::Colors;
use ezgui::canvas::{Canvas, GfxCtx};
use ezgui::input::UserInput;
use ezgui::menu;
use graphics;
use piston::input::{Key, MouseCursorEvent};
use piston::window::Size;
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
    Choosing(menu::Menu),
    PickingColor(Colors),
}

impl ColorPicker {
    pub fn new() -> ColorPicker {
        ColorPicker::Inactive
    }

    pub fn handle_event(self, input: &mut UserInput, window_size: &Size) -> ColorPicker {
        match self {
            ColorPicker::Inactive => {
                if input.unimportant_key_pressed(Key::D8, "Press 8 to configure colors") {
                    return ColorPicker::Choosing(menu::Menu::new(
                        Colors::iter().map(|c| c.to_string()).collect(),
                    ));
                }
                ColorPicker::Inactive
            }
            ColorPicker::Choosing(mut menu) => {
                // TODO arrow keys scroll canvas too
                match menu.event(input.use_event_directly()) {
                    menu::Result::Canceled => ColorPicker::Inactive,
                    menu::Result::StillActive => ColorPicker::Choosing(menu),
                    menu::Result::Done(choice) => {
                        ColorPicker::PickingColor(Colors::from_str(&choice).unwrap())
                    }
                }
            }
            ColorPicker::PickingColor(color) => {
                // TODO be able to confirm a choice and edit the ColorScheme
                if input.key_pressed(Key::D8, "Press 8 to stop configuring colors") {
                    return ColorPicker::Inactive;
                }

                if let Some(pos) = input.use_event_directly().mouse_cursor_args() {
                    // TODO argh too much casting
                    let (start_x, start_y) = get_screen_offset(window_size);
                    let x = (pos[0] - (start_x as f64)) / (TILE_DIMS as f64) / 255.0;
                    let y = (pos[1] - (start_y as f64)) / (TILE_DIMS as f64) / 255.0;
                    if x >= 0.0 && x <= 1.0 && y >= 0.0 && y <= 1.0 {
                        println!("current color is {:?}", get_color(x as f32, y as f32));
                    }
                }

                ColorPicker::PickingColor(color)
            }
        }
    }

    pub fn draw(&self, canvas: &Canvas, g: &mut GfxCtx) {
        match self {
            ColorPicker::Inactive => {}
            ColorPicker::Choosing(menu) => {
                // TODO sloppy to use a mouse tooltip. ideally should be easy to figure out how
                // many lines to display and center it.
                // TODO would be nice to display the text in the current color
                canvas.draw_mouse_tooltip(g, &menu.lines_to_display());
            }
            ColorPicker::PickingColor(_) => {
                let (start_x, start_y) = get_screen_offset(&g.window_size);

                for x in 0..WIDTH {
                    for y in 0..HEIGHT {
                        let color = get_color((x as f32) / 255.0, (y as f32) / 255.0);
                        let pixel = graphics::Rectangle::new(color);
                        pixel.draw(
                            [
                                (x * TILE_DIMS + start_x) as f64,
                                (y * TILE_DIMS + start_y) as f64,
                                TILE_DIMS as f64,
                                TILE_DIMS as f64,
                            ],
                            &g.orig_ctx.draw_state,
                            g.orig_ctx.transform,
                            g.gfx,
                        );
                    }
                }
            }
        }
    }
}

fn get_screen_offset(window_size: &Size) -> (u32, u32) {
    let total_width = TILE_DIMS * WIDTH;
    let total_height = TILE_DIMS * HEIGHT;
    let start_x = (window_size.width - total_width) / 2;
    let start_y = (window_size.height - total_height) / 2;
    (start_x, start_y)
}

fn get_color(x: f32, y: f32) -> graphics::types::Color {
    assert!(x >= 0.0 && x <= 1.0);
    assert!(y >= 0.0 && y <= 1.0);
    [x, y, (x + y) / 2.0, 1.0]
}
