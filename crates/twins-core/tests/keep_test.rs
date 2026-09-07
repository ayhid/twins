//! Tests for keep strategies and plans.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use twins_core::fsutil::{FileMeta, Identity};
use twins_core::group::{Group, Keeper, Strategy, plan, total_reclaimable};
use twins_core::hash::Digest;

fn meta(path: &str, age_secs: u64, ino: u64) -> FileMeta {
    let mtime = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 - age_secs);
    FileMeta::new(PathBuf::from(path), 10, mtime, Identity::new(1, ino))
}

fn group(files: Vec<FileMeta>) -> Group {
    Group::new(10, Digest::from_hex(&"00".repeat(32)).unwrap(), files)
}

fn keeper(s: Strategy) -> Keeper {
    Keeper::new(s, None)
}

#[test]
fn parse_strategy() {
    assert_eq!("oldest".parse::<Strategy>().unwrap(), Strategy::Oldest);
    assert_eq!("newest".parse::<Strategy>().unwrap(), Strategy::Newest);
    assert_eq!(
        "shortest-path".parse::<Strategy>().unwrap(),
        Strategy::ShortestPath
    );
    assert_eq!("in-dir".parse::<Strategy>().unwrap(), Strategy::InDir);
    let err = "random".parse::<Strategy>().unwrap_err().to_string();
    assert!(err.contains("random") && err.contains("oldest"), "{err}");
    assert_eq!(Strategy::default(), Strategy::Oldest);
    assert_eq!(Strategy::ShortestPath.to_string(), "shortest-path");
}

#[test]
fn oldest_keeps_the_earliest_mtime() {
    let g = group(vec![
        meta("/x/new", 10, 1),
        meta("/x/old", 100, 2),
        meta("/x/mid", 50, 3),
    ]);
    let (keep, remove) = keeper(Strategy::Oldest).choose(&g).unwrap();
    assert_eq!(keep.path(), Path::new("/x/old"));
    assert_eq!(remove.len(), 2);
}

#[test]
fn newest_keeps_the_latest_mtime() {
    let g = group(vec![meta("/x/new", 10, 1), meta("/x/old", 100, 2)]);
    let (keep, _) = keeper(Strategy::Newest).choose(&g).unwrap();
    assert_eq!(keep.path(), Path::new("/x/new"));
}

#[test]
fn shortest_path_prefers_fewest_components_then_shortest_name() {
    let g = group(vec![
        meta("/a/b/c/file", 100, 1),
        meta("/a/longer-name", 10, 2),
        meta("/a/short", 10, 3),
    ]);
    let (keep, _) = keeper(Strategy::ShortestPath).choose(&g).unwrap();
    assert_eq!(keep.path(), Path::new("/a/short"));
}

#[test]
fn in_dir_prefers_files_under_dir_then_falls_back_to_oldest() {
    let g = group(vec![
        meta("/keep/new", 10, 1),
        meta("/keep/old", 100, 2),
        meta("/other/ancient", 1000, 3),
        meta("/keeper/decoy", 2000, 4),
    ]);
    let k = Keeper::new(Strategy::InDir, Some(PathBuf::from("/keep/")));
    let (keep, _) = k.choose(&g).unwrap();
    assert_eq!(keep.path(), Path::new("/keep/old"));

    let g2 = group(vec![meta("/other/b", 10, 1), meta("/other/a", 100, 2)]);
    let (keep, _) = k.choose(&g2).unwrap();
    assert_eq!(keep.path(), Path::new("/other/a"));
}

#[test]
fn ties_break_on_depth_then_path() {
    let g = group(vec![
        meta("/a/b/z", 10, 1),
        meta("/a/y", 10, 2),
        meta("/a/x", 10, 3),
    ]);
    let (keep, _) = keeper(Strategy::Oldest).choose(&g).unwrap();
    assert_eq!(keep.path(), Path::new("/a/x"));
}

#[test]
fn never_removes_hardlinks_of_the_kept_file() {
    let g = group(vec![
        meta("/x/old", 100, 1),
        meta("/x/link", 100, 1),
        meta("/x/copy", 10, 2),
    ]);
    let (keep, remove) = keeper(Strategy::Oldest).choose(&g).unwrap();
    assert_eq!(keep.identity(), Identity::new(1, 1));
    assert_eq!(remove.len(), 1);
    assert_eq!(remove[0].path(), Path::new("/x/copy"));
}

#[test]
fn plan_reclaimable_counts_physical_bytes_once() {
    let g1 = group(vec![
        meta("/a", 100, 1),
        meta("/b", 10, 2),
        meta("/b2", 10, 2),
    ]);
    let g2 = group(vec![meta("/c", 100, 3), meta("/d", 10, 4)]);
    let actions = plan(&[g1, g2], &keeper(Strategy::Oldest));
    assert_eq!(actions[0].reclaimable(), 10);
    assert_eq!(actions[1].reclaimable(), 10);
    assert_eq!(total_reclaimable(&actions), 20);
}

#[test]
fn empty_group_yields_no_decision_and_no_action() {
    let g = group(vec![]);
    assert!(keeper(Strategy::Oldest).choose(&g).is_none());
    assert!(plan(&[g], &keeper(Strategy::Oldest)).is_empty());
}
