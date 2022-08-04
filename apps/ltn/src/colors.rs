use widgetry::Color;

lazy_static::lazy_static! {
    pub static ref CELLS: [Color; 6] = [
        Color::BLUE.alpha(0.5),
        Color::YELLOW.alpha(0.5),
        Color::hex("#3CAEA3").alpha(0.5),
        Color::PURPLE.alpha(0.5),
        Color::PINK.alpha(0.5),
        Color::ORANGE.alpha(0.5),
    ];

    pub static ref PLAN_ROUTE_BEFORE: Color = Color::RED;
    pub static ref PLAN_ROUTE_AFTER: Color = Color::CYAN;
    pub static ref PLAN_ROUTE_BIKE: Color = Color::GREEN;
    pub static ref PLAN_ROUTE_WALK: Color = Color::BLUE;
}

pub const DISCONNECTED_CELL: Color = Color::RED.alpha(0.5);

pub const HIGHLIGHT_BOUNDARY: Color = Color::RED.alpha(0.6);

pub const BLOCK_IN_BOUNDARY: Color = Color::BLUE.alpha(0.5);
pub const BLOCK_IN_FRONTIER: Color = Color::CYAN.alpha(0.2);

pub const ROAD_LABEL: Color = Color::BLACK;
pub const HOVER: Color = Color::CYAN.alpha(0.5);
