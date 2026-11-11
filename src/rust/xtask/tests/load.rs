//! `cargo xtask load` (WP4-1): arguments and the free-disk guard.

use xtask::load::{LoadArgs, available_bytes, cargo_args, parse_args};

#[test]
fn arguments_select_profile_and_clients() {
    let none: [&str; 0] = [];
    assert_eq!(
        parse_args(none).unwrap(),
        LoadArgs {
            release: false,
            clients: None
        }
    );
    assert_eq!(
        parse_args(["--release", "--clients", "8"]).unwrap(),
        LoadArgs {
            release: true,
            clients: Some(8)
        }
    );
    assert!(parse_args(["--clients"]).is_err());
    assert!(parse_args(["--clients", "0"]).is_err());
    assert!(parse_args(["--fast"]).is_err());
}

#[test]
fn free_disk_comes_from_posix_df() {
    let out = "Filesystem     1024-blocks     Used Available Capacity Mounted on\n\
               /dev/sda3         58719164 35619764  20084068      64% /\n";
    assert_eq!(available_bytes(out).unwrap(), 20_084_068 * 1024);
    assert!(available_bytes("garbage").is_err());
}

/// The load test is a module of the one server test binary `it` (DEC-565): the run names
/// that target and filters on the module, ignored tests only.
#[test]
fn the_cargo_run_selects_the_load_module_of_the_one_test_binary() {
    let debug = cargo_args(LoadArgs {
        release: false,
        clients: None,
    });
    assert_eq!(
        debug,
        [
            "test",
            "-p",
            "server",
            "--test",
            "it",
            "--",
            "--ignored",
            "--nocapture",
            "load::"
        ]
    );
    let release = cargo_args(LoadArgs {
        release: true,
        clients: Some(8),
    });
    assert_eq!(release[..2], ["test", "--release"]);
    assert_eq!(release[2..], debug[1..]);
}
