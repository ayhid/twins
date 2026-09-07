//! Tests for protected-location detection.

use std::path::Path;

use twins_core::safety::{is_protected, is_user_library};

#[test]
fn system_locations_are_protected() {
    for p in [
        "/",
        "/System",
        "/System/Library",
        "/usr/bin",
        "/Applications/Safari.app",
        "/private/var/db",
        "/etc",
    ] {
        assert!(is_protected(Path::new(p)), "{p}");
    }
}

#[test]
fn temp_areas_carved_out_of_private_var_are_allowed() {
    for p in [
        "/private/var/folders/xx/T",
        "/var/tmp/x",
        "/tmp/x",
        "/private/tmp",
    ] {
        assert!(!is_protected(Path::new(p)), "{p}");
    }
}

#[test]
fn user_locations_are_not_protected() {
    for p in ["/Users/me/Documents", "/Volumes/Data", "/Systemic", "/usr2"] {
        assert!(!is_protected(Path::new(p)), "{p}");
    }
}

#[test]
fn user_library_is_detected_from_home() {
    let home = Path::new("/Users/me");
    assert!(is_user_library(home, Path::new("/Users/me/Library")));
    assert!(is_user_library(
        home,
        Path::new("/Users/me/Library/Caches/x")
    ));
    assert!(!is_user_library(home, Path::new("/Users/me/LibraryOld")));
    assert!(!is_user_library(home, Path::new("/Users/other/Library")));
}
