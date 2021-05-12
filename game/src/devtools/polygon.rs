use geom::{Circle, Distance, LonLat, Pt2D, Ring};
use widgetry::{
    Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Panel, State, Text,
    VerticalAlignment, Widget,
};

use crate::app::App;
use crate::app::Transition;
use crate::common::CommonState;

const POINT_RADIUS: Distance = Distance::const_meters(10.0);
// Localized and internal, so don't put in ColorScheme.
const POINT_COLOR: Color = Color::RED;
const POLYGON_COLOR: Color = Color::BLUE.alpha(0.6);
const POINT_TO_MOVE: Color = Color::CYAN;
const LAST_PLACED_POINT: Color = Color::GREEN;

pub struct PolygonEditor {
    panel: Panel,
    name: String,
    points: Vec<LonLat>,
    mouseover_pt: Option<usize>,
    moving_pt: bool,
}

impl PolygonEditor {
    pub fn new(ctx: &mut EventCtx, name: String, mut points: Vec<LonLat>) -> Box<dyn State<App>> {
        points.pop();
        Box::new(PolygonEditor {
            panel: Panel::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Polygon editor").small_heading().into_widget(ctx),
                    ctx.style().btn_close_widget(ctx),
                ]),
                ctx.style()
                    .btn_outline
                    .text("export as an Osmosis polygon filter")
                    .hotkey(Key::X)
                    .build_def(ctx),
            ]))
            .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
            .build(ctx),
            name,
            points,
            mouseover_pt: None,
            moving_pt: false,
        })
    }
}

impl State<App> for PolygonEditor {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        let gps_bounds = app.primary.map.get_gps_bounds();

        ctx.canvas_movement();

        if self.moving_pt {
            if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
                self.points[self.mouseover_pt.unwrap()] = pt.to_gps(gps_bounds);
            }
            if ctx.input.key_released(Key::LeftControl) {
                self.moving_pt = false;
            }

            return Transition::Keep;
        }

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "export as an Osmosis polygon filter" => {
                    if self.points.len() >= 3 {
                        // Have to repeat the first point
                        self.points.push(self.points[0]);
                        LonLat::write_osmosis_polygon(&format!("{}.poly", self.name), &self.points)
                            .unwrap();
                        self.points.pop();
                    }
                }
                _ => unreachable!(),
            },
            _ => {}
        }

        if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
            self.mouseover_pt = self.points.iter().position(|pt| {
                Circle::new(pt.to_pt(gps_bounds), POINT_RADIUS / ctx.canvas.cam_zoom)
                    .contains_pt(cursor)
            });
        } else {
            self.mouseover_pt = None;
        }
        // TODO maybe click-and-drag is more intuitive
        if self.mouseover_pt.is_some() {
            if ctx.input.pressed(Key::LeftControl) {
                self.moving_pt = true;
            }
        } else if let Some(pt) = ctx.canvas.get_cursor_in_map_space() {
            if app.per_obj.left_click(ctx, "add a new point") {
                self.points.push(pt.to_gps(gps_bounds));
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        let pts: Vec<Pt2D> = app.primary.map.get_gps_bounds().convert(&self.points);

        if pts.len() == 2 {
            g.draw_polygon(
                POINT_COLOR,
                geom::Line::must_new(pts[0], pts[1]).make_polygons(POINT_RADIUS / 2.0),
            );
        }
        if pts.len() >= 3 {
            let mut pts = pts.clone();
            pts.push(pts[0]);
            g.draw_polygon(POLYGON_COLOR, Ring::must_new(pts).into_polygon());
        }
        for (idx, pt) in pts.iter().enumerate() {
            let color = if Some(idx) == self.mouseover_pt {
                POINT_TO_MOVE
            } else if idx == pts.len() - 1 {
                LAST_PLACED_POINT
            } else {
                POINT_COLOR
            };
            g.draw_polygon(
                color,
                Circle::new(*pt, POINT_RADIUS / g.canvas.cam_zoom).to_polygon(),
            );
        }

        self.panel.draw(g);
        if self.mouseover_pt.is_some() {
            CommonState::draw_custom_osd(g, app, Text::from("hold left Control to move point"));
        } else {
            CommonState::draw_osd(g, app);
        }
    }
}
