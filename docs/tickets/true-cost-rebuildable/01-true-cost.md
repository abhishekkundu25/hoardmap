# 01: True cost: scanner, model and API

**What to build:** clone-aware sizes. The bulk scanner reads private size, clone id
and extended flags; the tree gains `unique`, `Shared` and the tree-level clone
figures; Physical is corrected everywhere; status reports the new figures.
See the spec's "True cost" and "Testing decisions".

**Blocked by:** None.

**Status:** done

## Acceptance

- [x] `src/scan.rs`: `FSOPT_ATTR_CMN_EXTENDED` with `ATTR_CMNEXT_PRIVATESIZE`,
      `ATTR_CMNEXT_CLONEID`, `ATTR_CMNEXT_EXT_FLAGS`, gated on the volume's clone
      capability, parsed in bit order; a second `(dev, clone id)` set in `flatten`.
- [x] `src/tree.rs`: `unique`, `Shared` (replacing `hard_link`), `clone_aware`,
      `clone_counted_once`, `shared_unattributed`; every `Node` literal updated.
- [x] No new size mode: Physical is the corrected figure everywhere.
- [x] Node JSON has `unique` and `shared`; largest has `unique`;
      `/api/status` has `unique`, `clone_aware`, `clone_counted_once`,
      `shared_unattributed`.
- [x] `tests/scan.rs`: the clone fixture test from the spec (full clone counted once,
      partial clone `unique` about 1 MB, portable `clone_aware` false), and the
      existing same-tree test comparing every field but `unique`.
- [x] `tests/api.rs`: the new fields, `hard_link` assertions moved to `shared`.
- [x] `examples/scan_bench.rs` compares the fields both scanners know and prints the
      clone figures.
- [x] `cargo fmt --check`, both clippy runs, `timeout 120 cargo test --test scan --test
      treemap --test api` and `cargo test --lib` green.
