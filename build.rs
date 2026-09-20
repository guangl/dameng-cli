fn main() {
    println!(
        "cargo:rustc-env=DM_HOST_TARGET={}",
        std::env::var("TARGET").unwrap()
    );
}
