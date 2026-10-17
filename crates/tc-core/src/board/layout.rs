//! The grid a board's sections sit on: 12 columns, rows of a fixed height. Moving or resizing a section puts it where it was
//! dropped and pushes what it overlaps down; then everything floats up to the first free row, so the grid never has holes
//! above a section. On a narrow screen the sections are stacked in reading order instead (`stack`).

use serde::{Deserialize, Serialize};

/// The columns of the grid.
pub const COLUMNS: u32 = 12;

/// The tallest a section may be, in rows.
pub const MAX_ROWS: u32 = 40;

/// A section's place: `x` and `w` in columns, `y` and `h` in rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// A section's id and place: all the layout needs to know of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub id: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Cell {
    pub fn new(id: &str, r: Rect) -> Cell {
        Cell {
            id: id.to_owned(),
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
        }
    }

    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
        }
    }
}

/// The same place made to fit the grid: at least one column and row, no wider than 12, not past the right edge.
pub fn clamp(r: Rect) -> Rect {
    let w = r.w.clamp(1, COLUMNS);
    Rect {
        x: r.x.min(COLUMNS - w),
        y: r.y,
        w,
        h: r.h.clamp(1, MAX_ROWS),
    }
}

fn overlap(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// The row `r` floats up to among the cells already `placed`.
fn float_up(mut r: Rect, placed: &[Cell]) -> Rect {
    r.y = 0;
    while placed.iter().any(|p| overlap(&r, &p.rect())) {
        r.y += 1;
    }
    r
}

/// The indexes of `cells` in reading order (a section that starts where another does, and is `first`, goes before it). A
/// board holds a few dozen sections, so a plain insertion sort does, and spares the page a sort of its own for this type.
fn reading_order(cells: &[Cell], first: Option<&str>) -> Vec<usize> {
    let key = |i: usize| (cells[i].y, cells[i].x, first != Some(cells[i].id.as_str()));
    let mut order: Vec<usize> = (0..cells.len()).collect();
    for i in 1..order.len() {
        let mut j = i;
        while j > 0 && key(order[j]) < key(order[j - 1]) {
            order.swap(j, j - 1);
            j -= 1;
        }
    }
    order
}

/// Every cell floated up to the first free row.
pub fn compact(cells: &[Cell]) -> Vec<Cell> {
    settle(cells, None)
}

fn settle(cells: &[Cell], first: Option<&str>) -> Vec<Cell> {
    let mut out: Vec<Cell> = cells.to_vec();
    let mut placed: Vec<Cell> = Vec::with_capacity(cells.len());
    for i in reading_order(cells, first) {
        let rect = float_up(clamp(cells[i].rect()), &placed);
        out[i] = Cell::new(&cells[i].id, rect);
        placed.push(out[i].clone());
    }
    out
}

/// `cells` with `id` at `rect` (made to fit), the cells it overlaps pushed down, and everything floated up. An id that is
/// not there changes nothing but the compaction.
pub fn place(cells: &[Cell], id: &str, rect: Rect) -> Vec<Cell> {
    let moved: Vec<Cell> = cells
        .iter()
        .map(|c| if c.id == id { Cell::new(id, rect) } else { c.clone() })
        .collect();
    settle(&moved, Some(id))
}

/// The ids in the order a narrow screen shows them: top to bottom, left to right.
pub fn stack(cells: &[Cell]) -> Vec<String> {
    reading_order(cells, None)
        .into_iter()
        .map(|i| cells[i].id.clone())
        .collect()
}

fn find(cells: &[Cell], id: &str) -> Option<Rect> {
    cells.iter().find(|c| c.id == id).map(Cell::rect)
}

fn shift(v: u32, d: i32) -> u32 {
    v.saturating_add_signed(d)
}

/// One keyboard step: `id` moved by `dx` columns and `dy` rows (it stops at the edges).
pub fn move_by(cells: &[Cell], id: &str, dx: i32, dy: i32) -> Vec<Cell> {
    match find(cells, id) {
        Some(r) => place(
            cells,
            id,
            Rect {
                x: shift(r.x, dx),
                y: shift(r.y, dy),
                ..r
            },
        ),
        None => cells.to_vec(),
    }
}

/// One keyboard step: `id` made wider by `dw` columns and taller by `dh` rows (negative to shrink).
pub fn resize_by(cells: &[Cell], id: &str, dw: i32, dh: i32) -> Vec<Cell> {
    match find(cells, id) {
        Some(r) => place(
            cells,
            id,
            Rect {
                w: shift(r.w, dw).max(1),
                h: shift(r.h, dh).max(1),
                ..r
            },
        ),
        None => cells.to_vec(),
    }
}

/// Where a new section of size `w` by `h` goes: the first free spot in reading order, below everything if none.
pub fn first_free(cells: &[Cell], w: u32, h: u32) -> Rect {
    let size = clamp(Rect { x: 0, y: 0, w, h });
    let bottom = cells.iter().map(|c| c.y + c.h).max().unwrap_or(0);
    for y in 0..=bottom {
        for x in 0..=COLUMNS - size.w {
            let r = Rect { x, y, ..size };
            if !cells.iter().any(|c| overlap(&r, &c.rect())) {
                return r;
            }
        }
    }
    Rect { y: bottom, ..size }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(id: &str, x: u32, y: u32, w: u32, h: u32) -> Cell {
        Cell::new(id, Rect { x, y, w, h })
    }

    fn at(cells: &[Cell], id: &str) -> Rect {
        find(cells, id).unwrap()
    }

    #[test]
    fn a_place_is_made_to_fit_the_grid() {
        assert_eq!(
            clamp(Rect {
                x: 10,
                y: 2,
                w: 6,
                h: 0
            }),
            Rect { x: 6, y: 2, w: 6, h: 1 }
        );
        assert_eq!(
            clamp(Rect {
                x: 0,
                y: 0,
                w: 99,
                h: 99
            }),
            Rect {
                x: 0,
                y: 0,
                w: 12,
                h: MAX_ROWS
            }
        );
        assert_eq!(clamp(Rect { x: 3, y: 0, w: 0, h: 2 }).w, 1);
    }

    #[test]
    fn what_a_moved_section_lands_on_is_pushed_down() {
        let cells = vec![cell("a", 0, 0, 6, 2), cell("b", 6, 0, 6, 2), cell("c", 0, 2, 12, 2)];
        let out = place(&cells, "b", Rect { x: 0, y: 0, w: 6, h: 2 });
        assert_eq!(at(&out, "b"), Rect { x: 0, y: 0, w: 6, h: 2 });
        assert_eq!(at(&out, "a").y, 2);
        assert_eq!(at(&out, "c").y, 4);
        // Nothing overlaps.
        for (i, p) in out.iter().enumerate() {
            for q in &out[i + 1..] {
                assert!(!overlap(&p.rect(), &q.rect()), "{} {}", p.id, q.id);
            }
        }
    }

    #[test]
    fn everything_floats_up_to_the_first_free_row() {
        let cells = vec![cell("a", 0, 0, 6, 2), cell("b", 0, 7, 6, 2), cell("c", 6, 5, 6, 1)];
        let out = compact(&cells);
        assert_eq!(at(&out, "b").y, 2);
        assert_eq!(at(&out, "c").y, 0);
        // The order the caller gave is kept.
        assert_eq!(out.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
    }

    #[test]
    fn a_section_dropped_in_empty_space_below_floats_up_to_it() {
        let cells = vec![cell("a", 0, 0, 12, 2), cell("b", 0, 2, 6, 2)];
        let out = place(&cells, "b", Rect { x: 6, y: 9, w: 6, h: 2 });
        assert_eq!(at(&out, "b"), Rect { x: 6, y: 2, w: 6, h: 2 });
    }

    #[test]
    fn a_resize_that_runs_past_the_edge_is_cut() {
        let cells = vec![cell("a", 8, 0, 4, 1)];
        let out = resize_by(&cells, "a", 3, 0);
        // Seven columns no longer fit from column 8, so it slides left to keep its width.
        assert_eq!(at(&out, "a"), Rect { x: 5, y: 0, w: 7, h: 1 });
    }

    #[test]
    fn keyboard_steps_stop_at_the_edges() {
        let cells = vec![cell("a", 0, 0, 4, 2), cell("b", 4, 0, 4, 2)];
        assert_eq!(at(&move_by(&cells, "a", -1, -1), "a"), Rect { x: 0, y: 0, w: 4, h: 2 });
        assert_eq!(at(&resize_by(&cells, "a", -9, -9), "a").w, 1);
        assert_eq!(at(&resize_by(&cells, "a", -9, -9), "a").h, 1);
        // Moving a onto b pushes b down.
        let out = move_by(&cells, "a", 1, 0);
        assert_eq!(at(&out, "a").x, 1);
        assert_eq!(at(&out, "b").y, 2);
        // An id that is not there changes nothing.
        assert_eq!(move_by(&cells, "zz", 1, 1), cells);
    }

    #[test]
    fn a_narrow_screen_reads_top_to_bottom_left_to_right() {
        let cells = vec![cell("c", 0, 2, 12, 1), cell("b", 6, 0, 6, 2), cell("a", 0, 0, 6, 2)];
        assert_eq!(stack(&cells), ["a", "b", "c"]);
    }

    #[test]
    fn a_new_section_takes_the_first_free_spot() {
        assert_eq!(first_free(&[], 6, 3), Rect { x: 0, y: 0, w: 6, h: 3 });
        let cells = vec![cell("a", 0, 0, 6, 3)];
        assert_eq!(first_free(&cells, 6, 3), Rect { x: 6, y: 0, w: 6, h: 3 });
        let full = vec![cell("a", 0, 0, 12, 3)];
        assert_eq!(first_free(&full, 6, 3), Rect { x: 0, y: 3, w: 6, h: 3 });
    }
}
