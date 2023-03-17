use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use abstio::MapName;
use abstutil::Timer;
use blockfinding::{Block, Perimeter};
use geom::Polygon;
use map_model::osm::RoadRank;
use map_model::{IntersectionID, Map, RoadID, RoadSideID};

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

    // TODO Possibly this never happens anymore and can go away
    pub broken: bool,

    #[serde(skip_serializing, skip_deserializing)]
    pub custom_boundaries: BTreeMap<NeighbourhoodID, CustomBoundary>,
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

#[derive(Clone)]
pub struct CustomBoundary {
    pub name: String,
    pub boundary_polygon: Polygon,
    pub borders: BTreeSet<IntersectionID>,
    pub interior_roads: BTreeSet<RoadID>,
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

            broken: false,
            custom_boundaries: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.neighbourhoods.is_empty()
    }

    pub fn seed_using_heuristics(map: &Map, timer: &mut Timer) -> Partitioning {
        timer.start("seed partitioning with heuristics");

        timer.start("find single blocks");
        let input_single_blocks = Perimeter::find_all_single_blocks(map);
        timer.stop("find single blocks");

        // Merge holes upfront. Otherwise, it's usually impossible to expand a boundary with a
        // block containing a hole. Plus, there's no known scenario when somebody would want to
        // make a neighbourhood boundary involve a hole.
        timer.start("merge holes");
        let input = Perimeter::merge_holes(map, input_single_blocks);
        timer.stop("merge holes");

        let mut single_blocks = Vec::new();
        let mut single_block_perims = Vec::new();
        timer.start_iter("fix deadends and blockify", input.len());
        for mut perim in input {
            timer.next();
            // TODO Some perimeters don't blockify after collapsing dead-ends. So do this
            // upfront, and separately work on any blocks that don't show up.
            // https://github.com/a-b-street/abstreet/issues/841
            perim.collapse_deadends();
            if let Ok(block) = perim.to_block(map) {
                single_block_perims.push(block.perimeter.clone());
                single_blocks.push(block);
            }
        }

        timer.start("partition");
        let partitions = Perimeter::partition_by_predicate(single_block_perims, |r| {
            // "Interior" roads of a neighbourhood aren't classified as arterial
            map.get_r(r).get_rank() == RoadRank::Local
        });

        let mut merged = Vec::new();
        for perimeters in partitions {
            // If we got more than one result back, merging partially failed. Oh well?
            let stepwise_debug = false;
            merged.extend(Perimeter::merge_all(map, perimeters, stepwise_debug));
        }
        timer.stop("partition");

        timer.start_iter("blockify merged", merged.len());
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
            broken: false,
            custom_boundaries: BTreeMap::new(),
        };

        // TODO We could probably build this up as we go
        for id in p.all_block_ids() {
            if let Some(neighbourhood) = p.neighbourhood_containing(id) {
                p.block_to_neighbourhood.insert(id, neighbourhood);
            } else {
                error!(
                        "Block doesn't belong to any neighbourhood. Continuing without boundary adjustment. {:?}",
                        p.get_block(id).perimeter
                    );
                p.broken = true;
            }
        }

        timer.stop("seed partitioning with heuristics");
        p
    }

    /// Add all specified blocks to new_owner. `Ok(None)` is success.  `Ok(Some(x))` is also
    /// success, but means the old neighbourhood of SOME block in `add_all` is now gone and
    /// replaced with something new. (This call shouldn't be used to remove multiple blocks at
    /// once, since interpreting the result is confusing!)
    pub fn transfer_blocks(
        &mut self,
        map: &Map,
        add_all: Vec<BlockID>,
        new_owner: NeighbourhoodID,
    ) -> Result<Option<NeighbourhoodID>> {
        // Is the newly expanded neighbourhood a valid perimeter?
        let mut new_owner_blocks = self.neighbourhood_to_blocks(new_owner);
        new_owner_blocks.extend(add_all.clone());
        let mut new_neighbourhood_blocks = self.make_merged_blocks(map, new_owner_blocks)?;
        if new_neighbourhood_blocks.len() != 1 {
            // This happens when a hole would be created by adding this block. There are probably
            // some smaller blocks nearby to add first.
            bail!("Couldn't add block -- you may need to add an intermediate block first to avoid a hole, or there's a bug you can't workaround yet. Try adding pink blocks first.");
        }
        let new_neighbourhood_block = new_neighbourhood_blocks.pop().unwrap();

        let old_owners: BTreeSet<NeighbourhoodID> = add_all
            .iter()
            .map(|block| self.block_to_neighbourhood[block])
            .collect();
        // Are each of the old neighbourhoods, minus any new blocks, still valid?
        let mut return_value = None;
        for old_owner in old_owners {
            let mut old_owner_blocks = self.neighbourhood_to_blocks(old_owner);
            for x in &add_all {
                old_owner_blocks.remove(x);
            }
            if old_owner_blocks.is_empty() {
                self.neighbourhoods.remove(&old_owner).unwrap();
                return_value = Some(new_owner);
                continue;
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
        }

        // Set up the newly expanded neighbourhood
        self.neighbourhoods.get_mut(&new_owner).unwrap().block = new_neighbourhood_block;
        for id in add_all {
            self.block_to_neighbourhood.insert(id, new_owner);
        }
        Ok(return_value)
    }

    /// Needs to find an existing neighbourhood to take the block, or make a new one
    pub fn remove_block_from_neighbourhood(
        &mut self,
        map: &Map,
        id: BlockID,
    ) -> Result<Option<NeighbourhoodID>> {
        let old_owner = self.block_to_neighbourhood(id);
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
                return self.transfer_blocks(map, vec![id], *new_owner);
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
        let result = self.transfer_blocks(map, vec![id], new_owner);
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
        // TODO Could consider using the boundary_polygon calculated by Neighbourhood
        let area = if let Some(ref custom) = self.custom_boundaries.get(&id) {
            custom.boundary_polygon.area()
        } else {
            self.neighbourhood_block(id).polygon.area()
        };

        // Convert from m^2 to km^2
        let area = area / 1_000_000.0;
        format!("~{:.1} km²", area)
    }

    pub fn get_info(&self, id: NeighbourhoodID) -> &NeighbourhoodInfo {
        &self.neighbourhoods[&id]
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

    pub fn add_custom_boundary(&mut self, custom: CustomBoundary) -> NeighbourhoodID {
        let id = NeighbourhoodID(self.neighbourhood_id_counter);
        self.neighbourhood_id_counter += 1;
        self.custom_boundaries.insert(id, custom);
        id
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

    pub fn neighbourhood_to_blocks(&self, id: NeighbourhoodID) -> BTreeSet<BlockID> {
        let mut result = BTreeSet::new();
        for (block, n) in &self.block_to_neighbourhood {
            if *n == id {
                result.insert(*block);
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

    fn adjacent_blocks(&self, id: BlockID) -> BTreeSet<BlockID> {
        let mut blocks = self.calculate_frontier(&self.get_block(id).perimeter);
        blocks.retain(|x| *x != id);
        blocks
    }

    // Possibly returns multiple merged blocks. The input is never "lost" -- if any perimeter fails
    // to become a block, fail the whole operation.
    fn make_merged_blocks(&self, map: &Map, input: BTreeSet<BlockID>) -> Result<Vec<Block>> {
        let mut perimeters = Vec::new();
        for id in input {
            perimeters.push(self.get_block(id).perimeter.clone());
        }
        let mut blocks = Vec::new();
        let stepwise_debug = false;
        for perim in Perimeter::merge_all(map, perimeters, stepwise_debug) {
            blocks.push(perim.to_block(map)?);
        }
        Ok(blocks)
    }

    /// We want to add target_block to new_owner, but we can't. Find the blocks we may need to add
    /// first.
    pub fn find_intermediate_blocks(
        &self,
        new_owner: NeighbourhoodID,
        target_block: BlockID,
    ) -> Vec<BlockID> {
        let adjacent_to_target_block = self.adjacent_blocks(target_block);
        let _new_owner_frontier =
            self.calculate_frontier(&self.neighbourhood_block(new_owner).perimeter);

        let mut result = Vec::new();
        for id in adjacent_to_target_block.clone() {
            // A block already part of new_owner never makes sense
            if self.block_to_neighbourhood[&id] == new_owner {
                continue;
            }

            // TODO: intersect the two above -- aka, look for blocks adjacent both to target_block
            // and new_owner. But this seems too eager and maybe covered by the below condition.
            /*if self.block_to_neighbourhood(id) != new_owner && new_owner_frontier.contains(&id) {
                result.push(id);
                continue;
            }*/

            // Look for holes, totally surrounded by other holes or target_block.
            if self
                .adjacent_blocks(id)
                .into_iter()
                .all(|x| x == target_block || adjacent_to_target_block.contains(&x))
            {
                result.push(id);
            }
        }

        result
    }
}
