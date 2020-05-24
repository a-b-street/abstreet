mod misc;
mod parking_overhead;
mod summaries;
mod trip_table;

use crate::app::App;
use crate::game::Transition;
use ezgui::{hotkey, Btn, Color, EventCtx, Key, Widget};
pub use trip_table::TripTable;

// Oh the dashboards melted, but we still had the radio
#[derive(PartialEq)]
pub enum DashTab {
    TripTable,
    TripSummaries,
    ParkingOverhead,
    ActiveTraffic,
    BusRoutes,
}

impl DashTab {
    pub fn picker(self, ctx: &EventCtx, app: &App) -> Widget {
        let mut row = Vec::new();
        for (name, tab) in vec![
            ("trip table", DashTab::TripTable),
            ("trip summaries", DashTab::TripSummaries),
            ("parking overhead", DashTab::ParkingOverhead),
            ("active traffic", DashTab::ActiveTraffic),
            ("bus routes", DashTab::BusRoutes),
        ] {
            if tab == DashTab::TripSummaries && app.has_prebaked().is_none() {
                continue;
            }
            if self == tab {
                row.push(Btn::text_bg2(name).inactive(ctx));
            } else {
                row.push(Btn::text_bg2(name).build_def(ctx, None));
            }
        }
        Widget::row(vec![
            // TODO Centered, but actually, we need to set the padding of each button to divide the
            // available space evenly. Fancy fill rules... hmmm.
            Widget::row(row).bg(Color::WHITE).margin_vert(16),
            Btn::plaintext("X")
                .build(ctx, "close", hotkey(Key::Escape))
                .align_right(),
        ])
    }

    pub fn transition(self, ctx: &mut EventCtx, app: &App, action: &str) -> Transition {
        match action {
            "close" => Transition::Pop,
            "trip table" => Transition::Replace(TripTable::new(ctx, app)),
            "trip summaries" => Transition::Replace(summaries::TripSummaries::new(
                ctx,
                app,
                summaries::Filter::new(),
            )),
            "parking overhead" => {
                Transition::Replace(parking_overhead::ParkingOverhead::new(ctx, app))
            }
            "active traffic" => Transition::Replace(misc::ActiveTraffic::new(ctx, app)),
            "bus routes" => Transition::Replace(misc::BusRoutes::new(ctx, app)),
            _ => unreachable!(),
        }
    }
}
