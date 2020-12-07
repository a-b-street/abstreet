use std::collections::{HashMap, HashSet};

use map_model::{BuildingID, BuildingType};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, Line, Text};

use crate::App;

pub struct Buildings {
    // Every building in the map is here, to simplify lookup logic.
    pub buildings: HashMap<BuildingID, BldgState>,
    // This an unchanging base layer that can get covered up by drawing on top of it. Maybe we
    // could even replace the one in DrawMap.
    pub draw_all: Drawable,
    pub total_housing_units: usize,
}

#[derive(Clone)]
pub enum BldgState {
    // Score
    Undelivered(usize),
    Store,
    // Or not a relevant building
    Done,
}

impl Buildings {
    pub fn new(ctx: &mut EventCtx, app: &App, upzones: HashSet<BuildingID>) -> Buildings {
        let colors = &app.session.colors;

        let mut buildings = HashMap::new();
        let mut total_housing_units = 0;
        let mut batch = GeomBatch::new();
        for b in app.map.all_buildings() {
            if upzones.contains(&b.id) {
                buildings.insert(b.id, BldgState::Store);
                batch.push(colors.store, b.polygon.clone());
                continue;
            }

            if let BuildingType::Residential {
                num_housing_units, ..
            } = b.bldg_type
            {
                // There are some unused commercial buildings around!
                if num_housing_units > 0 {
                    buildings.insert(b.id, BldgState::Undelivered(num_housing_units));
                    total_housing_units += num_housing_units;

                    let color = if num_housing_units > 5 {
                        colors.apartment
                    } else {
                        colors.house
                    };
                    batch.push(color, b.polygon.clone());
                    // Call out non-single family homes
                    if num_housing_units > 1 {
                        // TODO Text can be slow to render, and this should be louder anyway
                        batch.append(
                            Text::from(Line(num_housing_units.to_string()).fg(Color::RED))
                                .render_to_batch(ctx.prerender)
                                .scale(0.2)
                                .centered_on(b.label_center),
                        );
                    }
                    continue;
                }
            } else if !b.amenities.is_empty() {
                // TODO Maybe just food?
                buildings.insert(b.id, BldgState::Store);
                batch.push(colors.store, b.polygon.clone());
                continue;
            }

            // If it's not a residence or store, just blank it out.
            buildings.insert(b.id, BldgState::Done);
            batch.push(colors.visited, b.polygon.clone());
        }

        Buildings {
            buildings,
            draw_all: ctx.upload(batch),
            total_housing_units,
        }
    }

    pub fn all_stores(&self) -> Vec<BuildingID> {
        let mut stores = Vec::new();
        for (b, state) in &self.buildings {
            if let BldgState::Store = state {
                stores.push(*b);
            }
        }
        stores
    }
}
