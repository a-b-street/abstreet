use crate::ui::UI;
use ezgui::{hotkey, Color, EventCtx, GfxCtx, Key, ModalMenu, Wizard, WrappedWizard};
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_model::{Map, NeighborhoodBuilder};

const POINT_RADIUS: Distance = Distance::const_meters(10.0);

pub enum NeighborhoodEditor {
    PickNeighborhood(Wizard),
    // Option<usize> is the point currently being hovered over
    EditNeighborhood(ModalMenu, NeighborhoodBuilder, Option<usize>),
    // usize is the point being moved
    MovingPoint(ModalMenu, NeighborhoodBuilder, usize),
}

impl NeighborhoodEditor {
    fn modal_menu(ctx: &EventCtx, name: &str) -> ModalMenu {
        ModalMenu::new(
            &format!("Neighborhood Editor for {}", name),
            vec![
                (hotkey(Key::Escape), "quit"),
                (hotkey(Key::S), "save"),
                (hotkey(Key::X), "export as an Osmosis polygon filter"),
                (hotkey(Key::P), "add a new point"),
            ],
            ctx,
        )
    }

    // True if done
    pub fn event(&mut self, ctx: &mut EventCtx, ui: &UI) -> bool {
        let gps_bounds = ui.primary.map.get_gps_bounds();
        match self {
            NeighborhoodEditor::PickNeighborhood(ref mut wizard) => {
                ctx.canvas.handle_event(ctx.input);

                if let Some(n) = pick_neighborhood(&ui.primary.map, wizard.wrap(ctx)) {
                    *self = NeighborhoodEditor::EditNeighborhood(
                        NeighborhoodEditor::modal_menu(ctx, &n.name),
                        n,
                        None,
                    );
                } else if wizard.aborted() {
                    return true;
                }
            }
            NeighborhoodEditor::EditNeighborhood(ref mut menu, ref mut n, ref mut current_idx) => {
                menu.handle_event(ctx, None);
                ctx.canvas.handle_event(ctx.input);

                if menu.action("quit") {
                    return true;
                } else if n.points.len() >= 3 && menu.action("save") {
                    n.save();
                    return true;
                } else if n.points.len() >= 3 && menu.action("export as an Osmosis polygon filter")
                {
                    n.save_as_osmosis().unwrap();
                } else if let Some(pt) = ctx
                    .canvas
                    .get_cursor_in_map_space()
                    .and_then(|c| c.to_gps(gps_bounds))
                {
                    if menu.action("add a new point") {
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
                        *self = NeighborhoodEditor::MovingPoint(
                            NeighborhoodEditor::modal_menu(ctx, &n.name),
                            n.clone(),
                            *idx,
                        );
                    }
                }
            }
            NeighborhoodEditor::MovingPoint(ref mut menu, ref mut n, idx) => {
                menu.handle_event(ctx, None);
                ctx.canvas.handle_event(ctx.input);

                if let Some(pt) = ctx
                    .canvas
                    .get_cursor_in_map_space()
                    .and_then(|c| c.to_gps(gps_bounds))
                {
                    n.points[*idx] = pt;
                }
                if ctx.input.key_released(Key::LeftControl) {
                    *self = NeighborhoodEditor::EditNeighborhood(
                        NeighborhoodEditor::modal_menu(ctx, &n.name),
                        n.clone(),
                        Some(*idx),
                    );
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
            NeighborhoodEditor::EditNeighborhood(_, n, current_idx) => (&n.points, *current_idx),
            NeighborhoodEditor::MovingPoint(_, n, current_idx) => (&n.points, Some(*current_idx)),
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

        match self {
            NeighborhoodEditor::EditNeighborhood(ref menu, _, _)
            | NeighborhoodEditor::MovingPoint(ref menu, _, _) => {
                menu.draw(g);
            }
            _ => {}
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

fn load_neighborhood_builder(
    map: &Map,
    wizard: &mut WrappedWizard,
    query: &str,
) -> Option<NeighborhoodBuilder> {
    let map_name = map.get_name().to_string();
    wizard
        .choose_something_no_keys::<NeighborhoodBuilder>(
            query,
            Box::new(move || abstutil::load_all_objects("neighborhoods", &map_name)),
        )
        .map(|(_, n)| n)
}
