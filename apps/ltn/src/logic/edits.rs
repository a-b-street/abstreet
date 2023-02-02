pub mod auto;
mod existing;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use abstutil::{deserialize_btreemap, serialize_btreemap};
use geom::{Angle, Distance, Line, Speed};
use map_model::{CrossingType, EditRoad, IntersectionID, Map, RoadID, RoutingParams, TurnID};
use widgetry::mapspace::{DrawCustomUnzoomedShapes, PerZoom};
use widgetry::{Color, Drawable, EventCtx, GeomBatch, GfxCtx, RewriteColor};

pub use self::existing::transform_existing_filters;
use crate::{colors, mut_edits, App};

/// Stored in App per-map state. Before making any changes, call `before_edit`.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Edits {
    // We use serialize_btreemap so that save::perma can detect and transform IDs
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub roads: BTreeMap<RoadID, RoadFilter>,
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub intersections: BTreeMap<IntersectionID, DiagonalFilter>,
    /// For roads with modified directions or speed limits, what's their current state?
    // TODO Misnomer; this includes speed limit changes now too. Not worth a backwards incompatible
    // change right now.
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub one_ways: BTreeMap<RoadID, EditRoad>,
    /// For roads with modified speeds, what's their current state?
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub speed_limits: BTreeMap<RoadID, Speed>,
    /// One road may have multiple crossings. They're sorted by increasing distance.
    #[serde(
        serialize_with = "serialize_btreemap",
        deserialize_with = "deserialize_btreemap"
    )]
    pub crossings: BTreeMap<RoadID, Vec<Crossing>>,

    /// Edit history is preserved recursively
    #[serde(skip_serializing, skip_deserializing)]
    pub previous_version: Box<Option<Edits>>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Crossing {
    pub kind: CrossingType,
    pub dist: Distance,
    pub user_modified: bool,
}

/// This logically changes every time an edit occurs. MapName isn't captured here.
#[derive(Default, PartialEq)]
pub struct ChangeKey {
    roads: BTreeMap<RoadID, RoadFilter>,
    intersections: BTreeMap<IntersectionID, DiagonalFilter>,
    one_ways: BTreeMap<RoadID, EditRoad>,
    crossings: BTreeMap<RoadID, Vec<Crossing>>,
}


impl Edits {
    /// Modify RoutingParams to respect these modal filters
    pub fn update_routing_params(&self, params: &mut RoutingParams) {
        params.avoid_roads.extend(self.roads.keys().cloned());
        for filter in self.intersections.values() {
            params
                .avoid_movements_between
                .extend(filter.avoid_movements_between_roads());
        }
    }

    pub fn allows_turn(&self, t: TurnID) -> bool {
        if let Some(filter) = self.intersections.get(&t.parent) {
            return filter.allows_turn(t.src.road, t.dst.road);
        }
        true
    }

    pub fn get_change_key(&self) -> ChangeKey {
        ChangeKey {
            roads: self.roads.clone(),
            intersections: self.intersections.clone(),
            one_ways: self.one_ways.clone(),
            crossings: self.crossings.clone(),
        }
    }
}
