pub use commuter::CommuterPatterns;
pub use traffic_signals::TrafficSignalDemand;
pub use trip_table::TripTable;

use widgetry::{Choice, EventCtx, Image, Line, Panel, TextExt, Widget};

use crate::app::App;
use crate::app::Transition;

mod commuter;
mod generic_trip_table;
mod misc;
mod parking_overhead;
mod selector;
mod summaries;
mod traffic_signals;
mod trip_table;

// Oh the dashboards melted, but we still had the radio
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DashTab {
    TripTable,
    TripSummaries,
    ParkingOverhead,
    ActiveTraffic,
    TransitRoutes,
    CommuterPatterns,
    TrafficSignals,
}

impl DashTab {
    pub fn picker(self, ctx: &EventCtx, app: &App) -> Widget {
        let mut choices = vec![
            Choice::new("Trip Table", DashTab::TripTable),
            Choice::new("Trip Summaries", DashTab::TripSummaries),
            Choice::new("Parking Overhead", DashTab::ParkingOverhead),
            Choice::new("Active Traffic", DashTab::ActiveTraffic),
            Choice::new("Transit Routes", DashTab::TransitRoutes),
            Choice::new("Commuter Patterns", DashTab::CommuterPatterns),
            Choice::new("Traffic Signal Demand", DashTab::TrafficSignals),
        ];
        if app.has_prebaked().is_none() {
            choices.remove(1);
        }
        Widget::row(vec![
            Image::from_path("system/assets/meters/trip_histogram.svg").into_widget(ctx),
            Line("Data").big_heading_plain().into_widget(ctx),
            Widget::dropdown(ctx, "tab", self, choices),
            format!("By {}", app.primary.sim.time().ampm_tostring())
                .text_widget(ctx)
                .centered_vert(),
            ctx.style().btn_close_widget(ctx),
        ])
    }

    pub fn transition(
        self,
        ctx: &mut EventCtx,
        app: &mut App,
        panel: &Panel,
    ) -> Option<Transition> {
        let tab = panel.dropdown_value("tab");
        if tab == self {
            return None;
        }

        Some(Transition::Replace(match tab {
            DashTab::TripTable => Box::new(TripTable::new(ctx, app)),
            DashTab::TripSummaries => {
                summaries::TripSummaries::new_state(ctx, app, summaries::Filter::new())
            }
            DashTab::ParkingOverhead => parking_overhead::ParkingOverhead::new_state(ctx, app),
            DashTab::ActiveTraffic => misc::ActiveTraffic::new_state(ctx, app),
            DashTab::TransitRoutes => misc::TransitRoutes::new_state(ctx, app),
            DashTab::CommuterPatterns => CommuterPatterns::new_state(ctx, app),
            DashTab::TrafficSignals => TrafficSignalDemand::new_state(ctx, app),
        }))
    }
}
