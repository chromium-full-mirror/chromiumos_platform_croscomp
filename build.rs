extern crate wayland_scanner;
extern crate bindgen;

use std::env;
use std::path::Path;
use std::path::PathBuf;
use wayland_scanner::*;

fn generate_protocol(f: &str, out: &str) {
    let out_dir_str = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);

    println!("cargo:rerun-if-changed={}", f);
    generate_code_with_destructor_events(
        f,
        out_dir.join(out),
        Side::Server,
        &[("wl_callback", "done")],
    );
}

fn main() {
    generate_protocol("./wayland.xml", "wayland_api.rs");
    generate_protocol("./linux-dmabuf-unstable-v1.xml",
		      "linux_dmabuf_unstable_v1_api.rs");
    generate_protocol("./subprojects/weston/protocol/croscomp.xml",
		      "croscomp_api.rs");

    let bindings = bindgen::Builder::default()
	.clang_arg("-I/usr/include/pixman-1")
	.clang_arg("-I./subprojects/weston/include")
	.clang_arg("-I./bindgen/block")
        .header("./subprojects/weston/include/libweston/backend-drm.h")
        .header("./subprojects/weston/desktop-shell/shell.h")
        .header("./subprojects/weston/compositor/weston.h")
	.generate_comments(false)
	.new_type_alias("wl_fixed_t")
	.default_enum_style(bindgen::EnumVariation::Rust{non_exhaustive: true})
	.bitfield_enum("weston_keyboard_modifier")
	.bitfield_enum("wl_shell_surface_resize")
	.layout_tests(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    let bindings = bindgen::Builder::default()
        .header("/usr/include/linux/input.h")
	.generate_comments(false)
        .generate()
        .expect("Unable to generate input.h bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("input.rs"))
        .expect("Couldn't write bindings!");

    println!("cargo:rustc-flags=-Lx86_64/subprojects/weston/compositor");
    println!("cargo:rustc-link-lib=exec_weston");
    println!("cargo:rustc-flags=-Lx86_64/subprojects/weston/desktop-shell");
    println!("cargo:rustc-link-lib=desktop-shell");
    println!("cargo:rustc-flags=-Lx86_64/subprojects/weston/libweston");
    println!("cargo:rustc-link-lib=weston-9");
    println!("cargo:rustc-flags=-Lx86_64/subprojects/libinput");
    println!("cargo:rustc-link-lib=input");
    println!("cargo:rustc-flags=-Lx86_64/subprojects/libevdev");
    println!("cargo:rustc-link-lib=evdev-fdo");
    println!("cargo:rustc-link-lib=wayland-server");
    println!("cargo:rustc-link-lib=pixman-1");
}
