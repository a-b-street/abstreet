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

    pub static ref PLAN_ROUTE_BEFORE: Color = Color::PURPLE;
    pub static ref PLAN_ROUTE_AFTER: Color = Color::CYAN;
    pub static ref PLAN_ROUTE_BIKE: Color = Color::GREEN;
    pub static ref PLAN_ROUTE_WALK: Color = Color::BLUE;

    pub static ref BUS_ROUTE: Color = Color::hex("#0672B9");

    // From https://content.tfl.gov.uk/lcds-chapter2-toolsandtechniques.pdf page 18
    pub static ref POROUS: Color = Color::hex("#99BA98");
    pub static ref SEMI_PERMEABLE: Color = Color::hex("#EFC796");
    pub static ref IMPERMEABLE: Color = Color::hex("#E99875");

    // From slow to fast, with the speed limit range defined elsewhere
    pub static ref SPEED_LIMITS: [Color; 4] = [
        Color::hex("#00AB4D"),
        Color::hex("#8ECA4D"),
        Color::hex("#F7BB00"),
        Color::hex("#BB0000"),
    ];

    pub static ref NETWORK_SEGREGATED_LANE: Color = Color::hex("#028A0F");
    pub static ref NETWORK_QUIET_STREET: Color = Color::hex("#03AC13");
    pub static ref NETWORK_PAINTED_LANE: Color = Color::hex("#90EE90");
    pub static ref NETWORK_THROUGH_TRAFFIC_STREET: Color = Color::hex("#F3A4A4");
}

pub const DISCONNECTED_CELL: Color = Color::RED.alpha(0.5);

pub const BLOCK_IN_BOUNDARY: Color = Color::BLUE.alpha(0.5);
pub const BLOCK_IN_FRONTIER: Color = Color::CYAN.alpha(0.2);

// TODO This doesn't show up easily against roads with dark red shortcuts
pub const LOCAL_ROAD_LABEL: Color = Color::BLACK;
pub const MAIN_ROAD_LABEL: Color = Color::WHITE;
pub const HOVER: Color = Color::CYAN.alpha(0.5);
