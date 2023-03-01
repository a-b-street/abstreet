use std::cell::RefCell;

use geom::{Bounds, Distance, Pt2D, Tessellation};
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
const DRAW_CURVEY_LABEL: bool = false;

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

    pub fn render<P: AsRef<Prerender>>(&self, prerender: &P, app: &dyn AppLike) -> GeomBatch {
        let prerender = prerender.as_ref();
        let r = app.map().get_r(self.id);

        if r.is_light_rail() {
            // No label or center-line
            return GeomBatch::new();
        }
        let name = r.get_name(app.opts().language.as_ref());
        let mut batch;
        if r.length() >= Distance::meters(30.0) && name != "???" {
            // Render a label, so split the center-line into two pieces
            let text_width =
                Distance::meters(Text::from(&name).rendered_width(prerender) * LABEL_SCALE_FACTOR);
            batch = render_center_line(app, r, Some(text_width));

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
                let mut center_line_color = app.cs().road_center_line(app.map());
                if r.is_private() && app.cs().private_road.is_some() {
                    center_line_color = center_line_color.lerp(app.cs().private_road.unwrap(), 0.5)
                }
                let txt = Text::from(Line(name).fg(center_line_color));
                let (pt, angle) = r.center_pts.must_dist_along(r.length() / 2.0);
                batch.append(
                    txt.render_autocropped(prerender)
                        .scale(LABEL_SCALE_FACTOR)
                        .centered_on(pt)
                        .rotate_around_batch_center(angle.reorient()),
                );
            }
        } else {
            // No label
            batch = render_center_line(app, r, None);
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

    fn get_outline(&self, map: &Map) -> Tessellation {
        // Highlight the entire thing, not just an outline
        Tessellation::from(map.get_r(self.id).get_thick_polygon())
    }

    fn get_bounds(&self, map: &Map) -> Bounds {
        map.get_r(self.id).get_thick_polygon().get_bounds()
    }

    fn contains_pt(&self, pt: Pt2D, map: &Map) -> bool {
        map.get_r(self.id).get_thick_polygon().contains_pt(pt)
    }

    fn get_zorder(&self) -> isize {
        self.zorder
    }
}

/// If `text_width` is defined, don't draw the center line in the middle of the road for this
/// amount of space
fn render_center_line(app: &dyn AppLike, r: &Road, text_width: Option<Distance>) -> GeomBatch {
    let mut center_line_color = app.cs().road_center_line(app.map());
    if r.is_private() && app.cs().private_road.is_some() {
        center_line_color = center_line_color.lerp(app.cs().private_road.unwrap(), 0.5)
    }

    let mut batch = GeomBatch::new();

    // Draw a center line every time two driving/bike/bus lanes of opposite direction are adjacent.
    let mut width = Distance::ZERO;
    for pair in r.lanes.windows(2) {
        width += pair[0].width;
        if pair[0].dir != pair[1].dir
            && pair[0].lane_type.is_for_moving_vehicles()
            && pair[1].lane_type.is_for_moving_vehicles()
        {
            let pl = r.shift_from_left_side(width).unwrap();
            if let Some(text_width) = text_width {
                // Draw dashed lines from the start of the road to where the text label begins,
                // then another set of dashes from where the text label ends to the end of the road
                let first_segment_distance = (pl.length() - text_width) / 2.0;
                let last_segment_distance = first_segment_distance + text_width;
                for slice in [
                    pl.slice(Distance::ZERO, first_segment_distance),
                    pl.slice(last_segment_distance, pl.length()),
                ] {
                    if let Ok((line, _)) = slice {
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
            } else {
                // Uninterrupted center line covering the entire road
                batch.extend(
                    center_line_color,
                    pl.dashed_lines(
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
