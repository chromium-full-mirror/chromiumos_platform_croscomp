// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use log::debug;

use nix::sys::signal::{sigprocmask, SigSet, SigmaskHow};
use std::os::unix::io::{AsRawFd, IntoRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::process::Command;

use crate::ffi;
use crate::Server;

#[derive(Debug)]
pub struct Compositor {
    pub server: Server,
    pub native: ffi::weston_compositor,
}

impl Compositor {
    pub fn new(ctx: *mut ffi::weston_log_context) -> Self {
        let server = Server::new();
        let display_native = server.native;
        Compositor {
            server: server,
            native: ffi::weston_compositor {
                destroy_signal: ffi::wl_signal::zeroed(),
                wl_display: display_native,
                create_surface_signal: ffi::wl_signal::zeroed(),
                activate_signal: ffi::wl_signal::zeroed(),
                transform_signal: ffi::wl_signal::zeroed(),
                idle_signal: ffi::wl_signal::zeroed(),
                wake_signal: ffi::wl_signal::zeroed(),
                show_input_panel_signal: ffi::wl_signal::zeroed(),
                hide_input_panel_signal: ffi::wl_signal::zeroed(),
                update_input_panel_signal: ffi::wl_signal::zeroed(),
                seat_created_signal: ffi::wl_signal::zeroed(),
                output_created_signal: ffi::wl_signal::zeroed(),
                output_destroyed_signal: ffi::wl_signal::zeroed(),
                output_moved_signal: ffi::wl_signal::zeroed(),
                output_resized_signal: ffi::wl_signal::zeroed(),
                output_heads_changed_signal: ffi::wl_signal::zeroed(),
                session_signal: ffi::wl_signal::zeroed(),
                session_active: true,
                fade_layer: ffi::weston_layer::zeroed(),
                cursor_layer: ffi::weston_layer::zeroed(),
                pending_output_list: ffi::wl_list::zeroed(),
                output_list: ffi::wl_list::zeroed(),
                head_list: ffi::wl_list::zeroed(),
                seat_list: ffi::wl_list::zeroed(),
                layer_list: ffi::wl_list::zeroed(),
                view_list: ffi::wl_list::zeroed(),
                plane_list: ffi::wl_list::zeroed(),
                key_binding_list: ffi::wl_list::zeroed(),
                modifier_binding_list: ffi::wl_list::zeroed(),
                button_binding_list: ffi::wl_list::zeroed(),
                touch_binding_list: ffi::wl_list::zeroed(),
                axis_binding_list: ffi::wl_list::zeroed(),
                debug_binding_list: ffi::wl_list::zeroed(),
                state: ffi::WESTON_COMPOSITOR_IDLE as u32,
                idle_source: std::ptr::null_mut(),
                idle_inhibit: 0,
                idle_time: 0,
                repaint_timer: std::ptr::null_mut(),
                default_pointer_grab: std::ptr::null_mut(),
                primary_plane: unsafe { std::mem::zeroed() },
                capabilities: 0,
                renderer: std::ptr::null_mut(),
                read_format: ffi::pixman_format_code_t::PIXMAN_r8g8b8a8,
                backend: std::ptr::null_mut(),
                launcher: std::ptr::null_mut(),
                plugin_api_list: ffi::wl_list::zeroed(),
                output_id_pool: 0,
                xkb_names: ffi::xkb_rule_names {
                    rules: std::ptr::null_mut(),
                    model: std::ptr::null_mut(),
                    layout: std::ptr::null_mut(),
                    variant: std::ptr::null_mut(),
                    options: std::ptr::null_mut(),
                },
                xkb_context: std::ptr::null_mut(),
                xkb_info: std::ptr::null_mut(),
                kb_repeat_rate: 0,
                kb_repeat_delay: 0,
                vt_switching: false,
                presentation_clock: 0,
                repaint_msec: 7,
                activate_serial: 1,
                pointer_constraints: std::ptr::null_mut(),
                exit_code: 0,
                user_data: std::ptr::null_mut(),
                exit: None,
                require_input: true,
                heads_changed_signal: ffi::wl_signal::zeroed(),
                heads_changed_source: std::ptr::null_mut(),
                touch_mode: ffi::weston_touch_mode::WESTON_TOUCH_MODE_NORMAL,
                touch_calibration: std::ptr::null_mut(),
                touch_calibration_save: None,
                calibrator_layer: ffi::weston_layer::zeroed(),
                touch_calibrator: std::ptr::null_mut(),
                weston_log_ctx: ctx,
                debug_scene: std::ptr::null_mut(),
                timeline: std::ptr::null_mut(),
                content_protection: std::ptr::null_mut(),
            },
        }
    }

    pub fn init(&mut self) {
        self.native.destroy_signal.init();
        self.native.create_surface_signal.init();
        self.native.activate_signal.init();
        self.native.transform_signal.init();
        self.native.idle_signal.init();
        self.native.wake_signal.init();
        self.native.show_input_panel_signal.init();
        self.native.hide_input_panel_signal.init();
        self.native.update_input_panel_signal.init();
        self.native.seat_created_signal.init();
        self.native.output_created_signal.init();
        self.native.output_destroyed_signal.init();
        self.native.output_moved_signal.init();
        self.native.output_resized_signal.init();
        self.native.heads_changed_signal.init();
        self.native.output_heads_changed_signal.init();
        self.native.session_signal.init();

        self.native.view_list.init();
        self.native.plane_list.init();
        self.native.layer_list.init();
        self.native.seat_list.init();
        self.native.pending_output_list.init();
        self.native.output_list.init();
        self.native.head_list.init();
        self.native.key_binding_list.init();
        self.native.modifier_binding_list.init();
        self.native.button_binding_list.init();
        self.native.touch_binding_list.init();
        self.native.axis_binding_list.init();
        self.native.debug_binding_list.init();
        self.native.plugin_api_list.init();

        let compositor = &mut self.native as *mut _;
        self.native.fade_layer.init(compositor);
        self.native
            .fade_layer
            .set_position(ffi::weston_layer_position::WESTON_LAYER_POSITION_FADE);
        self.native.cursor_layer.init(compositor);
        self.native
            .cursor_layer
            .set_position(ffi::weston_layer_position::WESTON_LAYER_POSITION_CURSOR);

        unsafe { ffi::weston_compositor_init(&mut self.native) };
    }

    pub fn add_heads_changed_listener(&mut self, listener: &mut ffi::wl_listener) {
        unsafe { ffi::weston_compositor_add_heads_changed_listener(&mut self.native, listener) }
    }

    pub fn bind_key<F>(&mut self, key: u32, modifier: ffi::weston_keyboard_modifier, f: F)
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

    pub fn bind_button<F>(&mut self, button: u32, modifier: ffi::weston_keyboard_modifier, f: F)
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
            use nix::fcntl::{fcntl, FdFlag, F_GETFD, F_SETFD};

            Command::new(path)
                .env("WAYLAND_SOCKET", fd.to_string())
                .pre_exec(move || {
                    sigprocmask(SigmaskHow::SIG_UNBLOCK, Some(&SigSet::all()), None)
                        .expect("sigprocmask failed");
                    let mut flags = FdFlag::from_bits_truncate(fcntl(fd, F_GETFD).unwrap());
                    flags.remove(FdFlag::FD_CLOEXEC);
                    fcntl(fd, F_SETFD(flags)).unwrap();
                    Ok(())
                })
                .spawn()?;
        }

        Ok(self.create_client(server_stream.into_raw_fd()))
    }

    pub fn wake(&mut self) {
        unsafe { ffi::weston_compositor_wake(&mut self.native) }
    }

    pub fn flush_heads_changed(&mut self) {
        unsafe { ffi::weston_compositor_flush_heads_changed(&mut self.native) }
    }

    pub fn set_xkb_rule_names(&mut self, names: &mut ffi::xkb_rule_names) {
        unsafe {
            ffi::weston_compositor_set_xkb_rule_names(&mut self.native, names);
        }
    }

    pub fn head_iter(&self) -> ffi::WlListIter<ffi::weston_head> {
        wl_list_iter!(&self.native.head_list, ffi::weston_head, compositor_link)
    }

    pub fn head_iter_mut(&mut self) -> ffi::WlListIterMut<ffi::weston_head> {
        wl_list_iter_mut!(
            &mut self.native.head_list,
            ffi::weston_head,
            compositor_link
        )
    }

    pub fn output_iter_mut(&mut self) -> ffi::WlListIterMut<ffi::weston_output> {
        wl_list_iter_mut!(&mut self.native.output_list, ffi::weston_output, link)
    }

    pub fn default_output(&mut self) -> Option<&mut ffi::weston_output> {
        self.output_iter_mut().next()
    }

    pub fn create_surface(&self) -> *mut ffi::weston_surface {
        unsafe { ffi::weston_surface_create(&self.native as *const _ as *mut _) }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        debug!("drop compositor");
        unsafe { ffi::weston_compositor_deinit(&mut self.native) }
    }
}
