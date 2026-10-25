# 03: Treemap layout and colours

**What to build:** `treemap::layout(tree, root, w, h, min, mode) -> Layout` lays a
subtree out squarified into pixel rectangles, and `treemap::Colours::new(tree)` ranks
extensions by bytes and maps the top 15 to the palette, the rest to grey.

Spec sections: Implementation decisions (Treemap layout, colour table), Testing
decisions (Treemap), user stories 13, 14, 19, 39.

Shape:
- `Mode { Logical, Physical }` with `fn bytes(&self, node) -> u64`.
- `Layout { root, w, h, rects: Vec<Rect>, dirs: Vec<Bounds> }`, `Rect { id, x, y, w,
  h, kind, ext, color }`, `Bounds { id, x, y, w, h }`; coordinates `f32`, all
  `Serialize`.
- Squarified algorithm over children largest first (the tree already sorts them);
  zero-byte children are skipped. A directory whose area is below `min * min` becomes
  one `Rect` of kind `dir` with the dark grey colour; a file below that is dropped.
- `Colours { by_ext: HashMap<String, &'static str>, grey, dir_grey }` with
  `color_of(ext) -> &'static str`; the palette is 15 hex strings readable on light
  and dark backgrounds. `Colours::new` walks the tree once.
- `treemap::extensions(tree, root, mode, colours) -> Vec<ExtStat { ext, color, bytes,
  physical, files }>` largest first, and `treemap::largest(tree, root, n, mode) ->
  Vec<NodeId>`: the n biggest files under root. Both are simple DFS; they live here so
  04 only wires JSON.
- Unit tests build small `Tree`s by hand with a helper.

**Blocked by:** None (can start immediately).

**Status:** done

- [ ] rects never overlap (pairwise check on a 50-file tree, with a 0.01 px tolerance)
- [ ] every rect and every bounds lies inside `0..w × 0..h`
- [ ] the summed area of a directory's rects equals its bounds' area within 1 px² when
      nothing was dropped
- [ ] a child's rects are inside its parent's bounds
- [ ] with `min = 1`, a 1-byte file in a 1 GB tree at 800×600 produces no rect, and a
      directory of such files is one `dir` rect
- [ ] aspect ratio: for four equal children in a square, every rect is a square
- [ ] `Mode::Physical` uses `physical` bytes
- [ ] a zero-size root yields empty `rects` and one `dirs` entry for the root
- [ ] `Colours`: the 16th extension by bytes is grey, the first 15 are distinct palette
      colours, `""` is a valid extension key
- [ ] `extensions` sums bytes and files per extension under the given root only
- [ ] `largest` returns files only, largest first, at most n
- [ ] `fmt`, `clippy -D warnings`, `cargo test` green
