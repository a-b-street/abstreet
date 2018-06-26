use ezgui::canvas::Canvas;
use ezgui::input::UserInput;
use ezgui::text_box::TextBox;
use map_model::Map;
use map_model::RoadID;
use piston::input::Key;
use piston::window::Size;
use std::usize;

pub enum WarpState {
    Empty,
    EnteringSearch(TextBox),
}

impl WarpState {
    pub fn event(
        self,
        input: &mut UserInput,
        map: &Map,
        canvas: &mut Canvas,
        window_size: &Size,
    ) -> WarpState {
        match self {
            WarpState::Empty => {
                if input.unimportant_key_pressed(
                    Key::J,
                    "Press J to start searching for a road to warp to",
                ) {
                    WarpState::EnteringSearch(TextBox::new())
                } else {
                    self
                }
            }
            WarpState::EnteringSearch(mut tb) => {
                if tb.event(input.use_event_directly()) {
                    input.consume_event();
                    warp(tb.line, map, canvas, window_size);
                    WarpState::Empty
                } else {
                    input.consume_event();
                    WarpState::EnteringSearch(tb)
                }
            }
        }
    }

    pub fn get_osd_lines(&self) -> Vec<String> {
        // TODO draw the cursor
        if let WarpState::EnteringSearch(text_box) = self {
            return vec![text_box.line.clone()];
        }
        Vec::new()
    }
}

fn warp(line: String, map: &Map, canvas: &mut Canvas, window_size: &Size) {
    match usize::from_str_radix(&line, 10) {
        Ok(idx) => {
            let id = RoadID(idx);
            println!("Warping to {}", id);
            let pt = map.get_r(id).first_pt();
            canvas.center_on_map_pt(pt[0], pt[1], window_size);
        }
        Err(_) => {
            println!("{} isn't a valid ID", line);
        }
    }
}
