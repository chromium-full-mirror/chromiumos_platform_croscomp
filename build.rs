extern crate wayland_scanner;

use std::env::var;
use std::path::Path;
use wayland_scanner::*;

fn generate_protocol(f: &str, out: &str) {
    let out_dir_str = var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);

    println!("cargo:rerun-if-changed={}", f);
    generate_code_with_destructor_events(
        f,
        out_dir.join(out),
        Side::Client,
        &[("wl_callback", "done")],
    );
}

fn main() {
    generate_protocol("./wayland.xml", "wayland_api.rs");
    generate_protocol("./linux-dmabuf-unstable-v1.xml",
		      "linux_dmabuf_unstable_v1_api.rs");
}
