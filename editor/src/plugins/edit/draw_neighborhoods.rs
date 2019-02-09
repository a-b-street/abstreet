use crate::objects::DrawCtx;
use crate::plugins::{load_neighborhood_builder, BlockingPlugin, PluginCtx};
use ezgui::{Color, GfxCtx, Key, Wizard, WrappedWizard};
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_model::{Map, NeighborhoodBuilder};

const POINT_RADIUS: Distance = Distance::const_meters(2.0);

pub enum DrawNeighborhoodState {
    PickNeighborhood(Wizard),
    // Option<usize> is the point currently being hovered over
    EditNeighborhood(NeighborhoodBuilder, Option<usize>),
    // usize is the point being moved
    MovingPoint(NeighborhoodBuilder, usize),
}

impl DrawNeighborhoodState {
    pub fn new(ctx: &mut PluginCtx) -> Option<DrawNeighborhoodState> {
        if ctx.input.action_chosen("manage neighborhoods") {
            return Some(DrawNeighborhoodState::PickNeighborhood(Wizard::new()));
        }
        None
    }
}

impl BlockingPlugin for DrawNeighborhoodState {
    fn blocking_event(&mut self, ctx: &mut PluginCtx) -> bool {
        let gps_bounds = ctx.primary.map.get_gps_bounds();
        match self {
            DrawNeighborhoodState::PickNeighborhood(ref mut wizard) => {
                if let Some(n) =
                    pick_neighborhood(&ctx.primary.map, wizard.wrap(&mut ctx.input, ctx.canvas))
                {
                    *self = DrawNeighborhoodState::EditNeighborhood(n, None);
                } else if wizard.aborted() {
                    return false;
                }
            }
            DrawNeighborhoodState::EditNeighborhood(ref mut n, ref mut current_idx) => {
                ctx.input.set_mode_with_prompt(
                    "Neighborhood Editor",
                    format!("Neighborhood Editor for {}", n.name),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("quit") {
                    return false;
                } else if n.points.len() >= 3 && ctx.input.modal_action("save") {
                    n.save();
                    return false;
                } else if n.points.len() >= 3
                    && ctx
                        .input
                        .modal_action("export as an Osmosis polygon filter")
                {
                    n.save_as_osmosis().unwrap();
                } else if let Some(pt) = ctx
                    .canvas
                    .get_cursor_in_map_space()
                    .and_then(|c| c.to_gps(gps_bounds))
                {
                    if ctx.input.modal_action("add a new point") {
                        n.points.push(pt);
                    }
                }

                if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                    *current_idx = n.points.iter().position(|pt| {
                        Circle::new(Pt2D::from_gps(*pt, gps_bounds).unwrap(), POINT_RADIUS)
                            .contains_pt(cursor)
                    });
                } else {
                    *current_idx = None;
                }
                if let Some(idx) = current_idx {
                    // TODO mouse dragging might be more intuitive, but it's unclear how to
                    // override part of canvas.handle_event
                    if ctx
                        .input
                        .key_pressed(Key::LeftControl, "hold to move this point")
                    {
                        *self = DrawNeighborhoodState::MovingPoint(n.clone(), *idx);
                    }
                }
            }
            DrawNeighborhoodState::MovingPoint(ref mut n, idx) => {
                ctx.input.set_mode_with_prompt(
                    "Neighborhood Editor",
                    format!("Neighborhood Editor for {}", n.name),
                    &ctx.canvas,
                );

                if let Some(pt) = ctx
                    .canvas
                    .get_cursor_in_map_space()
                    .and_then(|c| c.to_gps(gps_bounds))
                {
                    n.points[*idx] = pt;
                }
                if ctx.input.key_released(Key::LeftControl) {
                    *self = DrawNeighborhoodState::EditNeighborhood(n.clone(), Some(*idx));
                }
            }
        }
        true
    }

    fn draw(&self, g: &mut GfxCtx, ctx: &DrawCtx) {
        let (raw_pts, current_idx) = match self {
            DrawNeighborhoodState::PickNeighborhood(wizard) => {
                // TODO is this order wrong?
                wizard.draw(g);
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
                ctx.cs.get_def("neighborhood point", Color::RED),
                POINT_RADIUS / 2.0,
                &Line::new(pts[0], pts[1]),
            );
        }
        if pts.len() >= 3 {
            g.draw_polygon(
                ctx.cs
                    .get_def("neighborhood polygon", Color::BLUE.alpha(0.6)),
                &Polygon::new(&pts),
            );
        }
        for pt in &pts {
            g.draw_circle(
                ctx.cs.get("neighborhood point"),
                &Circle::new(*pt, POINT_RADIUS),
            );
        }
        if let Some(last) = pts.last() {
            g.draw_circle(
                ctx.cs
                    .get_def("neighborhood last placed point", Color::GREEN),
                &Circle::new(*last, POINT_RADIUS),
            );
        }
        if let Some(idx) = current_idx {
            g.draw_circle(
                ctx.cs.get_def("neighborhood point to move", Color::CYAN),
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
