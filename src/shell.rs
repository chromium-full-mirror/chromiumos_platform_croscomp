// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::exo;
use crate::ffi;
use crate::protocol;
use crate::remote_shell;
use crate::server;
use crate::Compositor;

use nix::sys::signal;

#[derive(Debug)]
pub struct Shell {
    pub native: ffi::desktop_shell,
    pub compositor: Rc<RefCell<Compositor>>,
}

impl Shell {
    pub fn new(compositor: &Rc<RefCell<Compositor>>) -> Self {
        Shell {
            native: unsafe { std::mem::zeroed() },
            compositor: Rc::clone(compositor),
        }
    }

    pub fn init(&mut self) {
        self.native.allow_zap = true;
        self.native.binding_modifier = ffi::weston_keyboard_modifier::MODIFIER_SUPER.0;
        self.native.exposay_modifier = 0;
        self.native.win_animation_type = ffi::animation_type::ANIMATION_NONE;
        self.native.win_close_animation_type = ffi::animation_type::ANIMATION_FADE;
        self.native.startup_animation_type = ffi::animation_type::ANIMATION_ZOOM;
        self.native.focus_animation_type = ffi::animation_type::ANIMATION_NONE;
        self.native.workspaces.num = 2;

        unsafe {
            let native = &mut self.compositor.borrow_mut().native;
            ffi::desktop_shell_init(&mut self.native, native)
        };

        {
            use crate::ffi::evdev::*;

            let mut compositor = self.compositor.borrow_mut();
            let rc = Rc::clone(&self.compositor);
            compositor.bind_key(
                KEY_BACKSPACE,
                ffi::weston_keyboard_modifier::MODIFIER_CTRL
                    | ffi::weston_keyboard_modifier::MODIFIER_ALT,
                move |_, _, key| {
                    println!("key: {:?}", key);
                    let mut compositor = rc.borrow_mut();
                    compositor.server.terminate();
                },
            );

            let no_mods = ffi::weston_keyboard_modifier(0);
            let mods = ffi::weston_keyboard_modifier(self.native.binding_modifier);
            compositor.bind_key(KEY_K, mods, move |keyboard, _, _| {
                if let Some(surface) = unsafe { keyboard.focus.as_ref() } {
                    let resource = server::Resource::<protocol::wl_surface::WlSurface>::from_c_ptr(
                        surface.resource,
                    );
                    let client = unsafe { &mut *resource.client() };
                    let (pid, _, _) = client.get_credentials();
                    signal::kill(nix::unistd::Pid::from_raw(pid), signal::SIGTERM).unwrap();
                }
            });

            let rc = Rc::clone(&self.compositor);
            let backlight_binding =
                move |_: &mut ffi::weston_keyboard, _: *const ffi::timespec, key: u32| {
                    let mut compositor = rc.borrow_mut();
                    let mut output = match compositor.default_output() {
                        Some(o) => o,
                        _ => return,
                    };
                    let backlight = (output.backlight_current
                        + match key {
                            KEY_F9 | KEY_BRIGHTNESSDOWN => -25,
                            KEY_F10 | KEY_BRIGHTNESSUP => 25,
                            _ => return,
                        })
                    .min(255)
                    .max(5);

                    output.backlight_current = backlight;

                    if let Some(set_backlight) = output.set_backlight {
                        unsafe { set_backlight(output, backlight as u32) }
                    }
                };

            compositor.bind_key(KEY_BRIGHTNESSDOWN, no_mods, backlight_binding.clone());
            compositor.bind_key(KEY_BRIGHTNESSUP, no_mods, backlight_binding.clone());
            compositor.bind_key(KEY_F9, mods, backlight_binding.clone());
            compositor.bind_key(KEY_F10, mods, backlight_binding.clone());
        }

        self.create_background_surface();
    }

    pub fn finish_init(rc: &Rc<RefCell<crate::Shell>>) {
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
