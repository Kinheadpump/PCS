//! Step 12: land regions, the region graph with travel costs, and the deposit repair.
//!
//! Voronoi seeds (one per block, picked by a stateless hash) grow by a cost-ordered flood
//! fill (Dijkstra with ties broken by cell index). Water never joins a region. Ridges, rivers,
//! and slopes make a step costly, so region borders follow them. Then pieces are merged into a
//! neighbor, and regions below the size minimum are merged into the neighbor with the longest
//! shared border. The repair adds a deposit to each region that has none. It writes an
//! overlay and never changes the step 11 output.

use crate::biome::WorldBiome;
use crate::deposit::{Deposits, Mineral, affinity};
use crate::geology::{Geology, roll};
use crate::world::{WORLD_SIZE, WorldParams, world_grid};
use base::Grid;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap};

/// Value of a cell in the working grid that has no region.
const NONE: u32 = u32::MAX;
/// Offsets of the 4 neighbors.
const DIRS: [(i32, i32); 4] = [(0, -1), (-1, 0), (1, 0), (0, 1)];

/// One land region. IDs run from 1. ID 0 means no region in the grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionInfo {
    /// Number of cells.
    pub size: u32,
    /// Centroid x (cell units).
    pub cx: u16,
    /// Centroid y (cell units).
    pub cy: u16,
    /// Bit mask of the minerals in the region (bit = Mineral index), with the repair.
    pub minerals: u8,
}

/// An edge of the region graph between two touching regions. Field a is less than field b.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionEdge {
    /// Region with the lower ID.
    pub a: u16,
    /// Region with the higher ID.
    pub b: u16,
    /// Number of cell pairs along the shared border.
    pub border: u32,
    /// Travel cost between the region centers. Cost 10 is one flat cell on easy ground.
    pub cost: u32,
}

/// A link from one region to a neighbor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionLink {
    /// ID of the neighbor.
    pub to: u16,
    /// Travel cost of the edge.
    pub cost: u32,
}

/// Output of step 12.
#[derive(Clone, Debug)]
pub struct Regions {
    /// Region ID of each cell (0 for water).
    pub id: Grid<u16>,
    /// Region data, indexed by ID minus 1.
    pub info: Vec<RegionInfo>,
    /// Graph edges sorted by (a, b).
    pub edges: Vec<RegionEdge>,
    /// Links of each region (indexed by ID minus 1), sorted by neighbor ID.
    pub links: Vec<Vec<RegionLink>>,
    /// Deposits with the repair: step 11 deposits plus one deposit for each region that had none.
    pub deposits: Deposits,
    /// Cells that the repair filled, ascending.
    pub repaired: Vec<u32>,
}

impl Regions {
    /// Number of regions.
    pub fn count(&self) -> usize {
        self.info.len()
    }

    /// Region ID of a cell (0 for water or outside).
    pub fn at(&self, x: i32, y: i32) -> u16 {
        self.id.get(x, y).copied().unwrap_or(0)
    }

    /// Data of a region by ID.
    pub fn region(&self, id: u16) -> Option<&RegionInfo> {
        usize::from(id)
            .checked_sub(1)
            .and_then(|i| self.info.get(i))
    }

    /// Links of a region by ID.
    pub fn neighbors(&self, id: u16) -> &[RegionLink] {
        match usize::from(id)
            .checked_sub(1)
            .and_then(|i| self.links.get(i))
        {
            Some(v) => v,
            None => &[],
        }
    }
}

/// Inputs of step 12: outputs of earlier stages only.
pub struct RegionInputs<'a> {
    /// Height.
    pub height: &'a Grid<i16>,
    /// Biome.
    pub biome: &'a Grid<WorldBiome>,
    /// River size.
    pub river: &'a Grid<u8>,
    /// Geology.
    pub geology: &'a Geology,
    /// Step 11 deposits.
    pub deposits: &'a Deposits,
}

fn is_land(b: WorldBiome) -> bool {
    b != WorldBiome::Ocean && b != WorldBiome::Lake
}

/// Cost of one step over a biome, 10 is easy.
fn biome_cost(b: WorldBiome) -> u32 {
    match b {
        WorldBiome::Ocean | WorldBiome::Lake | WorldBiome::HighMountain => 40,
        WorldBiome::Glacier | WorldBiome::Volcanic => 25,
        WorldBiome::Coast | WorldBiome::Grassland => 8,
        WorldBiome::Steppe => 9,
        WorldBiome::Floodplain | WorldBiome::Savanna => 10,
        WorldBiome::Shrubland => 11,
        WorldBiome::Tundra | WorldBiome::ColdDesert | WorldBiome::TemperateForest => 14,
        WorldBiome::Taiga | WorldBiome::Desert => 16,
        WorldBiome::TropicalForest => 20,
        WorldBiome::Rainforest => 26,
        WorldBiome::Marsh => 28,
    }
}

fn height_diff(inp: &RegionInputs, a: usize, b: usize) -> u32 {
    let h = inp.height.as_slice();
    (i32::from(h[a]) - i32::from(h[b])).unsigned_abs()
}

/// The seed cells. In each block the land cell with the lowest roll.
fn pick_seeds(seed: u64, inp: &RegionInputs, params: &WorldParams) -> Vec<u32> {
    let size = WORLD_SIZE as usize;
    let block = params.region_cell.max(2) as usize;
    let blocks = size.div_ceil(block);
    let mut best: Vec<Option<(u64, u32)>> = vec![None; blocks * blocks];
    for (i, &b) in inp.biome.as_slice().iter().enumerate() {
        if !is_land(b) {
            continue;
        }
        let (x, y) = (i % size, i / size);
        let slot = (y / block) * blocks + x / block;
        let r = roll(seed, i as u64, 1);
        if best[slot].is_none_or(|(br, _)| r < br) {
            best[slot] = Some((r, i as u32));
        }
    }
    let mut seeds: Vec<u32> = best.into_iter().flatten().map(|(_, c)| c).collect();
    seeds.sort_unstable();
    seeds
}

/// The 4 neighbors of a cell inside the world.
fn neighbors(cell: usize) -> impl Iterator<Item = usize> {
    let size = WORLD_SIZE as i32;
    let x = (cell % WORLD_SIZE as usize) as i32;
    let y = (cell / WORLD_SIZE as usize) as i32;
    DIRS.into_iter().filter_map(move |(dx, dy)| {
        let (nx, ny) = (x + dx, y + dy);
        (nx >= 0 && ny >= 0 && nx < size && ny < size).then(|| (ny * size + nx) as usize)
    })
}

/// Cost-ordered flood fill from the seeds. Water is a wall. Unreached land stays NONE.
fn flood_fill(seeds: &[u32], inp: &RegionInputs, params: &WorldParams) -> Vec<u32> {
    let cells = (WORLD_SIZE * WORLD_SIZE) as usize;
    let mut region = vec![NONE; cells];
    let mut dist = vec![u32::MAX; cells];
    let mut heap = BinaryHeap::new();
    for (r, &c) in seeds.iter().enumerate() {
        dist[c as usize] = 0;
        region[c as usize] = r as u32;
        heap.push(Reverse((0u32, c, r as u32)));
    }
    let biome = inp.biome.as_slice();
    let river = inp.river.as_slice();
    while let Some(Reverse((d, c, r))) = heap.pop() {
        let c = c as usize;
        if d != dist[c] || region[c] != r {
            continue;
        }
        for n in neighbors(c) {
            if !is_land(biome[n]) {
                continue;
            }
            let mut step = 10 + height_diff(inp, c, n) * params.region_slope_cost / 16;
            if biome[n] == WorldBiome::HighMountain {
                step += params.region_ridge_cost;
            }
            step += u32::from(river[n]) * params.region_river_cost;
            let nd = d + step;
            if nd < dist[n] {
                dist[n] = nd;
                region[n] = r;
                heap.push(Reverse((nd, n as u32, r)));
            }
        }
    }
    region
}

/// Give each land area without a seed its own region ID (areas are found in cell order).
fn label_unreached(region: &mut [u32], biome: &[WorldBiome], mut next: u32) -> u32 {
    for start in 0..region.len() {
        if region[start] != NONE || !is_land(biome[start]) {
            continue;
        }
        region[start] = next;
        let mut stack = vec![start];
        while let Some(c) = stack.pop() {
            for n in neighbors(c) {
                if region[n] == NONE && is_land(biome[n]) {
                    region[n] = next;
                    stack.push(n);
                }
            }
        }
        next += 1;
    }
    next
}

/// Label the 4-connected pieces of each region. Returns (piece of each cell, piece sizes).
fn pieces(region: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut piece = vec![NONE; region.len()];
    let mut sizes = Vec::new();
    for start in 0..region.len() {
        if region[start] == NONE || piece[start] != NONE {
            continue;
        }
        let id = sizes.len() as u32;
        piece[start] = id;
        let mut count = 0u32;
        let mut stack = vec![start];
        while let Some(c) = stack.pop() {
            count += 1;
            for n in neighbors(c) {
                if piece[n] == NONE && region[n] == region[start] {
                    piece[n] = id;
                    stack.push(n);
                }
            }
        }
        sizes.push(count);
    }
    (piece, sizes)
}

/// Merge each piece that is not the largest piece of its region into a neighboring region.
/// The neighbor is the one next to the lowest cell index of the piece that has one.
fn merge_pieces(region: &mut [u32]) {
    let (piece, sizes) = pieces(region);
    let mut main: BTreeMap<u32, u32> = BTreeMap::new();
    for (i, &p) in piece.iter().enumerate() {
        if p == NONE {
            continue;
        }
        let m = main.entry(region[i]).or_insert(p);
        if sizes[p as usize] > sizes[*m as usize] {
            *m = p;
        }
    }
    let mut target: BTreeMap<u32, u32> = BTreeMap::new();
    for (i, &p) in piece.iter().enumerate() {
        if p == NONE || main.get(&region[i]) == Some(&p) || target.contains_key(&p) {
            continue;
        }
        if let Some(n) = neighbors(i).find(|&n| region[n] != NONE && region[n] != region[i]) {
            target.insert(p, region[n]);
        }
    }
    for (i, &p) in piece.iter().enumerate() {
        if let Some(t) = target.get(&p) {
            region[i] = *t;
        }
    }
}

/// Merge each region below the size minimum into the neighbor with the longest shared border
/// (ties: lowest ID). Smallest regions go first (ties: lowest ID).
fn merge_small(region: &mut [u32], count: usize, min_cells: u32) {
    let size = WORLD_SIZE as usize;
    let mut sizes = vec![0u32; count];
    let mut adj: Vec<BTreeMap<u32, u32>> = vec![BTreeMap::new(); count];
    for i in 0..region.len() {
        if region[i] == NONE {
            continue;
        }
        sizes[region[i] as usize] += 1;
        let right = (i % size + 1 < size).then_some(i + 1);
        let down = (i + size < region.len()).then_some(i + size);
        for n in [right, down].into_iter().flatten() {
            if region[n] != NONE && region[n] != region[i] {
                *adj[region[i] as usize].entry(region[n]).or_insert(0) += 1;
                *adj[region[n] as usize].entry(region[i]).or_insert(0) += 1;
            }
        }
    }
    let mut parent: Vec<u32> = (0..count as u32).collect();
    let mut alive: Vec<bool> = sizes.iter().map(|&s| s > 0).collect();
    loop {
        let small = (0..count)
            .filter(|&r| alive[r] && sizes[r] < min_cells && !adj[r].is_empty())
            .min_by_key(|&r| (sizes[r], r));
        let Some(r) = small else { break };
        let mut to = NONE;
        let mut best = 0u32;
        for (&n, &c) in &adj[r] {
            if c > best {
                best = c;
                to = n;
            }
        }
        let to_i = to as usize;
        let links = std::mem::take(&mut adj[r]);
        for (n, c) in links {
            adj[n as usize].remove(&(r as u32));
            if n != to {
                *adj[n as usize].entry(to).or_insert(0) += c;
                *adj[to_i].entry(n).or_insert(0) += c;
            }
        }
        sizes[to_i] += sizes[r];
        sizes[r] = 0;
        alive[r] = false;
        parent[r] = to;
    }
    for cell in region.iter_mut() {
        if *cell == NONE {
            continue;
        }
        let mut r = *cell;
        while parent[r as usize] != r {
            r = parent[r as usize];
        }
        *cell = r;
    }
}

/// Renumber the regions 1.. in order of their lowest cell index.
fn compact(region: &[u32]) -> (Vec<u16>, usize) {
    let mut map: BTreeMap<u32, u16> = BTreeMap::new();
    let mut out = vec![0u16; region.len()];
    for (i, &r) in region.iter().enumerate() {
        if r == NONE {
            continue;
        }
        let next = (map.len() + 1) as u16;
        out[i] = *map.entry(r).or_insert(next);
    }
    (out, map.len())
}

/// Region data from the final grid.
fn region_info(id: &[u16], count: usize) -> Vec<RegionInfo> {
    let size = WORLD_SIZE as usize;
    let mut sum = vec![(0u64, 0u64, 0u32); count];
    for (i, &r) in id.iter().enumerate() {
        if r != 0 {
            let s = &mut sum[usize::from(r) - 1];
            s.0 += (i % size) as u64;
            s.1 += (i / size) as u64;
            s.2 += 1;
        }
    }
    sum.into_iter()
        .map(|(x, y, n)| RegionInfo {
            size: n,
            cx: (x / u64::from(n.max(1))) as u16,
            cy: (y / u64::from(n.max(1))) as u16,
            minerals: 0,
        })
        .collect()
}

type Graph = (Vec<RegionEdge>, Vec<Vec<RegionLink>>);

/// The graph: one edge for each pair of touching regions.
fn build_graph(id: &[u16], info: &[RegionInfo], inp: &RegionInputs, params: &WorldParams) -> Graph {
    let size = WORLD_SIZE as usize;
    let biome = inp.biome.as_slice();
    let river = inp.river.as_slice();
    let mut acc: BTreeMap<(u16, u16), (u32, u64)> = BTreeMap::new();
    for i in 0..id.len() {
        let right = (i % size + 1 < size).then_some(i + 1);
        let down = (i + size < id.len()).then_some(i + size);
        for n in [right, down].into_iter().flatten() {
            if id[i] == 0 || id[n] == 0 || id[i] == id[n] {
                continue;
            }
            let key = (id[i].min(id[n]), id[i].max(id[n]));
            let ground = (biome_cost(biome[i]) + biome_cost(biome[n])) / 2;
            let climb = height_diff(inp, i, n) * params.region_slope_cost / 16;
            let ford = u32::from(river[i].max(river[n])) * params.region_river_cost;
            let e = acc.entry(key).or_insert((0, 0));
            e.0 += 1;
            e.1 += u64::from(ground + climb + ford);
        }
    }
    let mut edges = Vec::with_capacity(acc.len());
    let mut links = vec![Vec::new(); info.len()];
    for ((a, b), (border, total)) in acc {
        let (ia, ib) = (&info[usize::from(a) - 1], &info[usize::from(b) - 1]);
        let dist = u64::from(ia.cx.abs_diff(ib.cx)) + u64::from(ia.cy.abs_diff(ib.cy));
        let mean = total / u64::from(border);
        let cost = ((dist.max(1) * mean) / 10).max(1) as u32;
        edges.push(RegionEdge { a, b, border, cost });
        links[usize::from(a) - 1].push(RegionLink { to: b, cost });
        links[usize::from(b) - 1].push(RegionLink { to: a, cost });
    }
    for l in &mut links {
        l.sort_by_key(|k| k.to);
    }
    (edges, links)
}

/// Repair: each region without a deposit gets one on the cell with the best host rock.
/// The best spot has the highest affinity of a mineral to a rock layer. Ties go to the lowest
/// cell index, then the lowest layer, then the lowest mineral.
fn repair(id: &[u16], info: &mut [RegionInfo], inp: &RegionInputs) -> (Deposits, Vec<u32>) {
    let geo = inp.geology;
    let mut mineral = inp.deposits.mineral.as_slice().to_vec();
    let mut richness = inp.deposits.richness.as_slice().to_vec();
    let mut host = inp.deposits.host.as_slice().to_vec();
    for (i, &r) in id.iter().enumerate() {
        if r != 0 && mineral[i] != 0 {
            info[usize::from(r) - 1].minerals |= 1 << (mineral[i] - 1);
        }
    }
    // (affinity, cell, mineral, layer) of the best spot of each region.
    let mut best: Vec<Option<(u8, u32, u8, u8)>> = vec![None; info.len()];
    for (i, &r) in id.iter().enumerate() {
        if r == 0 || info[usize::from(r) - 1].minerals != 0 {
            continue;
        }
        let p = usize::from(geo.province.as_slice()[i]);
        let rocks = &geo.layers[p][..usize::from(geo.layer_count[p])];
        let slot = &mut best[usize::from(r) - 1];
        for (layer, &rock) in rocks.iter().enumerate() {
            for m in Mineral::ALL {
                let a = affinity(rock, m);
                if a > 0 && slot.is_none_or(|(b, ..)| a > b) {
                    *slot = Some((a, i as u32, m as u8, layer as u8));
                }
            }
        }
    }
    let mut repaired = Vec::new();
    for (k, b) in best.into_iter().enumerate() {
        if let Some((_, cell, m, layer)) = b {
            let c = cell as usize;
            mineral[c] = m + 1;
            richness[c] = 1;
            host[c] = layer;
            info[k].minerals |= 1 << m;
            repaired.push(cell);
        }
    }
    repaired.sort_unstable();
    let deposits = Deposits {
        mineral: world_grid(mineral, 0),
        richness: world_grid(richness, 0),
        host: world_grid(host, 0),
    };
    (deposits, repaired)
}

/// Make the regions (step 12).
pub fn make_regions(seed: u64, inp: &RegionInputs, params: &WorldParams) -> Regions {
    let biome = inp.biome.as_slice();
    let seeds = pick_seeds(seed, inp, params);
    let mut region = flood_fill(&seeds, inp, params);
    let count = label_unreached(&mut region, biome, seeds.len() as u32) as usize;
    merge_pieces(&mut region);
    merge_small(&mut region, count, params.region_min_cells);
    let (id, n) = compact(&region);
    let mut info = region_info(&id, n);
    let (edges, links) = build_graph(&id, &info, inp, params);
    let (deposits, repaired) = repair(&id, &mut info, inp);
    Regions {
        id: world_grid(id, 0),
        info,
        edges,
        links,
        deposits,
        repaired,
    }
}
