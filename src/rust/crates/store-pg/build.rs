//! `sqlx::migrate!` embeds the migrations at compile time; rebuild when one is added or
//! changed (otherwise a cached build, as in CI, runs an old set).
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
