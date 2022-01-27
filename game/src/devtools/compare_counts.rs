use abstutil::Timer;
use map_gui::tools::compare_counts::{CompareCounts, Counts, Layer};
use map_gui::tools::PopupMsg;
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Panel, SimpleState, State, VerticalAlignment, Widget,
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
        let counts_a = match abstio::maybe_read_json::<Counts>(path1, &mut timer) {
            Ok(c) => c,
            Err(err) => {
                return PopupMsg::new_state(ctx, "Error", vec![err.to_string()]);
            }
        };
        let counts_b = match abstio::maybe_read_json::<Counts>(path2, &mut timer) {
            Ok(c) => c,
            Err(err) => {
                return PopupMsg::new_state(ctx, "Error", vec![err.to_string()]);
            }
        };
        let mut compare = CompareCounts::new(ctx, app, counts_a, counts_b, Layer::A);
        compare.autoselect_layer();

        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Traffic count comparator"),
            compare.get_panel_widget(ctx),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        <dyn SimpleState<_>>::new_state(panel, Box::new(GenericCompareCounts { compare }))
    }
}

impl SimpleState<App> for GenericCompareCounts {
    fn on_click(&mut self, _: &mut EventCtx, _: &mut App, _: &str, _: &Panel) -> Transition {
        unreachable!()
    }

    fn other_event(&mut self, ctx: &mut EventCtx, _: &mut App) -> Transition {
        self.compare.other_event(ctx);
        Transition::Keep
    }

    fn panel_changed(
        &mut self,
        _: &mut EventCtx,
        _: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition> {
        assert!(self.compare.panel_changed(panel));
        None
    }

    fn draw(&self, g: &mut GfxCtx, _: &App) {
        self.compare.draw(g);
    }
}
