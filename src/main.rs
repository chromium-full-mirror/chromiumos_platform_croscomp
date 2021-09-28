// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[macro_use]
extern crate bitflags;

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
mod server;
mod shell;
mod splash;
mod switcher;
mod terminal;

use compositor::Compositor;
use server::Server;
use shell::Shell;

const SCALE: i32 = 2;

fn bind_socket() -> UnixListener {
    let path = env::var_os("XDG_RUNTIME_DIR")
        .expect("environment variable XDG_RUNTIME_DIR is not set.")
        .into_string()
        .expect("invalid value");

    let socket_path = format!("{}/wayland-0", path);
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

extern "C" fn head_notify(_listener: *mut ffi::wl_listener, data: *mut ::std::os::raw::c_void) {
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

            let output = native.create_output("output\0");
            output.attach_head(head);

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
                println!("weston_output_enable return non-0");
            }
        }
    }
}

fn main() -> std::io::Result<()> {
    let ctx = unsafe { ffi::weston_log_ctx_create() };

    let compositor = Rc::new(RefCell::new(Compositor::new(ctx)));
    compositor.borrow_mut().init();

    unsafe { ffi::weston_log_set_handler(Some(ffi::vlog), Some(ffi::vlog_continue)) }

    let mut listener = ffi::wl_listener::new(head_notify);

    compositor
        .borrow_mut()
        .add_heads_changed_listener(&mut listener);

    let mut rules = ffi::xkb_rule_names {
        rules: std::ptr::null(),
        model: std::ptr::null(),
        layout: std::ptr::null(),
        variant: std::ptr::null(),
        options: std::ptr::null(),
    };

    compositor.borrow_mut().set_xkb_rule_names(&mut rules);

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

    compositor.borrow_mut().flush_heads_changed();

    compositor.borrow_mut().server.event_loop.on_signal(
        nix::sys::signal::SIGCHLD as i32,
        move |_sig| {
            let _ = waitpid(None, Some(WaitPidFlag::WNOHANG));
        },
    );

    let sock = bind_socket();

    let rc = Rc::clone(&compositor);
    compositor
        .borrow_mut()
        .server
        .event_loop
        .on_connect(sock, move |fd, _| {
            let compositor = rc.borrow_mut();
            compositor.create_client(fd);
        });

    let shell = Rc::new(RefCell::new(Shell::new(&compositor)));
    shell.borrow_mut().init();

    display::init(&shell);

    terminal::init(&shell);

    splash::init(&shell);

    switcher::init(&shell);

    move_resize::init(&shell);

    rotate::init(&shell);

    aura_shell::init(&shell);

    compositor.borrow_mut().wake();

    unsafe {
        let display = compositor.borrow_mut().server.native;
        ffi::wl_display_run(display)
    }

    Ok(())
}
