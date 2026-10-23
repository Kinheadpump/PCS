//! World map, history, and local map generation (integer math only).

pub mod biome;
pub mod chunk;
pub mod climate;
pub mod deposit;
pub mod embark;
pub mod geology;
pub mod height;
pub mod history;
pub mod hydro;
pub mod legend;
pub mod local;
pub mod noise;
pub mod plates;
pub mod region;
pub mod stamp;
pub mod world;

pub use biome::WorldBiome;
pub use deposit::{Deposits, Mineral};
pub use embark::{
    Biome, EmbarkRival, EmbarkSite, EmbarkSites, EmbarkSummary, LOCAL_SIZE,
    check_content as check_embark_content, embark_from_world, is_habitable,
};
pub use geology::{Geology, Rock, Setting};
pub use history::{
    Event, EventKind, History, check_content as check_history_content, generate_history,
};
pub use legend::{Legend, LegendKind, Reliability, make_legends};
pub use local::{Ground, LocalMapData, Ore, RockKind, generate_local, generate_local_with_sites};
pub use region::{RegionEdge, RegionInfo, RegionLink, Regions};
pub use stamp::{PlacedSite, SiteTile};
pub use world::{
    LAYER_STAGE_COUNT, LayerStage, REGION_STAGE_COUNT, RegionStage, STAGE_COUNT, Stage, WORLD_SIZE,
    WorldData, WorldParams, generate_world, try_generate_world,
};
