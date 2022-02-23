extern crate cc;
extern crate wayland_scanner;

use rayon::prelude::*;
use std::env;
use std::fs::File;
use std::io::ErrorKind;
use std::io::Write;
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
        .args(&["--bitfield-enum", "weston_activate_flag"])
        .args(&["--bitfield-enum", "wl_event_mask"])
        .args(&["--no-copy", "shell_surface"])
        .arg(header)
        .arg("--")
        .arg(format!("--sysroot={}", sysroot))
        .arg("-I./wayland-server")
        .arg("-I/usr/include/pixman-1")
        .arg("-I./weston/include")
        .arg("-I./bindgen/block")
        .status()
        .unwrap();

    assert!(status.success())
}

struct ConfigHeader {
    contents: String,
}

impl ConfigHeader {
    fn new() -> ConfigHeader {
        ConfigHeader {
            contents: "/* automatically generated header */\n\n".to_string(),
        }
    }

    fn define(&mut self, name: &str, value: impl std::fmt::Display) -> &mut Self {
        self.contents
            .push_str(&format!("#define {} {}\n", name, value.to_string()));

        self
    }

    fn define_escaped(&mut self, name: &str, value: impl std::fmt::Display) -> &mut Self {
        self.define(name, format!("\"{}\"", value));

        self
    }

    fn build(&self, path: &str, name: &str) {
        File::create(format!("{}/{}", path, name))
            .unwrap()
            .write(self.contents.as_bytes())
            .unwrap();
    }
}

fn generate_header_files(config: &Config) {
    ConfigHeader::new()
        .define("HAVE_ACCEPT4", 1)
        .define("HAVE_MEMFD_CREATE", 1)
        .define_escaped("LIBWESTON_MODULEDIR", "/dev/null")
        .define_escaped("DATADIR", &config.data_dir)
        .define("_GNU_SOURCE", 1)
        .define("_ALL_SOURCE", 1)
        .define("MAJOR_IN_SYSMACROS", 1)
        .build(&config.out_path, "config.h");

    ConfigHeader::new()
        .define("BUILD_ID", "c001bead")
        .build(&config.out_path, "git-version.h");

    std::fs::create_dir(format!("{}/libweston", config.out_path))
        .or_else(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })
        .unwrap();

    ConfigHeader::new()
        .define("WESTON_VERSION_MAJOR", 9)
        .define("WESTON_VERSION_MINOR", 0)
        .define("WESTON_VERSION_MICRO", 0)
        .define_escaped("WESTON_VERSION", "9.0.0")
        .build(&config.out_path, "libweston/version.h");
}

fn generate_rust_protocol_code(config: &Config) {
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

    let out_dir = Path::new(&config.out_path);

    protocols.par_iter().for_each(|e| {
        let spec = Path::new("protocol").join(e).with_extension("xml");
        let out = out_dir.join(e).with_extension("rs");
        println!("cargo:rerun-if-changed={}", spec.to_str().unwrap());
        generate_code_with_destructor_events(spec, out, Side::Server, &[("wl_callback", "done")]);
    });
}

fn generate_protocol_code(config: &Config) -> Vec<String> {
    let wp = "/usr/share/wayland-protocols";

    let stable = ["viewporter", "presentation-time", "xdg-shell"];

    let unstable = [
        ("input-method", 1),
        ("input-timestamps", 1),
        ("linux-dmabuf", 1),
        ("linux-explicit-synchronization", 1),
        ("xdg-output", 1),
        ("relative-pointer", 1),
        ("pointer-constraints", 1),
        ("fullscreen-shell", 1),
    ];

    let internal = [
        "wayland",
        "weston-content-protection",
        "weston-touch-calibration",
        "weston-debug",
        "weston-direct-display",
        "text-cursor-position",
    ];

    let protocols = stable
        .iter()
        .map(|name| {
            (
                format!("{}/stable/{}/{}.xml", wp, name, name),
                format!("{}/{}", config.out_path, name),
            )
        })
        .chain(unstable.iter().map(|(name, version)| {
            (
                format!(
                    "{}/unstable/{}/{}-unstable-v{}.xml",
                    wp, name, name, version
                ),
                format!("{}/{}-unstable-v{}", config.out_path, name, version),
            )
        }))
        .chain(internal.iter().map(|name| {
            (
                format!("protocol/{}.xml", name),
                format!("{}/{}", config.out_path, name),
            )
        }));

    fn generate(xml: &str, output: &str) -> String {
        for (t, suffix) in [
            ("client-header", "client-protocol.h"),
            ("server-header", "server-protocol.h"),
            ("private-code", "protocol.c"),
        ] {
            let output_file = format!("{}-{}", output, suffix);
            Command::new("wayland-scanner")
                .args([t, &xml, &output_file])
                .status()
                .unwrap();
        }

        format!("{}-protocol.c", output)
    }

    protocols
        .map(|(xml, output)| generate(&xml, &output))
        .collect()
}

fn subdir<A: std::fmt::Display>(prefix: &str, iter: impl IntoIterator<Item = A>) -> Vec<String> {
    iter.into_iter()
        .map(|f| format!("{}/{}", prefix, f))
        .collect()
}

fn build_libweston(config: &Config) {
    generate_header_files(&config);

    let protocol_c_files = generate_protocol_code(&config);

    let mut builder = cc::Build::new();

    let mut deps = vec!["libdrm", "pixman-1", "xkbcommon", "libffi"];

    builder
        .cargo_metadata(false)
        .files(subdir(
            "weston/libweston",
            [
                "animation.c",
                "bindings.c",
                "clipboard.c",
                "compositor.c",
                "content-protection.c",
                "data-device.c",
                "input.c",
                "linux-dmabuf.c",
                "linux-explicit-synchronization.c",
                "linux-sync-file.c",
                "log.c",
                "noop-renderer.c",
                "pixel-formats.c",
                "pixman-renderer.c",
                "plugin-registry.c",
                "screenshooter.c",
                "timeline.c",
                "touch-calibration.c",
                "weston-log-wayland.c",
                "weston-log-file.c",
                "weston-log-flight-rec.c",
                "weston-log.c",
                "weston-direct-display.c",
                "zoom.c",
            ],
        ))
        .files(subdir(
            "weston/shared",
            [
                "file-util.c",
                "image-loader.c",
                "matrix.c",
                "os-compatibility.c",
            ],
        ))
        .files(protocol_c_files)
        .extra_warnings(false)
        .flag("-std=gnu99")
        .include("weston/include")
        .include("weston/libweston")
        .include("weston")
        .include(&config.out_path);

    // Internal wayland-server
    builder.files(subdir(
        "wayland-server",
        [
            "wayland-server.c",
            "wayland-shm.c",
            "event-loop.c",
            "connection.c",
            "wayland-os.c",
            "wayland-util.c",
        ],
    ));

    #[cfg(feature = "wayland-backend")]
    {
        builder
            .files(subdir("weston/libweston/backend-wayland", ["wayland.c"]))
            .files(subdir("weston/shared", ["frame.c", "cairo-util.c"]));

        deps.extend(["wayland-client", "wayland-cursor", "cairo", "libpng"])
    }

    #[cfg(feature = "drm-backend")]
    {
        builder
            .files(subdir(
                "weston/libweston/backend-drm",
                &[
                    "drm.c",
                    "fb.c",
                    "modes.c",
                    "kms.c",
                    "state-helpers.c",
                    "state-propose.c",
                    "libbacklight.c",
                ],
            ))
            .files(subdir(
                "weston/libweston",
                &[
                    "launcher-direct.c",
                    "launcher-util.c",
                    "launcher-weston-launch.c",
                    "libinput-device.c",
                    "libinput-seat.c",
                ],
            ));

        deps.extend(["libinput", "libevdev", "libudev"]);
    }

    // builder.compile() normally prints these, but we need to list
    // them now so that croscomp-libweston will appear before the
    // pkg-config shared libraries on the link line.
    println!("cargo:rustc-link-lib=static=croscomp-libweston");
    println!("cargo:rustc-link-search=native={}", config.out_path);

    for d in deps {
        let lib = pkg_config::probe_library(d).unwrap();
        builder.includes(&lib.include_paths);
    }

    builder.compile("croscomp-libweston");
}

struct Config {
    out_path: String,
    data_dir: String,
}

fn main() {
    let config = Config {
        out_path: env::var("OUT_DIR").unwrap(),
        data_dir: "/usr/share".to_string(),
    };

    build_libweston(&config);

    run_bindgen("/usr/include/linux/input.h", "input.rs");
    run_bindgen("./weston/croscomp_weston.h", "croscomp_weston.rs");

    generate_rust_protocol_code(&config);
}
