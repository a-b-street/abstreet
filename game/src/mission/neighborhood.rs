use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{hotkey, Choice, Color, EventCtx, GfxCtx, Key, ModalMenu, Wizard, WrappedWizard};
use geom::{Circle, Distance, Line, Polygon, Pt2D};
use map_model::{Map, NeighborhoodBuilder};

const POINT_RADIUS: Distance = Distance::const_meters(10.0);

// This shouldn't get subsumed by WizardState, since it has such an interesting draw().
pub struct NeighborhoodPicker {
    wizard: Wizard,
}

impl NeighborhoodPicker {
    pub fn new() -> NeighborhoodPicker {
        NeighborhoodPicker {
            wizard: Wizard::new(),
        }
    }
}

impl State for NeighborhoodPicker {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        ctx.canvas_movement();

        if let Some(n) = pick_neighborhood(&ui.primary.map, self.wizard.wrap(ctx)) {
            self.wizard = Wizard::new();
            return Transition::Push(Box::new(NeighborhoodEditor {
                menu: ModalMenu::new(
                    format!("Neighborhood Editor for {}", n.name),
                    vec![
                        (hotkey(Key::Escape), "quit"),
                        (hotkey(Key::S), "save"),
                        (hotkey(Key::X), "export as an Osmosis polygon filter"),
                        (hotkey(Key::P), "add a new point"),
                    ],
                    ctx,
                ),
                neighborhood: n,
                mouseover_pt: None,
                moving_pt: false,
            }));
        } else if self.wizard.aborted() {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        // TODO is this order wrong?
        self.wizard.draw(g);
        if let Some(neighborhood) = self.wizard.current_menu_choice::<NeighborhoodBuilder>() {
            g.draw_polygon(
                ui.cs.get("neighborhood polygon"),
                &Polygon::new(
                    &ui.primary
                        .map
                        .get_gps_bounds()
                        .must_convert(&neighborhood.points),
                ),
            );
        }
    }
}

struct NeighborhoodEditor {
    menu: ModalMenu,
    neighborhood: NeighborhoodBuilder,
    mouseover_pt: Option<usize>,
    moving_pt: bool,
}

impl State for NeighborhoodEditor {
    fn event(&mut self, ctx: &mut EventCtx, ui: &mut UI) -> Transition {
        let gps_bounds = ui.primary.map.get_gps_bounds();

        self.menu.event(ctx);
        ctx.canvas_movement();

        if self.moving_pt {
            if let Some(pt) = ctx
                .canvas
                .get_cursor_in_map_space()
                .and_then(|c| c.to_gps(gps_bounds))
            {
                self.neighborhood.points[self.mouseover_pt.unwrap()] = pt;
            }
            if ctx.input.key_released(Key::LeftControl) {
                self.moving_pt = false;
            }
        } else {
            if self.menu.action("quit") {
                return Transition::Pop;
            } else if self.neighborhood.points.len() >= 3 && self.menu.action("save") {
                self.neighborhood.save();
            } else if self.neighborhood.points.len() >= 3
                && self.menu.action("export as an Osmosis polygon filter")
            {
                self.neighborhood.save_as_osmosis().unwrap();
            } else if let Some(pt) = ctx
                .canvas
                .get_cursor_in_map_space()
                .and_then(|c| c.to_gps(gps_bounds))
            {
                if self.menu.action("add a new point") {
                    self.neighborhood.points.push(pt);
                }
            }

            if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
                self.mouseover_pt = self.neighborhood.points.iter().position(|pt| {
                    Circle::new(
                        Pt2D::from_gps(*pt, gps_bounds).unwrap(),
                        POINT_RADIUS / ctx.canvas.cam_zoom,
                    )
                    .contains_pt(cursor)
                });
            } else {
                self.mouseover_pt = None;
            }
            // TODO maybe click-and-drag is more intuitive
            if self.mouseover_pt.is_some()
                && ctx
                    .input
                    .key_pressed(Key::LeftControl, "hold to move this point")
            {
                self.moving_pt = true;
            }
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, ui: &UI) {
        let pts: Vec<Pt2D> = ui
            .primary
            .map
            .get_gps_bounds()
            .must_convert(&self.neighborhood.points);

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
            let color = if Some(idx) == self.mouseover_pt {
                ui.cs.get_def("neighborhood point to move", Color::CYAN)
            } else if idx == pts.len() - 1 {
                ui.cs
                    .get_def("neighborhood last placed point", Color::GREEN)
            } else {
                ui.cs.get("neighborhood point")
            };
            g.draw_circle(color, &Circle::new(*pt, POINT_RADIUS / g.canvas.cam_zoom));
        }

        self.menu.draw(g);
    }
}

fn pick_neighborhood(map: &Map, mut wizard: WrappedWizard) -> Option<NeighborhoodBuilder> {
    let load_existing = "Load existing neighborhood";
    let create_new = "Create new neighborhood";
    if wizard.choose_string("What neighborhood to edit?", || {
        vec![load_existing, create_new]
    })? == load_existing
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
    wizard
        .choose(query, || {
            Choice::from(abstutil::load_all_objects(
                abstutil::path_all_neighborhoods(&map.get_name()),
            ))
        })
        .map(|(_, n)| n)
}
