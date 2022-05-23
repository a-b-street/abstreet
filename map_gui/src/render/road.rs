use std::cell::RefCell;

use geom::{Distance, Polygon, Pt2D};
use map_model::{Building, LaneType, Map, Road, RoadID, NORMAL_LANE_THICKNESS};
use widgetry::{Color, Drawable, GeomBatch, GfxCtx, Line, Prerender, Text};

use crate::colors::ColorSchemeChoice;
use crate::options::CameraAngle;
use crate::render::lane::DrawLane;
use crate::render::{DrawOptions, Renderable};
use crate::{AppLike, ID};

// The default font size is too large; shrink it down to fit on roads better.
const LABEL_SCALE_FACTOR: f64 = 0.1;
// Making the label follow the road's curvature usually looks better, but sometimes the letters
// squish together, so keep this experiment disabled for now.
const DRAW_CURVEY_LABEL: bool = true;

pub struct DrawRoad {
    pub id: RoadID,
    zorder: isize,

    draw: RefCell<Option<Drawable>>,
    pub lanes: Vec<DrawLane>,
}

impl DrawRoad {
    pub fn new(r: &Road) -> DrawRoad {
        DrawRoad {
            id: r.id,
            zorder: r.zorder,
            draw: RefCell::new(None),
            lanes: r.lanes.iter().map(|l| DrawLane::new(l, r)).collect(),
        }
    }

    fn render_center_line<P: AsRef<Prerender>>(
        &self,
        app: &dyn AppLike,
        prerender: &P,
    ) -> GeomBatch {
        let r = app.map().get_r(self.id);
        let name = r.get_name(app.opts().language.as_ref());
        let prerender = prerender.as_ref();
        let text_width =
            Distance::meters(Text::from(&name).rendered_width(prerender) * LABEL_SCALE_FACTOR);

        let center_line_color = if r.is_private() && app.cs().private_road.is_some() {
            app.cs()
                .road_center_line
                .lerp(app.cs().private_road.unwrap(), 0.5)
        } else {
            app.cs().road_center_line
        };

        let mut batch = GeomBatch::new();

        // Draw a center line every time two driving/bike/bus lanes of opposite direction are
        // adjacent.
        let mut width = Distance::ZERO;
        for pair in r.lanes.windows(2) {
            width += pair[0].width;
            if pair[0].dir != pair[1].dir
                && pair[0].lane_type.is_for_moving_vehicles()
                && pair[1].lane_type.is_for_moving_vehicles()
            {
                let pl = r.shift_from_left_side(width).unwrap();
                // Draw dashed lines from the start of the road to where the text label begins,
                // then another set of dashes from where the text label ends to the end of the
                // road
                let first_segment_distance = (pl.length() - text_width) / 2.0;
                let last_segment_distance = first_segment_distance + text_width;

                if let Ok((line, _)) = pl.slice(Distance::ZERO, first_segment_distance) {
                    batch.extend(
                        center_line_color,
                        line.dashed_lines(
                            Distance::meters(0.25),
                            Distance::meters(2.0),
                            Distance::meters(1.0),
                        ),
                    );
                }

                if let Ok((line, _)) = pl.slice(last_segment_distance, pl.length()) {
                    batch.extend(
                        center_line_color,
                        line.dashed_lines(
                            Distance::meters(0.25),
                            Distance::meters(2.0),
                            Distance::meters(1.0),
                        ),
                    );
                }
            }
        }

        batch
    }

    pub fn render<P: AsRef<Prerender>>(&self, prerender: &P, app: &dyn AppLike) -> GeomBatch {
        let prerender = prerender.as_ref();
        let r = app.map().get_r(self.id);
        let center_line_color = if r.is_private() && app.cs().private_road.is_some() {
            app.cs()
                .road_center_line
                .lerp(app.cs().private_road.unwrap(), 0.5)
        } else {
            app.cs().road_center_line
        };

        let mut batch = self.render_center_line(app, prerender);

        // Draw the label
        if !r.is_light_rail() {
            let name = r.get_name(app.opts().language.as_ref());
            if r.length() >= Distance::meters(30.0) && name != "???" {
                if DRAW_CURVEY_LABEL {
                    let fg = Color::WHITE;
                    if r.center_pts.quadrant() > 1 && r.center_pts.quadrant() < 4 {
                        batch.append(Line(name).fg(fg).outlined(Color::BLACK).render_curvey(
                            prerender,
                            &r.center_pts.reversed(),
                            LABEL_SCALE_FACTOR,
                        ));
                    } else {
                        batch.append(Line(name).fg(fg).outlined(Color::BLACK).render_curvey(
                            prerender,
                            &r.center_pts,
                            LABEL_SCALE_FACTOR,
                        ));
                    }
                } else {
                    // TODO If it's definitely straddling bus/bike lanes, change the color? Or
                    // even easier, just skip the center lines?
                    let bg = if r.is_private() && app.cs().private_road.is_some() {
                        app.cs()
                            .zoomed_road_surface(LaneType::Driving, r.get_rank())
                            .lerp(app.cs().private_road.unwrap(), 0.5)
                    } else {
                        app.cs()
                            .zoomed_road_surface(LaneType::Driving, r.get_rank())
                    };

                    let txt = Text::from(Line(name).fg(center_line_color)).bg(bg);
                    let (pt, angle) = r.center_pts.must_dist_along(r.length() / 2.0);
                    batch.append(
                        txt.render_autocropped(prerender)
                            .scale(LABEL_SCALE_FACTOR)
                            .centered_on(pt)
                            .rotate_around_batch_center(angle.reorient()),
                    );
                }
            }
        }

        // Driveways of connected buildings. These are grouped by road to limit what has to be
        // recalculated when road edits cause buildings to re-snap.
        for b in app.map().road_to_buildings(self.id) {
            draw_building_driveway(app, app.map().get_b(*b), &mut batch);
        }

        batch
    }

    pub fn clear_rendering(&mut self) {
        *self.draw.borrow_mut() = None;
        for l in &mut self.lanes {
            l.clear_rendering();
        }
    }
}

impl Renderable for DrawRoad {
    fn get_id(&self) -> ID {
        ID::Road(self.id)
    }

    fn draw(&self, g: &mut GfxCtx, app: &dyn AppLike, _: &DrawOptions) {
        let mut draw = self.draw.borrow_mut();
        if draw.is_none() {
            *draw = Some(g.upload(self.render(g, app)));
        }
        g.redraw(draw.as_ref().unwrap());
    }

    fn get_outline(&self, map: &Map) -> Polygon {
        // Highlight the entire thing, not just an outline
        map.get_r(self.id).get_thick_polygon()
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_r(self.id).get_thick_polygon().contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

fn draw_building_driveway(app: &dyn AppLike, bldg: &Building, batch: &mut GeomBatch) {
    if app.opts().camera_angle == CameraAngle::Abstract || !app.opts().show_building_driveways {
        return;
    }

    // Trim the driveway away from the sidewalk's center line, so that it doesn't overlap.  For
    // now, this cleanup is visual; it doesn't belong in the map_model layer.
    let orig_pl = &bldg.driveway_geom;
    let driveway = orig_pl
        .slice(
            Distance::ZERO,
            orig_pl.length() - app.map().get_l(bldg.sidewalk()).width / 2.0,
        )
        .map(|(pl, _)| pl)
        .unwrap_or_else(|_| orig_pl.clone());
    if driveway.length() > Distance::meters(0.1) {
        batch.push(
            if app.opts().color_scheme == ColorSchemeChoice::NightMode {
                Color::hex("#4B4B4B")
            } else {
                app.cs().zoomed_road_surface(
                    LaneType::Sidewalk,
                    app.map().get_parent(bldg.sidewalk()).get_rank(),
                )
            },
            driveway.make_polygons(NORMAL_LANE_THICKNESS),
        );
    }
}
