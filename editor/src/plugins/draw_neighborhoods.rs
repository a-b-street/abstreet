use abstutil;
use ezgui::{Canvas, GfxCtx, InputResult, Menu, Text, TextBox, UserInput};
use geom::{Circle, Line, Polygon, Pt2D};
use map_model::Map;
use objects::EDIT_MAP;
use piston::input::{Button, Key, ReleaseEvent};
use plugins::Colorizer;
use sim::Neighborhood;

const POINT_RADIUS: f64 = 2.0;

pub enum DrawNeighborhoodState {
    Empty,
    // Option<usize> is the point currently being hovered over, String is the possibly empty
    // pre-chosen name
    DrawingPoints(Vec<Pt2D>, Option<usize>, String),
    MovingPoint(Vec<Pt2D>, usize, String),
    NamingNeighborhood(TextBox, Vec<Pt2D>),
    // String name to each choice, pre-loaded
    ListingNeighborhoods(Menu<Neighborhood>),
}

impl DrawNeighborhoodState {
    pub fn new() -> DrawNeighborhoodState {
        DrawNeighborhoodState::Empty
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        canvas: &Canvas,
        map: &Map,
        osd: &mut Text,
    ) -> bool {
        let mut new_state: Option<DrawNeighborhoodState> = None;
        match self {
            DrawNeighborhoodState::Empty => {
                if input.unimportant_key_pressed(Key::N, EDIT_MAP, "start drawing a neighborhood") {
                    new_state = Some(DrawNeighborhoodState::DrawingPoints(
                        Vec::new(),
                        None,
                        "".to_string(),
                    ));
                }
            }
            DrawNeighborhoodState::DrawingPoints(ref mut pts, ref mut current_idx, name) => {
                osd.pad_if_nonempty();
                osd.add_line(format!("Currently editing {}", name));

                if input.key_pressed(Key::Tab, "list existing neighborhoods") {
                    let neighborhoods: Vec<(String, Neighborhood)> =
                        abstutil::load_all_objects("neighborhoods", map.get_name());
                    if neighborhoods.is_empty() {
                        warn!("Sorry, no existing neighborhoods");
                    } else {
                        new_state = Some(DrawNeighborhoodState::ListingNeighborhoods(Menu::new(
                            "Load which neighborhood?",
                            neighborhoods,
                        )));
                    }
                } else if input.key_pressed(Key::Escape, "throw away this neighborhood") {
                    new_state = Some(DrawNeighborhoodState::Empty);
                } else if input.key_pressed(Key::P, "add a new point here") {
                    pts.push(canvas.get_cursor_in_map_space());
                } else if pts.len() >= 3
                    && input.key_pressed(Key::Return, "confirm the neighborhood's shape")
                {
                    new_state = Some(DrawNeighborhoodState::NamingNeighborhood(
                        TextBox::new_prefilled("Name this neighborhood", name.clone()),
                        pts.clone(),
                    ));
                }

                if new_state.is_none() {
                    let cursor = canvas.get_cursor_in_map_space();
                    *current_idx = pts
                        .iter()
                        .position(|pt| Circle::new(*pt, POINT_RADIUS).contains_pt(cursor));
                    if let Some(idx) = current_idx {
                        // TODO mouse dragging might be more intuitive, but it's unclear how to
                        // override part of canvas.handle_event
                        if input.key_pressed(Key::LCtrl, "hold to move this point") {
                            new_state = Some(DrawNeighborhoodState::MovingPoint(
                                pts.clone(),
                                *idx,
                                name.clone(),
                            ));
                        }
                    }
                }
            }
            DrawNeighborhoodState::MovingPoint(ref mut pts, idx, name) => {
                osd.pad_if_nonempty();
                osd.add_line(format!("Currently editing {}", name));

                pts[*idx] = canvas.get_cursor_in_map_space();
                if let Some(Button::Keyboard(Key::LCtrl)) =
                    input.use_event_directly().release_args()
                {
                    new_state = Some(DrawNeighborhoodState::DrawingPoints(
                        pts.clone(),
                        Some(*idx),
                        name.clone(),
                    ));
                }
            }
            DrawNeighborhoodState::NamingNeighborhood(tb, pts) => match tb.event(input) {
                InputResult::Canceled => {
                    info!("Never mind!");
                    new_state = Some(DrawNeighborhoodState::Empty);
                }
                InputResult::Done(name, _) => {
                    let path = format!("../data/neighborhoods/{}/{}", map.get_name(), name);
                    abstutil::write_json(
                        &path,
                        &Neighborhood {
                            name,
                            points: pts.clone(),
                        },
                    ).expect("Saving neighborhood failed");
                    info!("Saved {}", path);
                    new_state = Some(DrawNeighborhoodState::Empty);
                }
                InputResult::StillActive => {}
            },
            DrawNeighborhoodState::ListingNeighborhoods(ref mut menu) => {
                match menu.event(input) {
                    InputResult::Canceled => {
                        new_state = Some(DrawNeighborhoodState::Empty);
                    }
                    InputResult::StillActive => {}
                    InputResult::Done(name, poly) => {
                        new_state = Some(DrawNeighborhoodState::DrawingPoints(
                            poly.points.clone(),
                            None,
                            name,
                        ));
                    }
                };
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            DrawNeighborhoodState::Empty => false,
            _ => true,
        }
    }

    pub fn draw(&self, g: &mut GfxCtx, canvas: &Canvas) {
        // TODO add colorscheme entries
        let red = [1.0, 0.0, 0.0, 1.0];
        let green = [0.0, 1.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 0.6];
        let cyan = [0.0, 1.0, 1.0, 1.0];

        let (pts, current_idx) = match self {
            DrawNeighborhoodState::Empty => {
                return;
            }
            DrawNeighborhoodState::DrawingPoints(pts, current_idx, _) => (pts, *current_idx),
            DrawNeighborhoodState::MovingPoint(pts, idx, _) => (pts, Some(*idx)),
            DrawNeighborhoodState::NamingNeighborhood(tb, pts) => {
                g.draw_polygon(blue, &Polygon::new(pts));
                tb.draw(g, canvas);
                return;
            }
            DrawNeighborhoodState::ListingNeighborhoods(menu) => {
                menu.draw(g, canvas);
                (&menu.current_choice().points, None)
            }
        };

        if pts.len() == 2 {
            g.draw_line(red, POINT_RADIUS / 2.0, &Line::new(pts[0], pts[1]));
        }
        if pts.len() >= 3 {
            g.draw_polygon(blue, &Polygon::new(pts));
        }
        for pt in pts {
            g.draw_circle(red, &Circle::new(*pt, POINT_RADIUS));
        }
        if let Some(last) = pts.last() {
            g.draw_circle(green, &Circle::new(*last, POINT_RADIUS));
        }
        if let Some(idx) = current_idx {
            g.draw_circle(cyan, &Circle::new(pts[idx], POINT_RADIUS));
        }
    }
}

impl Colorizer for DrawNeighborhoodState {}
