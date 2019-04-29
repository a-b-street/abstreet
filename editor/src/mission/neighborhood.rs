use crate::helpers::load_neighborhood_builder;
use crate::ui::UI;
use ezgui::{Color, EventCtx, GfxCtx, Key, Wizard, WrappedWizard};
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_model::{Map, NeighborhoodBuilder};

const POINT_RADIUS: Distance = Distance::const_meters(10.0);

pub enum NeighborhoodEditor {
    PickNeighborhood(Wizard),
    // Option<usize> is the point currently being hovered over
    EditNeighborhood(NeighborhoodBuilder, Option<usize>),
    // usize is the point being moved
    MovingPoint(NeighborhoodBuilder, usize),
}

impl NeighborhoodEditor {
    // True if done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> bool {
        ctx.canvas.handle_event(ctx.input);

        let gps_bounds = ui.primary.map.get_gps_bounds();
        match self {
            NeighborhoodEditor::PickNeighborhood(ref mut wizard) => {
                if let Some(n) =
                    pick_neighborhood(&ui.primary.map, wizard.wrap(&mut ctx.input, ctx.canvas))
                {
                    *self = NeighborhoodEditor::EditNeighborhood(n, None);
                } else if wizard.aborted() {
                    return true;
                }
            }
            NeighborhoodEditor::EditNeighborhood(ref mut n, ref mut current_idx) => {
                ctx.input.set_mode_with_prompt(
                    "Neighborhood Editor",
                    format!("Neighborhood Editor for {}", n.name),
                    &ctx.canvas,
                );
                if ctx.input.modal_action("quit") {
                    return true;
                } else if n.points.len() >= 3 && ctx.input.modal_action("save") {
                    n.save();
                    return true;
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
                        Circle::new(
                            Pt2D::from_gps(*pt, gps_bounds).unwrap(),
                            POINT_RADIUS / ctx.canvas.cam_zoom,
                        )
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
                        *self = NeighborhoodEditor::MovingPoint(n.clone(), *idx);
                    }
                }
            }
            NeighborhoodEditor::MovingPoint(ref mut n, idx) => {
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
                    *self = NeighborhoodEditor::EditNeighborhood(n.clone(), Some(*idx));
                }
            }
        }
        false
    }

    pub fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let (raw_pts, current_idx) = match self {
            NeighborhoodEditor::PickNeighborhood(wizard) => {
                // TODO is this order wrong?
                wizard.draw(g);
                if let Some(neighborhood) = wizard.current_menu_choice::<NeighborhoodBuilder>() {
                    (&neighborhood.points, None)
                } else {
                    return;
                }
            }
            NeighborhoodEditor::EditNeighborhood(n, current_idx) => (&n.points, *current_idx),
            NeighborhoodEditor::MovingPoint(n, current_idx) => (&n.points, Some(*current_idx)),
        };
        let gps_bounds = ui.primary.map.get_gps_bounds();
        let pts: Vec<Pt2D> = gps_bounds.must_convert(&raw_pts);

        if pts.len() == 2 {
            g.draw_line(
                ui.cs.get_def("neighborhood point", Color::RED),
                POINT_RADIUS / 2.0,
                &Line::new(pts[0], pts[1]),
            );
        }
        if pts.len() >= 3 {
            g.draw_polygon(
                ui.cs
                    .get_def("neighborhood polygon", Color::BLUE.alpha(0.6)),
                &Polygon::new(&pts),
            );
        }
        for (idx, pt) in pts.iter().enumerate() {
            let color = if Some(idx) == current_idx {
                ui.cs.get_def("neighborhood point to move", Color::CYAN)
            } else if idx == pts.len() - 1 {
                ui.cs
                    .get_def("neighborhood last placed point", Color::GREEN)
            } else {
                ui.cs.get("neighborhood point")
            };
            g.draw_circle(color, &Circle::new(*pt, POINT_RADIUS / g.canvas.cam_zoom));
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
