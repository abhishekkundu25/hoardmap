//! hoardmap: scan a directory tree and show it in the Mac app's window as a directory
//! list, a treemap and an extension list. Nothing here deletes a file.
//!
//! - [`tree`]: the scanned tree, an arena of nodes with children sorted by size.
//! - [`scan`]: builds a [`tree::Tree`] from the file system, in parallel, reporting progress.
//! - [`treemap`]: squarified layout of a subtree into rectangles, and extension colours.
//! - [`icicle`]: one row per depth, children under their parent by bytes.
//! - [`server`]: the HTTP server behind the app's window, with its launch token.
//! - [`api`]: the JSON endpoints the page calls, over the scan state.
//! - [`categories`]: the scanned folder split into kinds of thing, for the disk bar.
//! - [`disk`]: the size and free space of the folder's disk, and what its other volumes hold.
//! - [`cli`]: the `hoardmap` command's options; it opens the app on a folder.
//! - [`access`]: the Full Disk Access check, its Settings pane, and relaunching the app.

pub mod access;
pub mod api;
pub mod categories;
pub mod cli;
pub mod disk;
pub mod icicle;
pub mod rebuildable;
pub mod scan;
pub mod server;
pub mod tree;
pub mod treemap;
pub mod volumes;
