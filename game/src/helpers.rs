use std::collections::BTreeSet;

use geom::Duration;
pub use map_gui::helpers::{grey_out_map, nice_map_name, open_browser, ID};
use map_model::{IntersectionID, Map, RoadID};
use sim::{AgentType, TripMode, TripPhaseType};
use widgetry::{Btn, Checkbox, Color, EventCtx, Key, Line, Text, TextSpan, Widget};

use crate::app::App;

pub fn list_names<F: Fn(TextSpan) -> TextSpan>(txt: &mut Text, styler: F, names: BTreeSet<String>) {
    let len = names.len();
    for (idx, n) in names.into_iter().enumerate() {
        if idx != 0 {
            if idx == len - 1 {
                if len == 2 {
                    txt.append(Line(" and "));
                } else {
                    txt.append(Line(", and "));
                }
            } else {
                txt.append(Line(", "));
            }
        }
        txt.append(styler(Line(n)));
    }
}

// Shorter is better
pub fn cmp_duration_shorter(app: &App, after: Duration, before: Duration) -> Vec<TextSpan> {
    if after.epsilon_eq(before) {
        vec![Line("same")]
    } else if after < before {
        vec![
            Line((before - after).to_string(&app.opts.units)).fg(Color::GREEN),
            Line(" faster"),
        ]
    } else if after > before {
        vec![
            Line((after - before).to_string(&app.opts.units)).fg(Color::RED),
            Line(" slower"),
        ]
    } else {
        unreachable!()
    }
}

pub fn color_for_mode(app: &App, m: TripMode) -> Color {
    match m {
        TripMode::Walk => app.cs.unzoomed_pedestrian,
        TripMode::Bike => app.cs.unzoomed_bike,
        TripMode::Transit => app.cs.unzoomed_bus,
        TripMode::Drive => app.cs.unzoomed_car,
    }
}

pub fn color_for_agent_type(app: &App, a: AgentType) -> Color {
    match a {
        AgentType::Pedestrian => app.cs.unzoomed_pedestrian,
        AgentType::Bike => app.cs.unzoomed_bike,
        AgentType::Bus | AgentType::Train => app.cs.unzoomed_bus,
        AgentType::TransitRider => app.cs.bus_trip,
        AgentType::Car => app.cs.unzoomed_car,
    }
}

pub fn color_for_trip_phase(app: &App, tpt: TripPhaseType) -> Color {
    match tpt {
        TripPhaseType::Driving => app.cs.unzoomed_car,
        TripPhaseType::Walking => app.cs.unzoomed_pedestrian,
        TripPhaseType::Biking => app.cs.bike_trip,
        TripPhaseType::Parking => app.cs.parking_trip,
        TripPhaseType::WaitingForBus(_, _) => app.cs.bus_layer,
        TripPhaseType::RidingBus(_, _, _) => app.cs.bus_trip,
        TripPhaseType::Cancelled | TripPhaseType::Finished => unreachable!(),
        TripPhaseType::DelayedStart => Color::YELLOW,
    }
}

pub fn amenity_type(a: &str) -> Option<&'static str> {
    // NOTE: names are used in amenities function in other file
    // TODO: create categories for:
    // hairdresser beauty chemist
    // car_repair
    // laundry
    if a == "supermarket" || a == "convenience" {
        Some("groceries")
    } else if a == "restaurant"
        || a == "cafe"
        || a == "fast_food"
        || a == "food_court"
        || a == "ice_cream"
        || a == "pastry"
        || a == "deli"
        || a == "greengrocer"
        || a == "bakery"
        || a == "butcher"
        || a == "confectionery"
        || a == "beverages"
        || a == "alcohol"
    {
        Some("food")
    } else if a == "pub" || a == "bar" || a == "nightclub" || a == "lounge" {
        Some("bar")
    } else if a == "doctors"
        || a == "dentist"
        || a == "clinic"
        || a == "hospital"
        || a == "pharmacy"
        || a == "chiropractor"
        || a == "optician"
    {
        Some("medical")
    } else if a == "place_of_worship" {
        Some("church / temple")
    } else if a == "college" || a == "school" || a == "university" {
        Some("education")
    } else if a == "bank" || a == "post_office" {
        Some("bank / post office")
    } else if a == "theatre"
        || a == "arts_centre"
        || a == "library"
        || a == "cinema"
        || a == "art_gallery"
        || a == "museum"
    {
        Some("culture")
    } else if a == "childcare" || a == "kindergarten" {
        Some("childcare")
    } else if a == "second_hand"
        || a == "clothes"
        || a == "furniture"
        || a == "shoes"
        || a == "department_store"
        || a == "car"
        || a == "kiosk"
        || a == "hardware"
        || a == "mobile_phone"
        || a == "florist"
        || a == "electronics"
        || a == "car_parts"
        || a == "doityourself"
        || a == "jewelry"
        || a == "variety_store"
        || a == "gift"
        || a == "bicycle"
        || a == "books"
        || a == "sports"
        || a == "travel_agency"
        || a == "stationery"
        || a == "pet"
        || a == "computer"
        || a == "tyres"
        || a == "newsagent"
    {
        Some("shopping")
    } else {
        None
    }
}

// TODO Well, there goes the nice consolidation of stuff in BtnBuilder. :\
pub fn hotkey_btn<I: Into<String>>(ctx: &EventCtx, app: &App, label: I, key: Key) -> Widget {
    let label = label.into();
    let mut txt = Text::new();
    txt.append(key.txt(ctx));
    txt.append(Line(format!(" - {}", label)));
    Btn::text_bg(label, txt, app.cs.section_bg, app.cs.hovering).build_def(ctx, key)
}

pub fn intersections_from_roads(roads: &BTreeSet<RoadID>, map: &Map) -> BTreeSet<IntersectionID> {
    let mut results = BTreeSet::new();
    for r in roads {
        let r = map.get_r(*r);
        for i in vec![r.src_i, r.dst_i] {
            if results.contains(&i) {
                continue;
            }
            if map.get_i(i).roads.iter().all(|r| roads.contains(r)) {
                results.insert(i);
            }
        }
    }
    results
}

pub fn checkbox_per_mode(
    ctx: &mut EventCtx,
    app: &App,
    current_state: &BTreeSet<TripMode>,
) -> Widget {
    let mut filters = Vec::new();
    for m in TripMode::all() {
        filters.push(
            Checkbox::colored(
                ctx,
                m.ongoing_verb(),
                color_for_mode(app, m),
                current_state.contains(&m),
            )
            .margin_right(24),
        );
    }
    Widget::custom_row(filters)
}
