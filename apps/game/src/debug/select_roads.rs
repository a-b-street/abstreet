use maplit::btreeset;

use map_model::RoadID;
use widgetry::tools::PopupMsg;
use widgetry::{
    EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State, VerticalAlignment, Widget,
};

use crate::app::{App, Transition};
use crate::common::RoadSelector;

pub struct BulkSelect {
    panel: Panel,
    selector: RoadSelector,
}

impl BulkSelect {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, start: RoadID) -> Box<dyn State<App>> {
        let selector = RoadSelector::new(ctx, app, btreeset! {start});
        let panel = make_select_panel(ctx, &selector);
        Box::new(BulkSelect { panel, selector })
    }
}

fn make_select_panel(ctx: &mut EventCtx, selector: &RoadSelector) -> Panel {
    Panel::new_builder(Widget::col(vec![
        Widget::row(vec![
            Line("Select many roads").small_heading().into_widget(ctx),
            ctx.style().btn_close_widget(ctx),
        ]),
        selector.make_controls(ctx),
    ]))
    .aligned(HorizontalAlignment::Center, VerticalAlignment::Top)
    .build(ctx)
}

impl State<App> for BulkSelect {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        match self.panel.event(ctx) {
            Outcome::Clicked(x) => match x.as_ref() {
                "close" => {
                    return Transition::Pop;
                }
                x => {
                    if self.selector.event(ctx, app, Some(x)) {
                        self.panel = make_select_panel(ctx, &self.selector);
                    }
                }
            },
            _ => {
                if self.selector.event(ctx, app, None) {
                    self.panel = make_select_panel(ctx, &self.selector);
                }
            }
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        self.selector.draw(g, app, true);
    }
}
