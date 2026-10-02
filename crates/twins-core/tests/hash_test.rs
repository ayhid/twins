//! Tests for partial / full hashing and byte comparison.

use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use twins_core::hash::{self, Digest};

const SAMPLE: usize = 16 * 1024;

fn write(dir: &tempfile::TempDir, name: &str, content: &[u8]) -> PathBuf {
    let p = dir.path().join(name);
    fs::write(&p, content).unwrap();
    p
}

/// Two buffers with identical head and tail samples but a different middle.
fn colliding_pair() -> (Vec<u8>, Vec<u8>) {
    let mut a = vec![7u8; 3 * SAMPLE];
    let mut b = a.clone();
    a[SAMPLE + 100] = 1;
    b[SAMPLE + 100] = 2;
    (a, b)
}

#[test]
fn partial_ignores_the_middle_of_large_files() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = colliding_pair();
    let pa = write(&dir, "a", &a);
    let pb = write(&dir, "b", &b);

    let ha = hash::partial(&pa, a.len() as u64).unwrap();
    let hb = hash::partial(&pb, b.len() as u64).unwrap();

    assert_eq!(ha, hb, "same head and tail must give the same partial hash");
}

#[test]
fn partial_reads_small_files_entirely() {
    let dir = tempfile::tempdir().unwrap();
    let pa = write(&dir, "a", b"abc");
    let pb = write(&dir, "b", b"abd");
    let pc = write(&dir, "c", b"abc");

    let ha = hash::partial(&pa, 3).unwrap();
    let hb = hash::partial(&pb, 3).unwrap();
    let hc = hash::partial(&pc, 3).unwrap();

    assert_ne!(ha, hb);
    assert_eq!(ha, hc);
}

#[test]
fn full_is_blake3_and_separates_partial_collisions() {
    let dir = tempfile::tempdir().unwrap();
    let empty = write(&dir, "empty", b"");
    assert_eq!(
        hash::full(&empty).unwrap().to_string(),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
    );

    let (a, b) = colliding_pair();
    let pa = write(&dir, "a", &a);
    let pb = write(&dir, "b", &b);
    let da: Digest = hash::full(&pa).unwrap();
    let db: Digest = hash::full(&pb).unwrap();
    assert_ne!(da, db);
    assert_eq!(da.as_bytes(), blake3::hash(&a).as_bytes());
}

#[test]
fn equal_compares_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let big = vec![9u8; 300 * 1024 + 17];
    let mut tail = big.clone();
    *tail.last_mut().unwrap() = 8;
    let a = write(&dir, "a", &big);
    let b = write(&dir, "b", &big);
    let c = write(&dir, "c", &tail);
    let d = write(&dir, "d", &big[..big.len() - 1]);

    assert!(hash::equal(&a, &b).unwrap());
    assert!(!hash::equal(&a, &c).unwrap());
    assert!(!hash::equal(&a, &d).unwrap());
    assert!(!hash::equal(&d, &a).unwrap());
}

#[test]
fn missing_files_are_errors_carrying_the_path() {
    let dir = tempfile::tempdir().unwrap();
    let nope = dir.path().join("nope");
    let err = hash::full(&nope).unwrap_err();
    assert!(err.to_string().contains("nope"), "{err}");
    assert!(hash::partial(&nope, 10).is_err());
    assert!(hash::equal(&nope, &nope).is_err());
}

/// Larger than one 256 KiB chunk, so a cancel check sits before several reads.
fn mib_of(byte: u8) -> Vec<u8> {
    vec![byte; 1024 * 1024]
}

#[test]
fn full_cancellable_stops_on_raised_flag() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(&dir, "big", &mib_of(3));

    let e = hash::full_cancellable(&p, &AtomicBool::new(true)).unwrap_err();

    assert_eq!(e.op, "cancelled");
    assert_eq!(e.source.kind(), io::ErrorKind::Interrupted);
    assert_eq!(e.path, p);
}

#[test]
fn full_cancellable_matches_full() {
    let dir = tempfile::tempdir().unwrap();
    let big = write(&dir, "big", &mib_of(5));
    let empty = write(&dir, "empty", b"");
    let never = AtomicBool::new(false);

    assert_eq!(
        hash::full_cancellable(&big, &never).unwrap(),
        hash::full(&big).unwrap()
    );
    assert_eq!(
        hash::full_cancellable(&empty, &never).unwrap(),
        hash::full(&empty).unwrap()
    );
}

#[test]
fn equal_cancellable_stops_on_raised_flag() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(&dir, "a", &mib_of(4));
    let b = write(&dir, "b", &mib_of(4));

    let e = hash::equal_cancellable(&a, &b, &AtomicBool::new(true)).unwrap_err();

    assert_eq!(e.op, "cancelled");
    assert_eq!(e.source.kind(), io::ErrorKind::Interrupted);
    assert_eq!(e.path, a);
}

#[test]
fn equal_cancellable_matches_equal() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(&dir, "a", &mib_of(6));
    let b = write(&dir, "b", &mib_of(6));
    let mut other = mib_of(6);
    *other.last_mut().unwrap() = 7;
    let c = write(&dir, "c", &other);
    let never = AtomicBool::new(false);

    assert_eq!(
        hash::equal_cancellable(&a, &b, &never).unwrap(),
        hash::equal(&a, &b).unwrap()
    );
    assert!(hash::equal_cancellable(&a, &b, &never).unwrap());
    assert_eq!(
        hash::equal_cancellable(&a, &c, &never).unwrap(),
        hash::equal(&a, &c).unwrap()
    );
    assert!(!hash::equal_cancellable(&a, &c, &never).unwrap());
}
