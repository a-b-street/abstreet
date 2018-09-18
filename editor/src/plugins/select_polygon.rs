use ezgui::{Canvas, GfxCtx, TextBox, UserInput};
use geom::{Circle, Line, Polygon, Pt2D};
use piston::input::Key;
use plugins::Colorizer;

pub enum SelectPolygonState {
    Empty,
    SelectingPoints(Vec<Pt2D>),
    NamingPolygon(TextBox, Vec<Pt2D>),
}

impl SelectPolygonState {
    pub fn new() -> SelectPolygonState {
        SelectPolygonState::Empty
    }

    pub fn event(&mut self, input: &mut UserInput, canvas: &Canvas) -> bool {
        let mut new_state: Option<SelectPolygonState> = None;
        match self {
            SelectPolygonState::Empty => {
                if input.unimportant_key_pressed(Key::N, "start drawing a polygon") {
                    new_state = Some(SelectPolygonState::SelectingPoints(Vec::new()));
                }
            }
            SelectPolygonState::SelectingPoints(ref mut pts) => {
                if input.key_pressed(Key::Escape, "throw away this neighborhood polygon") {
                    new_state = Some(SelectPolygonState::Empty);
                } else if input.key_pressed(Key::P, "add a new point here") {
                    pts.push(canvas.get_cursor_in_map_space());
                } else if pts.len() >= 3
                    && input.key_pressed(Key::Return, "confirm the polygon's shape")
                {
                    new_state = Some(SelectPolygonState::NamingPolygon(
                        TextBox::new(),
                        pts.clone(),
                    ));
                }
                // TODO move existing points
            }
            SelectPolygonState::NamingPolygon(tb, pts) => {
                if tb.event(input.use_event_directly()) {
                    println!("TODO: save neighborhood {} with points {:?}", tb.line, pts);
                    new_state = Some(SelectPolygonState::Empty);
                }
                input.consume_event();
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            SelectPolygonState::Empty => false,
            _ => true,
        }
    }

    pub fn get_osd_lines(&self) -> Vec<String> {
        // TODO draw the cursor
        if let SelectPolygonState::NamingPolygon(tb, _) = self {
            return vec![tb.line.clone()];
        }
        Vec::new()
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        let pts = match self {
            SelectPolygonState::Empty => {
                return;
            }
            SelectPolygonState::SelectingPoints(pts) => pts,
            SelectPolygonState::NamingPolygon(_, pts) => pts,
        };

        // TODO add colorscheme entries
        let red = [1.0, 0.0, 0.0, 1.0];
        let green = [0.0, 1.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];
        let radius = 2.0;

        if pts.len() == 2 {
            g.draw_line(red, radius / 2.0, &Line::new(pts[0], pts[1]));
        }
        if pts.len() >= 3 {
            g.draw_polygon(blue, &Polygon::new(pts));
        }
        for pt in pts {
            g.draw_circle(red, &Circle::new(*pt, radius));
        }
        g.draw_circle(green, &Circle::new(*pts.last().unwrap(), radius));
    }
}

impl Colorizer for SelectPolygonState {}
