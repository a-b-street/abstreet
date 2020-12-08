//! Assorted tools and UI states that're useful for applications built to display maps.

use abstutil::MapName;
use geom::Polygon;
use widgetry::{GfxCtx, Line, Text};

pub use self::city_picker::CityPicker;
pub use self::colors::{ColorDiscrete, ColorLegend, ColorNetwork, ColorScale, DivergingScale};
pub use self::heatmap::{make_heatmap, Grid, HeatmapOptions};
pub use self::minimap::{Minimap, MinimapControls};
pub use self::navigate::Navigator;
pub use self::turn_explorer::TurnExplorer;
pub use self::ui::{ChooseSomething, PopupMsg, PromptInput};
use crate::AppLike;

mod city_picker;
mod colors;
mod heatmap;
mod minimap;
mod navigate;
mod turn_explorer;
mod ui;
#[cfg(not(target_arch = "wasm32"))]
mod updater;

// TODO This is A/B Street specific
pub fn loading_tips() -> Text {
    Text::from_multiline(vec![
        Line("Recent changes (November 8)"),
        Line(""),
        Line("- Download more cities from within the game"),
        Line("- You can now click agents while zoomed out"),
        Line("- New OpenStreetMap viewer, open it from the splash screen"),
        Line("- A web version has launched!"),
        Line("- Slow segments of a trip shown in the info panel"),
        Line("- Alleyways are now included in the map"),
        Line("- Check out the trip tables and summary changes (press 'q')"),
        Line("- Try out the new traffic signal editor!"),
    ])
}

/// Make it clear the map can't be interacted with right now.
pub fn grey_out_map(g: &mut GfxCtx, app: &dyn AppLike) {
    g.fork_screenspace();
    // TODO - OSD height
    g.draw_polygon(
        app.cs().fade_map_dark,
        Polygon::rectangle(g.canvas.window_width, g.canvas.window_height),
    );
    g.unfork();
}

// TODO Associate this with maps, but somehow avoid reading the entire file when listing them.
pub fn nice_map_name(name: &MapName) -> &str {
    match (name.city.as_ref(), name.map.as_ref()) {
        ("seattle", "ballard") => "Ballard",
        ("seattle", "downtown") => "Downtown Seattle",
        ("seattle", "huge_seattle") => "Seattle (entire area)",
        ("seattle", "lakeslice") => "Lake Washington corridor",
        ("seattle", "montlake") => "Montlake and Eastlake",
        ("seattle", "north_seattle") => "North Seattle",
        ("seattle", "slu") => "South Lake Union",
        ("seattle", "south_seattle") => "South Seattle",
        ("seattle", "udistrict") => "University District",
        ("seattle", "udistrict_ravenna") => "University District / Ravenna",
        ("seattle", "wallingford") => "Wallingford",
        ("seattle", "west_seattle") => "West Seattle",
        ("berlin", "center") => "Berlin (city center)",
        ("krakow", "center") => "Kraków (city center)",
        ("leeds", "center") => "Leeds (city center)",
        ("london", "southbank") => "London (Southbank)",
        ("paris", "center") => "Paris (city center)",
        ("paris", "north") => "Paris (north)",
        ("paris", "south") => "Paris (south)",
        ("paris", "east") => "Paris (east)",
        ("paris", "west") => "Paris (west)",
        ("tel_aviv", "center") => "Tel Aviv (city center)",
        ("xian", "center") => "Xi'an (city center)",
        _ => &name.map,
    }
}

pub fn open_browser(url: String) {
    let _ = webbrowser::open(&url);
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
