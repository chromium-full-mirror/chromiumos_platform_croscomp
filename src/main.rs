// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#[macro_use]
extern crate bitflags;

use nix::sys::signal::{kill, sigprocmask, SigSet, SigmaskHow};
use nix::sys::wait::{waitpid, WaitPidFlag};
use nix::unistd::dup;

use std::env::set_var;

use std::cell::RefCell;
use std::env;
use std::io::ErrorKind;
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::rc::Rc;

#[macro_use]
mod ffi;
mod aura_shell;
mod display;
mod event_loop;
mod exo;
mod input;
mod move_resize;
mod protocol;
mod remote_shell;
mod rotate;
mod server;
mod splash;
mod switcher;
mod terminal;

use server::Server;

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

#[derive(Debug)]
pub struct Compositor {
    server: Server,
    native: ffi::weston_compositor,
}

impl Compositor {
    fn new() -> Self {
        Compositor {
            server: Server::new(),
            native: unsafe { std::mem::zeroed() },
        }
    }

    fn init(&mut self, ctx: *mut ffi::weston_log_context) {
        unsafe {
            ffi::weston_compositor_init(
                &mut self.native,
                self.server.native,
                ctx,
                std::ptr::null_mut(),
            )
        };
    }

    fn add_heads_changed_listener(&mut self, listener: &mut ffi::wl_listener) {
        unsafe { ffi::weston_compositor_add_heads_changed_listener(&mut self.native, listener) }
    }

    fn bind_key<F>(&mut self, key: u32, modifier: ffi::weston_keyboard_modifier, f: F)
    where
        F: Fn(&mut ffi::weston_keyboard, *const ffi::timespec, u32),
    {
        extern "C" fn call_key_closure<F>(
            keyboard: *mut ffi::weston_keyboard,
            time: *const ffi::timespec,
            key: u32,
            data: *mut std::ffi::c_void,
        ) where
            F: Fn(&mut ffi::weston_keyboard, *const ffi::timespec, u32),
        {
            let keyboard = unsafe { &mut *keyboard };
            (unsafe { &mut *(data as *mut F) })(keyboard, time, key)
        }

        let handler = Box::new(f);

        unsafe {
            ffi::weston_compositor_add_key_binding(
                &mut self.native,
                key,
                modifier,
                Some(call_key_closure::<F>),
                Box::into_raw(handler) as *mut _,
            )
        };
    }

    fn bind_button<F>(&mut self, button: u32, modifier: ffi::weston_keyboard_modifier, f: F)
    where
        F: Fn(&mut ffi::weston_pointer, *const ffi::timespec, u32),
    {
        extern "C" fn call_button_closure<F>(
            pointer: *mut ffi::weston_pointer,
            time: *const ffi::timespec,
            key: u32,
            data: *mut std::ffi::c_void,
        ) where
            F: Fn(&mut ffi::weston_pointer, *const ffi::timespec, u32),
        {
            let pointer = unsafe { &mut *pointer };
            (unsafe { &mut *(data as *mut F) })(pointer, time, key)
        }

        let handler = Box::new(f);

        unsafe {
            ffi::weston_compositor_add_button_binding(
                &mut self.native,
                button,
                modifier,
                Some(call_button_closure::<F>),
                Box::into_raw(handler) as *mut _,
            )
        };
    }

    pub fn create_client(&self, fd: RawFd) -> *mut ffi::wl_client {
        unsafe { ffi::wl_client_create(self.server.native, fd) }
    }

    pub fn launch_client(&mut self, path: &str) -> std::io::Result<*mut ffi::wl_client> {
        let (client_stream, server_stream) = UnixStream::pair()?;
        let fd = client_stream.as_raw_fd();

        unsafe {
            Command::new(path)
                .pre_exec(move || {
                    sigprocmask(SigmaskHow::SIG_UNBLOCK, Some(&SigSet::all()), None)
                        .expect("sigprocmask failed");
                    let fd = dup(fd).expect("dup failed");
                    set_var("WAYLAND_SOCKET", fd.to_string());
                    Ok(())
                })
                .spawn()?;
        }

        Ok(self.create_client(server_stream.into_raw_fd()))
    }

    fn wake(&mut self) {
        unsafe { ffi::weston_compositor_wake(&mut self.native) }
    }

    fn flush_heads_changed(&mut self) {
        unsafe { ffi::weston_compositor_flush_heads_changed(&mut self.native) }
    }

    fn set_xkb_rule_names(&mut self, names: &mut ffi::xkb_rule_names) {
        unsafe {
            ffi::weston_compositor_set_xkb_rule_names(&mut self.native, names);
        }
    }

    fn head_iter(&self) -> ffi::WlListIter<ffi::weston_head> {
        wl_list_iter!(&self.native.head_list, ffi::weston_head, compositor_link)
    }

    fn head_iter_mut(&mut self) -> ffi::WlListIterMut<ffi::weston_head> {
        wl_list_iter_mut!(
            &mut self.native.head_list,
            ffi::weston_head,
            compositor_link
        )
    }

    fn create_surface(&self) -> *mut ffi::weston_surface {
        unsafe { ffi::weston_surface_create(&self.native as *const _ as *mut _) }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        println!("drop compositor");
        unsafe { ffi::weston_compositor_deinit(&mut self.native) }
    }
}

#[derive(Debug)]
pub struct Shell {
    native: ffi::desktop_shell,
    compositor: Rc<RefCell<Compositor>>,
}

impl Shell {
    fn new(compositor: &Rc<RefCell<Compositor>>) -> Self {
        Shell {
            native: unsafe { std::mem::zeroed() },
            compositor: Rc::clone(compositor),
        }
    }

    fn init(&mut self) {
        unsafe {
            let native = &mut self.compositor.borrow_mut().native;
            ffi::desktop_shell_init(&mut self.native, native)
        };

        self.create_background_surface();
    }

    fn finish_init(rc: &Rc<RefCell<crate::Shell>>) {
        unsafe {
            let shell = &rc.borrow();
            ffi::weston_compositor_init_input(&mut shell.compositor.borrow_mut().native);
        }
        exo::init(rc);
        remote_shell::init(rc);
    }

    fn create_background_surface(&mut self) {
        let compositor = self.compositor.borrow_mut();
        let mut surface = unsafe { &mut (*compositor.create_surface()) };

        surface.committed = None;
        // xx set label func
        surface.set_size(1024 * 1024, 1024 * 1024);
        surface.set_color(0.2, 0.1, 0.1, 1.0);
        surface.damage();

        let view = unsafe { &mut *surface.create_view() };
        view.set_position(0.0, 0.0);
        view.geometry_dirty();
        view.is_mapped = true;

        unsafe {
            ffi::weston_layer_entry_insert(
                &mut self.native.background_layer.view_list,
                &mut view.layer_link,
            );
        }
    }
}

fn main() -> std::io::Result<()> {
    let ctx = unsafe { ffi::weston_log_ctx_create() };

    let compositor = Rc::new(RefCell::new(Compositor::new()));
    compositor.borrow_mut().init(ctx);

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
