/// A filter placed somewhere along a road
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct RoadFilter {
    pub dist: Distance,
    pub filter_type: FilterType,
    pub user_modified: bool,
}

impl RoadFilter {
    pub fn new_by_user(dist: Distance, filter_type: FilterType) -> Self {
        Self {
            dist,
            filter_type,
            user_modified: true,
        }
    }
}

/// Just determines the icon, has no semantics yet
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FilterType {
    NoEntry,
    WalkCycleOnly,
    BusGate,
    SchoolStreet,
}

impl FilterType {
    pub fn svg_path(self) -> &'static str {
        match self {
            FilterType::NoEntry => "system/assets/tools/no_entry.svg",
            FilterType::WalkCycleOnly => "system/assets/tools/modal_filter.svg",
            FilterType::BusGate => "system/assets/tools/bus_gate.svg",
            FilterType::SchoolStreet => "system/assets/tools/school_street.svg",
        }
    }

    pub fn hide_color(self) -> Color {
        match self {
            FilterType::WalkCycleOnly => Color::hex("#0b793a"),
            FilterType::NoEntry => Color::RED,
            FilterType::BusGate => *colors::BUS_ROUTE,
            FilterType::SchoolStreet => Color::hex("#e31017"),
        }
    }
}


