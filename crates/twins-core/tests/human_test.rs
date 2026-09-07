//! Tests for human-readable size formatting and parsing.

use twins_core::human::{human_size, parse_size};

#[test]
fn human_size_uses_binary_units() {
    assert_eq!(human_size(0), "0 B");
    assert_eq!(human_size(1023), "1023 B");
    assert_eq!(human_size(1024), "1.0 KiB");
    assert_eq!(human_size(1_572_864), "1.5 MiB");
    assert_eq!(human_size(3 << 30), "3.0 GiB");
    assert_eq!(human_size(1 << 50), "1.0 PiB");
}

#[test]
fn parse_size_accepts_bare_numbers_and_units() {
    assert_eq!(parse_size("512").unwrap(), 512);
    assert_eq!(parse_size("10K").unwrap(), 10 * 1024);
    assert_eq!(parse_size("1.5MiB").unwrap(), 1_572_864);
    assert_eq!(parse_size("2 GB").unwrap(), 2 << 30);
    assert_eq!(parse_size(" 1 tib ").unwrap(), 1 << 40);
}

#[test]
fn parse_size_rejects_garbage() {
    for s in ["", "abc", "10X", "-1", "1..2", "K"] {
        assert!(parse_size(s).is_err(), "{s:?}");
    }
}
