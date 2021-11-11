extern crate wayland_scanner;

use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use wayland_scanner::*;

fn run_bindgen(header: &str, output: &str) {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let sysroot = env::var("SYSROOT").unwrap_or("/".to_string());

    let status = Command::new("bindgen")
        .args(&["-o", out_path.join(output).to_str().unwrap()])
        .arg("--no-doc-comments")
        .arg("--no-layout-tests")
        .args(&["--new-type-alias", "wl_fixed_t"])
        .args(&["--default-enum-style", "rust_non_exhaustive"])
        .args(&["--bitfield-enum", "weston_keyboard_modifier"])
        .args(&["--bitfield-enum", "wl_shell_surface_resize"])
        .args(&["--bitfield-enum", "weston_activate_flag"])
        .arg(header)
        .arg("--")
        .arg(format!("--sysroot={}", sysroot))
        .arg("-I/usr/include/pixman-1")
        .arg("-I./weston/include")
        .arg("-I./bindgen/block")
        .status()
        .unwrap();

    assert!(status.success())
}

fn main() {
    let protocols = [
        "wayland",
        "xdg-shell",
        "weston-screenshooter",
        "linux-dmabuf-unstable-v1",
        "croscomp",
        "alpha-compositing-unstable-v1",
        "aura-shell",
        "cursor-shapes-unstable-v1",
        "gaming-input-unstable-v2",
        "keyboard-configuration-unstable-v1",
        "keyboard-extension-unstable-v1",
        "pointer-gestures-unstable-v1",
        "remote-shell-unstable-v1",
        "secure-output-unstable-v1",
        "stylus-tools-unstable-v1",
        "stylus-unstable-v2",
        "vsync-feedback-unstable-v1",
    ];

    let out_dir_str = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_str);
    for e in &protocols {
        let spec = Path::new("protocol").join(e).with_extension("xml");
        let out = out_dir.join(e).with_extension("rs");
        println!("cargo:rerun-if-changed={}", spec.to_str().unwrap());
        generate_code_with_destructor_events(spec, out, Side::Server, &[("wl_callback", "done")]);
    }

    run_bindgen("/usr/include/linux/input.h", "input.rs");
    run_bindgen("./weston/croscomp_weston.h", "croscomp_weston.rs");

    let lib_paths = ["desktop-shell", "libweston"];

    let build_dir =
        env::var("MESON_BUILD_DIR").unwrap_or(format!("{}/../../../weston", out_dir_str));

    for p in &lib_paths {
        println!("cargo:rustc-flags=-L{}/{}", build_dir, p);
    }

    println!("cargo:rustc-link-lib=desktop-shell");
    println!("cargo:rustc-link-lib=weston-9");
    println!("cargo:rustc-link-lib=input");
    println!("cargo:rustc-link-lib=evdev");
    println!("cargo:rustc-link-lib=wayland-server");
    println!("cargo:rustc-link-lib=pixman-1");
}
