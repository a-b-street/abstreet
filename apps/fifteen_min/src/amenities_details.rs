use geom::Duration;
use map_gui::tools::draw_isochrone;
use map_model::{AmenityType, BuildingID};
use widgetry::table::{Col, Filter, Table};
use widgetry::tools::open_browser;
use widgetry::{
    Color, Drawable, EventCtx, GfxCtx, HorizontalAlignment, Line, Outcome, Panel, State, Text,
    Transition, VerticalAlignment, Widget,
};

use crate::isochrone::Isochrone;
use crate::{render, App};

pub struct ExploreAmenitiesDetails {
    table: Table<App, Entry, ()>,
    panel: Panel,
    draw: Drawable,
}

struct Entry {
    bldg: BuildingID,
    amenity_idx: usize,
    name: String,
    amenity_type: String,
    address: String,
    duration_away: Duration,
}

impl ExploreAmenitiesDetails {
    pub fn new_state(
        ctx: &mut EventCtx,
        app: &App,
        isochrone: &Isochrone,
        category: AmenityType,
    ) -> Box<dyn State<App>> {
        let mut batch = draw_isochrone(
            &app.map,
            &isochrone.time_to_reach_building,
            &isochrone.thresholds,
            &isochrone.colors,
        );
        batch.append(render::draw_star(ctx, app.map.get_b(isochrone.start[0])));

        let mut entries = Vec::new();
        for b in isochrone.amenities_reachable.get(category) {
            let bldg = app.map.get_b(*b);
            for (amenity_idx, amenity) in bldg.amenities.iter().enumerate() {
                if AmenityType::categorize(&amenity.amenity_type) == Some(category) {
                    entries.push(Entry {
                        bldg: bldg.id,
                        amenity_idx,
                        name: amenity.names.get(app.opts.language.as_ref()).to_string(),
                        amenity_type: amenity.amenity_type.clone(),
                        address: bldg.address.clone(),
                        duration_away: isochrone.time_to_reach_building[&bldg.id],
                    });
                    // Highlight the matching buildings
                    batch.push(Color::RED, bldg.polygon.clone());
                }
            }
        }

        let mut table: Table<App, Entry, ()> = Table::new(
            "time_to_reach_table",
            entries,
            // The label has extra junk to avoid crashing when one building has two stores,
            // possibly with the same name in the current language
            Box::new(|x| format!("{}: {} ({})", x.bldg.0, x.name, x.amenity_idx)),
            "Time to reach",
            Filter::empty(),
        );
        table.column(
            "Type",
            Box::new(|ctx, _, x| Text::from(&x.amenity_type).render(ctx)),
            Col::Sortable(Box::new(|rows| {
                rows.sort_by_key(|x| x.amenity_type.clone())
            })),
        );
        table.static_col("Name", Box::new(|x| x.name.clone()));
        table.static_col("Address", Box::new(|x| x.address.clone()));
        table.column(
            "Time to reach",
            Box::new(|ctx, app, x| {
                Text::from(x.duration_away.to_string(&app.opts.units)).render(ctx)
            }),
            Col::Sortable(Box::new(|rows| rows.sort_by_key(|x| x.duration_away))),
        );

        let panel = Panel::new_builder(Widget::col(vec![
            Widget::row(vec![
                Line(format!("{} within 15 minutes", category))
                    .small_heading()
                    .into_widget(ctx),
                ctx.style().btn_close_widget(ctx),
            ]),
            table.render(ctx, app),
        ]))
        .aligned(HorizontalAlignment::Center, VerticalAlignment::TopInset)
        .build(ctx);

        Box::new(Self {
            table,
            panel,
            draw: ctx.upload(batch),
        })
    }
}

impl State<App> for ExploreAmenitiesDetails {
    fn event(&mut self, ctx: &mut EventCtx, app: &mut App) -> Transition<App> {
        ctx.canvas_movement();

        match self.panel.event(ctx) {
            Outcome::Clicked(x) => {
                if self.table.clicked(&x) {
                    self.table.replace_render(ctx, app, &mut self.panel)
                } else if x == "close" {
                    return Transition::Pop;
                } else if let Some(idx) = x.split(':').next().and_then(|x| x.parse::<usize>().ok())
                {
                    let b = app.map.get_b(BuildingID(idx));
                    open_browser(b.orig_id.to_string());
                } else {
                    unreachable!()
                }
            }
            Outcome::Changed(_) => {
                self.table.panel_changed(&self.panel);
                self.table.replace_render(ctx, app, &mut self.panel)
            }
            _ => {}
        }

        Transition::Keep
    }

    fn draw(&self, g: &mut GfxCtx, app: &App) {
        g.redraw(&self.draw);
        self.panel.draw(g);
        if let Some(x) = self
            .panel
            .currently_hovering()
            .and_then(|x| x.split(':').next())
            .and_then(|x| x.parse::<usize>().ok())
        {
            g.draw_polygon(Color::CYAN, app.map.get_b(BuildingID(x)).polygon.clone());
        }
    }
}
