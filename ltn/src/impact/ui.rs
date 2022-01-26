use abstutil::prettyprint_usize;
use map_gui::load::FileLoader;
use sim::Scenario;
use widgetry::mapspace::{ToggleZoomed, World};
use widgetry::{
    Choice, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState,
    State, Text, TextExt, VerticalAlignment, Widget,
};

use super::{Obj, Results};
use crate::{App, BrowseNeighborhoods, Transition};

// TODO Share structure or pieces with Ungap's predict mode
// ... can't we just produce data of a certain shape, and have a UI pretty tuned for that?

pub struct ShowResults {
    layer: Layer,
    draw_all_neighborhoods: Drawable,
    draw_all_filters: ToggleZoomed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Layer {
    Before,
    After,
    Relative,
}

impl ShowResults {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let map_name = app.map.get_name().clone();
        if app
            .session
            .impact
            .as_ref()
            .map(|i| i.map != map_name)
            .unwrap_or(true)
        {
            let scenario_name = Scenario::default_scenario_for_map(&map_name);
            return FileLoader::<App, Scenario>::new_state(
                ctx,
                abstio::path_scenario(&map_name, &scenario_name),
                Box::new(move |ctx, app, timer, maybe_scenario| {
                    // TODO Handle corrupt files
                    let scenario = maybe_scenario.unwrap();
                    app.session.impact = Some(Results::from_scenario(ctx, app, scenario, timer));
                    Transition::Replace(ShowResults::new_state(ctx, app))
                }),
            );
        }

        if app.session.impact.as_ref().unwrap().change_key != app.session.modal_filters.change_key {
            ctx.loading_screen("recalculate impact", |ctx, timer| {
                // Avoid a double borrow
                let mut results = app.session.impact.take().unwrap();
                results.recalculate_impact(ctx, app, timer);
                app.session.impact = Some(results);
            });
        }

        let layer = Layer::Relative;
        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Impact prediction".text_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            "This shows how many driving trips cross each road".text_widget(ctx),
            Widget::row(vec![
                "Show what?".text_widget(ctx).centered_vert(),
                Widget::dropdown(
                    ctx,
                    "layer",
                    layer,
                    vec![
                        Choice::new("before", Layer::Before),
                        Choice::new("after", Layer::After),
                        Choice::new("relative", Layer::Relative),
                    ],
                ),
            ]),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);

        let mut batch = GeomBatch::new();
        for (_, (block, color)) in app.session.partitioning.all_neighborhoods() {
            batch.push(color.alpha(0.2), block.polygon.clone());
        }
        let draw_all_neighborhoods = batch.upload(ctx);
        <dyn SimpleState<_>>::new_state(
            panel,
            Box::new(ShowResults {
                layer,
                draw_all_filters: app.session.modal_filters.draw(ctx, &app.map, None),
                draw_all_neighborhoods,
            }),
        )
    }

    // TODO Or do an EnumMap of Layer
    fn world<'a>(&self, app: &'a App) -> &'a World<Obj> {
        let results = app.session.impact.as_ref().unwrap();
        match self.layer {
            Layer::Before => &results.before_world,
            Layer::After => &results.after_world,
            Layer::Relative => &results.relative_world,
        }
    }

    fn world_mut<'a>(&self, app: &'a mut App) -> &'a mut World<Obj> {
        let results = app.session.impact.as_mut().unwrap();
        match self.layer {
            Layer::Before => &mut results.before_world,
            Layer::After => &mut results.after_world,
            Layer::Relative => &mut results.relative_world,
        }
    }
}

impl SimpleState<App> for ShowResults {
    fn on_click(&mut self, ctx: &mut EventCtx, app: &mut App, x: &str, _: &Panel) -> Transition {
        if x == "close" {
            // Don't just Pop; if we updated the results, the UI won't warn the user about a slow
            // loading
            return Transition::Replace(BrowseNeighborhoods::new_state(ctx, app));
        }
        unreachable!()
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        // Just trigger hovering
        let _ = self.world_mut(app).event(ctx);
        Transition::Keep
    }

    fn panel_changed(
        &mut self,
        _: &mut EventCtx,
        _: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition> {
        self.layer = panel.dropdown_value("layer");
        None
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.world(app).draw(g);
        g.redraw(&self.draw_all_neighborhoods);
        self.draw_all_filters.draw(g);

        // TODO Manually generate tooltips last-minute. It'd be quite worth making the World be
        // able to handle this.
        let results = app.session.impact.as_ref().unwrap();
        if let Some(id) = self.world(app).get_hovering() {
            let count = match id {
                Obj::Road(r) => match self.layer {
                    Layer::Before => results.before_road_counts.get(r),
                    Layer::After => results.after_road_counts.get(r),
                    Layer::Relative => {
                        g.draw_mouse_tooltip(results.relative_road_tooltip(r));
                        return;
                    }
                },
                Obj::Intersection(i) => match self.layer {
                    Layer::Before => results.before_intersection_counts.get(i),
                    Layer::After => results.after_intersection_counts.get(i),
                    Layer::Relative => {
                        return;
                    }
                },
            };
            g.draw_mouse_tooltip(Text::from(Line(prettyprint_usize(count))));
        }
    }
}
