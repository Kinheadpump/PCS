//! The bucket-queue A* must give the same result as the plain binary-heap A*
//! (order: f, then h, then tile index).
#![allow(clippy::unwrap_used, clippy::expect_used)]

use base::Pcg32;
use sim::PathFinder;
use sim::path::{COST_DIAGONAL, COST_STRAIGHT, DIRS, octile};
use sim::scenario::map_from_ascii;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Reference A*: one global binary heap. Returns (found, expanded, cost, path).
fn reference(
    pass: &[Vec<bool>],
    start: (usize, usize),
    goal: (usize, usize),
) -> (bool, u32, u32, Vec<(u16, u16)>) {
    let (h, w) = (pass.len() as i32, pass[0].len() as i32);
    let ok = |x: i32, y: i32| x >= 0 && y >= 0 && x < w && y < h && pass[y as usize][x as usize];
    let (sx, sy, gx, gy) = (start.0 as i32, start.1 as i32, goal.0 as i32, goal.1 as i32);
    if !ok(sx, sy) || !ok(gx, gy) {
        return (false, 0, 0, vec![]);
    }
    if (sx, sy) == (gx, gy) {
        return (true, 0, 0, vec![]);
    }
    let n = (w * h) as usize;
    let mut g = vec![u32::MAX; n];
    let mut parent = vec![0usize; n];
    let mut closed = vec![false; n];
    let mut open = BinaryHeap::new();
    let s = (sy * w + sx) as usize;
    let t = (gy * w + gx) as usize;
    g[s] = 0;
    parent[s] = s;
    let h0 = octile(sx, sy, gx, gy);
    open.push(Reverse((h0, h0, s)));
    let mut expanded = 0;
    while let Some(Reverse((_, _, i))) = open.pop() {
        if closed[i] {
            continue;
        }
        closed[i] = true;
        expanded += 1;
        if i == t {
            let mut out = vec![];
            let mut c = i;
            while c != s {
                out.push(((c as i32 % w) as u16, (c as i32 / w) as u16));
                c = parent[c];
            }
            return (true, expanded, g[i], out);
        }
        let (x, y) = (i as i32 % w, i as i32 / w);
        for (dx, dy) in DIRS {
            let (nx, ny) = (x + dx, y + dy);
            if !ok(nx, ny) || (dx != 0 && dy != 0 && !(ok(nx, y) && ok(x, ny))) {
                continue;
            }
            let nu = (ny * w + nx) as usize;
            if closed[nu] {
                continue;
            }
            let cost = if dx != 0 && dy != 0 {
                COST_DIAGONAL
            } else {
                COST_STRAIGHT
            };
            let ng = g[i] + cost;
            if ng < g[nu] {
                g[nu] = ng;
                parent[nu] = i;
                let hh = octile(nx, ny, gx, gy);
                open.push(Reverse((ng + hh, hh, nu)));
            }
        }
    }
    (false, expanded, 0, vec![])
}

#[test]
fn bucket_queue_matches_binary_heap_on_random_maps() {
    let mut rng = Pcg32::from_seed(77);
    let mut finder = PathFinder::default();
    let mut out = Vec::new();
    let mut found_count = 0;
    for _ in 0..300 {
        let w = 3 + rng.below(40) as usize;
        let h = 3 + rng.below(40) as usize;
        let density = 5 + rng.below(40);
        let pass: Vec<Vec<bool>> = (0..h)
            .map(|_| (0..w).map(|_| rng.below(100) >= density).collect())
            .collect();
        let text: String = pass
            .iter()
            .map(|r| {
                r.iter()
                    .map(|&p| if p { '.' } else { '#' })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        let (map, _) = map_from_ascii(&text).unwrap();
        for _ in 0..10 {
            let a = (rng.below(w as u32) as usize, rng.below(h as u32) as usize);
            let b = (rng.below(w as u32) as usize, rng.below(h as u32) as usize);
            let (found, expanded, cost, path) = reference(&pass, a, b);
            let r = finder.find(
                &map,
                (a.0 as u16, a.1 as u16),
                (b.0 as u16, b.1 as u16),
                &mut out,
            );
            assert_eq!(
                (r.found, r.expanded, r.cost),
                (found, expanded, cost),
                "{a:?}->{b:?}\n{text}"
            );
            assert_eq!(out, path, "{a:?}->{b:?}\n{text}");
            found_count += u32::from(found);
        }
    }
    assert!(
        found_count > 500,
        "test must cover found paths: {found_count}"
    );
}
