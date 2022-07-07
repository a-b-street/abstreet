use street_network::MapConfig;

pub mod clip;
pub mod split_ways;

/// Configures the creation of a RawMap from OSM and other input data.
pub struct Options {
    pub map_config: MapConfig,

    pub onstreet_parking: OnstreetParking,
    pub public_offstreet_parking: PublicOffstreetParking,
    pub private_offstreet_parking: PrivateOffstreetParking,
    /// OSM railway=rail will be included as light rail if so. Cosmetic only.
    pub include_railroads: bool,
    /// If provided, read polygons from this GeoJSON file and add them to the RawMap as buildings.
    pub extra_buildings: Option<String>,
    /// Only include highways and arterials. This may make sense for some region-wide maps for
    /// particular use cases.
    pub skip_local_roads: bool,
    /// Only include crosswalks that match a `highway=crossing` OSM node.
    pub filter_crosswalks: bool,
    /// Configure public transit using this URL to a static GTFS feed in .zip format.
    pub gtfs_url: Option<String>,
    pub elevation: bool,
}

impl Options {
    pub fn default_for_side(driving_side: street_network::DrivingSide) -> Self {
        Self {
            map_config: MapConfig::default_for_side(driving_side),
            onstreet_parking: OnstreetParking::JustOSM,
            public_offstreet_parking: PublicOffstreetParking::None,
            private_offstreet_parking: PrivateOffstreetParking::FixedPerBldg(1),
            include_railroads: true,
            extra_buildings: None,
            skip_local_roads: false,
            filter_crosswalks: false,
            gtfs_url: None,
            elevation: false,
        }
    }
}

/// What roads will have on-street parking lanes? Data from
/// <https://wiki.openstreetmap.org/wiki/Key:parking:lane> is always used if available.
pub enum OnstreetParking {
    /// If not tagged, there won't be parking.
    JustOSM,
    /// If OSM data is missing, then try to match data from
    /// <http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface>. This is Seattle specific.
    Blockface(String),
    /// If OSM data is missing, then infer parking lanes on some percentage of
    /// "highway=residential" roads.
    SomeAdditionalWhereNoData {
        /// [0, 100]
        pct: usize,
    },
}

/// How many spots are available in public parking garages?
pub enum PublicOffstreetParking {
    None,
    /// Pull data from
    /// <https://data-seattlecitygis.opendata.arcgis.com/datasets/public-garages-or-parking-lots>, a
    /// Seattle-specific data source.
    Gis(String),
}

/// If a building doesn't have anything from public_offstreet_parking and isn't tagged as a garage
/// in OSM, how many private spots should it have?
pub enum PrivateOffstreetParking {
    FixedPerBldg(usize),
    // TODO Based on the number of residents?
}
