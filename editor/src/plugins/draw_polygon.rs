use ezgui::{Canvas, GfxCtx, TextBox, UserInput};
use geom::{Circle, Line, Polygon, Pt2D};
use piston::input::{Button, Key, ReleaseEvent};
use plugins::Colorizer;

const POINT_RADIUS: f64 = 2.0;

pub enum DrawPolygonState {
    Empty,
    // Option<usize> is the point currently being hovered over
    DrawingPoints(Vec<Pt2D>, Option<usize>),
    MovingPoint(Vec<Pt2D>, usize),
    NamingPolygon(TextBox, Vec<Pt2D>),
}

impl DrawPolygonState {
    pub fn new() -> DrawPolygonState {
        DrawPolygonState::Empty
    }

    pub fn event(&mut self, input: &mut UserInput, canvas: &Canvas) -> bool {
        let mut new_state: Option<DrawPolygonState> = None;
        match self {
            DrawPolygonState::Empty => {
                if input.unimportant_key_pressed(Key::N, "start drawing a polygon") {
                    new_state = Some(DrawPolygonState::DrawingPoints(Vec::new(), None));
                }
            }
            DrawPolygonState::DrawingPoints(ref mut pts, ref mut current_idx) => {
                if input.key_pressed(Key::Escape, "throw away this neighborhood polygon") {
                    new_state = Some(DrawPolygonState::Empty);
                } else if input.key_pressed(Key::P, "add a new point here") {
                    pts.push(canvas.get_cursor_in_map_space());
                } else if pts.len() >= 3
                    && input.key_pressed(Key::Return, "confirm the polygon's shape")
                {
                    new_state = Some(DrawPolygonState::NamingPolygon(TextBox::new(), pts.clone()));
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
                            new_state = Some(DrawPolygonState::MovingPoint(pts.clone(), *idx));
                        }
                    }
                }
            }
            DrawPolygonState::MovingPoint(ref mut pts, idx) => {
                pts[*idx] = canvas.get_cursor_in_map_space();
                if let Some(Button::Keyboard(Key::LCtrl)) =
                    input.use_event_directly().release_args()
                {
                    new_state = Some(DrawPolygonState::DrawingPoints(pts.clone(), Some(*idx)));
                }
            }
            DrawPolygonState::NamingPolygon(tb, pts) => {
                if tb.event(input.use_event_directly()) {
                    println!("TODO: save neighborhood {} with points {:?}", tb.line, pts);
                    new_state = Some(DrawPolygonState::Empty);
                }
                input.consume_event();
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

    pub fn get_osd_lines(&self) -> Vec<String> {
        // TODO draw the cursor
        if let DrawPolygonState::NamingPolygon(tb, _) = self {
            return vec![tb.line.clone()];
        }
        Vec::new()
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        // TODO add colorscheme entries
        let red = [1.0, 0.0, 0.0, 1.0];
        let green = [0.0, 1.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];
        let cyan = [0.0, 1.0, 1.0, 1.0];

        let (pts, current_idx) = match self {
            DrawPolygonState::Empty => {
                return;
            }
            DrawPolygonState::DrawingPoints(pts, current_idx) => (pts, *current_idx),
            DrawPolygonState::MovingPoint(pts, idx) => (pts, Some(*idx)),
            DrawPolygonState::NamingPolygon(_, pts) => {
                g.draw_polygon(blue, &Polygon::new(pts));
                return;
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
