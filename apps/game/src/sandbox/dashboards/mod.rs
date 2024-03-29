pub use commuter::CommuterPatterns;
pub use traffic_signals::TrafficSignalDemand;

use widgetry::{Choice, EventCtx, Image, Line, Panel, State, TextExt, Widget};

use crate::app::App;
use crate::app::Transition;

mod commuter;
mod generic_trip_table;
mod misc;
mod mode_shift;
mod parking_overhead;
mod risks;
mod selector;
mod traffic_signals;
mod travel_times;
mod trip_problems;
mod trip_table;

// Oh the dashboards melted, but we still had the radio
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum DashTab {
    TripTable,
    TravelTimes,
    RiskSummaries,
    ParkingOverhead,
    ActiveTraffic,
    TransitRoutes,
    CommuterPatterns,
    TrafficSignals,
    ModeShift,
}

impl DashTab {
    pub fn picker(self, ctx: &EventCtx, app: &App) -> Widget {
        let mut choices = vec![
            Choice::new("Trip Table", DashTab::TripTable),
            Choice::new("Travel Times", DashTab::TravelTimes),
            Choice::new("Risk Exposure", DashTab::RiskSummaries),
            Choice::new("Parking Overhead", DashTab::ParkingOverhead),
            Choice::new("Active Traffic", DashTab::ActiveTraffic),
            Choice::new("Transit Routes", DashTab::TransitRoutes),
            Choice::new("Commuter Patterns", DashTab::CommuterPatterns),
            Choice::new("Traffic Signal Demand", DashTab::TrafficSignals),
            Choice::new("Mode shift (experimental)", DashTab::ModeShift),
        ];
        if app.has_prebaked().is_none() {
            choices.remove(1);
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

    pub fn launch(self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        match self {
            DashTab::TripTable => Box::new(trip_table::TripTable::new(ctx, app)),
            DashTab::TravelTimes => {
                travel_times::TravelTimes::new_state(ctx, app, travel_times::Filter::new())
            }
            DashTab::RiskSummaries => risks::RiskSummaries::new_state(ctx, app, false),
            DashTab::ParkingOverhead => parking_overhead::ParkingOverhead::new_state(ctx, app),
            DashTab::ActiveTraffic => misc::ActiveTraffic::new_state(ctx, app),
            DashTab::TransitRoutes => misc::TransitRoutes::new_state(ctx, app),
            DashTab::CommuterPatterns => CommuterPatterns::new_state(ctx, app),
            DashTab::TrafficSignals => TrafficSignalDemand::new_state(ctx, app),
            DashTab::ModeShift => mode_shift::ModeShift::new_state(ctx, app),
        }
    }

    pub fn tab_changed(self, app: &mut App, panel: &Panel) -> Option<DashTab> {
        let tab: DashTab = panel.dropdown_value("tab");
        if tab == self {
            return None;
        }
        app.session.dash_tab = tab;
        Some(tab)
    }

    pub fn transition(
        self,
        ctx: &mut EventCtx,
        app: &mut App,
        panel: &Panel,
    ) -> Option<Transition> {
        let tab = self.tab_changed(app, panel)?;
        Some(Transition::Replace(tab.launch(ctx, app)))
    }
}
