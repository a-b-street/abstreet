use crate::common::ColorLegend;
use crate::game::{State, Transition};
use crate::ui::UI;
use ezgui::{
    hotkey, Color, Drawable, EventCtx, GeomBatch, GfxCtx, Key, Line, ModalMenu, MultiText,
    ScreenPt, Text,
};
use geom::{Distance, Duration, PolyLine, Polygon, Pt2D};
use sim::TripMode;

pub struct TripStats {
    should_record: bool,
    samples: Vec<StateAtTime>,
}

struct StateAtTime {
    time: Duration,
    // These're all cumulative
    finished_walk_trips: usize,
    finished_bike_trips: usize,
    finished_transit_trips: usize,
    finished_drive_trips: usize,
    aborted_trips: usize,
}

impl TripStats {
    pub fn new(should_record: bool) -> TripStats {
        TripStats {
            should_record,
            samples: Vec::new(),
        }
    }

    pub fn record(&mut self, ui: &UI) {
        if !self.should_record {
            return;
        }

        if let Some(ref state) = self.samples.last() {
            // Already have this
            if ui.primary.sim.time() == state.time {
                return;
            }
            // We just loaded a new savestate or reset or something. Clear out our memory.
            if ui.primary.sim.time() < state.time {
                self.samples.clear();
            }
        }

        let t = ui.primary.sim.get_finished_trips();
        let mut state = StateAtTime {
            time: ui.primary.sim.time(),
            finished_walk_trips: 0,
            finished_bike_trips: 0,
            finished_transit_trips: 0,
            finished_drive_trips: 0,
            aborted_trips: t.aborted_trips,
        };
        for (_, m, _) in t.finished_trips {
            match m {
                TripMode::Walk => {
                    state.finished_walk_trips += 1;
                }
                TripMode::Bike => {
                    state.finished_bike_trips += 1;
                }
                TripMode::Transit => {
                    state.finished_transit_trips += 1;
                }
                TripMode::Drive => {
                    state.finished_drive_trips += 1;
                }
            }
        }
        self.samples.push(state);
    }
}

pub struct ShowStats {
    menu: ModalMenu,
    draw: Drawable,
    legend: ColorLegend,
    labels: MultiText,
}

impl State for ShowStats {
    fn event(&mut self, ctx: &mut EventCtx, _: &mut UI) -> Transition {
        self.menu.handle_event(ctx, None);
        if self.menu.action("quit") {
            return Transition::Pop;
        }
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, _: &UI) {
        self.menu.draw(g);
        self.legend.draw(g);

        g.fork_screenspace();
        g.redraw(&self.draw);
        g.unfork();
        self.labels.draw(g);
    }
}

impl ShowStats {
    pub fn new(stats: &TripStats, ui: &UI, ctx: &mut EventCtx) -> Option<ShowStats> {
        if stats.samples.is_empty() {
            return None;
        }

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

        let lines: Vec<(&str, Color, Box<dyn Fn(&StateAtTime) -> usize>)> = vec![
            (
                "walking",
                ui.cs.get("unzoomed pedestrian"),
                Box::new(|s| s.finished_walk_trips),
            ),
            (
                "biking",
                ui.cs.get("unzoomed bike"),
                Box::new(|s| s.finished_bike_trips),
            ),
            (
                "transit",
                ui.cs.get("unzoomed bus"),
                Box::new(|s| s.finished_transit_trips),
            ),
            (
                "driving",
                ui.cs.get("unzoomed car"),
                Box::new(|s| s.finished_drive_trips),
            ),
            (
                "aborted",
                Color::PURPLE.alpha(0.5),
                Box::new(|s| s.aborted_trips),
            ),
        ];
        let legend = ColorLegend::new(
            "finished trips",
            lines
                .iter()
                .map(|(name, color, _)| (*name, *color))
                .collect(),
        );
        let max_y = stats
            .samples
            .iter()
            .map(|s| lines.iter().map(|(_, _, getter)| getter(s)).max().unwrap())
            .max()
            .unwrap();
        // Y-axis labels
        for i in 0..=5 {
            let percent = (i as f64) / 5.0;
            labels.add(
                Text::from(Line(((percent * (max_y as f64)) as usize).to_string())),
                ScreenPt::new(x1, y2 - percent * (y2 - y1)),
            );
        }
        // X-axis labels (currently nonlinear!)
        {
            let num_pts = stats.samples.len().min(5);
            for i in 0..num_pts {
                let percent_x = (i as f64) / ((num_pts - 1) as f64);
                let t =
                    stats.samples[(percent_x * ((stats.samples.len() - 1) as f64)) as usize].time;
                labels.add(
                    Text::from(Line(t.to_string())),
                    ScreenPt::new(x1 + percent_x * (x2 - x1), y2),
                );
            }
        }

        for (_, color, getter) in lines {
            let mut pts = Vec::new();
            if max_y == 0 {
                pts.push(Pt2D::new(x1, y2));
                pts.push(Pt2D::new(x2, y2));
            } else {
                let num_pts = stats.samples.len().min(10);
                for i in 0..num_pts {
                    let percent_x = (i as f64) / ((num_pts - 1) as f64);
                    let value = getter(
                        &stats.samples[(percent_x * ((stats.samples.len() - 1) as f64)) as usize],
                    );
                    let percent_y = (value as f64) / (max_y as f64);
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

        let mut txt = Text::prompt("Trip Stats");
        txt.add(Line(format!(
            "{} samples",
            abstutil::prettyprint_usize(stats.samples.len())
        )));
        Some(ShowStats {
            menu: ModalMenu::new("Trip Stats", vec![vec![(hotkey(Key::Escape), "quit")]], ctx)
                .set_prompt(ctx, txt),
            draw: ctx.prerender.upload(batch),
            labels,
            legend,
        })
    }
}
