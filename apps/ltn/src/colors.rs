use widgetry::Color;

lazy_static::lazy_static! {
    // A qualitative palette from colorbrewer2.org, skipping the red hue (used for levels of
    // shortcutting) and grey (too close to the basemap)
    pub static ref CELLS: [Color; 10] = [
        Color::hex("#8dd3c7"),
        Color::hex("#ffffb3"),
        Color::hex("#bebada"),
        Color::hex("#80b1d3"),
        Color::hex("#fdb462"),
        Color::hex("#b3de69"),
        Color::hex("#fccde5"),
        Color::hex("#bc80bd"),
        Color::hex("#ccebc5"),
        Color::hex("#ffed6f"),
    ];

    pub static ref PLAN_ROUTE_BEFORE: Color = Color::RED;
    pub static ref PLAN_ROUTE_AFTER: Color = Color::CYAN;
    pub static ref PLAN_ROUTE_BIKE: Color = Color::GREEN;
    pub static ref PLAN_ROUTE_WALK: Color = Color::BLUE;

    pub static ref BUS_ROUTE: Color = Color::hex("#0672B9");

    // From https://content.tfl.gov.uk/lcds-chapter2-toolsandtechniques.pdf page 18
    pub static ref POROUS: Color = Color::hex("#99BA98");
    pub static ref SEMI_PERMEABLE: Color = Color::hex("#EFC796");
    pub static ref IMPERMEABLE: Color = Color::hex("#E99875");
}

pub const DISCONNECTED_CELL: Color = Color::RED.alpha(0.5);

pub const HIGHLIGHT_BOUNDARY: Color = Color::RED.alpha(0.6);

pub const BLOCK_IN_BOUNDARY: Color = Color::BLUE.alpha(0.5);
pub const BLOCK_IN_FRONTIER: Color = Color::CYAN.alpha(0.2);

pub const ROAD_LABEL: Color = Color::BLACK;
pub const HOVER: Color = Color::CYAN.alpha(0.5);
