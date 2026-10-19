//! Dense 2D grid with row-major storage.

use serde::{Deserialize, Serialize};

/// A `width` by `height` grid of `T`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid<T> {
    width: u32,
    height: u32,
    cells: Vec<T>,
}

impl<T: Clone> Grid<T> {
    /// Make a grid where every cell is `fill`.
    pub fn new(width: u32, height: u32, fill: T) -> Self {
        let len = width as usize * height as usize;
        Self {
            width,
            height,
            cells: vec![fill; len],
        }
    }
}

impl<T> Grid<T> {
    /// Make a grid from row-major cells. Return `None` if the length is wrong.
    pub fn from_vec(width: u32, height: u32, cells: Vec<T>) -> Option<Self> {
        (cells.len() == width as usize * height as usize).then_some(Self {
            width,
            height,
            cells,
        })
    }

    /// Width in cells.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height in cells.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// True if `(x, y)` is inside the grid.
    pub fn in_bounds(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as u32) < self.width && (y as u32) < self.height
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        self.in_bounds(x, y)
            .then(|| y as usize * self.width as usize + x as usize)
    }

    /// Get a cell, or `None` outside the grid.
    pub fn get(&self, x: i32, y: i32) -> Option<&T> {
        self.index(x, y).map(|i| &self.cells[i])
    }

    /// Get a mutable cell, or `None` outside the grid.
    pub fn get_mut(&mut self, x: i32, y: i32) -> Option<&mut T> {
        self.index(x, y).map(|i| &mut self.cells[i])
    }

    /// Set a cell. Return false (and do nothing) outside the grid.
    pub fn set(&mut self, x: i32, y: i32, value: T) -> bool {
        match self.get_mut(x, y) {
            Some(cell) => {
                *cell = value;
                true
            }
            None => false,
        }
    }

    /// All cells in row-major order.
    pub fn as_slice(&self) -> &[T] {
        &self.cells
    }

    /// Iterate over `(x, y, &cell)` in row-major order.
    pub fn iter(&self) -> impl Iterator<Item = (i32, i32, &T)> {
        let w = self.width.max(1) as usize;
        self.cells
            .iter()
            .enumerate()
            .map(move |(i, c)| ((i % w) as i32, (i / w) as i32, c))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_set_and_bounds() {
        let mut g = Grid::new(3, 2, 0u8);
        assert!(g.set(2, 1, 9));
        assert_eq!(g.get(2, 1), Some(&9));
        assert_eq!(g.get(3, 0), None);
        assert_eq!(g.get(-1, 0), None);
        assert!(!g.set(0, 2, 1));
        assert_eq!(g.as_slice().len(), 6);
    }

    #[test]
    fn iteration_is_row_major() {
        let g = Grid::from_vec(2, 2, vec![1, 2, 3, 4]).unwrap();
        let v: Vec<_> = g.iter().map(|(x, y, c)| (x, y, *c)).collect();
        assert_eq!(v, vec![(0, 0, 1), (1, 0, 2), (0, 1, 3), (1, 1, 4)]);
    }

    #[test]
    fn from_vec_checks_length() {
        assert!(Grid::from_vec(2, 2, vec![1, 2, 3]).is_none());
    }

    #[test]
    fn empty_grid_is_safe() {
        let g = Grid::new(0, 0, 0u8);
        assert_eq!(g.get(0, 0), None);
        assert_eq!(g.iter().count(), 0);
    }
}
