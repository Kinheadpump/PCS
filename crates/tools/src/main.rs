//! Headless command-line tools.

use base::Grid;
use base::hash::Fnv1a;
use base::rng::Pcg32;
use std::process::ExitCode;

const HASH_GRID_SIZE: u32 = 64;

/// Make a grid of random values from a seed and return its stable hash.
fn grid_hash(seed: u64) -> u64 {
    let mut rng = Pcg32::from_seed(seed);
    let cells = (0..HASH_GRID_SIZE * HASH_GRID_SIZE)
        .map(|_| rng.next_u32())
        .collect();
    let Some(grid) = Grid::from_vec(HASH_GRID_SIZE, HASH_GRID_SIZE, cells) else {
        return 0;
    };
    let mut h = Fnv1a::new();
    h.write_u32(grid.width());
    h.write_u32(grid.height());
    for &v in grid.as_slice() {
        h.write_u32(v);
    }
    h.finish()
}

fn parse_seed(args: &[String]) -> Result<u64, String> {
    match args {
        [flag, value] if flag == "--seed" => value
            .parse::<u64>()
            .map_err(|e| format!("bad seed {value:?}: {e}")),
        _ => Err("usage: tools hash --seed <number>".to_owned()),
    }
}

fn run(args: &[String]) -> Result<String, String> {
    match args.split_first() {
        Some((cmd, rest)) if cmd == "hash" => Ok(format!("{:016x}", grid_hash(parse_seed(rest)?))),
        _ => Err("usage: tools hash --seed <number>".to_owned()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_hash_is_repeatable() {
        assert_eq!(grid_hash(1), grid_hash(1));
        assert_ne!(grid_hash(1), grid_hash(2));
    }

    #[test]
    fn grid_hash_seed_1_known_answer() {
        assert_eq!(grid_hash(1), 0xf751_a61d_a3d8_c141);
    }

    #[test]
    fn bad_arguments_return_errors() {
        assert!(run(&[]).is_err());
        assert!(run(&["hash".into()]).is_err());
        assert!(run(&["hash".into(), "--seed".into(), "x".into()]).is_err());
    }
}
