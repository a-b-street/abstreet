use ezgui::{Canvas, GfxCtx, Text, UserInput, Wizard, WrappedWizard};
use geom::{Circle, Line, Polygon};
use map_model::Map;
use objects::EDIT_MAP;
use piston::input::Key;
use plugins::{load_neighborhood, Colorizer};
use sim::Neighborhood;

const POINT_RADIUS: f64 = 2.0;

// load or new -> edit (drawing pts) -> MovingPt

pub enum DrawNeighborhoodState {
    Inactive,
    PickNeighborhood(Wizard),
    // Option<usize> is the point currently being hovered over
    EditNeighborhood(Neighborhood, Option<usize>),
    // usize is the point being moved
    MovingPoint(Neighborhood, usize),
}

impl DrawNeighborhoodState {
    pub fn new() -> DrawNeighborhoodState {
        DrawNeighborhoodState::Inactive
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
            DrawNeighborhoodState::Inactive => {
                if input.unimportant_key_pressed(Key::N, EDIT_MAP, "start drawing a neighborhood") {
                    new_state = Some(DrawNeighborhoodState::PickNeighborhood(Wizard::new()));
                }
            }
            DrawNeighborhoodState::PickNeighborhood(ref mut wizard) => {
                if let Some(n) = pick_neighborhood(map, wizard.wrap(input)) {
                    new_state = Some(DrawNeighborhoodState::EditNeighborhood(n, None));
                } else if wizard.aborted() {
                    new_state = Some(DrawNeighborhoodState::Inactive);
                }
            }
            DrawNeighborhoodState::EditNeighborhood(ref mut n, ref mut current_idx) => {
                osd.pad_if_nonempty();
                osd.add_line(format!("Currently editing {}", n.name));

                if input.key_pressed(Key::Escape, "quit") {
                    new_state = Some(DrawNeighborhoodState::Inactive);
                } else if input.key_pressed(Key::P, "add a new point here") {
                    n.points.push(canvas.get_cursor_in_map_space());
                } else if n.points.len() >= 3 && input.key_pressed(Key::Return, "save") {
                    n.save();
                    new_state = Some(DrawNeighborhoodState::Inactive);
                }

                if new_state.is_none() {
                    let cursor = canvas.get_cursor_in_map_space();
                    *current_idx = n
                        .points
                        .iter()
                        .position(|pt| Circle::new(*pt, POINT_RADIUS).contains_pt(cursor));
                    if let Some(idx) = current_idx {
                        // TODO mouse dragging might be more intuitive, but it's unclear how to
                        // override part of canvas.handle_event
                        if input.key_pressed(Key::LCtrl, "hold to move this point") {
                            new_state = Some(DrawNeighborhoodState::MovingPoint(n.clone(), *idx));
                        }
                    }
                }
            }
            DrawNeighborhoodState::MovingPoint(ref mut n, idx) => {
                osd.pad_if_nonempty();
                osd.add_line(format!("Currently editing {}", n.name));

                n.points[*idx] = canvas.get_cursor_in_map_space();
                if input.key_released(Key::LCtrl) {
                    new_state = Some(DrawNeighborhoodState::EditNeighborhood(
                        n.clone(),
                        Some(*idx),
                    ));
                }
            }
        }
        if let Some(s) = new_state {
            *self = s;
        }
        match self {
            DrawNeighborhoodState::Inactive => false,
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
            DrawNeighborhoodState::Inactive => {
                return;
            }
            DrawNeighborhoodState::PickNeighborhood(wizard) => {
                // TODO is this order wrong?
                wizard.draw(g, canvas);
                if let Some(neighborhood) = wizard.current_menu_choice::<Neighborhood>() {
                    (&neighborhood.points, None)
                } else {
                    return;
                }
            }
            DrawNeighborhoodState::EditNeighborhood(n, current_idx) => (&n.points, *current_idx),
            DrawNeighborhoodState::MovingPoint(n, current_idx) => (&n.points, Some(*current_idx)),
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

fn pick_neighborhood(map: &Map, mut wizard: WrappedWizard) -> Option<Neighborhood> {
    let load_existing = "Load existing neighborhood";
    let create_new = "Create new neighborhood";
    if wizard.choose_string(
        "What neighborhood to edit?",
        vec![load_existing, create_new],
    )? == load_existing
    {
        load_neighborhood(map, &mut wizard, "Load which neighborhood?")
    } else {
        let name = wizard.input_string("Name the neighborhood")?;
        Some(Neighborhood {
            name,
            map_name: map.get_name().to_string(),
            points: Vec::new(),
        })
    }
}
