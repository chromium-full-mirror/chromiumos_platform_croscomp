// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use log::debug;
use std::cell::RefCell;
use std::rc::Rc;

use crate::exo;
use crate::ffi;
use crate::protocol;
use crate::remote_shell;
use crate::server;
use crate::Compositor;

use nix::sys::signal;

pub struct Shell {
    pub native: ffi::desktop_shell,
    pub compositor: Rc<RefCell<Compositor>>,
    idle_listener: Box<dyn Drop>,
    wake_listener: Box<dyn Drop>,
}

impl Shell {
    pub fn new(rc: &Rc<RefCell<Compositor>>) -> Rc<RefCell<Self>> {
        let rc = {
            let compositor = &mut rc.borrow_mut();
            Rc::new(RefCell::new(Shell {
                native: ffi::desktop_shell {
                    compositor: &mut compositor.native as *mut _,
                    idle_listener: ffi::wl_listener::zeroed(),
                    wake_listener: ffi::wl_listener::zeroed(),
                    resized_listener: ffi::wl_listener::zeroed(),
                    destroy_listener: ffi::wl_listener::zeroed(),
                    show_input_panel_listener: ffi::wl_listener::zeroed(),
                    hide_input_panel_listener: ffi::wl_listener::zeroed(),
                    update_input_panel_listener: ffi::wl_listener::zeroed(),
                    fullscreen_layer: ffi::weston_layer::zeroed(),
                    panel_layer: ffi::weston_layer::zeroed(),
                    background_layer: ffi::weston_layer::zeroed(),
                    lock_layer: ffi::weston_layer::zeroed(),
                    input_panel_layer: ffi::weston_layer::zeroed(),
                    pointer_focus_listener: ffi::wl_listener::zeroed(),
                    grab_surface: std::ptr::null_mut(),
                    locked: false,
                    showing_input_panels: false,
                    prepare_event_sent: false,
                    text_input: ffi::desktop_shell__bindgen_ty_1 {
                        surface: std::ptr::null_mut(),
                        cursor_rectangle: ffi::pixman_box32::new(),
                    },
                    lock_surface: std::ptr::null_mut(),
                    lock_surface_listener: ffi::wl_listener::zeroed(),
                    workspaces: ffi::desktop_shell__bindgen_ty_2 {
                        array: ffi::wl_array::new(),
                        current: 0,
                        num: 2,
                        client_list: ffi::wl_list::zeroed(),
                        animation: ffi::weston_animation {
                            frame: None,
                            frame_counter: 0,
                            link: ffi::wl_list::zeroed(),
                        },
                        anim_sticky_list: ffi::wl_list::zeroed(),
                        anim_dir: 0,
                        anim_timestamp: ffi::timespec { tv_sec: 0, tv_nsec: 0 },
                        anim_current: 0.0,
                        anim_from: std::ptr::null_mut(),
                        anim_to: std::ptr::null_mut(),
                    },
                    input_panel: ffi::desktop_shell__bindgen_ty_3 {
                        binding: std::ptr::null_mut(),
                        surfaces: ffi::wl_list::zeroed(),
                    },
                    allow_zap: true,
                    binding_modifier: ffi::weston_keyboard_modifier::MODIFIER_SUPER.0,
                    win_animation_type: ffi::animation_type::ANIMATION_NONE,
                    win_close_animation_type: ffi::animation_type::ANIMATION_FADE,
                    startup_animation_type: ffi::animation_type::ANIMATION_ZOOM,
                    focus_animation_type: ffi::animation_type::ANIMATION_NONE,
                    minimized_layer: ffi::weston_layer::zeroed(),
                    seat_create_listener: ffi::wl_listener::zeroed(),
                    output_create_listener: ffi::wl_listener::zeroed(),
                    output_move_listener: ffi::wl_listener::zeroed(),
                    output_list: ffi::wl_list::zeroed(),
                    client: std::ptr::null_mut(),
                    startup_time: ffi::timespec { tv_sec: 0, tv_nsec: 0 },
                    initialized_inputs: false,
                },
                compositor: Rc::clone(rc),

                idle_listener: ffi::WlListener::new(&mut compositor.native.idle_signal, || {
                    debug!("-------------------- idle signal fired")
                }),
                wake_listener: ffi::WlListener::new(&mut compositor.native.wake_signal, || {
                    debug!("-------------------- wake signal fired")
                }),
            }))
        };

        rc.borrow_mut().init();
        rc
    }

    fn init_layers(&mut self) {
        let compositor = &mut self.compositor.borrow_mut().native as *mut _;

        self.native.fullscreen_layer.init(compositor);
        self.native
            .fullscreen_layer
            .set_position(ffi::weston_layer_position::WESTON_LAYER_POSITION_FULLSCREEN);
        self.native.panel_layer.init(compositor);
        self.native
            .panel_layer
            .set_position(ffi::weston_layer_position::WESTON_LAYER_POSITION_UI);
        self.native.background_layer.init(compositor);
        self.native
            .background_layer
            .set_position(ffi::weston_layer_position::WESTON_LAYER_POSITION_BACKGROUND);

        self.native.lock_layer.init(compositor);
        self.native.input_panel_layer.init(compositor);
        self.native.minimized_layer.init(compositor);

        unsafe { ffi::desktop_shell_init(&mut self.native, compositor) };
    }

    fn init_bindings(&mut self) {
        use crate::ffi::evdev::*;

        let mut compositor = self.compositor.borrow_mut();
        let rc = Rc::clone(&self.compositor);
        compositor.bind_key(
            KEY_BACKSPACE,
            ffi::weston_keyboard_modifier::MODIFIER_CTRL
                | ffi::weston_keyboard_modifier::MODIFIER_ALT,
            move |_, _, _| {
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

    fn init(&mut self) {
        self.native.workspaces.client_list.init();

        self.init_layers();

        self.init_bindings();

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

impl Drop for Shell {
    fn drop(&mut self) {
        debug!("-------------------- drop Shell")
    }
}

pub type Surface = ffi::shell_surface;

impl Surface {
    pub fn new(
        shell: &mut Shell,
        surface: *mut ffi::weston_surface,
        view: *mut ffi::weston_view,
    ) -> *mut Self {
        unsafe { ffi::shell_surface_create(&mut shell.native as *mut _, surface, view) }
    }

    pub fn get<'a>(surface: &mut ffi::weston_surface) -> Option<&'a mut Surface> {
        unsafe { ffi::get_shell_surface(surface).as_mut() }
    }

    pub fn r#move(&mut self, pointer: &mut ffi::weston_pointer, client_initiated: bool) {
        unsafe {
            ffi::shell_surface_move(self, pointer as *mut _, client_initiated);
        }
    }

    pub fn resize(&mut self, pointer: &mut ffi::weston_pointer, edges: u32) {
        unsafe {
            ffi::shell_surface_resize(self, pointer as *mut _, edges);
        }
    }

    pub fn set_size(&mut self, width: i32, height: i32) {
        unsafe { ffi::shell_surface_set_size(self, width, height) }
    }

    pub fn set_resizing(&mut self, edges: ffi::wl_shell_surface_resize) {
        unsafe { ffi::shell_surface_set_resizing(self, edges.0) }
    }

    pub fn get_geometry(&mut self) -> ffi::weston_geometry {
        unsafe { ffi::shell_surface_get_geometry(self) }
    }

    pub fn set_maximized(&mut self, enable: bool) -> ffi::weston_size {
        unsafe { ffi::shell_surface_set_maximized(self, enable) }
    }

    pub fn set_fullscreen(
        &mut self,
        enable: bool,
        output: *mut ffi::weston_output,
    ) -> ffi::weston_size {
        unsafe { ffi::shell_surface_set_fullscreen(self, enable, output) }
    }

    pub fn set_minimized(&mut self) {
        unsafe { ffi::shell_surface_set_minimized(self) }
    }
}
