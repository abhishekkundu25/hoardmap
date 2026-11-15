//! The finished tree split by when its files arrived, for the Added bar.
//!
//! A file's age is the time from its [`Node::added`] to the end of the scan. The bar
//! shows running totals: what arrived in the past day, the past week and so on. Which
//! spans it shows comes from the tree: a span that holds nearly everything, or hardly
//! more than the one before it, says nothing and is left out, so a Mac two months old
//! is not measured in years.
//!
//! [`Ages::new`] is a pure function of the tree and a time: no file system access.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::tree::{Kind, Node, Tree};

const DAY: u32 = 86_400;

/// The spans the bar may show, shortest first: key and length in seconds.
const LADDER: [(&str, u32); 6] = [
    ("1d", DAY),
    ("7d", 7 * DAY),
    ("30d", 30 * DAY),
    ("90d", 90 * DAY),
    ("1y", 365 * DAY),
    ("3y", 3 * 365 * DAY),
];

/// A span adds to the bar only with this many bytes more than the one before it, and
/// a twentieth more.
const LEAST: u64 = 1 << 20;

/// What a filter by age keeps: the files that arrived within a span of the ladder, or
/// the ones older than it. The number is the span's place in the ladder.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Span {
    Within(u8),
    Over(u8),
}

impl Span {
    /// `7d` for within the past seven days, `over-7d` for older than that.
    pub fn from_key(key: &str) -> Option<Span> {
        let (over, name) = match key.strip_prefix("over-") {
            Some(name) => (true, name),
            None => (false, key),
        };
        let at = LADDER.iter().position(|&(k, _)| k == name)? as u8;
        Some(if over {
            Span::Over(at)
        } else {
            Span::Within(at)
        })
    }

    pub fn key(self) -> String {
        match self {
            Span::Within(at) => LADDER[at as usize].0.to_string(),
            Span::Over(at) => format!("over-{}", LADDER[at as usize].0),
        }
    }
}

/// The current time as [`Node::added`] counts it.
pub fn now() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs().min(u64::from(u32::MAX)) as u32)
}

/// One finished tree by age, as of `now`.
#[derive(Debug)]
pub struct Ages {
    now: u32,
    steps: Vec<(Span, u64)>,
    rest: u64,
    bulk: Option<(u32, u64)>,
}

impl Ages {
    /// The tree's files by age at `now`, seconds since the Unix epoch. A file added
    /// after `now` counts as just added, and one with no time as older than every span.
    pub fn new(tree: &Tree, now: u32) -> Ages {
        // Bytes on disk by span, each file in the shortest span that holds it, and by
        // the day it arrived.
        let mut within = [0u64; LADDER.len()];
        let mut older = 0u64;
        let mut days = vec![0u64; (now / DAY) as usize + 1];
        for node in &tree.nodes {
            if node.kind == Kind::Dir || node.physical == 0 {
                continue;
            }
            let age = now.saturating_sub(node.added);
            match LADDER.iter().position(|&(_, length)| age <= length) {
                Some(at) => within[at] += node.physical,
                None => older += node.physical,
            }
            days[(node.added.min(now) / DAY) as usize] += node.physical;
        }
        let total = within.iter().sum::<u64>() + older;
        let mut steps = Vec::new();
        let (mut run, mut last) = (0u64, 0u64);
        for (at, bytes) in within.into_iter().enumerate() {
            run += bytes;
            // From here on a span holds all but a two-hundredth of the tree.
            if u128::from(run) * 200 >= u128::from(total) * 199 {
                break;
            }
            if run - last >= LEAST.max(run / 20) {
                steps.push((Span::Within(at as u8), run));
                last = run;
            }
        }
        let rest = total - last;
        let bulk = steps.last().and_then(|&(span, _)| {
            let Span::Within(at) = span else {
                return None;
            };
            // The whole days before the longest span shown.
            let before = (now.saturating_sub(LADDER[at as usize].1) / DAY) as usize;
            let peak = (0..before).max_by_key(|&day| days[day])?;
            // A setup or a restore can run past midnight: the days beside it count too.
            let around: u64 = days[peak.saturating_sub(1)..(peak + 2).min(before)]
                .iter()
                .sum();
            let most = u128::from(around) * 5 >= u128::from(rest) * 4;
            (most && around > 0).then_some((peak as u32 * DAY + DAY / 2, around.min(rest)))
        });
        Ages {
            now,
            steps,
            rest,
            bulk,
        }
    }

    /// The time the ages are counted from.
    pub fn now(&self) -> u32 {
        self.now
    }

    /// The spans worth showing, shortest first, each with the bytes on disk of the
    /// files that arrived within it: running totals, so each holds the ones before
    /// it. Empty when no span tells the files apart.
    pub fn steps(&self) -> &[(Span, u64)] {
        &self.steps
    }

    /// The files older than the longest span in [`Ages::steps`], and their bytes on
    /// disk. `None` when there are no steps.
    pub fn rest(&self) -> Option<(Span, u64)> {
        match self.steps.last() {
            Some(&(Span::Within(at), _)) => Some((Span::Over(at), self.rest)),
            _ => None,
        }
    }

    /// When four fifths of [`Ages::rest`] arrived within a day of one day: that day,
    /// as a time at its middle, and the bytes. A new Mac or a restore looks like this.
    pub fn bulk(&self) -> Option<(u32, u64)> {
        self.bulk
    }

    /// Whether the entry `node` arrived in `span`.
    pub fn holds(&self, span: Span, node: &Node) -> bool {
        let age = self.now.saturating_sub(node.added);
        match span {
            Span::Within(at) => age <= LADDER[at as usize].1,
            Span::Over(at) => age > LADDER[at as usize].1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{ROOT, Shared};
    use std::collections::HashMap;

    const NOW: u32 = 1_800_000_000;
    const GB: u64 = 1 << 30;

    /// A folder of files, each `(age in days, bytes)`.
    fn tree(files: &[(u32, u64)]) -> Tree {
        let node = |kind, parent, added, bytes| Node {
            name: String::new(),
            parent,
            kind,
            size: bytes,
            physical: bytes,
            files: 0,
            dirs: 0,
            mtime: 0,
            added,
            children: Vec::new(),
            shared: Shared::None,
        };
        let mut nodes = vec![node(Kind::Dir, None, 0, files.iter().map(|f| f.1).sum())];
        for &(age, bytes) in files {
            let id = nodes.len() as u32;
            nodes[0].children.push(id);
            nodes.push(node(Kind::File, Some(ROOT), NOW - age * DAY, bytes));
        }
        Tree {
            nodes,
            raw_names: HashMap::new(),
            errors: Vec::new(),
            error_count: 0,
            clone_aware: false,
            clone_counted_once: 0,
        }
    }

    fn shown(ages: &Ages) -> Vec<(String, u64)> {
        let steps = ages.steps().iter().copied();
        steps
            .chain(ages.rest())
            .map(|(span, bytes)| (span.key(), bytes))
            .collect()
    }

    fn keyed(pairs: &[(&str, u64)]) -> Vec<(String, u64)> {
        pairs.iter().map(|&(k, b)| (k.to_string(), b)).collect()
    }

    #[test]
    fn an_old_mac_is_measured_up_to_years() {
        let files = [
            (0, 2 * GB),
            (3, 9 * GB),
            (20, 30 * GB),
            (60, 40 * GB),
            (200, 100 * GB),
            (700, 100 * GB),
            (2000, 200 * GB),
        ];
        let ages = Ages::new(&tree(&files), NOW);
        assert_eq!(
            shown(&ages),
            keyed(&[
                ("1d", 2 * GB),
                ("7d", 11 * GB),
                ("30d", 41 * GB),
                ("90d", 81 * GB),
                ("1y", 181 * GB),
                ("3y", 281 * GB),
                ("over-3y", 200 * GB),
            ])
        );
        assert_eq!(
            ages.bulk(),
            Some((NOW / DAY * DAY - 2000 * DAY + DAY / 2, 200 * GB))
        );
    }

    #[test]
    fn a_mac_two_months_old_is_measured_in_weeks_and_names_its_first_day() {
        let files = [(0, 2 * GB), (3, 9 * GB), (20, 30 * GB), (60, 440 * GB)];
        let ages = Ages::new(&tree(&files), NOW);
        assert_eq!(
            shown(&ages),
            keyed(&[
                ("1d", 2 * GB),
                ("7d", 11 * GB),
                ("30d", 41 * GB),
                ("over-30d", 440 * GB),
            ])
        );
        let (day, bytes) = ages.bulk().unwrap();
        assert_eq!(bytes, 440 * GB);
        assert_eq!(day / DAY, NOW / DAY - 60);
    }

    #[test]
    fn a_span_that_adds_little_is_left_out() {
        // The week holds a megabyte more than the day: not worth a part of its own.
        let files = [(0, 20 * GB), (3, 1 << 20), (20, 30 * GB), (500, 100 * GB)];
        let ages = Ages::new(&tree(&files), NOW);
        let keys: Vec<String> = shown(&ages).into_iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["1d", "30d", "over-30d"]);
    }

    #[test]
    fn files_of_one_age_have_no_steps() {
        for files in [vec![(0, GB), (0, 2 * GB)], vec![(900, GB)], vec![]] {
            let ages = Ages::new(&tree(&files), NOW);
            assert_eq!(shown(&ages), [], "{files:?}");
            assert_eq!(ages.bulk(), None);
        }
    }

    #[test]
    fn the_rest_spread_over_many_days_has_no_day_to_name() {
        let mut files = vec![(0, 5 * GB)];
        files.extend((100..200).map(|day| (day, GB)));
        let ages = Ages::new(&tree(&files), NOW);
        assert_eq!(ages.bulk(), None);
    }

    #[test]
    fn the_steps_and_the_rest_add_up_to_the_tree() {
        let files = [(0, 3 * GB), (10, 7 * GB), (100, 11 * GB), (5000, 13 * GB)];
        let tree = tree(&files);
        let ages = Ages::new(&tree, NOW);
        let longest = ages.steps().last().unwrap().1;
        assert_eq!(longest + ages.rest().unwrap().1, tree.root().physical);
    }

    #[test]
    fn a_time_ahead_of_the_clock_is_just_added_and_none_is_oldest() {
        let mut tree = tree(&[(0, GB), (0, GB), (400, 8 * GB)]);
        tree.nodes[1].added = NOW + 5 * DAY;
        tree.nodes[2].added = 0;
        let ages = Ages::new(&tree, NOW);
        assert_eq!(
            shown(&ages),
            keyed(&[("1d", GB), ("3y", 9 * GB), ("over-3y", GB)])
        );
        let day = Span::from_key("1d").unwrap();
        assert!(ages.holds(day, &tree.nodes[1]));
        assert!(!ages.holds(day, &tree.nodes[2]));
        assert!(ages.holds(Span::from_key("over-3y").unwrap(), &tree.nodes[2]));
    }

    #[test]
    fn keys_name_their_spans() {
        for (at, (key, _)) in LADDER.iter().enumerate() {
            assert_eq!(Span::from_key(key), Some(Span::Within(at as u8)));
            let over = format!("over-{key}");
            assert_eq!(Span::from_key(&over), Some(Span::Over(at as u8)));
            assert_eq!(Span::Over(at as u8).key(), over);
        }
        for key in ["", "2d", "over-", "over-2d", "over-over-1d", "1D"] {
            assert_eq!(Span::from_key(key), None, "{key}");
        }
    }
}
