use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::Timer;
use geom::Polygon;
use map_model::osm::RoadRank;
use map_model::{Block, Map, Perimeter, RoadID, RoadSideID};

use crate::App;

/// An opaque ID, won't be contiguous as we adjust boundaries
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NeighbourhoodID(pub usize);

/// Identifies a single / unmerged block, which never changes
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BlockID(usize);

// Some states want this
impl widgetry::mapspace::ObjectID for NeighbourhoodID {}
impl widgetry::mapspace::ObjectID for BlockID {}

#[derive(Clone, Serialize, Deserialize)]
pub struct Partitioning {
    pub map: MapName,
    neighbourhoods: BTreeMap<NeighbourhoodID, NeighbourhoodInfo>,
    // The single / unmerged blocks never change
    single_blocks: Vec<Block>,

    neighbourhood_id_counter: usize,

    // Invariant: This is a surjection, every block belongs to exactly one neighbourhood
    block_to_neighbourhood: BTreeMap<BlockID, NeighbourhoodID>,

    use_expensive_blockfinding: bool,
    pub broken: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NeighbourhoodInfo {
    pub block: Block,
    /// Draw a special cone of light when focused on this neighbourhood. It doesn't change which
    /// roads can be edited.
    pub override_drawing_boundary: Option<Polygon>,
}

impl NeighbourhoodInfo {
    fn new(block: Block) -> Self {
        Self {
            block,
            override_drawing_boundary: None,
        }
    }
}

impl Partitioning {
    /// Only valid before the LTN tool has been activated this session
    pub fn empty() -> Partitioning {
        Partitioning {
            map: MapName::new("zz", "temp", "orary"),
            neighbourhoods: BTreeMap::new(),
            single_blocks: Vec::new(),

            neighbourhood_id_counter: 0,

            block_to_neighbourhood: BTreeMap::new(),

            use_expensive_blockfinding: false,
            broken: false,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.neighbourhoods.is_empty()
    }

    pub fn seed_using_heuristics(app: &App, timer: &mut Timer) -> Partitioning {
        // Try the easy thing first, but then give up
        'METHOD: for use_expensive_blockfinding in [false, true] {
            let map = &app.map;
            timer.start("find single blocks");
            let mut single_blocks = Vec::new();
            let mut single_block_perims = Vec::new();
            for mut perim in Perimeter::find_all_single_blocks(map) {
                // TODO Some perimeters don't blockify after collapsing dead-ends. So do this
                // upfront, and separately work on any blocks that don't show up.
                // https://github.com/a-b-street/abstreet/issues/841
                perim.collapse_deadends();
                if let Ok(block) = perim.to_block(map) {
                    single_block_perims.push(block.perimeter.clone());
                    single_blocks.push(block);
                }
            }
            timer.stop("find single blocks");

            timer.start("partition");
            let partitions = Perimeter::partition_by_predicate(single_block_perims, |r| {
                // "Interior" roads of a neighbourhood aren't classified as arterial
                map.get_r(r).get_rank() == RoadRank::Local
            });

            let mut merged = Vec::new();
            for perimeters in partitions {
                // If we got more than one result back, merging partially failed. Oh well?
                let stepwise_debug = false;
                merged.extend(Perimeter::merge_all(
                    map,
                    perimeters,
                    stepwise_debug,
                    use_expensive_blockfinding,
                ));
            }
            timer.stop("partition");

            timer.start_iter("blockify", merged.len());
            let mut blocks = Vec::new();
            for perimeter in merged {
                timer.next();
                match perimeter.to_block(map) {
                    Ok(block) => {
                        blocks.push(block);
                    }
                    Err(err) => {
                        warn!("Failed to make a block from a merged perimeter: {}", err);
                    }
                }
            }

            let mut neighbourhoods = BTreeMap::new();
            for block in blocks {
                neighbourhoods.insert(
                    NeighbourhoodID(neighbourhoods.len()),
                    NeighbourhoodInfo::new(block),
                );
            }
            let neighbourhood_id_counter = neighbourhoods.len();
            let mut p = Partitioning {
                map: map.get_name().clone(),
                neighbourhoods,
                single_blocks,

                neighbourhood_id_counter,
                block_to_neighbourhood: BTreeMap::new(),
                use_expensive_blockfinding,
                broken: false,
            };

            // TODO We could probably build this up as we go
            for id in p.all_block_ids() {
                if let Some(neighbourhood) = p.neighbourhood_containing(id) {
                    p.block_to_neighbourhood.insert(id, neighbourhood);
                } else {
                    if !use_expensive_blockfinding {
                        // Try the expensive check, then
                        error!(
                            "Block doesn't belong to any neighbourhood? Retrying with expensive checks {:?}",
                            p.get_block(id).perimeter
                        );
                        continue 'METHOD;
                    }
                    // This will break everything downstream, so bail out immediately
                    error!(
                        "Block still doesn't belong to any neighbourhood, even with expensive checks. Continuing without boundary adjustment. {:?}",
                        p.get_block(id).perimeter
                    );
                    p.broken = true;
                }
            }

            return p;
        }
        unreachable!()
    }

    // TODO Explain return value
    pub fn transfer_block(
        &mut self,
        map: &Map,
        id: BlockID,
        old_owner: NeighbourhoodID,
        new_owner: NeighbourhoodID,
    ) -> Result<Option<NeighbourhoodID>> {
        assert_ne!(old_owner, new_owner);

        // Is the newly expanded neighbourhood a valid perimeter?
        let new_owner_blocks: Vec<BlockID> = self
            .block_to_neighbourhood
            .iter()
            .filter_map(|(block, neighbourhood)| {
                if *neighbourhood == new_owner || *block == id {
                    Some(*block)
                } else {
                    None
                }
            })
            .collect();
        let mut new_neighbourhood_blocks = self.make_merged_blocks(map, new_owner_blocks)?;
        if new_neighbourhood_blocks.len() != 1 {
            // This happens when a hole would be created by adding this block. There are probably
            // some smaller blocks nearby to add first.
            bail!("Couldn't add block -- you may need to add an intermediate block first to avoid a hole, or there's a bug you can't workaround yet");
        }
        let new_neighbourhood_block = new_neighbourhood_blocks.pop().unwrap();

        // Is the old neighbourhood, minus this block, still valid?
        // TODO refactor Neighbourhood to BlockIDs?
        let old_owner_blocks: Vec<BlockID> = self
            .block_to_neighbourhood
            .iter()
            .filter_map(|(block, neighbourhood)| {
                if *neighbourhood == old_owner && *block != id {
                    Some(*block)
                } else {
                    None
                }
            })
            .collect();
        if old_owner_blocks.is_empty() {
            // We're deleting the old neighbourhood!
            self.neighbourhoods.get_mut(&new_owner).unwrap().block = new_neighbourhood_block;
            self.neighbourhoods.remove(&old_owner).unwrap();
            self.block_to_neighbourhood.insert(id, new_owner);
            // Tell the caller to recreate this SelectBoundary state, switching to the neighbourhood
            // we just donated to, since the old is now gone
            return Ok(Some(new_owner));
        }

        let mut old_neighbourhood_blocks =
            self.make_merged_blocks(map, old_owner_blocks.clone())?;
        // We might be splitting the old neighbourhood into multiple pieces! Pick the largest piece
        // as the old_owner (so the UI for trimming a neighbourhood is less jarring), and create new
        // neighbourhoods for the others.
        old_neighbourhood_blocks.sort_by_key(|block| block.perimeter.interior.len());
        self.neighbourhoods.get_mut(&old_owner).unwrap().block =
            old_neighbourhood_blocks.pop().unwrap();
        let new_splits = !old_neighbourhood_blocks.is_empty();
        for split_piece in old_neighbourhood_blocks {
            let new_neighbourhood = NeighbourhoodID(self.neighbourhood_id_counter);
            self.neighbourhood_id_counter += 1;
            self.neighbourhoods
                .insert(new_neighbourhood, NeighbourhoodInfo::new(split_piece));
        }
        if new_splits {
            // We need to update the owner of all single blocks in these new pieces
            for id in old_owner_blocks {
                self.block_to_neighbourhood
                    .insert(id, self.neighbourhood_containing(id).unwrap());
            }
        }

        self.neighbourhoods.get_mut(&new_owner).unwrap().block = new_neighbourhood_block;
        self.block_to_neighbourhood.insert(id, new_owner);
        Ok(None)
    }

    /// Needs to find an existing neighbourhood to take the block, or make a new one
    pub fn remove_block_from_neighbourhood(
        &mut self,
        map: &Map,
        id: BlockID,
        old_owner: NeighbourhoodID,
    ) -> Result<Option<NeighbourhoodID>> {
        // Find all RoadSideIDs in the block matching the current neighbourhood perimeter. Look for
        // the first one that borders another neighbourhood, and transfer the block there.
        // TODO This can get unintuitive -- if we remove a block bordering two other
        // neighbourhoods, which one should we donate to?
        let current_perim_set: BTreeSet<RoadSideID> = self.neighbourhoods[&old_owner]
            .block
            .perimeter
            .roads
            .iter()
            .cloned()
            .collect();
        for road_side in &self.get_block(id).perimeter.roads {
            if !current_perim_set.contains(road_side) {
                continue;
            }
            // Is there another neighbourhood that has the other side of this road on its perimeter?
            // TODO We could map road -> BlockID then use block_to_neighbourhood
            let other_side = road_side.other_side();
            if let Some((new_owner, _)) = self
                .neighbourhoods
                .iter()
                .find(|(_, info)| info.block.perimeter.roads.contains(&other_side))
            {
                let new_owner = *new_owner;
                return self.transfer_block(map, id, old_owner, new_owner);
            }
        }

        // We didn't find any match, so we're jettisoning a block near the edge of the map (or a
        // buggy area missing blocks). Create a new neighbourhood with just this block.
        let new_owner = NeighbourhoodID(self.neighbourhood_id_counter);
        self.neighbourhood_id_counter += 1;
        self.neighbourhoods.insert(
            new_owner,
            NeighbourhoodInfo::new(self.get_block(id).clone()),
        );
        let result = self.transfer_block(map, id, old_owner, new_owner);
        if result.is_err() {
            // Revert the change above!
            self.neighbourhoods.remove(&new_owner).unwrap();
        }
        result
    }
}

// Read-only
impl Partitioning {
    pub fn neighbourhood_block(&self, id: NeighbourhoodID) -> &Block {
        &self.neighbourhoods[&id].block
    }

    pub fn neighbourhood_area_km2(&self, id: NeighbourhoodID) -> String {
        // Convert from m^2 to km^2
        let area = self.neighbourhood_block(id).polygon.area() / 1_000_000.0;
        format!("~{:.1} km²", area)
    }

    pub fn neighbourhood_boundary_polygon(&self, app: &App, id: NeighbourhoodID) -> Polygon {
        let info = &self.neighbourhoods[&id];
        if let Some(polygon) = info.override_drawing_boundary.clone() {
            return polygon;
        }
        // The neighbourhood's perimeter hugs the "interior" of the neighbourhood. If we just use the
        // other side of the perimeter road, the highlighted area nicely shows the boundary road
        // too. (But sometimes this breaks, of course)
        match info
            .block
            .perimeter
            .clone()
            .flip_side_of_road()
            .to_block(&app.map)
        {
            Ok(block) => block.polygon,
            Err(_) => info.block.polygon.clone(),
        }
    }
    pub fn override_neighbourhood_boundary_polygon(
        &mut self,
        id: NeighbourhoodID,
        polygon: Polygon,
    ) {
        self.neighbourhoods
            .get_mut(&id)
            .unwrap()
            .override_drawing_boundary = Some(polygon);
    }

    pub fn all_neighbourhoods(&self) -> &BTreeMap<NeighbourhoodID, NeighbourhoodInfo> {
        &self.neighbourhoods
    }

    // Just used for initial creation
    fn neighbourhood_containing(&self, find_block: BlockID) -> Option<NeighbourhoodID> {
        // TODO We could probably build this mapping up when we do Perimeter::merge_all
        let find_block = self.get_block(find_block);
        for (id, info) in &self.neighbourhoods {
            if info.block.perimeter.contains(&find_block.perimeter) {
                return Some(*id);
            }
        }
        None
    }

    pub fn all_single_blocks(&self) -> Vec<(BlockID, &Block)> {
        self.single_blocks
            .iter()
            .enumerate()
            .map(|(idx, block)| (BlockID(idx), block))
            .collect()
    }

    pub fn all_block_ids(&self) -> Vec<BlockID> {
        (0..self.single_blocks.len()).map(BlockID).collect()
    }

    pub fn get_block(&self, id: BlockID) -> &Block {
        &self.single_blocks[id.0]
    }

    pub fn block_to_neighbourhood(&self, id: BlockID) -> NeighbourhoodID {
        self.block_to_neighbourhood[&id]
    }

    pub fn all_blocks_in_neighbourhood(&self, id: NeighbourhoodID) -> Vec<BlockID> {
        let mut result = Vec::new();
        for (block, n) in &self.block_to_neighbourhood {
            if *n == id {
                result.push(*block);
            }
        }
        result
    }

    pub fn some_block_in_neighbourhood(&self, id: NeighbourhoodID) -> BlockID {
        for (block, neighbourhood) in &self.block_to_neighbourhood {
            if id == *neighbourhood {
                return *block;
            }
        }
        unreachable!("{:?} has no blocks", id);
    }

    /// Blocks on the "frontier" are adjacent to the perimeter, either just inside or outside.
    pub fn calculate_frontier(&self, perim: &Perimeter) -> BTreeSet<BlockID> {
        let perim_roads: BTreeSet<RoadID> = perim.roads.iter().map(|id| id.road).collect();

        let mut frontier = BTreeSet::new();
        for (block_id, block) in self.all_single_blocks() {
            for road_side_id in &block.perimeter.roads {
                // If the perimeter has this RoadSideID on the same side, we're just inside. If it has
                // the other side, just on the outside. Either way, on the frontier.
                if perim_roads.contains(&road_side_id.road) {
                    frontier.insert(block_id);
                    break;
                }
            }
        }
        frontier
    }

    // Possibly returns multiple merged blocks. The input is never "lost" -- if any perimeter fails
    // to become a block, fail the whole operation.
    fn make_merged_blocks(&self, map: &Map, input: Vec<BlockID>) -> Result<Vec<Block>> {
        let mut perimeters = Vec::new();
        for id in input {
            perimeters.push(self.get_block(id).perimeter.clone());
        }
        let mut blocks = Vec::new();
        let stepwise_debug = false;
        for perim in Perimeter::merge_all(
            map,
            perimeters,
            stepwise_debug,
            self.use_expensive_blockfinding,
        ) {
            blocks.push(perim.to_block(map)?);
        }
        Ok(blocks)
    }
}
