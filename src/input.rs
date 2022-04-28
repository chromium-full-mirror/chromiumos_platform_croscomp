// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::ptr::NonNull;

use crate::ffi;
use crate::protocol::wl_keyboard::KeyState;
use crate::protocol::wl_pointer::ButtonState;

pub trait PointerGrabHandler {
    fn focus(&mut self, _pointer: &mut ffi::weston_pointer) {}

    fn motion(
        &mut self,
        _pointer: &mut ffi::weston_pointer,
        _time: &ffi::timespec,
        _event: *mut ffi::weston_pointer_motion_event,
    ) {
    }

    fn button(
        &mut self,
        _pointer: &mut ffi::weston_pointer,
        _time: &ffi::timespec,
        _button: u32,
        _state: ButtonState,
    ) {
    }

    fn axis(
        &mut self,
        _pointer: &mut ffi::weston_pointer,
        _time: *const ffi::timespec,
        _event: *mut ffi::weston_pointer_axis_event,
    ) {
    }

    fn axis_source(&mut self, _pointer: &mut ffi::weston_pointer, _source: u32) {}

    fn frame(&mut self, _pointer: &mut ffi::weston_pointer) {}

    fn cancel(&mut self, _pointer: &mut ffi::weston_pointer) {}
}

pub struct PointerGrab {
    grab: ffi::weston_pointer_grab,
    handler: Option<NonNull<dyn PointerGrabHandler>>,
}

impl PointerGrab {
    fn cast(
        grab: *mut ffi::weston_pointer_grab,
    ) -> (
        &'static mut ffi::weston_pointer,
        &'static mut dyn PointerGrabHandler,
    ) {
        let grab = unsafe { &mut *container_of_mut!(grab, PointerGrab, grab) };
        unsafe {
            (
                &mut *grab.grab.pointer,
                &mut *grab.handler.unwrap().as_ptr(),
            )
        }
    }

    extern "C" fn focus(grab: *mut ffi::weston_pointer_grab) {
        let (pointer, handler) = Self::cast(grab);

        handler.focus(pointer);
    }

    extern "C" fn motion(
        grab: *mut ffi::weston_pointer_grab,
        time: *const ffi::timespec,
        event: *mut ffi::weston_pointer_motion_event,
    ) {
        let (pointer, handler) = Self::cast(grab);
        let time = unsafe { &*time };

        handler.motion(pointer, time, event);
    }

    extern "C" fn button(
        grab: *mut ffi::weston_pointer_grab,
        time: *const ffi::timespec,
        button: u32,
        state: u32,
    ) {
        let (pointer, handler) = Self::cast(grab);
        let time = unsafe { &*time };

        handler.button(pointer, time, button, ButtonState::from_raw(state).unwrap())
    }

    extern "C" fn axis(
        grab: *mut ffi::weston_pointer_grab,
        time: *const ffi::timespec,
        event: *mut ffi::weston_pointer_axis_event,
    ) {
        let (pointer, handler) = Self::cast(grab);
        let time = unsafe { &*time };
        let event = unsafe { &mut *event };

        handler.axis(pointer, time, event)
    }

    extern "C" fn axis_source(grab: *mut ffi::weston_pointer_grab, source: u32) {
        let (pointer, handler) = Self::cast(grab);

        handler.axis_source(pointer, source)
    }

    extern "C" fn frame(grab: *mut ffi::weston_pointer_grab) {
        let (pointer, handler) = Self::cast(grab);

        handler.frame(pointer)
    }

    extern "C" fn cancel(grab: *mut ffi::weston_pointer_grab) {
        let (pointer, handler) = Self::cast(grab);

        handler.cancel(pointer);
    }

    pub fn new(pointer: *mut ffi::weston_pointer) -> PointerGrab {
        static INTERFACE: ffi::weston_pointer_grab_interface = ffi::weston_pointer_grab_interface {
            focus: Some(PointerGrab::focus),
            motion: Some(PointerGrab::motion),
            button: Some(PointerGrab::button),
            axis: Some(PointerGrab::axis),
            axis_source: Some(PointerGrab::axis_source),
            frame: Some(PointerGrab::frame),
            cancel: Some(PointerGrab::cancel),
        };

        PointerGrab {
            grab: ffi::weston_pointer_grab { interface: &INTERFACE, pointer: pointer },
            handler: None,
        }
    }

    pub fn start(&mut self, handler: *mut dyn PointerGrabHandler) {
        // This is a separate function since it needs a stable address for self
        self.handler = NonNull::new(handler);
        unsafe { ffi::weston_pointer_start_grab(self.grab.pointer, &mut self.grab) };
    }
}

impl Drop for PointerGrab {
    fn drop(&mut self) {
        unsafe { ffi::weston_pointer_end_grab(self.grab.pointer) }
    }
}

pub trait KeyboardGrabHandler {
    fn key(
        &mut self,
        _keyboard: &mut ffi::weston_keyboard,
        _time: &ffi::timespec,
        _key: u32,
        _state: KeyState,
    ) {
    }

    fn modifiers(
        &mut self,
        _keyboard: &mut ffi::weston_keyboard,
        _serial: u32,
        _mods_depressed: u32,
        _mods_latched: u32,
        _mods_locked: u32,
        _group: u32,
    ) {
    }

    fn cancel(&mut self, _keyboard: &mut ffi::weston_keyboard) {}
}

pub struct KeyboardGrab {
    pub grab: ffi::weston_keyboard_grab,
    handler: Option<NonNull<dyn KeyboardGrabHandler>>,
}

impl KeyboardGrab {
    fn cast(
        grab: *mut ffi::weston_keyboard_grab,
    ) -> (
        &'static mut ffi::weston_keyboard,
        &'static mut dyn KeyboardGrabHandler,
    ) {
        let grab = unsafe { &mut *container_of_mut!(grab, KeyboardGrab, grab) };
        unsafe {
            (
                &mut *grab.grab.keyboard,
                &mut *grab.handler.unwrap().as_ptr(),
            )
        }
    }

    extern "C" fn key(
        grab: *mut ffi::weston_keyboard_grab,
        time: *const ffi::timespec,
        key: u32,
        state: u32,
    ) {
        let (keyboard, handler) = Self::cast(grab);
        let time = unsafe { &*time };
        handler.key(keyboard, time, key, KeyState::from_raw(state).unwrap());
    }

    extern "C" fn modifiers(
        grab: *mut ffi::weston_keyboard_grab,
        serial: u32,
        mods_depressed: u32,
        mods_latched: u32,
        mods_locked: u32,
        group: u32,
    ) {
        let (keyboard, handler) = Self::cast(grab);
        handler.modifiers(
            keyboard,
            serial,
            mods_depressed,
            mods_latched,
            mods_locked,
            group,
        )
    }

    extern "C" fn cancel(grab: *mut ffi::weston_keyboard_grab) {
        let (keyboard, handler) = Self::cast(grab);
        handler.cancel(keyboard)
    }

    pub fn new(keyboard: *mut ffi::weston_keyboard) -> KeyboardGrab {
        static INTERFACE: ffi::weston_keyboard_grab_interface =
            ffi::weston_keyboard_grab_interface {
                key: Some(KeyboardGrab::key),
                modifiers: Some(KeyboardGrab::modifiers),
                cancel: Some(KeyboardGrab::cancel),
            };

        KeyboardGrab {
            grab: ffi::weston_keyboard_grab {
                interface: &INTERFACE,
                keyboard: keyboard,
            },
            handler: None,
        }
    }

    pub fn start(&mut self, handler: *mut dyn KeyboardGrabHandler) {
        self.handler = NonNull::new(handler);
        unsafe { ffi::weston_keyboard_start_grab(self.grab.keyboard, &mut self.grab) };
    }
}

impl Drop for KeyboardGrab {
    fn drop(&mut self) {
        unsafe { ffi::weston_keyboard_end_grab(self.grab.keyboard) }
    }
}
