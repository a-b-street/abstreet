use geom::Distance;
use widgetry::mapspace::{ObjectID, World, WorldOutcome};
use widgetry::tools::{open_browser, ColorLegend, PopupMsg};
use widgetry::{
    Color, EventCtx, GeomBatch, GfxCtx, Line, Outcome, Panel, State, Text, TextExt, Widget,
};

use crate::components::{AppwidePanel, BottomPanel, Mode};
use crate::render::colors;
use crate::{App, Transition};

pub struct Census {
    appwide_panel: AppwidePanel,
    bottom_panel: Panel,
    world: World<ZoneID>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct ZoneID(usize);
impl ObjectID for ZoneID {}

impl Census {
    pub fn new_state(ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        // What's the max number of cars in an OA?
        // (Clamp to >= 1 to avoid division by zero)
        let max_cars = app
            .per_map
            .map
            .all_census_zones()
            .iter()
            .map(|(_, z)| z.total_cars())
            .max()
            .unwrap_or(0)
            .max(1);
        let buckets = bucketize(max_cars);

        let appwide_panel = AppwidePanel::new(ctx, app, Mode::Census);
        let legend = make_legend(ctx, buckets);
        let bottom_panel = BottomPanel::new(
            ctx,
            &appwide_panel,
            Widget::row(vec![
                ctx.style()
                    .btn_outline
                    .text("About")
                    .build_def(ctx)
                    .centered_vert(),
                "Total vehicles owned:".text_widget(ctx).centered_vert(),
                legend,
            ]),
        );

        // Just force the layers panel to align above the bottom panel
        app.session
            .layers
            .event(ctx, &app.cs, Mode::Census, Some(&bottom_panel));

        let mut world = World::new();

        for (idx, (polygon, zone)) in app.per_map.map.all_census_zones().into_iter().enumerate() {
            let n = zone.total_cars();
            let color = if n < buckets[1] {
                colors::SPEED_LIMITS[0]
            } else if n < buckets[2] {
                colors::SPEED_LIMITS[1]
            } else if n < buckets[3] {
                colors::SPEED_LIMITS[2]
            } else {
                colors::SPEED_LIMITS[3]
            };

            let mut draw_normal = GeomBatch::new();
            draw_normal.push(color.alpha(0.5), polygon.clone());
            draw_normal.push(Color::RED, polygon.to_outline(Distance::meters(5.0)));

            let mut draw_hover = GeomBatch::new();
            draw_hover.push(color.alpha(0.5), polygon.clone());
            draw_hover.push(Color::RED, polygon.to_outline(Distance::meters(10.0)));

            world
                .add(ZoneID(idx))
                .hitbox(polygon.clone())
                .draw(draw_normal)
                .draw_hovered(draw_hover)
                .clickable()
                .tooltip(Text::from_multiline(vec![
                    Line(format!(
                        "Output Area {} has {} vehicles total",
                        zone.id,
                        zone.total_cars()
                    )),
                    Line(""),
                    Line(format!("Households with 0 vehicles: {}", zone.cars_0)),
                    Line(format!("Households with 1 vehicles: {}", zone.cars_1)),
                    Line(format!("Households with 2 vehicles: {}", zone.cars_2)),
                    Line(format!(
                        "Households with 3 or more vehicles: {}",
                        zone.cars_3
                    )),
                ]))
                .build(ctx);
        }

        Box::new(Self {
            appwide_panel,
            bottom_panel,
            world,
        })
    }
}

impl State<App> for Census {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition {
        if let Some(t) =
            self.appwide_panel
                .event(ctx, app, &crate::save::PreserveState::Census, help)
        {
            self.world.hack_unset_hovering();
            return t;
        }
        if let Some(t) =
            app.session
                .layers
                .event(ctx, &app.cs, Mode::Census, Some(&self.bottom_panel))
        {
            return t;
        }
        if let Outcome::Clicked(x) = self.bottom_panel.event(ctx) {
            if x == "About" {
                // TODO Very England-specific! Actually, plumb through metadata from popgetter
                // about the layers
                return Transition::Push(PopupMsg::new_state(ctx, "About", vec!["This shows car or van availability per household, thanks to UK census 2021 data from ONS.", "The ONS data counts households with 0, 1, 2, and >= 3 cars or vans.", "This layer summarizes this by counting the total vehicles available through the entire Output Area.", "", "WARNING: This layer is experimental; there may be data quality problems!"]));
            } else {
                unreachable!()
            }
        }

        if let WorldOutcome::ClickedObject(ZoneID(idx)) = self.world.event(ctx) {
            open_browser(format!("https://www.ons.gov.uk/census/maps/choropleth/housing/number-of-cars-or-vans/number-of-cars-5a/no-cars-or-vans-in-household?oa={}", app.per_map.map.all_census_zones()[idx].1.id));
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        self.appwide_panel.draw(g);
        self.bottom_panel.draw(g);
        app.session.layers.draw(g, app);
        app.per_map.draw_major_road_labels.draw(g);
        app.per_map.draw_all_filters.draw(g);
        app.per_map.draw_poi_icons.draw(g);
        self.world.draw(g);
    }

    fn recreate(&mut self, ctx: &mut EventCtx, app: &mut App) -> Box<dyn State<App>> {
        Self::new_state(ctx, app)
    }
}

fn help() -> Vec<&'static str> {
    vec!["This shows census data that may be useful to decide where LTNs could be placed."]
}

// TODO Don't we have something in widgetry like this?
fn bucketize(max_cars: u16) -> [u16; 5] {
    let max = max_cars as f64;
    let p25 = (0.25 * max) as u16;
    let p50 = (0.5 * max) as u16;
    let p75 = (0.75 * max) as u16;
    [0, p25, p50, p75, max_cars]
}

fn make_legend(ctx: &mut EventCtx, buckets: [u16; 5]) -> Widget {
    ColorLegend::categories(
        ctx,
        vec![
            (colors::SPEED_LIMITS[0], &buckets[0].to_string()),
            (colors::SPEED_LIMITS[1], &buckets[1].to_string()),
            (colors::SPEED_LIMITS[2], &buckets[2].to_string()),
            (colors::SPEED_LIMITS[3], &buckets[3].to_string()),
        ],
        &buckets[4].to_string(),
    )
}
