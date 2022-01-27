use std::collections::BTreeSet;

use abstutil::prettyprint_usize;
use map_gui::load::FileLoader;
use map_gui::tools::checkbox_per_mode;
use sim::{Scenario, TripMode};
use widgetry::mapspace::{ToggleZoomed, World};
use widgetry::{
    Choice, Drawable, EventCtx, GeomBatch, GfxCtx, HorizontalAlignment, Line, Panel, SimpleState,
    Slider, State, Text, TextExt, Toggle, VerticalAlignment, Widget,
};

use super::{end_of_day, Filters, Impact, Obj};
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
                let mut impact = std::mem::take(&mut app.session.impact);
                impact.recalculate_impact(ctx, app, timer);
                app.session.impact = impact;
            });
        }

        // Start with the relative layer if anything has changed
        let layer = {
            if app.session.impact.before_road_counts == app.session.impact.after_road_counts {
                Layer::Before
            } else {
                Layer::Relative
            }
        };
        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Impact prediction".text_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            Text::from(Line("This tool starts with a travel demand model, calculates the route every trip takes before and after changes, and displays volumes along roads and intersections")).wrap_to_pct(ctx, 20).into_widget(ctx),
            // TODO Dropdown for the scenario, and explain its source/limitations
            app.session.impact.filters.to_panel(ctx, app),
            Widget::row(vec![
                "Show counts:".text_widget(ctx).centered_vert().margin_right(20),
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
        let impact = &app.session.impact;
        match self.layer {
            Layer::Before => &impact.before_world,
            Layer::After => &impact.after_world,
            Layer::Relative => &impact.relative_world,
        }
    }

    fn world_mut<'a>(&self, app: &'a mut App) -> &'a mut World<Obj> {
        let impact = &mut app.session.impact;
        match self.layer {
            Layer::Before => &mut impact.before_world,
            Layer::After => &mut impact.after_world,
            Layer::Relative => &mut impact.relative_world,
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

    // TODO The sliders should only trigger updates when the user lets go; way too slow otherwise
    fn panel_changed(
        &mut self,
        ctx: &mut EventCtx,
        app: &mut App,
        panel: &mut Panel,
    ) -> Option<Transition> {
        let layer = panel.dropdown_value("layer");
        if layer != self.layer {
            self.layer = layer;
            return None;
        }

        let filters = Filters::from_panel(panel);
        if filters == app.session.impact.filters {
            return None;
        }

        // Avoid a double borrow
        let mut impact = std::mem::take(&mut app.session.impact);
        impact.filters = Filters::from_panel(panel);
        ctx.loading_screen("update filters", |ctx, timer| {
            impact.recalculate_filters(ctx, app, timer);
            impact.recalculate_impact(ctx, app, timer);
        });
        app.session.impact = impact;

        None
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.world(app).draw(g);
        g.redraw(&self.draw_all_neighborhoods);
        self.draw_all_filters.draw(g);

        // TODO Manually generate tooltips last-minute. It'd be quite worth making the World be
        // able to handle this.
        let impact = &app.session.impact;
        if let Some(id) = self.world(app).get_hovering() {
            let count = match id {
                Obj::Road(r) => match self.layer {
                    Layer::Before => impact.before_road_counts.get(r),
                    Layer::After => impact.after_road_counts.get(r),
                    Layer::Relative => {
                        g.draw_mouse_tooltip(impact.relative_road_tooltip(r));
                        return;
                    }
                },
                Obj::Intersection(i) => match self.layer {
                    Layer::Before => impact.before_intersection_counts.get(i),
                    Layer::After => impact.after_intersection_counts.get(i),
                    Layer::Relative => {
                        return;
                    }
                },
            };
            g.draw_mouse_tooltip(Text::from(Line(prettyprint_usize(count))));
        }
    }
}

impl Filters {
    fn from_panel(panel: &Panel) -> Filters {
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
