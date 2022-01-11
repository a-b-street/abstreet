use abstutil::Timer;
use geom::Distance;
use map_gui::tools::{CityPicker, DrawRoadLabels, Navigator, URLManager};
use widgetry::mapspace::{ToggleZoomed, World, WorldOutcome};
use widgetry::{
    lctrl, Color, EventCtx, GfxCtx, HorizontalAlignment, Key, Outcome, Panel, State, TextExt,
    VerticalAlignment, Widget,
};

use super::Neighborhood;
use crate::app::{App, Transition};
use crate::debug::DebugMode;
use crate::ltn::partition::{NeighborhoodID, Partitioning};

pub struct BrowseNeighborhoods {
    panel: Panel,
    world: World<NeighborhoodID>,
    labels: DrawRoadLabels,
    draw_all_filters: ToggleZoomed,
}

impl BrowseNeighborhoods {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        URLManager::update_url_map_name(app);

        let world = ctx.loading_screen("calculate neighborhoods", |ctx, timer| {
            detect_neighborhoods(ctx, app, timer)
        });
        let draw_all_filters = app.session.modal_filters.draw(ctx, &app.primary.map, None);

        let panel = Panel::new_builder(Widget::col(vec![
            map_gui::tools::app_header(ctx, app, "Low traffic neighborhoods"),
            Widget::row(vec![
                "Click a neighborhood".text_widget(ctx).centered_vert(),
                ctx.style()
                    .btn_plain
                    .icon("system/assets/tools/search.svg")
                    .hotkey(Key::K)
                    .build_widget(ctx, "search")
                    .align_right(),
            ]),
        ]))
        .aligned(HorizontalAlignment::Left, VerticalAlignment::Top)
        .build(ctx);
        Box::new(BrowseNeighborhoods {
            panel,
            world,
            labels: DrawRoadLabels::only_major_roads(),
            draw_all_filters,
        })
    }
}

impl State<App> for BrowseNeighborhoods {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Outcome::Clicked(x) = self.panel.event(ctx) {
            match x.as_ref() {
                "Home" => {
                    return Transition::Pop;
                }
                "change map" => {
                    return Transition::Push(CityPicker::new_state(
                        ctx,
                        app,
                        Box::new(|ctx, app| {
                            // TODO If we leave the LTN tool and change maps elsewhere, this won't
                            // work! Do we have per-map session state?
                            app.session.partitioning = Partitioning::empty();
                            Transition::Replace(BrowseNeighborhoods::new_state(ctx, app))
                        }),
                    ));
                }
                "search" => {
                    return Transition::Push(Navigator::new_state(ctx, app));
                }
                _ => unreachable!(),
            }
        }

        if let WorldOutcome::ClickedObject(id) = self.world.event(ctx) {
            return Transition::Push(super::connectivity::Viewer::new_state(
                ctx,
                app,
                Neighborhood::new(
                    ctx,
                    app,
                    app.session.partitioning.neighborhoods[&id]
                        .0
                        .perimeter
                        .clone(),
                ),
            ));
        }

        if ctx.input.pressed(lctrl(Key::D)) {
            return Transition::Push(DebugMode::new_state(ctx, app));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.panel.draw(g);
        self.world.draw(g);
        self.draw_all_filters.draw(g);
        if g.canvas.is_unzoomed() {
            self.labels.draw(g, app);
        }
    }
}

fn detect_neighborhoods(
    ctx: &mut EventCtx,
    app: &mut App,
    timer: &mut Timer,
) -> World<NeighborhoodID> {
    // TODO Or if the map doesn't match? Do we take care of this in SessionState for anything?!
    if app.session.partitioning.neighborhoods.is_empty() {
        app.session.partitioning = Partitioning::seed_using_heuristics(app, timer);
    }

    let mut world = World::bounded(app.primary.map.get_bounds());
    for (id, (block, color)) in &app.session.partitioning.neighborhoods {
        world
            .add(*id)
            .hitbox(block.polygon.clone())
            .draw_color(color.alpha(0.5))
            .hover_outline(Color::BLACK, Distance::meters(5.0))
            .clickable()
            .build(ctx);
    }
    world
}
