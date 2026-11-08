fn main() {
    cc::Build::new()
        .file("src/core/buffer.c")
        .warnings(true)
        .compile("vimpp_core");
    println!("cargo:rerun-if-changed=src/core/buffer.c");
}
