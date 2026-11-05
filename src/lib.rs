//! hoardmap: scan a directory tree and show it in the Mac app's window as a directory
//! list, a treemap and an extension list. Nothing here deletes a file.
//!
//! - [`tree`]: the scanned tree, an arena of nodes with children sorted by size.
//! - [`scan`]: builds a [`tree::Tree`] from the file system, in parallel, reporting progress.
//! - [`treemap`]: squarified layout of a subtree into rectangles, and extension colours.
//! - [`icicle`]: one row per depth, children under their parent by bytes.
//! - [`server`]: the HTTP server behind the app's window, with its launch token.
//! - [`api`]: the JSON endpoints the page calls, over the scan state.
//! - [`cli`]: the `hoardmap` command's options; it opens the app on a folder.
//! - [`access`]: the Full Disk Access check, its Settings pane, and relaunching the app.

pub mod access;
pub mod api;
pub mod cli;
pub mod icicle;
pub mod rebuildable;
pub mod scan;
pub mod server;
pub mod tree;
pub mod treemap;
pub mod volumes;
