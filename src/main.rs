// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[macro_use]
extern crate bitflags;

use log::{debug, info, warn};
use structopt::StructOpt;

use nix::sys::stat::{umask, Mode};
use nix::sys::wait::{waitpid, WaitPidFlag};

use std::cell::RefCell;
use std::env;
use std::io::ErrorKind;
use std::os::unix::net::{UnixListener, UnixStream};
use std::rc::Rc;

#[macro_use]
mod ffi;
mod aura_shell;
mod compositor;
mod display;
mod event_loop;
mod exo;
mod input;
mod move_resize;
mod protocol;
mod remote_shell;
mod rotate;
mod screenshooter;
mod server;
mod shell;
mod splash;
mod switcher;
mod terminal;
mod xdg_shell;

use compositor::Compositor;
use server::Server;
use shell::Shell;

const SCALE: i32 = 2;

struct Umask {
    old_mode: Mode,
}

impl Umask {
    fn with_mode(mode: Mode) -> Umask {
        Umask { old_mode: umask(mode) }
    }
}

impl Drop for Umask {
    fn drop(&mut self) {
        umask(self.old_mode);
    }
}

fn bind_socket(name: &str) -> UnixListener {
    let path = env::var_os("XDG_RUNTIME_DIR")
        .expect("environment variable XDG_RUNTIME_DIR is not set.")
        .into_string()
        .expect("invalid value");

    let _guard = Umask::with_mode(Mode::empty());

    let socket_path = format!("{}/{}", path, name);
    if let Ok(sock) = UnixListener::bind(&socket_path) {
        return sock;
    }

    match UnixStream::connect(&socket_path) {
        Ok(_) => panic!("croscomp already running"),
        Err(error) if error.kind() == ErrorKind::ConnectionRefused => {
            let _ = std::fs::remove_file(&socket_path);
        }
        _ => (),
    }

    match UnixListener::bind(&socket_path) {
        Ok(sock) => sock,
        Err(error) => {
            if error.kind() == ErrorKind::AddrInUse {
                panic!("address in use")
            } else {
                panic!("other error")
            }
        }
    }
}

extern "C" fn drm_head_notify(_listener: *mut ffi::wl_listener, data: *mut ::std::os::raw::c_void) {
    let native = unsafe { &mut *(data as *mut ffi::weston_compositor) };
    let compositor = unsafe { &mut *container_of_mut!(native as *mut _, Compositor, native) };

    for head in compositor
        .head_iter_mut()
        .filter(|h| h.connected && !h.is_enabled())
    {
        unsafe {
            let api = ffi::weston_plugin_api_get(
                native,
                ffi::WESTON_DRM_OUTPUT_API_NAME.as_ptr() as *const _,
                std::mem::size_of::<ffi::weston_drm_output_api>() as ffi::size_t,
            ) as *mut ffi::weston_drm_output_api;

            let output = native.create_output_with_head(head);

            use ffi::weston_drm_backend_output_mode::*;
            let set_mode = (*api).set_mode.unwrap();
            set_mode(
                output,
                WESTON_DRM_BACKEND_OUTPUT_PREFERRED,
                std::ptr::null(),
            );
            output.set_scale(SCALE);

            use ffi::wl_output_transform::*;
            output.set_transform(WL_OUTPUT_TRANSFORM_NORMAL);
            let set_gbm_format = (*api).set_gbm_format.unwrap();
            set_gbm_format(output, std::ptr::null());

            let set_seat = (*api).set_seat.unwrap();
            set_seat(output, "\0".as_ptr() as *const _);

            if ffi::weston_output_enable(output) != 0 {
                warn!("weston_output_enable return non-0");
            }
        }
    }
}

extern "C" fn wayland_head_notify(
    _listener: *mut ffi::wl_listener,
    data: *mut ::std::os::raw::c_void,
) {
    let native = unsafe { &mut *(data as *mut ffi::weston_compositor) };
    let compositor = unsafe { &mut *container_of_mut!(native as *mut _, Compositor, native) };

    for head in compositor
        .head_iter_mut()
        .filter(|h| h.connected && !h.is_enabled())
    {
        unsafe {
            let api = ffi::weston_plugin_api_get(
                native,
                ffi::WESTON_WINDOWED_OUTPUT_API_NAME.as_ptr() as *const _,
                std::mem::size_of::<ffi::weston_windowed_output_api>() as ffi::size_t,
            ) as *mut ffi::weston_windowed_output_api;

            let output = native.create_output_with_head(head);

            output.set_scale(1);

            use ffi::wl_output_transform::*;
            output.set_transform(WL_OUTPUT_TRANSFORM_NORMAL);

            let set_size = (*api).output_set_size.unwrap();
            set_size(output, 1024, 600);

            if ffi::weston_output_enable(output) != 0 {
                warn!("weston_output_enable return non-0");
            }
        }
    }
}

extern "C" fn weston_log_bridge(msg: *const ::std::os::raw::c_char) {
    unsafe {
        info!(
            "{}",
            std::ffi::CStr::from_ptr(msg)
                .to_str()
                .expect("invalid utf-8")
        )
    }
}

fn load_drm_backend(compositor: Rc<RefCell<Compositor>>) -> Result<ffi::wl_listener, ()> {
    let mut config = ffi::weston_drm_backend_config {
        base: ffi::weston_backend_config {
            struct_version: ffi::WESTON_DRM_BACKEND_CONFIG_VERSION,
            struct_size: std::mem::size_of::<ffi::weston_drm_backend_config>() as ffi::size_t,
        },
        tty: -1,
        use_pixman: false,
        seat_id: std::ptr::null_mut(),
        gbm_format: std::ptr::null_mut(),
        configure_device: None,
        pageflip_timeout: 0,
        specific_device: std::ptr::null_mut(),
        use_pixman_shadow: false,
        continue_without_input: true,
    };

    let _ret = unsafe {
        ffi::weston_compositor_load_backend(
            &mut compositor.borrow_mut().native,
            ffi::weston_compositor_backend::WESTON_BACKEND_DRM,
            &mut config.base,
        )
    };

    Ok(ffi::wl_listener::new(drm_head_notify))
}

fn load_wayland_backend(compositor: Rc<RefCell<Compositor>>) -> Result<ffi::wl_listener, ()> {
    let mut config = ffi::weston_wayland_backend_config {
        base: ffi::weston_backend_config {
            struct_version: ffi::WESTON_WAYLAND_BACKEND_CONFIG_VERSION,
            struct_size: std::mem::size_of::<ffi::weston_wayland_backend_config>() as ffi::size_t,
        },
        use_pixman: true,
        sprawl: false,
        display_name: 0 as *mut _,
        fullscreen: false,
        cursor_theme: 0 as *mut _,
        cursor_size: 32,
    };

    let native = &mut compositor.borrow_mut().native;

    unsafe {
        let res = ffi::weston_compositor_load_backend(
            native,
            ffi::weston_compositor_backend::WESTON_BACKEND_WAYLAND,
            &mut config.base,
        );
        if res == -1 {
            return Err(());
        };

        let api = ffi::weston_plugin_api_get(
            native,
            ffi::WESTON_WINDOWED_OUTPUT_API_NAME.as_ptr() as *const _,
            std::mem::size_of::<ffi::weston_windowed_output_api>() as ffi::size_t,
        ) as *mut ffi::weston_windowed_output_api;

        let create_head = (*api).create_head.unwrap();
        create_head(native, "head\0".as_ptr() as *mut _);
    }

    Ok(ffi::wl_listener::new(wayland_head_notify))
}

#[derive(StructOpt, Debug)]
#[structopt(name = "croscomp", about = "ChromeOS compositor")]
struct Opt {
    /// Use wayland backend
    #[structopt(long)]
    use_wayland: bool,

    /// Don't run splash screen
    #[structopt(long)]
    no_splash: bool,

    /// Name of wayland socket. Relative to XDG_RUNTIME_DIR.
    #[structopt(long)]
    socket: Option<String>,
}

fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();

    unsafe { ffi::weston_set_log_handler(Some(weston_log_bridge)) };
    env_logger::init();
    let ctx = unsafe { ffi::weston_log_ctx_create() };

    let compositor = Rc::new(RefCell::new(Compositor::new(ctx)));
    compositor.borrow_mut().init();

    let mut rules = ffi::xkb_rule_names {
        rules: std::ptr::null(),
        model: std::ptr::null(),
        layout: std::ptr::null(),
        variant: std::ptr::null(),
        options: std::ptr::null(),
    };

    compositor.borrow_mut().set_xkb_rule_names(&mut rules);

    let mut listener = if opt.use_wayland {
        load_wayland_backend(compositor.clone()).unwrap()
    } else {
        load_drm_backend(compositor.clone()).unwrap()
    };

    compositor
        .borrow_mut()
        .add_heads_changed_listener(&mut listener);

    compositor.borrow_mut().flush_heads_changed();

    compositor.borrow_mut().server.event_loop.on_signal(
        nix::sys::signal::SIGCHLD as i32,
        move |_sig| {
            let _ = waitpid(None, Some(WaitPidFlag::WNOHANG));
        },
    );

    let socket_name = match (&opt.socket, opt.use_wayland) {
        (None, true) => "wayland-10",
        (None, false) => "wayland-0",
        (Some(socket), _) => socket,
    };
    let sock = bind_socket(socket_name);

    let rc = Rc::clone(&compositor);
    compositor
        .borrow_mut()
        .server
        .event_loop
        .on_connect(sock, move |fd, _| {
            let compositor = rc.borrow_mut();
            compositor.create_client(fd);
        });

    let shell = Shell::new(&compositor);

    display::init(&shell);

    terminal::init(&shell);

    if !opt.no_splash {
        splash::init(&shell);
    }

    switcher::init(&shell);

    move_resize::init(&shell);

    rotate::init(&shell);

    aura_shell::init(&shell);

    screenshooter::init(&shell);

    xdg_shell::init(&shell);

    compositor.borrow_mut().wake();

    unsafe {
        let display = compositor.borrow_mut().server.native;
        ffi::wl_display_run(display)
    }

    debug!(
        "# exit from main: shell strong count {}",
        Rc::strong_count(&shell)
    );

    Ok(())
}
