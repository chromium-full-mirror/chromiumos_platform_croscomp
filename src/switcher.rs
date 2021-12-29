// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

use crate::ffi;
use crate::ffi::evdev::KEY_TAB;
use crate::input;
use crate::Shell;

struct Thumbnail {
    view: *mut ffi::weston_view,
    transform: ffi::weston_transform,
}

struct Switcher {
    shell: Rc<RefCell<crate::Shell>>,
    grab: input::KeyboardGrab,
    current: usize,
    view_destroy_listener: ffi::wl_listener,

    ui: Vec<*mut ffi::weston_view>,
    thumbnails: Vec<Thumbnail>,
}

impl Switcher {
    fn start(shell: &Rc<RefCell<Shell>>, keyboard: &mut ffi::weston_keyboard) {
        let mut switcher = Box::new(Switcher {
            shell: Rc::clone(shell),
            grab: input::KeyboardGrab::new(keyboard),
            current: 0,
            view_destroy_listener: ffi::wl_listener::new(view_destroy_notify),
            ui: Vec::new(),
            thumbnails: Vec::new(),
        });

        if keyboard.focus.is_null() {
            return;
        }

        switcher.view_destroy_listener.link.init();

        unsafe {
            ffi::lower_fullscreen_layer(&mut switcher.shell.borrow_mut().native, 0 as *mut _);
        }

        let output = unsafe { &(*(*keyboard.focus).output) };
        keyboard.set_focus(0 as *mut _);

        let p = switcher.as_mut() as *mut _;
        switcher.grab.start(p);

        switcher.build_ui(output);

        switcher.pick(true);

        let _ = Box::into_raw(switcher);
    }

    fn build_ui(&mut self, output: &ffi::weston_output) {
        let mut shell = self.shell.borrow_mut();
        let margin = 30;
        let q = output.height / 4;

        {
            let compositor = shell.compositor.borrow_mut();

            let mut surface = { unsafe { &mut (*compositor.create_surface()) } };

            surface.committed = None;
            // xx set label func
            surface.set_color(0.2, 0.2, 0.25, 0.99);
            surface.set_size(output.width - 2 * margin, output.height - 2 * q);
            surface.damage();

            let view = unsafe { &mut *surface.create_view() };
            view.set_position((output.x + margin) as f32, (output.y + q) as f32);
            view.radius = 100.0;
            view.geometry_dirty();
            view.is_mapped = true;

            self.ui.push(view);
        }

        {
            let workspace = unsafe { &mut *ffi::get_current_workspace(&mut shell.native) };

            let mut x = 2 * margin;
            self.thumbnails = wl_list_iter_mut!(
                &mut workspace.layer.view_list.link,
                ffi::weston_view,
                layer_link.link
            )
            .map(|view| {
                let surface = unsafe { &mut *view.surface };
                let icon = unsafe { &mut *surface.create_view() };

                // xxx need to use the shell surface extents here instead
                let scale = (2 * q - 2 * margin) as f32 / surface.height as f32;
                let y = (output.height - (surface.height as f32 * scale) as i32) / 2;
                icon.set_position(x as f32, y as f32);
                icon.geometry_dirty();
                icon.is_mapped = true;

                x = x + (surface.width as f32 * scale) as i32 + margin;

                Thumbnail {
                    view: icon as *mut ffi::weston_view,
                    transform: ffi::weston_transform {
                        matrix: ffi::weston_matrix::new().scale(scale, scale, 1.0),
                        link: ffi::wl_list::zeroed(),
                    },
                }
            })
            .collect();

            // Ugh, have to add the transform after adding to the Vec,
            // so that the views have a stable memory location.
            for t in &mut self.thumbnails {
                unsafe {
                    ffi::wl_list_insert(
                        &mut (*t.view).geometry.transformation_list as *mut ffi::wl_list,
                        &mut t.transform.link as *mut ffi::wl_list,
                    );
                }
            }
        }

        for view in self.ui.iter() {
            unsafe {
                ffi::weston_layer_entry_insert(
                    &mut shell.native.fullscreen_layer.view_list,
                    &mut (**view).layer_link,
                );
            }
        }

        for t in self.thumbnails.iter() {
            unsafe {
                ffi::weston_layer_entry_insert(
                    &mut shell.native.fullscreen_layer.view_list,
                    &mut (*t.view).layer_link,
                );
            }
        }
    }

    fn pick(&mut self, forwards: bool) {
        let len = self.thumbnails.len();
        if forwards {
            self.current = (self.current + len - 1) % len;
        } else {
            self.current = (self.current + 1) % len;
        }

        for (i, t) in self.thumbnails.iter().enumerate() {
            unsafe {
                let view = &mut *t.view;
                let surface = &mut *view.surface;
                view.alpha = if i == self.current { 1.0 } else { 0.25 };
                view.geometry_dirty();
                surface.damage();
            }
        }
    }

    fn shutdown(&mut self) {
        let seat = unsafe { &mut *(*self.grab.grab.keyboard).seat };

        let t = &mut self.thumbnails[self.current as usize];
        let view = unsafe { &mut *t.view };
        view.activate(
            seat,
            ffi::weston_activate_flag::WESTON_ACTIVATE_FLAG_CONFIGURE.0,
        );

        for view in self.ui.drain(..) {
            unsafe {
                let surface = (*view).surface;
                ffi::weston_surface_destroy(surface);
            }
        }

        for t in self.thumbnails.drain(..) {
            unsafe { ffi::weston_view_destroy(t.view) };
        }

        let _ = unsafe { Box::from_raw(self as *mut _) };
    }
}

impl input::KeyboardGrabHandler for Switcher {
    fn key(
        &mut self,
        keyboard: &mut ffi::weston_keyboard,
        _time: &ffi::timespec,
        key: u32,
        state: u32,
    ) {
        use ffi::wl_keyboard_key_state::*;
        const SHIFT: ffi::weston_keyboard_modifier = ffi::weston_keyboard_modifier::MODIFIER_SHIFT;

        let seat = unsafe { &mut *(*keyboard).seat };
        let forwards = !seat.modifier_state.is_set(SHIFT);

        if key == KEY_TAB && state == WL_KEYBOARD_KEY_STATE_PRESSED as u32 {
            self.pick(forwards);
        }
    }

    fn modifiers(
        &mut self,
        keyboard: &mut ffi::weston_keyboard,
        _serial: u32,
        _mods_depressed: u32,
        _mods_latched: u32,
        _mods_locked: u32,
        _group: u32,
    ) {
        let seat = unsafe { &mut *(*keyboard).seat };

        if !seat
            .modifier_state
            .is_set(ffi::weston_keyboard_modifier::MODIFIER_ALT)
        {
            self.shutdown()
        }
    }

    fn cancel(&mut self, _keyboard: &mut ffi::weston_keyboard) {
        self.shutdown()
    }
}

extern "C" fn view_destroy_notify(
    listener: *mut ffi::wl_listener,
    _data: *mut ::std::os::raw::c_void,
) {
    let switcher = unsafe { &mut *container_of_mut!(listener, Switcher, view_destroy_listener) };

    switcher.current = 0;
    switcher.pick(true);
}

pub fn init(rc: &Rc<RefCell<Shell>>) {
    let shell = rc.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    let shell = Rc::clone(&rc);
    compositor.bind_key(
        KEY_TAB,
        ffi::weston_keyboard_modifier::MODIFIER_ALT,
        move |keyboard, _, _| Switcher::start(&shell, keyboard),
    );

    let shell = Rc::clone(&rc);
    compositor.bind_key(
        KEY_TAB,
        ffi::weston_keyboard_modifier::MODIFIER_ALT | ffi::weston_keyboard_modifier::MODIFIER_SHIFT,
        move |keyboard, _, _| Switcher::start(&shell, keyboard),
    );
}
