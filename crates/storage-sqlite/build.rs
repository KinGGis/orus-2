fn main() {
    // Re-run if any migration file changes so that diesel_migrations::embed_migrations!()
    // picks up new SQL files at compile time.
    println!("cargo:rerun-if-changed=migrations");
}
