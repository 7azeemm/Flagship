fn main() {
    // sqlx embeds migrations at compile time. Track the directory as well as
    // existing files so adding a migration rebuilds the binary automatically.
    println!("cargo:rerun-if-changed=migrations");
}
