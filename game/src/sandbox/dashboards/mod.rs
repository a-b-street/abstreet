mod commuter;
mod generic_trip_table;
mod misc;
mod parking_overhead;
mod summaries;
mod table;
mod traffic_signals;
mod trip_table;

use crate::app::App;
use crate::game::Transition;
pub use commuter::CommuterPatterns;
pub use traffic_signals::TrafficSignalDemand;
pub use trip_table::FinishedTripTable;
use widgetry::{Btn, Color, EventCtx, Key, Widget};

// Oh the dashboards melted, but we still had the radio
#[derive(Clone, Copy, PartialEq)]
pub enum DashTab {
    FinishedTripTable,
    CancelledTripTable,
    UnfinishedTripTable,
    TripSummaries,
    ParkingOverhead,
    ActiveTraffic,
    TransitRoutes,
    CommuterPatterns,
    TrafficSignals,
}

impl DashTab {
    pub fn picker(self, ctx: &EventCtx, app: &App) -> Widget {
        let mut row = Vec::new();
        for (name, tab) in vec![
            ("trip table", DashTab::FinishedTripTable),
            ("trip summaries", DashTab::TripSummaries),
            ("parking overhead", DashTab::ParkingOverhead),
            ("active traffic", DashTab::ActiveTraffic),
            ("transit routes", DashTab::TransitRoutes),
            ("commuter patterns", DashTab::CommuterPatterns),
            ("traffic signal demand", DashTab::TrafficSignals),
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
        Widget::custom_row(vec![
            // TODO Centered, but actually, we need to set the padding of each button to divide the
            // available space evenly. Fancy fill rules... hmmm.
            Widget::custom_row(row).bg(Color::WHITE).margin_vert(16),
            Btn::plaintext("X")
                .build(ctx, "close", Key::Escape)
                .align_right(),
        ])
    }

    pub fn transition(self, ctx: &mut EventCtx, app: &mut App, action: &str) -> Transition {
        match action {
            "close" => Transition::Pop,
            "trip table" => Transition::Replace(FinishedTripTable::new(ctx, app)),
            "trip summaries" => Transition::Replace(summaries::TripSummaries::new(
                ctx,
                app,
                summaries::Filter::new(),
            )),
            "parking overhead" => {
                Transition::Replace(parking_overhead::ParkingOverhead::new(ctx, app))
            }
            "active traffic" => Transition::Replace(misc::ActiveTraffic::new(ctx, app)),
            "transit routes" => Transition::Replace(misc::TransitRoutes::new(ctx, app)),
            "commuter patterns" => Transition::Replace(CommuterPatterns::new(ctx, app)),
            "traffic signal demand" => Transition::Replace(TrafficSignalDemand::new(ctx, app)),

            // TODO Misleading. These doesn't show up under the DashTab, but we're hijacking this
            // for some of the sub-tabs.
            "finished trips" => Transition::Replace(FinishedTripTable::new(ctx, app)),
            "cancelled trips" => Transition::Replace(trip_table::CancelledTripTable::new(ctx, app)),
            "unfinished trips" => {
                Transition::Replace(trip_table::UnfinishedTripTable::new(ctx, app))
            }
            _ => unreachable!(),
        }
    }
}
