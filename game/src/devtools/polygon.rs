use crate::app::App;
use crate::common::CommonState;
use crate::game::{State, Transition};
use ezgui::{
    hotkey, Btn, Color, Composite, EventCtx, GfxCtx, HorizontalAlignment, Key, Line, Outcome, Text,
    VerticalAlignment, Widget,
};
use geom::{Circle, Distance, LonLat, Pt2D, Ring};
use std::fs::File;
use std::io::{Error, Write};

const POINT_RADIUS: Distance = Distance::const_meters(10.0);
// Localized and internal, so don't put in ColorScheme.
const POINT_COLOR: Color = Color::RED;
const POLYGON_COLOR: Color = Color::BLUE.alpha(0.6);
const POINT_TO_MOVE: Color = Color::CYAN;
const LAST_PLACED_POINT: Color = Color::GREEN;

pub struct PolygonEditor {
    composite: Composite,
    name: String,
    points: Vec<LonLat>,
    mouseover_pt: Option<usize>,
    moving_pt: bool,
}

impl PolygonEditor {
    pub fn new(ctx: &mut EventCtx, name: String, mut points: Vec<LonLat>) -> Box<dyn State> {
        points.pop();
        Box::new(PolygonEditor {
            composite: Composite::new(Widget::col(vec![
                Widget::row(vec![
                    Line("Polygon editor").small_heading().draw(ctx),
                    Btn::text_fg("X")
                        .build(ctx, "close", hotkey(Key::Escape))
                        .align_right(),
                ]),
                Btn::text_fg("export as an Osmosis polygon filter").build_def(ctx, hotkey(Key::X)),
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

impl State for PolygonEditor {
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

        match self.composite.event(ctx) {
            Some(Outcome::Clicked(x)) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                "export as an Osmosis polygon filter" => {
                    if self.points.len() >= 3 {
                        save_as_osmosis(&self.name, &self.points).unwrap();
                    }
                }
                _ => unreachable!(),
            },
            None => {}
        }

        if let Some(cursor) = ctx.canvas.get_cursor_in_map_space() {
            self.mouseover_pt = self.points.iter().position(|pt| {
                Circle::new(
                    Pt2D::from_gps(*pt, gps_bounds),
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
            g.draw_line(
                POINT_COLOR,
                POINT_RADIUS / 2.0,
                &geom::Line::must_new(pts[0], pts[1]),
            );
        }
        if pts.len() >= 3 {
            g.draw_polygon(POLYGON_COLOR, &Ring::must_new(pts.clone()).to_polygon());
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
            CommonState::draw_osd(g, app);
        }
    }
}

// https://wiki.openstreetmap.org/wiki/Osmosis/Polygon_Filter_File_Format
fn save_as_osmosis(name: &str, pts: &Vec<LonLat>) -> Result<(), Error> {
    let path = format!("{}.poly", name);
    let mut f = File::create(&path)?;

    writeln!(f, "{}", name)?;
    writeln!(f, "1")?;
    for gps in pts {
        writeln!(f, "     {}    {}", gps.x(), gps.y())?;
    }
    // Have to repeat the first point
    {
        writeln!(f, "     {}    {}", pts[0].x(), pts[0].y())?;
    }
    writeln!(f, "END")?;
    writeln!(f, "END")?;

    println!("Exported {}", path);
    Ok(())
}
