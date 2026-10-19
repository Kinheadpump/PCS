//! World map, history, and local map generation (integer math only).

pub mod biome;
pub mod climate;
pub mod embark;
pub mod height;
pub mod hydro;
pub mod local;
pub mod noise;
pub mod plates;
pub mod world;

pub use biome::WorldBiome;
pub use embark::{Biome, EmbarkSummary, LOCAL_SIZE};
pub use local::{Ground, LocalMapData, Ore, RockKind, generate_local};
pub use world::{STAGE_COUNT, Stage, WORLD_SIZE, WorldData, WorldParams, generate_world};
