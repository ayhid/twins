//! End-to-end tests of the `twins` binary.

use assert_cmd::Command;

fn twins() -> Command {
    Command::cargo_bin("twins").expect("twins binary is built")
}

#[test]
fn version_prints_crate_version() {
    let expected = format!("twins {}\n", env!("CARGO_PKG_VERSION"));
    twins().arg("version").assert().success().stdout(expected);
}
