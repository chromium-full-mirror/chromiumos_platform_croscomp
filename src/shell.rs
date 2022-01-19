// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use log::debug;
use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use crate::exo;
use crate::ffi;
use crate::move_resize::{MoveGrab, ResizeGrab};
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
    activate_listener: Box<dyn Any>,
}

impl Shell {
    pub fn new(rc: &Rc<RefCell<Compositor>>) -> Rc<RefCell<Self>> {
        let rc = {
            let compositor = &mut rc.borrow_mut();
            let compositor_rc = Rc::clone(rc);
            Rc::new(RefCell::new(Shell {
                native: ffi::desktop_shell {
                    compositor: &mut compositor.native as *mut _,

                    fullscreen_layer: ffi::weston_layer::zeroed(),
                    background_layer: ffi::weston_layer::zeroed(),

                    workspaces: ffi::desktop_shell__bindgen_ty_1 {
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

                    binding_modifier: ffi::weston_keyboard_modifier::MODIFIER_SUPER.0,
                    minimized_layer: ffi::weston_layer::zeroed(),
                },
                compositor: Rc::clone(rc),

                idle_listener: ffi::WlListener::new(&mut compositor.native.idle_signal, |_| {
                    debug!("-------------------- idle signal fired")
                }),
                wake_listener: ffi::WlListener::new(&mut compositor.native.wake_signal, |_| {
                    debug!("-------------------- wake signal fired")
                }),
                activate_listener: ffi::WlListener::new(
                    &mut compositor.native.activate_signal,
                    move |data| unsafe {
                        let data = *(data as *mut ffi::weston_surface_activation_data);
                        let mut compositor = compositor_rc.borrow_mut();

                        compositor.server.event_loop.when_idle(move || {
                            if let Some(shsurf) = Surface::get(data.surface) {
                                shsurf.borrow_mut().activate(data.seat)
                            }
                        })
                    },
                ),
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
        self.native.background_layer.init(compositor);
        self.native
            .background_layer
            .set_position(ffi::weston_layer_position::WESTON_LAYER_POSITION_BACKGROUND);

        self.native.minimized_layer.init(compositor);

        unsafe { ffi::init_workspaces(&mut self.native) };
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
        let mods_with_shift = ffi::weston_keyboard_modifier(self.native.binding_modifier)
            | ffi::weston_keyboard_modifier::MODIFIER_SHIFT;

        let pointer_activate_binding = move |pointer: &mut ffi::weston_pointer, _, _| {
            if pointer.grab != &mut pointer.default_grab as *mut _ {
                return;
            }
            if let Some(view) = unsafe { pointer.focus.as_mut() } {
                view.activate(pointer.seat, 0)
            }
        };
        let touch_activate_binding = move |touch: &mut ffi::weston_touch, _| {
            if touch.grab != &mut touch.default_grab as *mut _ {
                return;
            }
            if let Some(view) = unsafe { touch.focus.as_mut() } {
                view.activate(touch.seat, 0)
            }
        };
        compositor.bind_button(BTN_LEFT, no_mods, pointer_activate_binding);
        compositor.bind_button(BTN_RIGHT, no_mods, pointer_activate_binding);
        compositor.bind_touch(no_mods, touch_activate_binding);

        compositor.bind_key(KEY_M, mods_with_shift, move |keyboard, _, _| {
            if let Some(shsurf) = Surface::get(keyboard.focus) {
                let mut shsurf = shsurf.borrow_mut();
                let enable = !shsurf.state.maximized;
                shsurf.set_maximized(enable);
            }
        });
        compositor.bind_key(KEY_F, mods_with_shift, move |keyboard, _, _| {
            if let Some(shsurf) = Surface::get(keyboard.focus) {
                let mut shsurf = shsurf.borrow_mut();
                let enable = !shsurf.state.fullscreen;
                shsurf.set_fullscreen(enable, std::ptr::null_mut());
            }
        });

        compositor.bind_key(KEY_K, mods_with_shift, move |keyboard, _, _| {
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

        compositor.bind_button(BTN_LEFT, mods, |pointer, _, _| {
            Surface::from_view(pointer.focus)
                .borrow_mut()
                .r#move(pointer, false)
        });

        let resize_binding = |pointer: &mut ffi::weston_pointer, _, _| {
            if !pointer.focus.is_null() {
                let view = unsafe { &mut *pointer.focus };
                let surface = unsafe { &mut *view.surface };
                use ffi::wl_shell_surface_resize as resize;

                let (x, y) = view.from_global::<_, f32>(pointer.grab_x, pointer.grab_y);

                let (x, y) = (x as i32, y as i32);
                let hmargin = surface.width / 3;
                let vmargin = surface.height / 3;
                let edges = if x < hmargin {
                    resize::WL_SHELL_SURFACE_RESIZE_LEFT
                } else if x < surface.width - hmargin {
                    resize::WL_SHELL_SURFACE_RESIZE_NONE
                } else {
                    resize::WL_SHELL_SURFACE_RESIZE_RIGHT
                } | if y < vmargin {
                    resize::WL_SHELL_SURFACE_RESIZE_TOP
                } else if y < surface.height - vmargin {
                    resize::WL_SHELL_SURFACE_RESIZE_NONE
                } else {
                    resize::WL_SHELL_SURFACE_RESIZE_BOTTOM
                };

                Surface::from_view(pointer.focus)
                    .borrow_mut()
                    .resize(pointer, edges)
            }
        };

        compositor.bind_button(BTN_RIGHT, mods, resize_binding);
        compositor.bind_button(BTN_LEFT, mods_with_shift, resize_binding);

        unsafe {
            ffi::weston_install_debug_key_binding(&mut compositor.native as *mut _, mods.0)
        }

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
        debug!("-------------------- drop Shell");
        unsafe {
            ffi::destroy_workspaces(&mut self.native);
        }
    }
}

fn get_work_area(output: *const ffi::weston_output) -> ffi::weston_geometry {
    // Should be accounting for panels here or other UI elements that
    // limit the maximized size.

    let output = unsafe { output.as_ref().unwrap() };
    ffi::weston_geometry {
        x: output.x,
        y: output.y,
        width: output.width,
        height: output.height,
    }
}

fn get_focused_output(compositor: *mut ffi::weston_compositor) -> *mut ffi::weston_output {
    let compositor = unsafe { compositor.as_mut().unwrap() };

    for seat in compositor.seat_iter_mut() {
        unsafe {
            if let Some(touch) = seat.get_touch() {
                if let Some(focus) = touch.focus.as_ref() {
                    if !focus.output.is_null() {
                        return focus.output;
                    }
                }
            }

            if let Some(pointer) = seat.get_pointer() {
                if let Some(focus) = pointer.focus.as_ref() {
                    if !focus.output.is_null() {
                        return focus.output;
                    }
                }
            }

            if let Some(keyboard) = seat.get_keyboard() {
                if let Some(focus) = keyboard.focus.as_ref() {
                    if !focus.output.is_null() {
                        return focus.output;
                    }
                }
            }
        }
    }

    std::ptr::null_mut()
}

pub type Surface = ffi::shell_surface;

pub type SurfaceState = ffi::shell_surface_surface_state;

impl SurfaceState {
    pub fn new() -> SurfaceState {
        SurfaceState {
            fullscreen: false,
            maximized: false,
            lowered: false,
            activated: false,
        }
    }
}

pub fn lower_fullscreen_layer(
    shell: *mut ffi::desktop_shell,
    _lowering_output: *mut ffi::weston_output,
) {
    let shell = unsafe { &mut *shell };

    let workspace = unsafe { &mut *ffi::get_current_workspace(shell) };

    for view in wl_list_iter_mut!(&mut shell.fullscreen_layer.view_list.link, ffi::weston_view, layer_link.link) {
        unsafe {
            ffi::weston_layer_entry_remove(&mut view.layer_link);

            ffi::weston_layer_entry_insert(
                &mut workspace.layer.view_list,
                &mut view.layer_link,
            );

	    view.damage_below();

	    let surface = &mut *view.surface;
	    surface.damage();
	}
    }
}

impl Surface {
    pub fn new(
        shell: &mut Shell,
        surface: *mut ffi::weston_surface,
        view: *mut ffi::weston_view,
    ) -> Rc<RefCell<Self>> {
        let shsurf = Rc::new(RefCell::new(Self {
            destroy_signal: ffi::wl_signal::zeroed(),

            state_change_signal: ffi::wl_signal::zeroed(),

            view,
            surface,
            last_width: 0,
            last_height: 0,
            shell: &mut shell.native as *mut _,
            children_list: ffi::wl_list::zeroed(),
            children_link: ffi::wl_list::zeroed(),
            saved_x: 0,
            saved_y: 0,
            saved_position_valid: true,
            unresponsive: false,
            grabbed: false,
            resize_edges: 0,
            fullscreen: ffi::shell_surface__bindgen_ty_1 {
                transform: ffi::weston_transform::new(),
                black_view: 0 as *mut _,
            },
            workspace_transform: ffi::weston_transform::new(),
            fullscreen_output: 0 as *mut _,
            output: 0 as *mut _,
            output_destroy_listener: ffi::wl_listener::zeroed(),
            pending_state: SurfaceState::new(),
            state: SurfaceState::new(),
            pending_size: ffi::weston_size { width: 0, height: 0 },
            geometry: ffi::weston_geometry { x: 0, y: 0, width: 0, height: 0 },
            focus_count: 0,
            destroying: false,
        }));

        {
            let mut shsurf = shsurf.borrow_mut();

            shsurf.fullscreen.transform.link.init();
            shsurf.destroy_signal.init();
            shsurf.state_change_signal.init();
            shsurf.workspace_transform.link.init();
            shsurf.children_list.init();
            shsurf.children_link.init();
        }

        if let Some(surface) = unsafe { surface.as_mut() } {
            surface.committed = Some(Surface::committed);
            surface.committed_private = Rc::into_raw(Rc::clone(&shsurf)) as *mut _;
        }

        shsurf
    }

    pub fn configure_fullscreen(&mut self) {
        let view = unsafe { &mut *self.view };
        let shell = unsafe { &mut *self.shell };

        unsafe {
            ffi::weston_layer_entry_remove(&mut view.layer_link);

            ffi::weston_layer_entry_insert(
                &mut shell.fullscreen_layer.view_list,
                &mut view.layer_link,
            );
        }

        if let Some(output) = unsafe { self.fullscreen_output.as_mut() } {
            let x = output.x + (output.width - self.geometry.width) / 2 - self.geometry.x;
            let y = output.y + (output.height - self.geometry.height) / 2 - self.geometry.y;
            view.set_position(x as f32, y as f32);
        } else {
            view.set_position(0.0, 0.0);
        }
    }

    pub fn configure_maximized(&mut self) {
        let output = unsafe { &mut *self.output };

        let work_area = get_work_area(output);
        let view = unsafe { &mut *self.view };
        view.set_position(
            work_area.x as f32 - view.geometry.x,
            work_area.y as f32 - view.geometry.y,
        );
    }

    pub fn center_on_output(&mut self, output: *const ffi::weston_output) {
        let view = unsafe { &mut *self.view };

        if let Some(output) = unsafe { output.as_ref() } {
            let geometry = self.geometry;
            let x = output.x - geometry.x + (output.width - geometry.width) / 2;
            let y = output.y - geometry.y + (output.height - geometry.height) / 2;

            view.set_position(x as f32, y as f32);
        } else {
            view.set_position(0.0, 0.0);
        }
    }

    pub fn set_initial_position(&mut self) {
        let view = unsafe { &mut *self.view };
        let shell = unsafe { &mut *self.shell };

        let origin = unsafe {
            get_focused_output(shell.compositor)
                .as_ref()
                .map(|output| (output.x as f32, output.y as f32))
                .unwrap_or((0.0, 0.0))
        };

        // todo: random placement within work area.

        view.set_position(origin.0 + 20.0, origin.1 + 20.0);
    }

    pub fn child_iter_mut<'a>(&'a mut self) -> ffi::WlListIterMut<'a, Surface> {
        wl_list_iter_mut!(&mut self.children_list, Surface, children_link)
    }

    pub fn get_last_child<'a>(&'a mut self) -> Option<&'a mut Surface> {
        self.child_iter_mut()
            .rev()
            .find(|shsurf| unsafe { shsurf.view.as_ref().map_or(false, |view| view.is_mapped()) })
    }

    pub fn from_view<'a>(view: *mut ffi::weston_view) -> Rc<RefCell<Surface>> {
        let view = unsafe { &mut *view };
        let surface = unsafe { &mut *view.surface };

        Surface::get(surface.get_main_surface()).unwrap()
    }

    pub fn emit_state_changed(&mut self) {
        self.state_change_signal.emit(&mut ())
    }

    fn raise_or_insert(&mut self) {
        unsafe {
            let shell = &mut *self.shell;
	    let view = &mut *self.view;
	    let workspace = &mut *ffi::get_current_workspace(shell);

            ffi::weston_layer_entry_remove(&mut view.layer_link);

	    if self.state.fullscreen {
		ffi::weston_layer_entry_insert(
                    &mut shell.fullscreen_layer.view_list,
                    &mut view.layer_link,
		);
	    } else {
		ffi::weston_layer_entry_insert(
                    &mut workspace.layer.view_list,
                    &mut view.layer_link,
		);
	    }
	}
    }

    fn activate(&mut self, seat: *mut ffi::weston_seat) {
        if self.state.activated || self.pending_state.activated {
            return;
        }

        if !self.output.is_null() {
            lower_fullscreen_layer(self.shell, self.output);
        }

        if let Some(state) = unsafe { ffi::ensure_focus_state(self.shell, seat).as_mut() } {
            let old_focus = unsafe { ffi::focus_state_set_focus(state, self.surface) };
            if let Some(shsurf) = Surface::get(old_focus) {
                let mut shsurf = shsurf.borrow_mut();
                shsurf.focus_count = shsurf.focus_count - 1;
                if shsurf.focus_count == 0 {
                    shsurf.pending_state.activated = false;
                    shsurf.emit_state_changed();
                }
            }

            self.focus_count = self.focus_count + 1;
            if self.focus_count == 1 {
                self.pending_state.activated = true;
                self.emit_state_changed();
            }
        }

	self.raise_or_insert()
    }

    pub fn map(&mut self, _sx: i32, _sy: i32) {
        let shell = unsafe { &mut *self.shell };
        let view = unsafe { &mut *self.view };
        let surface = unsafe { &mut *self.surface };

        if self.state.fullscreen {
            self.center_on_output(self.fullscreen_output)
        } else if self.state.maximized {
            self.configure_maximized()
        } else {
            self.set_initial_position()
        }

	self.raise_or_insert();

        view.update_transform();
        view.is_mapped = true;
        if self.state.maximized {
            surface.output = self.output;
            view.set_output(self.output);
        }

        let compositor = unsafe { &mut *shell.compositor };
        for seat in compositor.seat_iter_mut() {
            view.activate(seat, 0)
        }
    }

    extern "C" fn committed(surface: *mut ffi::weston_surface, sx: i32, sy: i32) {
        let surface = unsafe { &mut *surface };
        let rc = Surface::get(surface).unwrap();
        let mut shsurf = rc.borrow_mut();
        let view = unsafe { &mut *shsurf.view };

        if surface.width == 0 {
            return;
        }

        let was_fullscreen = shsurf.state.fullscreen;
        let was_maximized = shsurf.state.maximized;

        shsurf.state = shsurf.pending_state;

        if !surface.is_mapped() {
            shsurf.map(sx, sy);
            surface.is_mapped = true;
            return;
        }

        if sx == 0
            && sy == 0
            && shsurf.last_width == surface.width
            && shsurf.last_height == surface.height
            && was_fullscreen == shsurf.state.fullscreen
            && was_maximized == shsurf.state.maximized
        {
            return;
        }

        if was_fullscreen {
            shsurf.fullscreen.transform.link.remove();
            shsurf.fullscreen.transform.link.init();

            if !shsurf.fullscreen.black_view.is_null() {
                unsafe {
                    let view = &mut *shsurf.fullscreen.black_view;

                    ffi::weston_surface_destroy(view.surface);
                }
            }
            shsurf.fullscreen.black_view = std::ptr::null_mut();

            let view = unsafe { &mut *shsurf.view };
            if shsurf.saved_position_valid {
                view.set_position(shsurf.saved_x as f32, shsurf.saved_y as f32);
            } else {
                shsurf.set_initial_position();
            }
            shsurf.saved_position_valid = false;
        }

        if was_maximized {
            shsurf.set_output(get_focused_output(surface.compositor));
            if shsurf.saved_position_valid {
                view.set_position(shsurf.saved_x as f32, shsurf.saved_y as f32);
            } else {
                shsurf.set_initial_position();
            }
            shsurf.saved_position_valid = false;
        }

        if (shsurf.state.fullscreen || shsurf.state.maximized) && !shsurf.saved_position_valid {
            shsurf.saved_x = view.geometry.x as i32;
            shsurf.saved_y = view.geometry.y as i32;
            shsurf.saved_position_valid = true;
        }

        if shsurf.state.fullscreen {
            shsurf.configure_fullscreen();
        } else if shsurf.state.maximized {
            shsurf.configure_maximized();
            surface.output = shsurf.output;
        } else {
            use ffi::wl_shell_surface_resize as resize;
            let resize_dx = if shsurf.resize_edges & resize::WL_SHELL_SURFACE_RESIZE_LEFT.0 != 0 {
                shsurf.last_width - surface.height
            } else {
                0
            };
            let resize_dy = if shsurf.resize_edges & resize::WL_SHELL_SURFACE_RESIZE_TOP.0 != 0 {
                shsurf.last_height - surface.height
            } else {
                0
            };

            let from: (f32, _) = view.to_global(0.0, 0.0);
            let to: (f32, _) = view.to_global((sx + resize_dx) as f32, (sy + resize_dy) as f32);
            let x = view.geometry.x + to.0 - from.0;
            let y = view.geometry.y + to.1 - from.1;
            view.set_position(x, y);
        }

        shsurf.last_width = surface.width;
        shsurf.last_height = surface.height;

        if !surface.output.is_null() {
            for view in surface.view_iter_mut() {
                view.update_transform()
            }
        }
    }

    pub fn set_output(&mut self, output: *mut ffi::weston_output) {
        unsafe { ffi::shell_surface_set_output(self, output) }
    }

    pub fn get(surface: *mut ffi::weston_surface) -> Option<Rc<RefCell<Surface>>> {
        unsafe {
            let surface = surface.as_mut()?;
            if surface.committed == Some(Surface::committed) {
                let ptr = surface.committed_private as *const _;
                Rc::increment_strong_count(ptr);
                Some(Rc::from_raw(ptr))
            } else {
                None
            }
        }
    }

    pub fn r#move(&mut self, pointer: &mut ffi::weston_pointer, _client_initiated: bool) {
        MoveGrab::start(pointer, self)
    }

    pub fn resize(
        &mut self,
        pointer: &mut ffi::weston_pointer,
        edges: ffi::wl_shell_surface_resize,
    ) {
        ResizeGrab::start(pointer, self, edges);
    }

    pub fn set_size(&mut self, width: i32, height: i32) {
        self.pending_size = ffi::weston_size { width, height };
        self.emit_state_changed()
    }

    pub fn set_resizing(&mut self, edges: ffi::wl_shell_surface_resize) {
        self.resize_edges = edges.0;
        //self.pending_state.resizing =
        //	    edges != ffi::wl_shell_surface_resize::WL_SHELL_SURFACE_RESIZE_NONE;
        self.emit_state_changed()
    }

    pub fn get_geometry(&self) -> ffi::weston_geometry {
        self.geometry
    }

    pub fn get_maximized_size(&self) -> ffi::weston_size {
        let output = unsafe { &mut *self.output };

        let work_area = get_work_area(output);

        ffi::weston_size {
            width: work_area.height,
            height: work_area.width,
        }
    }

    pub fn set_maximized(&mut self, enable: bool) {
        self.pending_state.maximized = enable;
        self.pending_size = if enable {
            let surface = unsafe { &mut *self.surface };
            let output = if surface.is_mapped() {
                surface.output
            } else {
                get_focused_output(surface.compositor)
            };

            self.set_output(output);
            self.get_maximized_size()
        } else {
            ffi::weston_size { width: 0, height: 0 }
        };

        self.emit_state_changed();
    }

    pub fn set_fullscreen(&mut self, enable: bool, output: *mut ffi::weston_output) {
        self.pending_state.fullscreen = enable;
        self.pending_size = if enable {
            let surface = unsafe { &mut *self.surface };
            let output = if output.is_null() && !surface.is_mapped() {
                get_focused_output(surface.compositor)
            } else {
                output
            };

            self.set_output(output);
            self.fullscreen_output = output;

            if let Some(output) = unsafe { self.output.as_ref() } {
                ffi::weston_size {
                    width: output.width,
                    height: output.height,
                }
            } else {
                ffi::weston_size { width: 0, height: 0 }
            }
        } else if self.state.maximized {
            self.get_maximized_size()
        } else {
            ffi::weston_size { width: 0, height: 0 }
        };

        self.emit_state_changed();
    }

    pub fn set_minimized(&mut self) {
        let view = unsafe { &mut *self.view };
        let shell = unsafe { &mut *self.shell };
        let compositor = unsafe { &mut *shell.compositor };

        unsafe {
            ffi::weston_layer_entry_remove(&mut view.layer_link);

            ffi::weston_layer_entry_insert(
                &mut shell.minimized_layer.view_list,
                &mut view.layer_link,
            );

            let current_ws = ffi::get_current_workspace(shell);
            ffi::drop_focus_state(shell, current_ws, view.surface);

        }

        for keyboard in compositor.seat_iter_mut()
            .filter_map(|seat| seat.get_keyboard())
            .filter(|keyboard| keyboard.focus == self.surface) {
                keyboard.set_focus(0 as *mut _)
            }

        view.damage_below();
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.children_list.remove();
        self.children_link.remove();
    }
}

#[no_mangle]
pub extern "C" fn get_shell_surface(surface: *mut ffi::weston_surface) -> *mut ffi::shell_surface {
    match Surface::get(surface) {
        Some(rc) => rc.as_ptr(),
        None => 0 as *mut _,
    }
}
