use crate::app::App;
use crate::common::CommonState;
use crate::game::{State, Transition};
use crate::managed::WrappedComposite;
use ezgui::{
    hotkey, Choice, Color, Composite, EventCtx, GfxCtx, Key, Line, Outcome, Text, Wizard,
    WrappedWizard,
};
use geom::{Circle, Distance, Polygon, Pt2D};
use map_model::{Map, NeighborhoodBuilder};

const POINT_RADIUS: Distance = Distance::const_meters(10.0);
// Localized and internal, so don't put in ColorScheme.
const POINT_COLOR: Color = Color::RED;
const POLYGON_COLOR: Color = Color::BLUE.alpha(0.6);
const POINT_TO_MOVE: Color = Color::CYAN;
const LAST_PLACED_POINT: Color = Color::GREEN;

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
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        ctx.canvas_movement();

        if let Some(n) = pick_neighborhood(&app.primary.map, self.wizard.wrap(ctx)) {
            self.wizard = Wizard::new();
            return Transition::Push(Box::new(NeighborhoodEditor {
                composite: WrappedComposite::quick_menu(
                    ctx,
                    app,
                    format!("Neighborhood Editor for {}", n.name),
                    vec![],
                    vec![
                        (hotkey(Key::S), "save"),
                        (hotkey(Key::X), "export as an Osmosis polygon filter"),
                    ],
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

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        // TODO is this order wrong?
        self.wizard.draw(g);
        if let Some(neighborhood) = self.wizard.current_menu_choice::<NeighborhoodBuilder>() {
            g.draw_polygon(
                POLYGON_COLOR,
                &Polygon::new(
                    &app.primary
                        .map
                        .get_gps_bounds()
                        .must_convert(&neighborhood.points),
                ),
            );
        }
    }
}

struct NeighborhoodEditor {
    composite: Composite,
    neighborhood: NeighborhoodBuilder,
    mouseover_pt: Option<usize>,
    moving_pt: bool,
}

impl State for NeighborhoodEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let gps_bounds = app.primary.map.get_gps_bounds();

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

            return Transition::Keep;
        }

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "X" => {
                    return Transition::Pop;
                }
                "save" => {
                    if self.neighborhood.points.len() >= 3 {
                        self.neighborhood.save();
                    }
                }
                "export as an Osmosis polygon filter" => {
                    if self.neighborhood.points.len() >= 3 {
                        self.neighborhood.save_as_osmosis().unwrap();
                    }
                }
                _ => unreachable!(),
            },
            None => {}
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
        if self.mouseover_pt.is_some() {
            if ctx
                .input
                .key_pressed(Key::LeftControl, "hold to move this point")
            {
                self.moving_pt = true;
            }
        } else if let Some(pt) = ctx
            .canvas
            .get_cursor_in_map_space()
            .and_then(|c| c.to_gps(gps_bounds))
        {
            if app.per_obj.left_click(ctx, "add a new point") {
                self.neighborhood.points.push(pt);
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let pts: Vec<Pt2D> = app
            .primary
            .map
            .get_gps_bounds()
            .must_convert(&self.neighborhood.points);

        if pts.len() == 2 {
            g.draw_line(
                POINT_COLOR,
                POINT_RADIUS / 2.0,
                &geom::Line::new(pts[0], pts[1]),
            );
        }
        if pts.len() >= 3 {
            g.draw_polygon(POLYGON_COLOR, &Polygon::new(&pts));
        }
        for (idx, pt) in pts.iter().enumerate() {
            let color = if Some(idx) == self.mouseover_pt {
                POINT_TO_MOVE
            } else if idx == pts.len() - 1 {
                LAST_PLACED_POINT
            } else {
                POINT_COLOR
            };
            g.draw_circle(color, &Circle::new(*pt, POINT_RADIUS / g.canvas.cam_zoom));
        }

        self.composite.draw(g);
        if self.mouseover_pt.is_some() {
            CommonState::draw_custom_osd(
                g,
                app,
                Text::from(Line("hold left Control to move point")),
            );
        } else {
            CommonState::draw_osd(g, app, &None);
        }
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
            city_name: map.get_city_name().to_string(),
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
                abstutil::path_all_neighborhoods(map.get_city_name(), map.get_name()),
            ))
        })
        .map(|(_, n)| n)
}
