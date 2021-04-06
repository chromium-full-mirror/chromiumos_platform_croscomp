// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use crate::Shell;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ffi;
use crate::ffi::evdev::{KEY_F1, KEY_F2};

struct Terminal {
    client_surface: Option<*mut ffi::weston_surface>,

    shell: Rc<RefCell<crate::Shell>>,

    client: Option<*const ffi::wl_client>,

    surface_create_listener: ffi::wl_listener,
    client_destroy_listener: ffi::wl_listener,
}

extern "C" fn client_destroy_notify(
    listener: *mut ffi::wl_listener,
    _data: *mut ::std::os::raw::c_void,
) {
    let mut t = unsafe { &mut *container_of_mut!(listener, Terminal, client_destroy_listener) };

    t.client = None;
    t.client_surface = None;
}

impl Terminal {
    fn hide(&mut self) {
        let mut shell = self.shell.borrow_mut();

        if let Some(surface) = self.client_surface {
            unsafe {
                let ds = ffi::weston_surface_get_desktop_surface(surface);
                ffi::weston_desktop_api_fullscreen_requested(
                    &mut shell.native,
                    ds,
                    false,
                    0 as *mut _,
                );
                ffi::weston_desktop_api_minimized_requested(&mut shell.native, ds);
            }
        }
    }

    fn show(&mut self, keyboard: &mut ffi::weston_keyboard) {
        let mut shell = self.shell.borrow_mut();

        match (self.client_surface, self.client) {
            (Some(surface), _) => unsafe {
                let ds = ffi::weston_surface_get_desktop_surface(surface);
                let view = ffi::get_default_view(surface);
                ffi::activate(
                    &mut shell.native,
                    view,
                    keyboard.seat,
                    ffi::weston_activate_flag::WESTON_ACTIVATE_FLAG_CONFIGURE as u32,
                );

                ffi::weston_desktop_api_fullscreen_requested(
                    &mut shell.native,
                    ds,
                    true,
                    (*surface).output,
                );
            },
            (None, None) => {
                let terminal_client_path = "/usr/bin/croscomp-terminal\0";

                let client = unsafe {
                    ffi::weston_client_start(
                        &mut shell.compositor.borrow_mut().native,
                        terminal_client_path.as_ptr() as *const i8,
                    )
                };
                unsafe {
                    ffi::wl_client_add_destroy_listener(
                        client,
                        &mut self.client_destroy_listener as *mut ffi::wl_listener,
                    )
                };
                self.client = Some(client);
            }
            _ => {}
        }
    }
}

extern "C" fn surface_create_notify(
    listener: *mut ffi::wl_listener,
    data: *mut ::std::os::raw::c_void,
) {
    let mut t = unsafe { &mut *container_of_mut!(listener, Terminal, surface_create_listener) };
    let surface = data as *mut ffi::weston_surface;
    let client = unsafe { ffi::wl_resource_get_client((*surface).resource) };

    if t.client == Some(client) {
        t.client_surface = Some(surface);
    }
}

pub fn init(shell: &Rc<RefCell<Shell>>) {
    let t = Rc::new(RefCell::new(Terminal {
        client_surface: None,
        shell: Rc::clone(shell),
        client: None,
        surface_create_listener: ffi::wl_listener::new(surface_create_notify),
        client_destroy_listener: ffi::wl_listener::new(client_destroy_notify),
    }));

    let s = shell.borrow();
    let mut compositor = s.compositor.borrow_mut();

    // signal add
    unsafe {
        ffi::wl_list_insert(
            &mut compositor.native.create_surface_signal.listener_list,
            &mut t.borrow_mut().surface_create_listener.link,
        );
    }

    let rc = Rc::clone(&t);
    compositor.bind_key(
        KEY_F1,
        ffi::weston_keyboard_modifier::MODIFIER_CTRL | ffi::weston_keyboard_modifier::MODIFIER_ALT,
        move |_, _, _| rc.borrow_mut().hide(),
    );

    let rc = Rc::clone(&t);
    compositor.bind_key(
        KEY_F2,
        ffi::weston_keyboard_modifier::MODIFIER_CTRL | ffi::weston_keyboard_modifier::MODIFIER_ALT,
        move |keyboard, _, _| rc.borrow_mut().show(keyboard),
    );
}
