//! `tools dump-save <file>`: print a save as RON.

use sim::Sim;

/// Turn the bytes of a save into RON text. Errors are text, never panics.
pub fn dump_bytes(bytes: &[u8]) -> Result<String, String> {
    let sim = sim::save::from_bytes(bytes).map_err(|e| e.to_string())?;
    dump_sim(&sim)
}

/// Write a state as pretty RON.
pub fn dump_sim(sim: &Sim) -> Result<String, String> {
    let config = ron::ser::PrettyConfig::default();
    ron::ser::to_string_pretty(sim, config).map_err(|e| format!("cannot write RON: {e}"))
}

/// Run the subcommand with its arguments.
pub fn run(args: &[String]) -> Result<String, String> {
    let [path] = args else {
        return Err("usage: tools dump-save <file>".to_owned());
    };
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    dump_bytes(&bytes).map_err(|e| format!("{path}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldgen::{EmbarkSummary, generate_local};

    #[test]
    fn save_round_trips_through_dump() {
        let data = generate_local(&EmbarkSummary::temperate_forest());
        let sim = Sim::new(&data, 1, 0);
        let bytes = sim::save::to_bytes(&sim);
        let text = dump_bytes(&bytes).unwrap_or_default();
        assert!(text.contains("colonists"), "dump has the colonists");
        let loaded = sim::save::from_bytes(&bytes).map(|s| s.state_hash());
        assert_eq!(loaded, Ok(sim.state_hash()));
        // The RON text reads back to the same state.
        let back: Result<Sim, _> = ron::from_str(&text);
        assert_eq!(back.map(|s| s.state_hash()).ok(), Some(sim.state_hash()));
    }

    #[test]
    fn bad_input_gives_errors() {
        assert!(dump_bytes(b"").is_err());
        assert!(dump_bytes(b"NOTASAVEFILE_NOTASAVEFILE_NOTASAVE").is_err());
        assert!(run(&[]).is_err());
        assert!(run(&["does/not/exist.sav".to_owned()]).is_err());
    }
}
