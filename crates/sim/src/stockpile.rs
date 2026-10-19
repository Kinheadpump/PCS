//! Stockpile zones: rectangles of tiles where colonists store items.

use base::Id;
use serde::{Deserialize, Serialize};

/// Typed ID of a stockpile.
pub type StockpileId = Id<Stockpile>;

/// Which materials a stockpile accepts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemFilter {
    /// Every material.
    All,
    /// Only these material indices (sorted, no duplicates).
    Only(Vec<u16>),
}

impl ItemFilter {
    /// A filter for a list of materials. It sorts the list and removes duplicates.
    pub fn only(mut materials: Vec<u16>) -> Self {
        materials.sort_unstable();
        materials.dedup();
        ItemFilter::Only(materials)
    }

    /// The filter with a sorted list and no duplicates.
    pub fn normalized(&self) -> Self {
        match self {
            ItemFilter::All => ItemFilter::All,
            ItemFilter::Only(list) => ItemFilter::only(list.clone()),
        }
    }

    /// True if the filter accepts the material.
    pub fn accepts(&self, material: u16) -> bool {
        match self {
            ItemFilter::All => true,
            ItemFilter::Only(list) => list.binary_search(&material).is_ok(),
        }
    }
}

/// A stockpile zone. The corners are inclusive: `x0 <= x1` and `y0 <= y1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stockpile {
    /// Left column.
    pub x0: u16,
    /// Top row.
    pub y0: u16,
    /// Right column.
    pub x1: u16,
    /// Bottom row.
    pub y1: u16,
    /// Accepted materials.
    pub filter: ItemFilter,
}

impl Stockpile {
    /// True if the tile is inside the zone.
    pub fn contains(&self, x: u16, y: u16) -> bool {
        (self.x0..=self.x1).contains(&x) && (self.y0..=self.y1).contains(&y)
    }

    /// True if the two zones share a tile.
    pub fn overlaps(&self, other: &Stockpile) -> bool {
        self.x0 <= other.x1 && other.x0 <= self.x1 && self.y0 <= other.y1 && other.y0 <= self.y1
    }

    /// Number of tiles in the zone.
    pub fn area(&self) -> u32 {
        (u32::from(self.x1) - u32::from(self.x0) + 1)
            * (u32::from(self.y1) - u32::from(self.y0) + 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_accepts_listed_materials() {
        let f = ItemFilter::only(vec![5, 2, 5]);
        assert_eq!(f, ItemFilter::Only(vec![2, 5]));
        assert!(f.accepts(2) && f.accepts(5) && !f.accepts(3));
        assert!(ItemFilter::All.accepts(99));
    }

    #[test]
    fn zones_overlap_and_contain() {
        let a = Stockpile {
            x0: 0,
            y0: 0,
            x1: 2,
            y1: 2,
            filter: ItemFilter::All,
        };
        let b = Stockpile {
            x0: 2,
            y0: 2,
            x1: 4,
            y1: 4,
            filter: ItemFilter::All,
        };
        let c = Stockpile {
            x0: 3,
            y0: 0,
            x1: 4,
            y1: 1,
            filter: ItemFilter::All,
        };
        assert!(a.overlaps(&b) && !a.overlaps(&c));
        assert!(a.contains(2, 2) && !a.contains(3, 2));
        assert_eq!(a.area(), 9);
    }
}
