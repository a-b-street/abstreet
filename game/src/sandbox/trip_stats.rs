use crate::common::ColorLegend;
use crate::ui::UI;
use abstutil::Counter;
use ezgui::{
    Color, Drawable, EventCtx, GeomBatch, GfxCtx, Line, MultiText, ScreenPt, ScreenRectangle, Text,
};
use geom::{Distance, Duration, PolyLine, Polygon, Pt2D};
use sim::TripMode;
use std::collections::BTreeMap;

// TODO Show active trips too
pub struct ShowTripStats {
    draw: Drawable,
    legend: ColorLegend,
    labels: MultiText,
    rect: ScreenRectangle,
}

impl ShowTripStats {
    pub fn new(ui: &UI, ctx: &mut EventCtx) -> Option<ShowTripStats> {
        if ui.primary.sim.get_analytics().finished_trips.is_empty() {
            return None;
        }

        let lines: Vec<(&str, Color, Option<TripMode>)> = vec![
            (
                "walking",
                ui.cs.get("unzoomed pedestrian"),
                Some(TripMode::Walk),
            ),
            ("biking", ui.cs.get("unzoomed bike"), Some(TripMode::Bike)),
            (
                "transit",
                ui.cs.get("unzoomed bus"),
                Some(TripMode::Transit),
            ),
            ("driving", ui.cs.get("unzoomed car"), Some(TripMode::Drive)),
            ("aborted", Color::PURPLE.alpha(0.5), None),
        ];

        // What times do we use for interpolation?
        let num_x_pts = 100;
        let mut times = Vec::new();
        for i in 0..num_x_pts {
            let percent_x = (i as f64) / ((num_x_pts - 1) as f64);
            let t = ui.primary.sim.time() * percent_x;
            times.push(t);
        }

        // Gather the data
        let mut counts = Counter::new();
        let mut pts_per_mode: BTreeMap<Option<TripMode>, Vec<(Duration, usize)>> =
            lines.iter().map(|(_, _, m)| (*m, Vec::new())).collect();
        for (t, m) in &ui.primary.sim.get_analytics().finished_trips {
            counts.inc(*m);
            if *t > times[0] {
                times.remove(0);
                for (_, _, mode) in &lines {
                    pts_per_mode
                        .get_mut(mode)
                        .unwrap()
                        .push((*t, counts.get(*mode)));
                }
            }
        }

        Some(plot(
            "finished trips",
            lines
                .into_iter()
                .map(|(name, color, m)| (name, color, pts_per_mode.remove(&m).unwrap()))
                .collect(),
            ctx,
        ))
    }

    pub fn draw(&self, g: &mut GfxCtx) {
        self.legend.draw(g);

        g.fork_screenspace();
        g.redraw(&self.draw);
        g.unfork();
        self.labels.draw(g);

        g.canvas.mark_covered_area(self.rect.clone());
    }
}

fn plot(
    title: &str,
    series: Vec<(&str, Color, Vec<(Duration, usize)>)>,
    ctx: &EventCtx,
) -> ShowTripStats {
    let mut batch = GeomBatch::new();
    let mut labels = MultiText::new();

    let x1 = 0.2 * ctx.canvas.window_width;
    let x2 = 0.8 * ctx.canvas.window_width;
    let y1 = 0.2 * ctx.canvas.window_height;
    let y2 = 0.8 * ctx.canvas.window_height;
    batch.push(
        Color::grey(0.8),
        Polygon::rectangle_topleft(
            Pt2D::new(x1, y1),
            Distance::meters(x2 - x1),
            Distance::meters(y2 - y1),
        ),
    );

    // Assume every series has data at exactly the same durations
    let num_x_labels = 5;
    let max_x = series[0].2.last().unwrap().0;
    for i in 0..num_x_labels {
        let percent_x = (i as f64) / ((num_x_labels - 1) as f64);
        let t = series[0].2[(percent_x * ((series[0].2.len() - 1) as f64)) as usize].0;
        labels.add(
            Text::from(Line(t.to_string())),
            ScreenPt::new(x1 + percent_x * (x2 - x1), y2),
        );
    }

    // Don't assume the data is cumulative
    let max_y = series
        .iter()
        .map(|(_, _, pts)| pts.iter().map(|(_, cnt)| *cnt).max().unwrap())
        .max()
        .unwrap();
    let num_y_labels = 5;
    for i in 0..num_y_labels {
        let percent_y = (i as f64) / ((num_y_labels - 1) as f64);
        labels.add(
            Text::from(Line(abstutil::prettyprint_usize(
                (percent_y * (max_y as f64)) as usize,
            ))),
            ScreenPt::new(x1, y2 - percent_y * (y2 - y1)),
        );
    }

    let legend = ColorLegend::new(
        Text::prompt(title),
        series
            .iter()
            .map(|(name, color, _)| (*name, *color))
            .collect(),
    );

    for (_, color, raw_pts) in series {
        let mut pts = Vec::new();
        if max_y == 0 {
            pts.push(Pt2D::new(x1, y2));
            pts.push(Pt2D::new(x2, y2));
        } else {
            for (t, y) in raw_pts {
                let percent_x = t / max_x;
                let percent_y = (y as f64) / (max_y as f64);
                pts.push(Pt2D::new(
                    x1 + (x2 - x1) * percent_x,
                    // Y inversion! :D
                    y2 - (y2 - y1) * percent_y,
                ));
            }
        }
        batch.push(
            color,
            PolyLine::new(pts).make_polygons(Distance::meters(5.0)),
        );
    }

    ShowTripStats {
        draw: ctx.prerender.upload(batch),
        labels,
        legend,
        rect: ScreenRectangle { x1, y1, x2, y2 },
    }
}
