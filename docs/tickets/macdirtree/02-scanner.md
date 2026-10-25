# 02: Scanner

**What to build:** `scan::scan(path, &Progress) -> io::Result<Tree>` turns a folder
into the arena tree from `src/tree.rs`: parallel with rayon, symlinks not followed,
stays on the root's device, hard links counted once, physical size, mtime roll-up,
unreadable directories counted and listed, progress atomics updated as it goes.

Spec sections: Implementation decisions (Tree, Scan), Testing decisions (Scanner),
user stories 10, 11, 12, 36, 38.

Shape:
- `scan::Progress { dirs: AtomicU64, files: AtomicU64, bytes: AtomicU64 }` with
  `Default` and a `snapshot() -> (u64, u64, u64)`.
- Workers build `Local { name, kind, size, physical, mtime, ino, nlink, dev, children }`
  subtrees with `rayon` (`par_iter` over the subdirectories of each directory); the
  root's `st_dev` is the device to stay on; a directory on another device becomes an
  empty dir node with 0 bytes.
- Flattening is sequential and depth-first from the root, pushing nodes into the arena
  and sorting each directory's children by `size` descending (ties by name). A
  `HashSet<(u64, u64)>` of `(dev, ino)` for `nlink > 1` decides hard-link duplicates
  during the flatten, so the first occurrence in arena order keeps the bytes.
- Errors: `read_dir` or `symlink_metadata` failures are recorded as `ScanError`
  (first 1000) and counted; the scan goes on. A root that cannot be read is
  `Err`.
- `rayon` is already a dependency; do not add others.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] `tests/scan.rs` builds a fixture in a unique temp folder: `a/big.bin` 300 000 B,
      `a/small.txt` 10 B, `b/x.jpg` 2000 B, `b/c/` empty, `.hidden` 5 B, `link.txt`
      hard link to `a/small.txt`, `sym -> a/big.bin`, and `locked/` with mode 000
      holding a 100 B file (the error checks are skipped when running as root)
- [ ] root `size` is 302 015 (hard link counted once, symlink by its own lstat size
      excluded or included consistently and asserted), `files` and `dirs` counts match,
      `a` sorts before `b`, children largest first
- [ ] the second occurrence of the hard-linked pair has `hard_link: true` and size 0
- [ ] `physical` of `a/big.bin` is a multiple of 512 and ≥ 300 000
- [ ] `mtime` of the root equals the latest mtime in the fixture (set one file's mtime
      into the future with `filetime`-free `File::set_modified`)
- [ ] `locked/` appears with `error_count == 1` and its path in `errors`
- [ ] the empty `b/c` is a dir node with 0 bytes and the fixture's `b` counts it in `dirs`
- [ ] `Progress` ends with `files` and `dirs` equal to the tree's totals
- [ ] a bench-shaped sanity check: scanning the crate's own `target` folder (if present)
      finishes and produces a tree with `nodes.len() > 1`
- [ ] `fmt`, `clippy -D warnings`, `cargo test` green
