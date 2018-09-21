use abstutil;
use ezgui::{Canvas, GfxCtx, InputResult, Menu, TextBox, TextOSD, UserInput};
use geom::{Circle, Line, Polygon, Pt2D};
use map_model::Map;
use objects::EDIT_MAP;
use piston::input::{Button, Key, ReleaseEvent};
use plugins::Colorizer;
use polygons;
use std::collections::BTreeMap;

const POINT_RADIUS: f64 = 2.0;

pub enum DrawPolygonState {
    Empty,
    // Option<usize> is the point currently being hovered over, String is the possibly empty
    // pre-chosen name
    DrawingPoints(Vec<Pt2D>, Option<usize>, String),
    MovingPoint(Vec<Pt2D>, usize, String),
    NamingPolygon(TextBox, Vec<Pt2D>),
    // String name to each choice, pre-loaded
    ListingPolygons(Menu, BTreeMap<String, polygons::PolygonSelection>),
}

impl DrawPolygonState {
    pub fn new() -> DrawPolygonState {
        DrawPolygonState::Empty
    }

    pub fn event(
        &mut self,
        input: &mut UserInput,
        canvas: &Canvas,
        map: &Map,
        osd: &mut TextOSD,
    ) -> bool {
        let mut new_state: Option<DrawPolygonState> = None;
        match self {
            DrawPolygonState::Empty => {
                if input.unimportant_key_pressed(Key::N, EDIT_MAP, "start drawing a polygon") {
                    new_state = Some(DrawPolygonState::DrawingPoints(
                        Vec::new(),
                        None,
                        "".to_string(),
                    ));
                }
            }
            DrawPolygonState::DrawingPoints(ref mut pts, ref mut current_idx, name) => {
                osd.pad_if_nonempty();
                osd.add_line(format!("Currently editing {}", name));

                if input.key_pressed(Key::Tab, "list existing polygons") {
                    let polygons = polygons::load_all_polygons(map.get_name());
                    if polygons.is_empty() {
                        println!("Sorry, no existing polygons");
                    } else {
                        new_state = Some(DrawPolygonState::ListingPolygons(
                            Menu::new("Load which polygon?", polygons.keys().cloned().collect()),
                            polygons,
                        ));
                    }
                } else if input.key_pressed(Key::Escape, "throw away this neighborhood polygon") {
                    new_state = Some(DrawPolygonState::Empty);
                } else if input.key_pressed(Key::P, "add a new point here") {
                    pts.push(canvas.get_cursor_in_map_space());
                } else if pts.len() >= 3
                    && input.key_pressed(Key::Return, "confirm the polygon's shape")
                {
                    new_state = Some(DrawPolygonState::NamingPolygon(
                        TextBox::new_prefilled("Name this polygon", name.clone()),
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
                            new_state = Some(DrawPolygonState::MovingPoint(
                                pts.clone(),
                                *idx,
                                name.clone(),
                            ));
                        }
                    }
                }
            }
            DrawPolygonState::MovingPoint(ref mut pts, idx, name) => {
                osd.pad_if_nonempty();
                osd.add_line(format!("Currently editing {}", name));

                pts[*idx] = canvas.get_cursor_in_map_space();
                if let Some(Button::Keyboard(Key::LCtrl)) =
                    input.use_event_directly().release_args()
                {
                    new_state = Some(DrawPolygonState::DrawingPoints(
                        pts.clone(),
                        Some(*idx),
                        name.clone(),
                    ));
                }
            }
            DrawPolygonState::NamingPolygon(tb, pts) => match tb.event(input) {
                InputResult::Canceled => {
                    println!("Never mind!");
                    new_state = Some(DrawPolygonState::Empty);
                }
                InputResult::Done(name) => {
                    let path = format!("../data/polygons/{}/{}", map.get_name(), name);
                    abstutil::write_json(
                        &path,
                        &polygons::PolygonSelection {
                            name,
                            points: pts.clone(),
                        },
                    ).expect("Saving polygon selection failed");
                    println!("Saved {}", path);
                    new_state = Some(DrawPolygonState::Empty);
                }
                InputResult::StillActive => {}
            },
            DrawPolygonState::ListingPolygons(ref mut menu, polygons) => {
                match menu.event(input) {
                    InputResult::Canceled => {
                        new_state = Some(DrawPolygonState::Empty);
                    }
                    InputResult::StillActive => {}
                    InputResult::Done(choice) => {
                        new_state = Some(DrawPolygonState::DrawingPoints(
                            polygons[&choice].points.clone(),
                            None,
                            choice,
                        ));
                    }
                };
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            DrawPolygonState::Empty => false,
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
            DrawPolygonState::Empty => {
                return;
            }
            DrawPolygonState::DrawingPoints(pts, current_idx, _) => (pts, *current_idx),
            DrawPolygonState::MovingPoint(pts, idx, _) => (pts, Some(*idx)),
            DrawPolygonState::NamingPolygon(tb, pts) => {
                g.draw_polygon(blue, &Polygon::new(pts));
                tb.draw(g, canvas);
                return;
            }
            DrawPolygonState::ListingPolygons(menu, polygons) => {
                menu.draw(g, canvas);
                (&polygons[menu.current_choice()].points, None)
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

impl Colorizer for DrawPolygonState {}
