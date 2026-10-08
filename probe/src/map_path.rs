//! Read-only map geometry and bounded routes for manual movement.
use crate::{movement::Movement, Logger};
use mod_api_stable::{GameModeKindV1, StableJsonDoc, StableMapCustomizer};
use std::{cmp::Reverse, collections::BinaryHeap, sync::Arc};

pub struct MapObserver(pub Arc<Movement>, pub Arc<Logger>);
impl StableMapCustomizer for MapObserver {
    fn customize(&self, mode: Option<GameModeKindV1>, doc: &mut StableJsonDoc<'_>) {
        let grid = doc.get_json("walls").and_then(|s| Grid::from_json(&s));
        self.1.write(&format!(
            "NAVIGATION map mode={mode:?} walls_read={} grid=30x30; document unchanged",
            grid.is_some()
        ));
        self.0.set_navigation(grid);
    }
}
#[derive(Clone)]
pub struct Grid {
    blocked: Vec<bool>,
}
const SIDE: usize = 30;
const CELL: u64 = 32_000;
impl Grid {
    pub fn from_json(json: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(json).ok()?;
        let rows = value.as_array()?;
        if rows.len() != SIDE {
            return None;
        }
        let mut blocked = Vec::with_capacity(SIDE * SIDE);
        for row in rows {
            let row = row.as_array()?;
            if row.len() != SIDE {
                return None;
            }
            // Native navigation builder 0x192e91c and 0x192eb0c treats
            // exactly 1 as a wall. The outer index is y, inner index x.
            for cell in row {
                blocked.push(cell.as_bool().or_else(|| cell.as_u64().map(|n| n == 1))?);
            }
        }
        Some(Self { blocked })
    }
    fn cell(p: (u64, u64)) -> usize {
        (p.1 / CELL).min(29) as usize * SIDE + (p.0 / CELL).min(29) as usize
    }
    fn point(i: usize) -> (u64, u64) {
        (
            (i % SIDE) as u64 * CELL + CELL / 2,
            (i / SIDE) as u64 * CELL + CELL / 2,
        )
    }
    fn open(&self, x: i32, y: i32) -> bool {
        (0..30).contains(&x)
            && (0..30).contains(&y)
            && !self.blocked[y as usize * SIDE + x as usize]
    }
    fn nearest(&self, point: (u64, u64)) -> Option<usize> {
        (0..SIDE * SIDE)
            .filter(|i| !self.blocked[*i])
            .min_by_key(|i| {
                let p = Self::point(*i);
                u128::from(p.0.abs_diff(point.0)).pow(2) + u128::from(p.1.abs_diff(point.1)).pow(2)
            })
    }
    pub fn clear(&self, a: (u64, u64), b: (u64, u64)) -> bool {
        let n = (a.0.abs_diff(b.0).max(a.1.abs_diff(b.1)) / 2_000).max(1);
        let mut previous = Self::cell(a);
        (0..=n).all(|i| {
            let p = (
                a.0 as i64 + (b.0 as i64 - a.0 as i64) * i as i64 / n as i64,
                a.1 as i64 + (b.1 as i64 - a.1 as i64) * i as i64 / n as i64,
            );
            let next = Self::cell((p.0 as u64, p.1 as u64));
            let (px, py) = (previous % SIDE, previous / SIDE);
            let (nx, ny) = (next % SIDE, next / SIDE);
            let corners_clear = px == nx
                || py == ny
                || (!self.blocked[py * SIDE + nx] && !self.blocked[ny * SIDE + px]);
            previous = next;
            !self.blocked[next] && corners_clear
        })
    }
    pub fn route(&self, start: (u64, u64), goal: (u64, u64)) -> Option<Vec<(u64, u64)>> {
        if start.0 > 960_000 || start.1 > 960_000 || goal.0 > 960_000 || goal.1 > 960_000 {
            return None;
        }
        let end = if self.blocked[Self::cell(goal)] {
            Self::point(self.nearest(goal)?)
        } else {
            goal
        };
        if self.clear(start, end) {
            return Some(vec![end]);
        }
        let s = if self.blocked[Self::cell(start)] {
            self.nearest(start)?
        } else {
            Self::cell(start)
        };
        let g = Self::cell(end);
        let mut costs = vec![u32::MAX; SIDE * SIDE];
        let mut parent = vec![usize::MAX; SIDE * SIDE];
        let mut heap = BinaryHeap::new();
        costs[s] = 0;
        heap.push(Reverse((0, s)));
        while let Some(Reverse((cost, at))) = heap.pop() {
            if cost != costs[at] {
                continue;
            }
            if at == g {
                break;
            }
            let (x, y) = ((at % SIDE) as i32, (at / SIDE) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 || !self.open(x + dx, y + dy) {
                        continue;
                    }
                    if dx != 0 && dy != 0 && (!self.open(x + dx, y) || !self.open(x, y + dy)) {
                        continue;
                    }
                    let next = (y + dy) as usize * SIDE + (x + dx) as usize;
                    let c = cost + if dx != 0 && dy != 0 { 14 } else { 10 };
                    if c < costs[next] {
                        costs[next] = c;
                        parent[next] = at;
                        heap.push(Reverse((c, next)));
                    }
                }
            }
        }
        if costs[g] == u32::MAX {
            return None;
        }
        let mut path = vec![end];
        let mut at = g;
        while at != s {
            path.push(Self::point(at));
            at = parent[at];
        }
        path.reverse();
        let mut result = Vec::new();
        let mut from = start;
        let mut i = 0;
        while i < path.len() {
            let mut j = i;
            while j + 1 < path.len() && self.clear(from, path[j + 1]) {
                j += 1;
            }
            if !self.clear(from, path[j]) {
                return None;
            }
            result.push(path[j]);
            from = path[j];
            i = j + 1;
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_wall_document_uses_y_x_cells_and_native_one_wall_value() {
        let mut rows = vec![vec![0u64; 30]; 30];
        rows[3][2] = 1;
        rows[1][5] = 2;
        let g = Grid::from_json(&serde_json::to_string(&rows).unwrap()).unwrap();
        assert!(g.blocked[Grid::cell((80_000, 112_000))]);
        assert!(!g.blocked[Grid::cell((112_000, 80_000))]);
        assert!(!g.blocked[Grid::cell((176_000, 48_000))]);
        assert!(Grid::from_json("[[0]]").is_none());
        let mut corner = Grid {
            blocked: vec![false; 900],
        };
        corner.blocked[1] = true;
        assert!(!corner.clear((16_000, 16_000), (48_000, 48_000)));
    }
    #[test]
    fn routes_around_walls_without_cutting_corners_and_keeps_exact_clear_goal() {
        let mut g = Grid {
            blocked: vec![false; 900],
        };
        assert_eq!(
            g.route((16_000, 16_000), (112_000, 16_000)),
            Some(vec![(112_000, 16_000)])
        );
        for y in 0..4 {
            g.blocked[y * 30 + 2] = true;
        }
        let start = (16_000, 16_000);
        let end = (144_000, 16_000);
        let route = g.route(start, end).unwrap();
        assert!(route.len() > 1);
        assert_eq!(route.last(), Some(&end));
        let mut from = start;
        for p in route {
            assert!(g.clear(from, p));
            from = p;
        }
        for y in 0..30 {
            g.blocked[y * 30 + 2] = true;
        }
        assert!(g.route(start, end).is_none());
    }
}
