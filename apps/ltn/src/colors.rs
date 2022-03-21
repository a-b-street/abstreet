use widgetry::Color;

lazy_static::lazy_static! {
    /// Rotate through these colors for neighborhoods or cells. Use 4-color (ehem, 6-color?)
    /// theorem to make adjacent things different
    pub static ref ADJACENT_STUFF: [Color; 4] = [
        Color::hex("#D898B3"),
        Color::hex("#B7A3D7"),
        Color::hex("#F5D89B"),
        Color::hex("#90CCD3"),
    ];

    pub static ref FILTER_OUTER: Color = Color::hex("#E85E5E");
    pub static ref FILTER_INNER: Color = Color::WHITE;
}

pub const CAR_FREE_CELL: Color = Color::GREEN.alpha(0.5);
pub const DISCONNECTED_CELL: Color = Color::RED.alpha(0.5);

pub const OUTLINE: Color = Color::BLACK;

pub const HIGHLIGHT_BOUNDARY_UNZOOMED: Color = Color::RED.alpha(0.6);
pub const HIGHLIGHT_BOUNDARY_ZOOMED: Color = Color::RED.alpha(0.5);

pub const RAT_RUN_PATH: Color = Color::RED;

pub const BLOCK_IN_BOUNDARY: Color = Color::BLUE.alpha(0.5);
pub const BLOCK_IN_FRONTIER: Color = Color::CYAN.alpha(0.2);

pub const PLAN_ROUTE_BEFORE: Color = Color::BLUE;
pub const PLAN_ROUTE_AFTER: Color = Color::RED;
