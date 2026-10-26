//! Squarified treemap layout of a subtree into pixel rectangles, and the colour table
//! that maps extensions to palette entries.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::tree::{Kind, Node, NodeId, Tree, extension};

/// Which byte figure sizes a node: logical bytes or bytes allocated on disk.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Logical,
    Physical,
}

impl Mode {
    pub fn bytes(self, node: &Node) -> u64 {
        match self {
            Mode::Logical => node.size,
            Mode::Physical => node.physical,
        }
    }

    /// `"logical"` or `"physical"`, case-insensitive.
    pub fn parse(s: &str) -> Option<Mode> {
        if s.eq_ignore_ascii_case("logical") {
            Some(Mode::Logical)
        } else if s.eq_ignore_ascii_case("physical") {
            Some(Mode::Physical)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ParseModeError;

impl std::fmt::Display for ParseModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("mode must be logical or physical")
    }
}

impl std::error::Error for ParseModeError {}

impl FromStr for Mode {
    type Err = ParseModeError;

    fn from_str(s: &str) -> Result<Mode, ParseModeError> {
        Mode::parse(s).ok_or(ParseModeError)
    }
}

/// A file, or a directory too small to recurse into, placed in the viewport.
/// Coordinates are absolute pixels.
#[derive(Clone, Debug, Serialize)]
pub struct Rect {
    pub id: NodeId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub kind: Kind,
    /// Lower-cased extension; empty for a collapsed directory.
    pub ext: String,
    pub color: &'static str,
}

/// The area a laid-out directory covers.
#[derive(Clone, Debug, Serialize)]
pub struct Bounds {
    pub id: NodeId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Serialize)]
pub struct Layout {
    pub root: NodeId,
    pub w: f32,
    pub h: f32,
    pub rects: Vec<Rect>,
    /// Every directory that was recursed into, the root first.
    pub dirs: Vec<Bounds>,
}

/// Lays the subtree under `root` into a `w` by `h` viewport. Children fill their
/// directory's bounds in rows along the shorter side (Bruls, Huizing, van Wijk 2000).
/// A directory with an area under `min * min` becomes one dir-grey rect; a file under
/// that is dropped. A viewport without area yields no rects and the root's bounds only;
/// an unknown root yields an empty layout.
pub fn layout(
    tree: &Tree,
    root: NodeId,
    w: f32,
    h: f32,
    min: f32,
    mode: Mode,
    colours: &Colours,
) -> Layout {
    let mut out = Layout {
        root,
        w,
        h,
        rects: Vec::new(),
        dirs: Vec::new(),
    };
    let Some(node) = tree.get(root) else {
        return out;
    };
    if !(w > 0.0 && h > 0.0) {
        out.dirs.push(Bounds {
            id: root,
            x: 0.0,
            y: 0.0,
            w,
            h,
        });
        return out;
    }
    let mut lay = Squarify {
        tree,
        min2: min * min,
        mode,
        colours,
        out: &mut out,
    };
    if node.kind == Kind::Dir {
        lay.dir(root, 0.0, 0.0, w, h);
    } else {
        lay.out.dirs.push(Bounds {
            id: root,
            x: 0.0,
            y: 0.0,
            w,
            h,
        });
        lay.place(root, 0.0, 0.0, w, h);
    }
    out
}

struct Squarify<'a> {
    tree: &'a Tree,
    min2: f32,
    mode: Mode,
    colours: &'a Colours,
    out: &'a mut Layout,
}

impl Squarify<'_> {
    fn dir(&mut self, id: NodeId, x: f32, y: f32, w: f32, h: f32) {
        self.out.dirs.push(Bounds { id, x, y, w, h });
        let node = &self.tree.nodes[id as usize];
        let mut items: Vec<(NodeId, f32)> = node
            .children
            .iter()
            .map(|&c| (c, self.mode.bytes(&self.tree.nodes[c as usize])))
            .filter(|&(_, b)| b > 0)
            .map(|(c, b)| (c, b as f32))
            .collect();
        if items.is_empty() {
            return;
        }
        // The tree sorts by logical size; physical sizes may be out of order.
        if !items.is_sorted_by(|a, b| a.1 >= b.1) {
            items.sort_unstable_by(|a, b| b.1.total_cmp(&a.1));
        }
        let total: f32 = items.iter().map(|&(_, a)| a).sum();
        let scale = w * h / total;
        for item in &mut items {
            item.1 *= scale;
        }

        let (mut rx, mut ry, mut rw, mut rh) = (x, y, w, h);
        let mut i = 0;
        while i < items.len() && rw > 0.0 && rh > 0.0 {
            // Grow the row while its worst aspect ratio does not get worse.
            let side = rw.min(rh);
            let mut sum = items[i].1;
            let (mut lo, mut hi) = (sum, sum);
            let mut cur = worst(sum, lo, hi, side);
            let mut j = i + 1;
            while j < items.len() {
                let a = items[j].1;
                let next = worst(sum + a, lo.min(a), hi.max(a), side);
                if next > cur {
                    break;
                }
                sum += a;
                lo = lo.min(a);
                hi = hi.max(a);
                cur = next;
                j += 1;
            }
            let last_row = j == items.len();
            let row = &items[i..j];
            // The last row and the last item of each row take the exact remainder so
            // float error leaves no gap.
            if rw >= rh {
                let cw = if last_row { rw } else { sum / rh };
                let mut cy = ry;
                for (k, &(id, a)) in row.iter().enumerate() {
                    let ih = if k + 1 == row.len() {
                        ry + rh - cy
                    } else {
                        a / cw
                    };
                    self.item(id, a, rx, cy, cw, ih);
                    cy += ih;
                }
                rx += cw;
                rw -= cw;
            } else {
                let ch = if last_row { rh } else { sum / rw };
                let mut cx = rx;
                for (k, &(id, a)) in row.iter().enumerate() {
                    let iw = if k + 1 == row.len() {
                        rx + rw - cx
                    } else {
                        a / ch
                    };
                    self.item(id, a, cx, ry, iw, ch);
                    cx += iw;
                }
                ry += ch;
                rh -= ch;
            }
            i = j;
        }
    }

    fn item(&mut self, id: NodeId, area: f32, x: f32, y: f32, w: f32, h: f32) {
        if !(w > 0.0 && h > 0.0) {
            return;
        }
        let node = &self.tree.nodes[id as usize];
        if node.kind == Kind::Dir {
            // A folder without children here is one a running scan has not opened up
            // yet; it shows as one block.
            if area < self.min2 || node.children.is_empty() {
                self.out.rects.push(Rect {
                    id,
                    x,
                    y,
                    w,
                    h,
                    kind: Kind::Dir,
                    ext: String::new(),
                    color: self.colours.dir_grey,
                });
            } else {
                self.dir(id, x, y, w, h);
            }
        } else if area >= self.min2 {
            self.place(id, x, y, w, h);
        }
    }

    fn place(&mut self, id: NodeId, x: f32, y: f32, w: f32, h: f32) {
        let node = &self.tree.nodes[id as usize];
        let ext = extension(&node.name);
        let color = self.colours.color_of(&ext);
        self.out.rects.push(Rect {
            id,
            x,
            y,
            w,
            h,
            kind: node.kind,
            ext,
            color,
        });
    }
}

/// The worst aspect ratio in a row of total area `sum`, smallest item `lo`, largest
/// `hi`, laid along a side of length `side`.
fn worst(sum: f32, lo: f32, hi: f32, side: f32) -> f32 {
    let s2 = sum * sum;
    let w2 = side * side;
    (w2 * hi / s2).max(s2 / (w2 * lo))
}

/// Fifteen colours distinct from each other and readable on light and dark backgrounds.
pub const PALETTE: [&str; 15] = [
    "#d9534f", "#e8843a", "#c8a415", "#7cb342", "#2e9e6b", "#26a69a", "#3b9fd8", "#4a6fd4",
    "#7e5fd1", "#b35bc7", "#d85a9c", "#a0785a", "#8f9a3c", "#5a8fa8", "#7a8fc2",
];

/// Files whose extension is outside the top 15.
pub const GREY: &str = "#9a9a9a";

/// Directories too small to recurse into.
pub const DIR_GREY: &str = "#5c5c5c";

/// Extension to colour for one scanned tree.
#[derive(Clone, Debug)]
pub struct Colours {
    pub by_ext: HashMap<String, &'static str>,
    pub grey: &'static str,
    pub dir_grey: &'static str,
}

impl Colours {
    /// Ranks extensions by logical bytes over the whole tree; the top 15 with any
    /// bytes get the palette in order, everything else is grey.
    pub fn new(tree: &Tree) -> Colours {
        let mut bytes: HashMap<String, u64> = HashMap::new();
        for node in &tree.nodes {
            if node.kind != Kind::Dir {
                *bytes.entry(extension(&node.name)).or_insert(0) += node.size;
            }
        }
        let mut ranked: Vec<(String, u64)> = bytes.into_iter().filter(|e| e.1 > 0).collect();
        ranked.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let by_ext = ranked
            .into_iter()
            .zip(PALETTE)
            .map(|((ext, _), color)| (ext, color))
            .collect();
        Colours {
            by_ext,
            grey: GREY,
            dir_grey: DIR_GREY,
        }
    }

    pub fn color_of(&self, ext: &str) -> &'static str {
        self.by_ext.get(ext).copied().unwrap_or(self.grey)
    }
}

/// Bytes and file count of one extension under a root.
#[derive(Clone, Debug, Serialize)]
pub struct ExtStat {
    pub ext: String,
    pub color: &'static str,
    pub bytes: u64,
    pub physical: u64,
    pub files: u32,
}

/// Per-extension totals over the non-directories under `root`, largest in `mode`
/// first, ties by extension. A hard-link duplicate adds 0 bytes but counts as a file.
pub fn extensions(tree: &Tree, root: NodeId, mode: Mode, colours: &Colours) -> Vec<ExtStat> {
    let mut by_ext: HashMap<String, (u64, u64, u32)> = HashMap::new();
    for_each_file(tree, root, |_, node| {
        let e = by_ext.entry(extension(&node.name)).or_insert((0, 0, 0));
        e.0 += node.size;
        e.1 += node.physical;
        e.2 += 1;
    });
    let mut out: Vec<ExtStat> = by_ext
        .into_iter()
        .map(|(ext, (bytes, physical, files))| ExtStat {
            color: colours.color_of(&ext),
            ext,
            bytes,
            physical,
            files,
        })
        .collect();
    let key = |s: &ExtStat| match mode {
        Mode::Logical => s.bytes,
        Mode::Physical => s.physical,
    };
    out.sort_unstable_by(|a, b| key(b).cmp(&key(a)).then_with(|| a.ext.cmp(&b.ext)));
    out
}

/// The `n` largest non-directories under `root` in `mode`, largest first, ties by id.
pub fn largest(tree: &Tree, root: NodeId, n: usize, mode: Mode) -> Vec<NodeId> {
    if n == 0 {
        return Vec::new();
    }
    // A min-heap of the n best seen so far; the smallest falls off when it overflows.
    let mut heap: BinaryHeap<Reverse<(u64, Reverse<NodeId>)>> = BinaryHeap::with_capacity(n + 1);
    for_each_file(tree, root, |id, node| {
        heap.push(Reverse((mode.bytes(node), Reverse(id))));
        if heap.len() > n {
            heap.pop();
        }
    });
    let mut top: Vec<(u64, Reverse<NodeId>)> = heap.into_iter().map(|r| r.0).collect();
    top.sort_unstable_by(|a, b| b.cmp(a));
    top.into_iter().map(|(_, Reverse(id))| id).collect()
}

/// Calls `f` on every non-directory under `root`, `root` included when it is one.
fn for_each_file<'a>(tree: &'a Tree, root: NodeId, mut f: impl FnMut(NodeId, &'a Node)) {
    let Some(start) = tree.get(root) else {
        return;
    };
    if start.kind != Kind::Dir {
        f(root, start);
        return;
    }
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        for &c in &tree.nodes[id as usize].children {
            let child = &tree.nodes[c as usize];
            if child.kind == Kind::Dir {
                stack.push(c);
            } else {
                f(c, child);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worst_of_one_square_item_is_one() {
        assert_eq!(worst(100.0, 100.0, 100.0, 10.0), 1.0);
    }

    #[test]
    fn row_grows_while_the_worst_ratio_improves() {
        // Four equal items of area 25 in a 10 by 10 square: the second item brings the
        // worst ratio from 4 to 1, the third would raise it to 2.25, so a row holds two.
        assert_eq!(worst(25.0, 25.0, 25.0, 10.0), 4.0);
        assert_eq!(worst(50.0, 25.0, 25.0, 10.0), 1.0);
        assert_eq!(worst(75.0, 25.0, 25.0, 10.0), 2.25);
    }

    #[test]
    fn worst_is_the_larger_of_the_two_extremes() {
        // 90 and 10 along a side of 10: the big one is 10 by 9, the small one 10 by 1.
        assert!((worst(100.0, 10.0, 90.0, 10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn parse_accepts_the_two_modes_case_insensitively() {
        assert_eq!(Mode::parse("logical"), Some(Mode::Logical));
        assert_eq!(Mode::parse("Physical"), Some(Mode::Physical));
        assert_eq!(Mode::parse("bytes"), None);
        assert_eq!("physical".parse::<Mode>(), Ok(Mode::Physical));
        assert_eq!("".parse::<Mode>(), Err(ParseModeError));
    }

    #[test]
    fn mode_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&Mode::Physical).unwrap(),
            "\"physical\""
        );
        assert_eq!(
            serde_json::from_str::<Mode>("\"logical\"").unwrap(),
            Mode::Logical
        );
    }

    #[test]
    fn palette_entries_are_distinct_hex_colours() {
        for (i, c) in PALETTE.iter().enumerate() {
            assert_eq!(c.len(), 7);
            assert!(c.starts_with('#'));
            assert!(u32::from_str_radix(&c[1..], 16).is_ok());
            assert!(!PALETTE[i + 1..].contains(c), "{c} repeats");
        }
        assert!(!PALETTE.contains(&GREY));
        assert!(!PALETTE.contains(&DIR_GREY));
    }
}
