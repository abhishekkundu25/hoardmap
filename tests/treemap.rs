//! Treemap layout, colours, extension and largest-file statistics on hand-built trees.

use std::collections::HashSet;
use std::time::Instant;

use macdirtree::tree::{Kind, Node, NodeId, ROOT, Tree, extension};
use macdirtree::treemap::{
    Bounds, Colours, Layout, Mode, PALETTE, Rect, extensions, largest, layout,
};

/// A tree described by nesting. `File` gets physical bytes rounded up to 4096;
/// `Phys` sets both figures; `Hard` is a later occurrence of a hard-linked file.
enum Spec {
    Dir(String, Vec<Spec>),
    File(String, u64),
    Phys(String, u64, u64),
    Hard(String),
}

fn d(name: &str, children: Vec<Spec>) -> Spec {
    Spec::Dir(name.into(), children)
}

fn f(name: &str, size: u64) -> Spec {
    Spec::File(name.into(), size)
}

fn p(name: &str, size: u64, physical: u64) -> Spec {
    Spec::Phys(name.into(), size, physical)
}

fn hard(name: &str) -> Spec {
    Spec::Hard(name.into())
}

fn build(spec: Spec) -> Tree {
    let mut tree = Tree {
        nodes: Vec::new(),
        errors: Vec::new(),
        error_count: 0,
    };
    add(&mut tree, None, spec);
    tree
}

fn add(tree: &mut Tree, parent: Option<NodeId>, spec: Spec) -> NodeId {
    let id = tree.nodes.len() as NodeId;
    let blank = |name: &str, kind| Node {
        name: name.to_string(),
        parent,
        kind,
        size: 0,
        physical: 0,
        files: 0,
        dirs: 0,
        mtime: 0,
        children: Vec::new(),
        hard_link: false,
    };
    match spec {
        Spec::Dir(name, children) => {
            tree.nodes.push(blank(&name, Kind::Dir));
            let mut ids: Vec<NodeId> = children
                .into_iter()
                .map(|c| add(tree, Some(id), c))
                .collect();
            ids.sort_by_key(|&c| std::cmp::Reverse(tree.nodes[c as usize].size));
            let mut size = 0;
            let mut physical = 0;
            let mut files = 0;
            let mut dirs = 0;
            for &c in &ids {
                let n = &tree.nodes[c as usize];
                size += n.size;
                physical += n.physical;
                if n.kind == Kind::Dir {
                    files += n.files;
                    dirs += n.dirs + 1;
                } else {
                    files += 1;
                }
            }
            let node = &mut tree.nodes[id as usize];
            node.size = size;
            node.physical = physical;
            node.files = files;
            node.dirs = dirs;
            node.children = ids;
        }
        Spec::File(name, size) => {
            let mut n = blank(&name, Kind::File);
            n.size = size;
            n.physical = size.div_ceil(4096) * 4096;
            tree.nodes.push(n);
        }
        Spec::Phys(name, size, physical) => {
            let mut n = blank(&name, Kind::File);
            n.size = size;
            n.physical = physical;
            tree.nodes.push(n);
        }
        Spec::Hard(name) => {
            let mut n = blank(&name, Kind::File);
            n.hard_link = true;
            tree.nodes.push(n);
        }
    }
    id
}

fn files(prefix: &str, sizes: &[u64]) -> Vec<Spec> {
    sizes
        .iter()
        .enumerate()
        .map(|(i, &s)| f(&format!("{prefix}{i}.{}", EXTS[i % EXTS.len()]), s))
        .collect()
}

const EXTS: [&str; 5] = ["mov", "jpg", "txt", "rs", ""];

fn fifty_files() -> Tree {
    let sizes: Vec<u64> = (1..=25).map(|i| i * i * 1000).collect();
    let small: Vec<u64> = (1..=25).map(|i| 777 * i).collect();
    build(d(
        "/t",
        vec![
            d("a", files("a", &sizes)),
            d(
                "b",
                vec![
                    d("c", files("c", &small[..10])),
                    d("d", files("d", &small[10..])),
                ],
            ),
        ],
    ))
}

type Box4 = (f32, f32, f32, f32);

fn rect_box(r: &Rect) -> Box4 {
    (r.x, r.y, r.w, r.h)
}

fn bounds_box(b: &Bounds) -> Box4 {
    (b.x, b.y, b.w, b.h)
}

/// `a` lies inside `b`, allowing `tol` pixels of slack on each edge.
fn inside(a: Box4, b: Box4, tol: f32) -> bool {
    a.0 >= b.0 - tol
        && a.1 >= b.1 - tol
        && a.0 + a.2 <= b.0 + b.2 + tol
        && a.1 + a.3 <= b.1 + b.3 + tol
}

fn lay(tree: &Tree, w: f32, h: f32, min: f32, mode: Mode) -> Layout {
    layout(tree, ROOT, w, h, min, mode, &Colours::new(tree))
}

#[test]
fn rects_never_overlap() {
    let tree = fifty_files();
    let l = lay(&tree, 800.0, 600.0, 0.0, Mode::Logical);
    assert_eq!(l.rects.len(), 50);
    for (i, a) in l.rects.iter().enumerate() {
        for b in &l.rects[i + 1..] {
            let ox = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
            let oy = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
            assert!(
                ox <= 0.01 || oy <= 0.01,
                "{} and {} overlap by {ox} x {oy}",
                a.id,
                b.id
            );
        }
    }
}

#[test]
fn rects_and_bounds_lie_inside_the_viewport() {
    let tree = fifty_files();
    for &(w, h) in &[(800.0, 600.0), (37.0, 1000.0), (1600.0, 3.0)] {
        let l = lay(&tree, w, h, 1.0, Mode::Logical);
        for r in &l.rects {
            assert!(r.w > 0.0 && r.h > 0.0);
            assert!(
                inside(rect_box(r), (0.0, 0.0, w, h), 0.01),
                "{r:?} in {w}x{h}"
            );
        }
        for b in &l.dirs {
            assert!(
                inside(bounds_box(b), (0.0, 0.0, w, h), 0.01),
                "{b:?} in {w}x{h}"
            );
        }
    }
}

#[test]
fn rect_areas_of_a_directory_sum_to_its_bounds_when_nothing_is_dropped() {
    let tree = fifty_files();
    let l = lay(&tree, 800.0, 600.0, 0.0, Mode::Logical);
    assert_eq!(l.dirs.len(), 5);
    for b in &l.dirs {
        let sum: f32 = l
            .rects
            .iter()
            .filter(|r| tree.ancestors(r.id).contains(&b.id))
            .map(|r| r.w * r.h)
            .sum();
        assert!(
            (sum - b.w * b.h).abs() < 1.0,
            "dir {}: {sum} vs {}",
            b.id,
            b.w * b.h
        );
    }
    let whole: f32 = l.rects.iter().map(|r| r.w * r.h).sum();
    assert!((whole - 800.0 * 600.0).abs() < 1.0);
}

#[test]
fn child_rects_lie_inside_their_parents_bounds() {
    let tree = fifty_files();
    let l = lay(&tree, 800.0, 600.0, 1.0, Mode::Logical);
    for r in &l.rects {
        for anc in tree.ancestors(r.id) {
            if let Some(b) = l.dirs.iter().find(|b| b.id == anc) {
                assert!(
                    inside(rect_box(r), bounds_box(b), 0.01),
                    "{r:?} outside {b:?}"
                );
            }
        }
    }
    for b in &l.dirs {
        for anc in tree.ancestors(b.id) {
            if let Some(p) = l.dirs.iter().find(|p| p.id == anc) {
                assert!(
                    inside(bounds_box(b), bounds_box(p), 0.01),
                    "{b:?} outside {p:?}"
                );
            }
        }
    }
}

#[test]
fn sub_pixel_files_are_dropped_and_sub_pixel_dirs_collapse() {
    let tiny: Vec<Spec> = (0..2000).map(|_| f("t.txt", 1)).collect();
    let tree = build(d(
        "/t",
        vec![f("big.mov", 1 << 30), f("one.txt", 1), d("dust", tiny)],
    ));
    let l = lay(&tree, 800.0, 600.0, 1.0, Mode::Logical);
    let big = tree.nodes.iter().position(|n| n.name == "big.mov").unwrap() as NodeId;
    let one = tree.nodes.iter().position(|n| n.name == "one.txt").unwrap() as NodeId;
    let dust = tree.nodes.iter().position(|n| n.name == "dust").unwrap() as NodeId;
    let ids: Vec<NodeId> = l.rects.iter().map(|r| r.id).collect();
    assert!(ids.contains(&big));
    assert!(
        !ids.contains(&one),
        "a 1-byte file in a 1 GB tree has no rect"
    );
    let dust_rects: Vec<_> = l.rects.iter().filter(|r| r.id == dust).collect();
    assert_eq!(dust_rects.len(), 1);
    assert_eq!(dust_rects[0].kind, Kind::Dir);
    assert_eq!(dust_rects[0].ext, "");
    assert_eq!(dust_rects[0].color, Colours::new(&tree).dir_grey);
    assert_eq!(l.rects.len(), 2);
    assert_eq!(l.dirs.len(), 1, "the collapsed dir is not recursed into");
    let json = serde_json::to_value(&l).unwrap();
    assert_eq!(json["rects"][1]["kind"], "dir");
}

#[test]
fn four_equal_children_in_a_square_are_squares() {
    let tree = build(d("/t", files("f", &[100, 100, 100, 100])));
    let l = lay(&tree, 400.0, 400.0, 1.0, Mode::Logical);
    assert_eq!(l.rects.len(), 4);
    for r in &l.rects {
        assert!(
            (r.w - 200.0).abs() < 0.01 && (r.h - 200.0).abs() < 0.01,
            "{r:?}"
        );
    }
    let corners: HashSet<(i32, i32)> = l.rects.iter().map(|r| (r.x as i32, r.y as i32)).collect();
    assert_eq!(corners.len(), 4);
}

#[test]
fn physical_mode_uses_physical_bytes() {
    let tree = build(d(
        "/t",
        vec![p("a.txt", 100, 4096), p("b.txt", 100, 3 * 4096)],
    ));
    let logical = lay(&tree, 400.0, 100.0, 1.0, Mode::Logical);
    let physical = lay(&tree, 400.0, 100.0, 1.0, Mode::Physical);
    let area = |l: &Layout, name: &str| {
        let id = tree.nodes.iter().position(|n| n.name == name).unwrap() as NodeId;
        let r = l.rects.iter().find(|r| r.id == id).unwrap();
        r.w * r.h
    };
    assert!((area(&logical, "a.txt") - 20_000.0).abs() < 1.0);
    assert!((area(&logical, "b.txt") - 20_000.0).abs() < 1.0);
    assert!((area(&physical, "a.txt") - 10_000.0).abs() < 1.0);
    assert!((area(&physical, "b.txt") - 30_000.0).abs() < 1.0);
    assert_eq!(Mode::Physical.bytes(tree.root()), 4 * 4096);
    assert_eq!(Mode::Logical.bytes(tree.root()), 200);
}

#[test]
fn zero_size_root_yields_no_rects_and_the_root_bounds() {
    let tree = fifty_files();
    for &(w, h) in &[
        (0.0, 600.0),
        (800.0, 0.0),
        (0.0, 0.0),
        (-5.0, 10.0),
        (f32::NAN, 10.0),
    ] {
        let l = lay(&tree, w, h, 1.0, Mode::Logical);
        assert!(l.rects.is_empty());
        assert_eq!(l.dirs.len(), 1);
        assert_eq!(l.dirs[0].id, ROOT);
        assert_eq!(l.root, ROOT);
    }
    let empty = build(d("/t", vec![]));
    let l = lay(&empty, 800.0, 600.0, 1.0, Mode::Logical);
    assert!(l.rects.is_empty());
    assert_eq!(l.dirs.len(), 1);
    assert_eq!((l.dirs[0].w, l.dirs[0].h), (800.0, 600.0));
}

#[test]
fn layout_of_a_subtree_fills_the_viewport() {
    let tree = fifty_files();
    let colours = Colours::new(&tree);
    let a = tree.nodes.iter().position(|n| n.name == "a").unwrap() as NodeId;
    let l = layout(&tree, a, 300.0, 200.0, 0.0, Mode::Logical, &colours);
    assert_eq!(l.root, a);
    assert_eq!(l.rects.len(), 25);
    assert_eq!(l.dirs.len(), 1);
    let whole: f32 = l.rects.iter().map(|r| r.w * r.h).sum();
    assert!((whole - 60_000.0).abs() < 1.0);
}

#[test]
fn colours_rank_the_top_15_extensions_and_grey_the_rest() {
    let names: [&str; 17] = [
        "a.mov", "b.jpg", "c.png", "d.txt", "e.rs", "f.pdf", "g.zip", "h.mp3", "i.iso", "j.dmg",
        "k.app", "l.doc", "noext", "n.md", "o.json", "p.log", "q.tmp",
    ];
    let mut specs = Vec::new();
    for (i, name) in names.iter().enumerate() {
        specs.push(f(name, 1000 - i as u64 * 10));
    }
    let tree = build(d("/t", specs));
    let colours = Colours::new(&tree);
    let top: Vec<&str> = (0..15)
        .map(|i| colours.color_of(&extension(names[i])))
        .collect();
    assert_eq!(top, PALETTE.to_vec());
    assert_eq!(top.iter().collect::<HashSet<_>>().len(), 15);
    assert_eq!(
        colours.color_of(""),
        PALETTE[12],
        "no extension is a key like any other"
    );
    assert_eq!(colours.color_of("log"), colours.grey);
    assert_eq!(colours.color_of("tmp"), colours.grey);
    assert_eq!(colours.color_of("never"), colours.grey);
    assert_ne!(colours.dir_grey, colours.grey);

    let empty = Colours::new(&build(d("/t", vec![])));
    assert!(empty.by_ext.is_empty());
    assert_eq!(empty.color_of("mov"), empty.grey);
}

#[test]
fn extensions_sum_bytes_and_files_under_the_root_only() {
    let tree = build(d(
        "/t",
        vec![
            d(
                "sub",
                vec![
                    p("a.MOV", 500, 4096),
                    p("b.mov", 300, 4096),
                    p("c.txt", 100, 4096),
                    hard("d.mov"),
                    p("Makefile", 50, 4096),
                ],
            ),
            p("outside.mov", 10_000, 12_288),
            p("outside.txt", 10, 4096),
        ],
    ));
    let colours = Colours::new(&tree);
    let sub = tree.nodes.iter().position(|n| n.name == "sub").unwrap() as NodeId;
    let stats = extensions(&tree, sub, Mode::Logical, &colours);
    let flat: Vec<(&str, u64, u64, u32)> = stats
        .iter()
        .map(|s| (s.ext.as_str(), s.bytes, s.physical, s.files))
        .collect();
    assert_eq!(
        flat,
        vec![
            ("mov", 800, 8192, 3),
            ("txt", 100, 4096, 1),
            ("", 50, 4096, 1)
        ]
    );
    assert_eq!(stats[0].color, colours.color_of("mov"));

    let all = extensions(&tree, ROOT, Mode::Logical, &colours);
    assert_eq!(all[0].ext, "mov");
    assert_eq!(all[0].bytes, 10_800);
    assert_eq!(all[0].files, 4);

    let tree = build(d(
        "/t",
        vec![p("a.big", 10, 100_000), p("b.small", 20, 4096)],
    ));
    let by_physical = extensions(&tree, ROOT, Mode::Physical, &colours);
    assert_eq!(by_physical[0].ext, "big");
    let by_logical = extensions(&tree, ROOT, Mode::Logical, &colours);
    assert_eq!(by_logical[0].ext, "small");
    let json = serde_json::to_value(&by_logical).unwrap();
    assert_eq!(json[0]["ext"], "small");
    assert!(json[0]["color"].is_string());
}

#[test]
fn largest_returns_files_only_largest_first_at_most_n() {
    let tree = build(d(
        "/t",
        vec![
            d(
                "sub",
                vec![p("s1", 900, 4096), p("s2", 50, 1 << 20), hard("dup")],
            ),
            p("f1", 1000, 4096),
            p("f2", 100, 4096),
            p("f3", 1, 4096),
        ],
    ));
    let name = |id: NodeId| tree.nodes[id as usize].name.as_str();
    let top: Vec<&str> = largest(&tree, ROOT, 3, Mode::Logical)
        .into_iter()
        .map(name)
        .collect();
    assert_eq!(top, vec!["f1", "s1", "f2"]);
    let all: Vec<&str> = largest(&tree, ROOT, 100, Mode::Logical)
        .into_iter()
        .map(name)
        .collect();
    assert_eq!(all, vec!["f1", "s1", "f2", "s2", "f3", "dup"]);
    assert!(all.iter().all(|n| *n != "sub" && *n != "/t"));
    let phys: Vec<&str> = largest(&tree, ROOT, 1, Mode::Physical)
        .into_iter()
        .map(name)
        .collect();
    assert_eq!(phys, vec!["s2"]);
    let sub = tree.nodes.iter().position(|n| n.name == "sub").unwrap() as NodeId;
    let under: Vec<&str> = largest(&tree, sub, 10, Mode::Logical)
        .into_iter()
        .map(name)
        .collect();
    assert_eq!(under, vec!["s1", "s2", "dup"]);
    assert!(largest(&tree, ROOT, 0, Mode::Logical).is_empty());
}

#[test]
fn layout_of_200k_files_is_fast() {
    let mut dirs = Vec::new();
    for i in 0..200u64 {
        let sizes: Vec<u64> = (0..1000u64)
            .map(|f| 1 + (f * 7919 + i * 104_729) % 5_000_000)
            .collect();
        dirs.push(d("d", files("f", &sizes)));
    }
    let tree = build(d("/t", dirs));
    assert_eq!(tree.root().files, 200_000);
    let colours = Colours::new(&tree);
    let start = Instant::now();
    let l = layout(&tree, ROOT, 1600.0, 900.0, 1.0, Mode::Logical, &colours);
    let elapsed = start.elapsed();
    println!(
        "200k files laid out in {elapsed:?}: {} rects, {} dirs",
        l.rects.len(),
        l.dirs.len()
    );
    assert!(!l.rects.is_empty());
    assert_eq!(l.dirs.len(), 201);
    assert!(elapsed.as_secs() < 30, "layout took {elapsed:?}");
}
