//! Icicle layout of a subtree: one row per depth, the root across the top, every
//! child under its parent with a width proportional to its bytes, largest first.

use serde::Serialize;

use crate::tree::{Kind, NodeId, Tree, extension};
use crate::treemap::{Colours, Mode};

/// Rows are never thinner than this; deeper levels are left out instead.
pub const MIN_ROW: f32 = 18.0;

/// Directory bars. Files keep their extension colour.
pub const DIR_COLOR: &str = "#8796ab";

/// One node's bar. Coordinates are absolute pixels.
#[derive(Clone, Debug, Serialize)]
pub struct Bar {
    pub id: NodeId,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub depth: u32,
    /// Bytes in the requested mode.
    pub bytes: u64,
    pub kind: Kind,
    pub name: String,
    /// Lower-cased extension; empty for a directory.
    pub ext: String,
    pub color: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct Icicle {
    pub root: NodeId,
    pub w: f32,
    pub h: f32,
    pub rows: u32,
    /// Parents before their children, the root first.
    pub rects: Vec<Bar>,
}

/// Lays the subtree under `root` into a `w` by `h` viewport. A node narrower than
/// `min` pixels is left out with everything under it; levels that would make rows
/// thinner than [`MIN_ROW`] are left out too. An unknown root or a viewport without
/// area yields no bars.
pub fn icicle(
    tree: &Tree,
    root: NodeId,
    w: f32,
    h: f32,
    min: f32,
    mode: Mode,
    colours: &Colours,
) -> Icicle {
    let mut out = Icicle {
        root,
        w,
        h,
        rows: 0,
        rects: Vec::new(),
    };
    if tree.get(root).is_none() || !(w > 0.0 && h > 0.0) {
        return out;
    }
    let max_rows = ((h / MIN_ROW).floor() as u32).max(1);
    // (id, depth, x, width), filled before the row height is known.
    let mut placed = vec![(root, 0u32, 0.0f32, w)];
    let mut i = 0;
    while i < placed.len() {
        let (id, depth, x, width) = placed[i];
        i += 1;
        if depth + 1 >= max_rows {
            continue;
        }
        let node = &tree.nodes[id as usize];
        let mut children: Vec<(NodeId, u64)> = node
            .children
            .iter()
            .map(|&c| (c, mode.bytes(&tree.nodes[c as usize])))
            .filter(|&(_, b)| b > 0)
            .collect();
        // The tree sorts by logical size; physical sizes may be out of order.
        if !children.is_sorted_by(|a, b| a.1 >= b.1) {
            children.sort_by_key(|&(_, b)| std::cmp::Reverse(b));
        }
        let total: u64 = children.iter().map(|&(_, b)| b).sum();
        let mut before = 0u64;
        for (child, bytes) in children {
            // From the running sum, so float error does not pile up along a row.
            let cx = x + width * (before as f64 / total as f64) as f32;
            before += bytes;
            let end = x + width * (before as f64 / total as f64) as f32;
            if end - cx >= min {
                placed.push((child, depth + 1, cx, end - cx));
            }
        }
    }
    out.rows = placed.iter().map(|p| p.1).max().unwrap_or(0) + 1;
    let row = h / out.rows as f32;
    out.rects = placed
        .into_iter()
        .map(|(id, depth, x, width)| {
            let node = &tree.nodes[id as usize];
            let (ext, color) = if node.kind == Kind::Dir {
                (String::new(), DIR_COLOR)
            } else {
                let ext = extension(&node.name);
                let color = colours.color_of(&ext);
                (ext, color)
            };
            Bar {
                id,
                x,
                y: depth as f32 * row,
                w: width,
                h: row,
                depth,
                bytes: mode.bytes(node),
                kind: node.kind,
                name: node.name.clone(),
                ext,
                color,
            }
        })
        .collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Node;

    fn node(
        name: &str,
        parent: Option<NodeId>,
        kind: Kind,
        size: u64,
        children: Vec<NodeId>,
    ) -> Node {
        Node {
            name: name.into(),
            parent,
            kind,
            size,
            physical: size.div_ceil(4096) * 4096,
            files: 0,
            dirs: 0,
            mtime: 0,
            children,
            hard_link: false,
        }
    }

    /// root (1000) > a (600) > [a1.mov 500, a2.txt 100]; b.jpg 399; c.txt 1
    fn sample() -> Tree {
        let mut tree = Tree {
            nodes: vec![
                node("/r", None, Kind::Dir, 1000, vec![1, 4, 5]),
                node("a", Some(0), Kind::Dir, 600, vec![2, 3]),
                node("a1.mov", Some(1), Kind::File, 500, vec![]),
                node("a2.txt", Some(1), Kind::File, 100, vec![]),
                node("b.jpg", Some(0), Kind::File, 399, vec![]),
                node("c.txt", Some(0), Kind::File, 1, vec![]),
            ],
            errors: vec![],
            error_count: 0,
        };
        tree.nodes[1].physical = 8192;
        tree
    }

    fn bar(layout: &Icicle, id: NodeId) -> &Bar {
        layout.rects.iter().find(|b| b.id == id).unwrap()
    }

    #[test]
    fn root_spans_the_top_row_and_children_split_it_by_bytes() {
        let tree = sample();
        let layout = icicle(
            &tree,
            0,
            1000.0,
            300.0,
            0.5,
            Mode::Logical,
            &Colours::new(&tree),
        );
        assert_eq!(layout.rows, 3);
        let root = bar(&layout, 0);
        assert_eq!((root.x, root.y, root.w, root.h), (0.0, 0.0, 1000.0, 100.0));
        let a = bar(&layout, 1);
        assert_eq!((a.x, a.y, a.w, a.depth), (0.0, 100.0, 600.0, 1));
        assert_eq!(a.color, DIR_COLOR);
        let b = bar(&layout, 4);
        assert_eq!((b.x, b.w), (600.0, 399.0));
        assert_eq!(b.ext, "jpg");
        assert_eq!(b.bytes, 399);
        let a2 = bar(&layout, 3);
        assert_eq!((a2.x, a2.y, a2.w), (500.0, 200.0, 100.0));
    }

    #[test]
    fn children_fill_their_parent_exactly() {
        let tree = sample();
        let layout = icicle(
            &tree,
            0,
            777.0,
            300.0,
            0.0,
            Mode::Logical,
            &Colours::new(&tree),
        );
        for parent in [0, 1] {
            let p = bar(&layout, parent);
            let kids: Vec<&Bar> = layout
                .rects
                .iter()
                .filter(|b| tree.nodes[b.id as usize].parent == Some(parent))
                .collect();
            let sum: f32 = kids.iter().map(|b| b.w).sum();
            assert!((sum - p.w).abs() < 0.01, "{sum} vs {}", p.w);
            for k in kids {
                assert!(k.x >= p.x - 0.01 && k.x + k.w <= p.x + p.w + 0.01);
            }
        }
    }

    #[test]
    fn narrow_nodes_are_left_out_with_their_subtree() {
        let tree = sample();
        let layout = icicle(
            &tree,
            0,
            1000.0,
            300.0,
            2.0,
            Mode::Logical,
            &Colours::new(&tree),
        );
        assert!(layout.rects.iter().all(|b| b.id != 5), "c.txt is 1 px wide");
        let layout = icicle(
            &tree,
            0,
            1000.0,
            300.0,
            700.0,
            Mode::Logical,
            &Colours::new(&tree),
        );
        assert_eq!(layout.rects.len(), 1);
        assert_eq!(layout.rows, 1);
    }

    #[test]
    fn rows_never_get_thinner_than_min_row() {
        let tree = sample();
        let layout = icicle(
            &tree,
            0,
            1000.0,
            40.0,
            0.5,
            Mode::Logical,
            &Colours::new(&tree),
        );
        assert_eq!(layout.rows, 2);
        assert!(layout.rects.iter().all(|b| b.depth < 2 && b.h >= MIN_ROW));
    }

    #[test]
    fn a_subtree_root_spans_the_viewport() {
        let tree = sample();
        let layout = icicle(
            &tree,
            1,
            600.0,
            200.0,
            0.5,
            Mode::Logical,
            &Colours::new(&tree),
        );
        let a = bar(&layout, 1);
        assert_eq!((a.x, a.y, a.w, a.depth), (0.0, 0.0, 600.0, 0));
        assert_eq!(bar(&layout, 2).w, 500.0);
    }

    #[test]
    fn physical_mode_uses_physical_bytes_and_reorders() {
        let tree = sample();
        let layout = icicle(
            &tree,
            0,
            1000.0,
            300.0,
            0.5,
            Mode::Physical,
            &Colours::new(&tree),
        );
        // a: 8192, b.jpg: 4096, c.txt: 4096 of 16384
        assert_eq!(bar(&layout, 1).w, 500.0);
        assert_eq!(bar(&layout, 4).w, 250.0);
        assert_eq!(bar(&layout, 5).w, 250.0);
    }

    #[test]
    fn no_area_or_unknown_root_yields_nothing() {
        let tree = sample();
        let colours = Colours::new(&tree);
        assert!(
            icicle(&tree, 0, 0.0, 300.0, 1.0, Mode::Logical, &colours)
                .rects
                .is_empty()
        );
        assert!(
            icicle(&tree, 0, f32::NAN, 300.0, 1.0, Mode::Logical, &colours)
                .rects
                .is_empty()
        );
        assert!(
            icicle(&tree, 99, 100.0, 300.0, 1.0, Mode::Logical, &colours)
                .rects
                .is_empty()
        );
    }
}
