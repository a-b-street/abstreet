use std::collections::BTreeSet;

use maplit::btreeset;

use map_gui::tools::PopupMsg;
use map_model::RoadID;
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
        Line("Select many roads").small_heading().into_widget(ctx),
        selector.make_controls(ctx),
        Widget::row(vec![
            ctx.style()
                .btn_outline
                .text(format!(
                    "Export {} roads to shared-row",
                    selector.roads.len()
                ))
                .build_widget(ctx, "export roads to shared-row"),
            ctx.style()
                .btn_outline
                .text("export one road to Streetmix")
                .build_def(ctx),
            ctx.style()
                .btn_outline
                .text("export list of roads")
                .build_def(ctx),
            ctx.style().btn_close_widget(ctx),
        ])
        .evenly_spaced(),
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
                "export roads to shared-row" => {
                    let path = crate::debug::shared_row::export(
                        self.selector.roads.iter().cloned().collect(),
                        self.selector.intersections.iter().cloned().collect(),
                        &app.primary.map,
                    );
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "Roads exported",
                        vec![format!("Roads exported to shared-row format at {}", path)],
                    ));
                }
                "export one road to Streetmix" => {
                    let path = crate::debug::streetmix::export(
                        *self.selector.roads.iter().next().unwrap(),
                        &app.primary.map,
                    );
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "One road exported",
                        vec![format!(
                            "One arbitrary road from your selection exported to Streetmix format \
                             at {}",
                            path
                        )],
                    ));
                }
                "export list of roads" => {
                    let mut osm_ids: BTreeSet<map_model::osm::WayID> = BTreeSet::new();
                    for r in &self.selector.roads {
                        osm_ids.insert(app.primary.map.get_r(*r).orig_id.osm_way_id);
                    }
                    abstio::write_json("osm_ways.json".to_string(), &osm_ids);
                    return Transition::Push(PopupMsg::new_state(
                        ctx,
                        "List of roads exported",
                        vec!["Wrote osm_ways.json"],
                    ));
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
