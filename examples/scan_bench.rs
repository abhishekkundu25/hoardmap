//! Times the two folder-listing methods on one folder and checks they build the same
//! tree. macOS only, since the bulk method is.
//!
//!   cargo run --release --example scan_bench -- ~ 5

#[cfg(target_os = "macos")]
fn main() {
    use std::path::PathBuf;
    use std::time::Instant;

    use hoardmap::scan::{Method, Progress, scan_with};
    use hoardmap::tree::Tree;

    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(
        args.next()
            .unwrap_or_else(|| std::env::var("HOME").unwrap()),
    );
    let runs: usize = args.next().map_or(5, |n| n.parse().expect("runs"));

    let fingerprint = |tree: &Tree| -> Vec<(u64, u64, u32, u32, i64, bool)> {
        tree.nodes
            .iter()
            .map(|n| (n.size, n.physical, n.files, n.dirs, n.mtime, n.hard_link))
            .collect()
    };
    let scan = |method| {
        let started = Instant::now();
        let tree = scan_with(&path, &Progress::default(), method).expect("scan");
        (started.elapsed(), tree)
    };

    // One unmeasured pass each warms the file system cache.
    let (_, bulk) = scan(Method::Bulk);
    let (_, portable) = scan(Method::Portable);
    println!("{} walker threads", hoardmap::scan::default_threads());
    let root = bulk.root();
    println!(
        "{}: {} files, {} folders, {} unreadable",
        path.display(),
        root.files,
        root.dirs,
        bulk.error_count
    );
    // Apps keep writing while the two scans run, so on a live folder a few entries can
    // differ in size or modification time; a folder nothing writes to matches exactly.
    let (a, b) = (fingerprint(&bulk), fingerprint(&portable));
    let differing: Vec<usize> = (0..bulk.nodes.len().min(portable.nodes.len()))
        .filter(|&i| a[i] != b[i] || bulk.nodes[i].name != portable.nodes[i].name)
        .collect();
    println!(
        "entries that differ between the two scans: {} of {} (node counts {} and {})",
        differing.len(),
        bulk.nodes.len(),
        bulk.nodes.len(),
        portable.nodes.len()
    );
    for &i in differing.iter().take(5) {
        println!("  {}", bulk.path(i as u32).display());
        println!("    bulk     {:?}", a[i]);
        println!("    portable {:?}", b[i]);
    }
    drop((bulk, portable));

    for method in [Method::Portable, Method::Bulk] {
        let mut times: Vec<f64> = (0..runs).map(|_| scan(method).0.as_secs_f64()).collect();
        times.sort_by(f64::total_cmp);
        println!(
            "{method:?}: median {:.2} s, best {:.2} s over {runs} runs",
            times[runs / 2],
            times[0]
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("the bulk listing is macOS only");
}
