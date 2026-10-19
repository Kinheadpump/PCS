//! World map, history, and local map generation (integer math only).

pub mod embark;
pub mod local;
pub mod noise;

pub use embark::{Biome, EmbarkSummary, LOCAL_SIZE};
pub use local::{Ground, LocalMapData, Ore, RockKind, generate_local};
