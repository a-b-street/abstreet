use abstutil::Timer;
use map_gui::tools::compare_counts::{CompareCounts, Layer};
use synthpop::TrafficCounts;
use widgetry::tools::PopupMsg;
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState, State, VerticalAlignment,
    Widget,
};

use crate::app::{App, Transition};

pub struct GenericCompareCounts {
    compare: CompareCounts,
}

impl GenericCompareCounts {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &mut App,
        path1: String,
        path2: String,
    ) -> Box<dyn State<App>> {
        let mut timer = Timer::throwaway();
        // TODO File loaders
        let counts_a = match abstio::maybe_read_json::<TrafficCounts>(path1, &mut timer) {
            Ok(c) => c,
            Err(err) => {
                return PopupMsg::new_state(ctx, "Error", vec![err.to_string()]);
            }
        };
        let counts_b = match abstio::maybe_read_json::<TrafficCounts>(path2, &mut timer) {
            Ok(c) => c,
            Err(err) => {
                return PopupMsg::new_state(ctx, "Error", vec![err.to_string()]);
            }
        };
        let clickable_roads = false;
        let mut compare =
            CompareCounts::new(ctx, app, counts_a, counts_b, Layer::A, clickable_roads);
        compare.autoselect_layer();

        let panel = Panel::new_builder(Widget::col(vec![
            Line("Traffic count comparator")
                .small_heading()
                .into_widget(ctx),
            compare.get_panel_widget(ctx).named("compare counts"),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        <dyn SimpleState<_>>::new_state(panel, Box::new(GenericCompareCounts { compare }))
    }
}

impl SimpleState<App> for GenericCompareCounts {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        panel: &mut Panel,
    ) -> Transition {
        let widget = self
            .compare
            .on_click(ctx, app, x)
            .expect("button click didn't belong to CompareCounts");
        panel.replace(ctx, "compare counts", widget);
        Transition::Keep
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        self.compare.other_event(ctx);
        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.compare.draw(g, app);
    }
}
