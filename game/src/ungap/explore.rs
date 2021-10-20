use geom::{ArrowCap, Distance, PolyLine};
use map_gui::tools::URLManager;
use widgetry::{Color, EventCtx, GfxCtx, Outcome, Panel, State, TextExt, Widget};

use crate::app::{App, Transition};
use crate::ungap::{Layers, Tab, TakeLayers};

pub struct ExploreMap {
    top_panel: Panel,
    layers: Layers,
}

impl TakeLayers for ExploreMap {
    fn take_layers(self) -> Layers {
        self.layers
    }
}

impl ExploreMap {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App, layers: Layers) -> Box<dyn State<App>> {
        app.opts.show_building_driveways = false;

        URLManager::update_url_free_param(
            app.primary
                .map
                .get_name()
                .path()
                .strip_prefix(&abstio::path(""))
                .unwrap()
                .to_string(),
        );

        Box::new(ExploreMap {
            top_panel: Tab::Explore.make_left_panel(
                ctx,
                app,
                Widget::col(vec![
                    "Zoom in to see detailed lane information".text_widget(ctx),
                    Widget::row(vec![
                        "To explore elevation data,"
                            .text_widget(ctx)
                            .centered_vert(),
                        ctx.style()
                            .btn_plain
                            .icon_text("system/assets/tools/layers.svg", "Show more layers")
                            .build_def(ctx),
                    ]),
                ]),
            ),
            layers,
        })
    }
}

impl State<App> for ExploreMap {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if ctx.canvas_movement() {
            URLManager::update_url_cam(ctx, app.primary.map.get_gps_bounds());
        }

        if let Outcome::Clicked(x) = self.top_panel.event(ctx) {
            match x.as_ref() {
                "Show more layers" => {
                    self.layers.show_panel(ctx, app);
                }
                x => {
                    return Tab::Explore
                        .handle_action::<ExploreMap>(ctx, app, x)
                        .unwrap();
                }
            }
        }

        if let Some(t) = self.layers.event(ctx, app) {
            return t;
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.top_panel.draw(g);
        self.layers.draw(g, app);

        if self.top_panel.currently_hovering() == Some(&"Show more layers".to_string()) {
            g.fork_screenspace();
            if let Ok(pl) = PolyLine::new(vec![
                self.top_panel.center_of("Show more layers").to_pt(),
                self.layers.layer_icon_pos().to_pt(),
            ]) {
                g.draw_polygon(
                    Color::RED,
                    pl.make_arrow(Distance::meters(20.0), ArrowCap::Triangle),
                );
            }
            g.unfork();
        }
    }
}
