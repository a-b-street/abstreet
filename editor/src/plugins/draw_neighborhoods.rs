use ezgui::{Color, GfxCtx, Wizard, WrappedWizard};
use geom::{Circle, Line, Polygon, Pt2D};
use map_model::Map;
use objects::{Ctx, EDIT_MAP};
use piston::input::Key;
use plugins::{load_neighborhood_builder, Plugin, PluginCtx};
use sim::NeighborhoodBuilder;

const POINT_RADIUS: f64 = 2.0;

// load or new -> edit (drawing pts) -> MovingPt

pub enum DrawNeighborhoodState {
    Inactive,
    PickNeighborhood(Wizard),
    // Option<usize> is the point currently being hovered over
    EditNeighborhood(NeighborhoodBuilder, Option<usize>),
    // usize is the point being moved
    MovingPoint(NeighborhoodBuilder, usize),
}

impl DrawNeighborhoodState {
    pub fn new() -> DrawNeighborhoodState {
        DrawNeighborhoodState::Inactive
    }
}

impl Plugin for DrawNeighborhoodState {
    fn event(&mut self, ctx: PluginCtx) -> bool {
        let (input, canvas, map, osd) = (ctx.input, ctx.canvas, &ctx.primary.map, ctx.osd);
        let gps_bounds = map.get_gps_bounds();

        // TODO This can easily be outside of the map boundary...
        let get_cursor_in_gps = || canvas.get_cursor_in_map_space().to_gps(gps_bounds);

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
                } else if input.key_pressed(Key::X, "export this as an Osmosis polygon filter") {
                    n.save_as_osmosis().unwrap();
                } else if input.key_pressed(Key::P, "add a new point here") {
                    n.points.push(get_cursor_in_gps());
                } else if n.points.len() >= 3 && input.key_pressed(Key::Return, "save") {
                    n.save();
                    new_state = Some(DrawNeighborhoodState::Inactive);
                }

                if new_state.is_none() {
                    let cursor = canvas.get_cursor_in_map_space();
                    *current_idx = n.points.iter().position(|pt| {
                        Circle::new(Pt2D::from_gps(*pt, gps_bounds).unwrap(), POINT_RADIUS)
                            .contains_pt(cursor)
                    });
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

                n.points[*idx] = get_cursor_in_gps();
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

    fn draw(&self, g: &mut GfxCtx, ctx: Ctx) {
        let (raw_pts, current_idx) = match self {
            DrawNeighborhoodState::Inactive => {
                return;
            }
            DrawNeighborhoodState::PickNeighborhood(wizard) => {
                // TODO is this order wrong?
                wizard.draw(g, ctx.canvas);
                if let Some(neighborhood) = wizard.current_menu_choice::<NeighborhoodBuilder>() {
                    (&neighborhood.points, None)
                } else {
                    return;
                }
            }
            DrawNeighborhoodState::EditNeighborhood(n, current_idx) => (&n.points, *current_idx),
            DrawNeighborhoodState::MovingPoint(n, current_idx) => (&n.points, Some(*current_idx)),
        };
        let gps_bounds = ctx.map.get_gps_bounds();
        let pts: Vec<Pt2D> = raw_pts
            .into_iter()
            .map(|pt| Pt2D::from_gps(*pt, gps_bounds).unwrap())
            .collect();

        if pts.len() == 2 {
            g.draw_line(
                ctx.cs.get("neighborhood point", Color::RED),
                POINT_RADIUS / 2.0,
                &Line::new(pts[0], pts[1]),
            );
        }
        if pts.len() >= 3 {
            g.draw_polygon(
                ctx.cs
                    .get("neighborhood polygon", Color::rgba(0, 0, 255, 0.6)),
                &Polygon::new(&pts),
            );
        }
        for pt in &pts {
            g.draw_circle(
                ctx.cs.get("neighborhood point", Color::RED),
                &Circle::new(*pt, POINT_RADIUS),
            );
        }
        if let Some(last) = pts.last() {
            g.draw_circle(
                ctx.cs.get("neighborhood last placed point", Color::GREEN),
                &Circle::new(*last, POINT_RADIUS),
            );
        }
        if let Some(idx) = current_idx {
            g.draw_circle(
                ctx.cs.get("neighborhood point to move", Color::CYAN),
                &Circle::new(pts[idx], POINT_RADIUS),
            );
        }
    }
}

fn pick_neighborhood(map: &Map, mut wizard: WrappedWizard) -> Option<NeighborhoodBuilder> {
    let load_existing = "Load existing neighborhood";
    let create_new = "Create new neighborhood";
    if wizard.choose_string(
        "What neighborhood to edit?",
        vec![load_existing, create_new],
    )? == load_existing
    {
        load_neighborhood_builder(map, &mut wizard, "Load which neighborhood?")
    } else {
        let name = wizard.input_string("Name the neighborhood")?;
        Some(NeighborhoodBuilder {
            name,
            map_name: map.get_name().to_string(),
            points: Vec::new(),
        })
    }
}
