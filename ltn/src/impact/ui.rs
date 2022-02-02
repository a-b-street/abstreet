use std::collections::BTreeSet;

use map_gui::load::FileLoader;
use map_gui::tools::{checkbox_per_mode, PopupMsg};
use synthpop::{Scenario, TripMode};
use widgetry::{
    Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState, Slider,
    State, Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use super::{end_of_day, Filters, Impact};
use crate::{App, BrowseNeighborhoods, Toggle3Zoomed, Transition};

// TODO Share structure or pieces with Ungap's predict mode
// ... can't we just produce data of a certain shape, and have a UI pretty tuned for that?

pub struct ShowResults {
    draw_all_neighborhoods: Drawable,
    draw_all_filters: Toggle3Zoomed,
}

impl ShowResults {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        let map_name = app.map.get_name().clone();
        if app.session.impact.map != map_name {
            let scenario_name = Scenario::default_scenario_for_map(&map_name);
            return FileLoader::<App, Scenario>::new_state(
                ctx,
                abstio::path_scenario(&map_name, &scenario_name),
                Box::new(move |ctx, app, timer, maybe_scenario| {
                    // TODO Handle corrupt files
                    let scenario = maybe_scenario.unwrap();
                    app.session.impact = Impact::from_scenario(ctx, app, scenario, timer);
                    Transition::Replace(ShowResults::new_state(ctx, app))
                }),
            );
        }

        if app.session.impact.change_key != app.session.modal_filters.change_key {
            ctx.loading_screen("recalculate impact", |ctx, timer| {
                // Avoid a double borrow
                let mut impact = std::mem::replace(&mut app.session.impact, Impact::empty(ctx));
                impact.map_edits_changed(ctx, app, timer);
                app.session.impact = impact;
            });
        }

        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Impact prediction".text_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Text::from(Line("This tool starts with a travel demand model, calculates the route every trip takes before and after changes, and displays volumes along roads and intersections")).wrap_to_pct(ctx, 20).into_widget(ctx),
            // TODO Dropdown for the scenario, and explain its source/limitations
            app.session.impact.filters.to_panel(ctx, app),
            app.session.impact.compare_counts.get_panel_widget(ctx).named("compare counts"),
            ctx.style().btn_plain.text("Save before/after counts to files").build_def(ctx),
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
                draw_all_filters: app.session.modal_filters.draw(ctx, &app.map, None),
                draw_all_neighborhoods,
            }),
        )
    }
}

impl SimpleState<App> for ShowResults {
    fn on_click(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        x: &str,
        panel: &mut Panel,
    ) -> Transition {
        match x {
            "close" => {
                // Don't just Pop; if we updated the results, the UI won't warn the user about a slow
                // loading
                Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
            }
            "Save before/after counts to files" => {
                let path1 = "counts_a.json";
                let path2 = "counts_b.json";
                abstio::write_json(
                    path1.to_string(),
                    &app.session.impact.compare_counts.counts_a,
                );
                abstio::write_json(
                    path2.to_string(),
                    &app.session.impact.compare_counts.counts_b,
                );
                Transition::Push(PopupMsg::new_state(
                    ctx,
                    "Saved",
                    vec![format!("Saved {} and {}", path1, path2)],
                ))
            }
            x => {
                // Avoid a double borrow
                let mut impact = std::mem::replace(&mut app.session.impact, Impact::empty(ctx));
                let widget = impact
                    .compare_counts
                    .on_click(ctx, app, x)
                    .expect("button click didn't belong to CompareCounts");
                app.session.impact = impact;
                panel.replace(ctx, "compare counts", widget);
                Transition::Keep
            }
        }
    }

    fn other_event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        app.session.impact.compare_counts.other_event(ctx);
        Transition::Keep
    }

    // TODO The sliders should only trigger updates when the user lets go; way too slow otherwise
    fn panel_changed(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition> {
        let filters = Filters::from_panel(panel);
        if filters == app.session.impact.filters {
            return None;
        }

        // Avoid a double borrow
        let mut impact = std::mem::replace(&mut app.session.impact, Impact::empty(ctx));
        impact.filters = Filters::from_panel(panel);
        ctx.loading_screen("update filters", |ctx, timer| {
            impact.trips_changed(ctx, app, timer);
        });
        app.session.impact = impact;

        None
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw_all_neighborhoods);
        app.session.impact.compare_counts.draw(g);
        self.draw_all_filters.draw(g);
    }
}

impl Filters {
    fn from_panel(panel: &mut Panel) -> Filters {
        let (p1, p2) = (
            panel.slider("depart from").get_percent(),
            panel.slider("depart until").get_percent(),
        );
        let departure_time = (end_of_day().percent_of(p1), end_of_day().percent_of(p2));
        let modes = TripMode::all()
            .into_iter()
            .filter(|m| panel.is_checked(m.ongoing_verb()))
            .collect::<BTreeSet<_>>();
        Filters {
            modes,
            include_borders: panel.is_checked("include borders"),
            departure_time,
        }
    }

    fn to_panel(&self, ctx: &mut EventCtx, app: &App) -> Widget {
        Widget::col(vec![
            "Filter trips".text_widget(ctx),
            Toggle::switch(ctx, "include borders", None, self.include_borders),
            Widget::row(vec![
                "Departing from:".text_widget(ctx).margin_right(20),
                Slider::area(
                    ctx,
                    0.15 * ctx.canvas.window_width,
                    self.departure_time.0.to_percent(end_of_day()),
                    "depart from",
                ),
            ]),
            Widget::row(vec![
                "Departing until:".text_widget(ctx).margin_right(20),
                Slider::area(
                    ctx,
                    0.15 * ctx.canvas.window_width,
                    self.departure_time.1.to_percent(end_of_day()),
                    "depart until",
                ),
            ]),
            checkbox_per_mode(ctx, app, &self.modes),
            // TODO Filter by trip purpose
        ])
        .section(ctx)
    }
}
