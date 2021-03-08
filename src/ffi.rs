// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_macros)]

use std::marker::PhantomData;
use std::convert::{From, Into};
use std::ops;

// Put these uglies here

macro_rules! offset_of {
    ($container:ty, $( $field:ident ).+) => {
        &(*(0usize as *const $container)).$( $field ).+ as *const _ as usize
    }
}

macro_rules! container_of {
    ($ptr:expr, $container:ty, $( $field:ident ).+) => ({
        (($ptr as usize) - offset_of!($container, $( $field ).+)) as *const $container
    })
}

macro_rules! container_of_mut {
    ($ptr:expr, $container:ty, $( $field:ident ).+) => ({
        (($ptr as usize) - offset_of!($container, $( $field ).+)) as *mut $container
    })
}

#[derive(Clone)]
pub struct WlListIter<'a, T: 'a> {
    offset: usize,
    head: *const wl_list,
    next: *const wl_list,
    phantom: PhantomData<&'a T>
}

impl<'a, T> Iterator for WlListIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
	if self.next == self.head {
	    None
	} else {
	    let current = self.next;
	    self.next = unsafe { (*self.next).next };

	    unsafe { Some(&*(((current as usize) - self.offset) as *mut T)) }
	}
    }
}

#[derive(Clone)]
pub struct WlListIterMut<'a, T: 'a> {
    offset: usize,
    head: *mut wl_list,
    next: *mut wl_list,
    phantom: PhantomData<&'a T>
}

impl<'a, T> Iterator for WlListIterMut<'a, T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
	if self.next == self.head {
	    None
	} else {
	    let current = self.next;
	    self.next = unsafe { (*self.next).next };

	    unsafe { Some(&mut *(((current as usize) - self.offset) as *mut T)) }
	}
    }
}

impl wl_list {
    pub fn zeroed() -> Self {
	unsafe { std::mem::zeroed() }
    }

    pub fn init(&mut self) {
	self.next = self as *mut _;
	self.prev = self as *mut _;
    }

    pub fn iter<'a, T>(&self, offset: usize) -> WlListIter<'a, T> {
	WlListIter {
	    offset: offset,
	    head: self as *const wl_list,
	    next: self.next as *const wl_list,
	    phantom: PhantomData
	}
    }

    pub fn iter_mut<'a, T>(&mut self, offset: usize) -> WlListIterMut<'a, T> {
	WlListIterMut {
	    offset: offset,
	    head: self as *mut wl_list,
	    next: self.next as *mut wl_list,
	    phantom: PhantomData
	}
    }

}

macro_rules! wl_list_iter {
    ($ptr:expr, $container:ty, $( $field:ident ).+) => ({
	let offset = unsafe { offset_of!($container, $( $field ).+) };

	$ptr.iter::<$container>(offset)
    })
}

macro_rules! wl_list_iter_mut {
    ($ptr:expr, $container:ty, $( $field:ident ).+) => ({
	let offset = unsafe { offset_of!($container, $( $field ).+) };

	$ptr.iter_mut::<$container>(offset)
    })
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

type NotifyFunc =
    unsafe extern "C" fn(listener: *mut wl_listener, data: *mut ::std::os::raw::c_void);

impl wl_listener {
    pub fn new(notify: NotifyFunc) -> Self {
	wl_listener {
	    link: wl_list::zeroed(),
	    notify: Some(notify)
	}
    }
}

impl wl_client {
    pub fn create_resource(&mut self,
			   interface: &wl_interface,
			   version: u32,
			   id: u32) -> *mut wl_resource {
	unsafe { wl_resource_create(self, interface, version as i32, id) }
    }
}

impl wl_display {
    pub fn create_global<I>(&mut self, version: u32) -> super::Global<I>
    where I: wayland_server::Interface {
	unsafe extern "C" fn call_dispatcher<I>(
	    data: *const ::std::ffi::c_void,
	    object: *mut ::std::ffi::c_void,
	    opcode: u32,
	    _message: *const wl_message,
	    args: *mut wl_argument) -> ::std::os::raw::c_int
	where I: wayland_server::Interface {
	    let request = <I::Request as wayland_server::MessageGroup>
		::from_raw_c(object, opcode,
			     args as *const wayland_sys::common::wl_argument);

	    let mut resource = super::Resource::<I> {
		native: object as *mut wl_resource,
		phantom: PhantomData
	    };

	    let handlers = &mut *(data as *mut super::GlobalHandlers<I>);

	    if let (Ok(request), Some(dispatcher)) = (request, handlers.dispatcher.as_ref()) {
		(*dispatcher)(&mut resource, &request)
	    }
	    0
	}

	unsafe extern "C" fn call_destroy_closure<I>(native: *mut wl_resource)
	where I: wayland_server::Interface {
	    let b = &*(wl_resource_get_user_data(native)
		       as *mut Box<dyn Fn(&mut super::Resource<I>)>);

	    let mut resource = super::Resource::<I> {
		native,
		phantom: PhantomData
	    };

	    b(&mut resource)
	}

	unsafe extern "C" fn call_bind_closure<I>(
            client: *mut wl_client,
            data: *mut ::std::os::raw::c_void,
            version: u32,
            id: u32) 
	where I: wayland_server::Interface {
	    let client = &mut *client;

	    let native = client.create_resource(
		&mut *(I::c_interface() as *mut _), version, id);

	    let mut resource = super::Resource::<I> {
		native,
		phantom: PhantomData
	    };

	    let handlers = &mut *(data as *mut super::GlobalHandlers<I>);

	    println!("handlers: {:?}", handlers as *const _);

	    if let Some(ref handle_bind) = handlers.handle_bind {
		println!("calling handle_bind for {}: {:?}",
			 I::NAME, &(*handle_bind) as *const _);
		(*handle_bind)(&mut resource);
		// xxx return value from handle_bind should indicate whether
		// or not bind was successful
	    }

	    if let Some(ref dispatcher) = handlers.dispatcher {
		println!("setting dispatcher for {}: {:?}",
			 I::NAME, &(*dispatcher) as *const _);

		wl_resource_set_dispatcher(
		    resource.native,
		    Some(call_dispatcher::<I>),
		    handlers as *mut _ as *mut std::ffi::c_void,
		    std::ptr::null_mut(),
		    None)
	    }

	    if let Some(ref mut handle_destroy) = handlers.handle_destroy {
		wl_resource_set_user_data(resource.native,
					  handle_destroy as *mut _
					  as *mut std::ffi::c_void);
		wl_resource_set_destructor(resource.native,
					   Some(call_destroy_closure::<I>));
	    }
	}

	let mut global = super::Global {
	    native: std::ptr::null_mut(),
	    handlers: Box::new(super::GlobalHandlers {
		handle_bind: None,
		dispatcher: None,
		handle_destroy: None,
	    })
	};

	println!("in create_global:");
	println!("  global: {:?}", &global as *const _);
	println!("  allocated handlers: {:?}", &(*global.handlers) as *const _);

	global.native = unsafe {
	    wl_global_create(self,
			     I::c_interface() as *const _,
			     version as i32,
			     &mut *global.handlers as *mut _
			     as *mut std::ffi::c_void,
			     Some(call_bind_closure::<I>))
	};

	global
    }
}

impl weston_keyboard_modifier {
    pub fn is_set(&self, bit: weston_keyboard_modifier) -> bool {
	(*self & bit) == bit
   }
}

impl wl_signal {
    pub fn add(&mut self, listener: &mut wl_listener) {
	unsafe {
	    wl_list_insert(self.listener_list.prev, &mut listener.link);
	}
    }
}

impl weston_head {
    pub fn is_enabled(&self) -> bool {
	unsafe { self.output.as_ref().map_or(false, |output| output.enabled) }
    }
}

impl weston_compositor {
    pub fn schedule_repaint(&mut self) {
	unsafe { weston_compositor_schedule_repaint(self) }
    }

    pub fn create_output(&mut self, name: &str) -> &mut weston_output {
	unsafe {
	    &mut *weston_compositor_create_output(self, name.as_ptr() as *const i8)
	}
    }
}

impl weston_surface {
    pub fn create_view(&mut self) -> *mut weston_view {
	unsafe { weston_view_create(self) }
    }
    pub fn set_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
	unsafe { weston_surface_set_color(self, r, g, b, a); }
    }
    pub fn set_size(&mut self, width: i32, height: i32) {
	unsafe { weston_surface_set_size(self, width, height) }
    }

    pub fn damage(&mut self) {
	unsafe { weston_surface_damage(self) }
    }
}

impl weston_output {
    pub fn attach_head(&mut self, head: &mut weston_head) -> i32 {
	unsafe { weston_output_attach_head(self, head) }
    }

    pub fn set_scale(&mut self, scale: i32) {
	unsafe { weston_output_set_scale(self, scale) }
    }

    pub fn set_transform(&mut self, transform: wl_output_transform) {
	unsafe { weston_output_set_transform(self, transform as u32) }
    }

    pub fn enable(&mut self) -> i32 {
	unsafe { weston_output_enable(self) }
    }
}

impl weston_view {
    pub fn set_position(&mut self, x: f32, y: f32) {
	unsafe { weston_view_set_position(self, x, y) }
    }

    pub fn set_output(&mut self, output: *mut weston_output) {
	unsafe { weston_view_set_output(self, output) }
    }

    pub fn geometry_dirty(&mut self) {
	unsafe { weston_view_geometry_dirty(self) }
    }

    pub fn from_global<T: Into<f32>, U: From<f32>>(&mut self, x: T, y: T) -> (U, U) {
	let local =
	    if self.transform.enabled {
		let global = weston_vector { f: [ x.into(), y.into(), 0.0, 1.0 ] };
		let local = self.transform.inverse.transform(&global);

		((local.f[0] / local.f[3]),
		 (local.f[1] / local.f[3]))
	    } else {
		(x.into() - self.geometry.x,
		 y.into() - self.geometry.y)
	    };
	(local.0.into(), local.1.into())
    }
}

impl shell_surface {
    pub fn get<'a>(surface: &mut weston_surface) -> Option<&'a mut shell_surface> {
	unsafe { get_shell_surface(surface).as_mut() }
    }

    pub fn set_size(&mut self, width: i32, height: i32) {
	unsafe { shell_surface_set_size(self, width, height) }
    }

    pub fn set_resizing(&mut self, edges: wl_shell_surface_resize) {
	unsafe { shell_surface_set_resizing(self, edges.0) }
    }

    pub fn get_geometry(&mut self) -> weston_geometry {
	unsafe { weston_desktop_surface_get_geometry(self.desktop_surface) }
    }

}

impl wl_shell_surface_resize {
    pub fn has(&self, bit: wl_shell_surface_resize) -> bool {
	*self & bit == bit
    }
}

impl weston_keyboard {
    pub fn start_grab(&mut self, grab: *mut weston_keyboard_grab) {
	unsafe {
	    weston_keyboard_start_grab(self, grab)
	}
    }

    pub fn set_focus(&mut self, surface: *mut weston_surface) {
	unsafe {
	    weston_keyboard_set_focus(self, surface)
	}
    }
}

impl weston_matrix {
    pub fn new() -> weston_matrix {
	weston_matrix {
	    d: [ 1.0, 0.0, 0.0, 0.0,
		 0.0, 1.0, 0.0, 0.0,
		 0.0, 0.0, 1.0, 0.0,
		 0.0, 0.0, 0.0, 1.0 ],
	    type_: 0
	}
    }

    pub fn multiply(&mut self, other: &Self) {
	unsafe { weston_matrix_multiply(self, other) }
    }

    pub fn scale(&mut self, x: f32, y: f32, z: f32) -> &mut Self{
	let m = weston_matrix {
	    d: [ x,   0.0, 0.0, 0.0,
		 0.0, y,   0.0, 0.0,
		 0.0, 0.0, z,   0.0,
		 0.0, 0.0, 0.0, 1.0 ],
	    type_: weston_matrix_transform_type::WESTON_MATRIX_TRANSFORM_SCALE as u32
	};

	self.multiply(&m);

	self
    }

    pub fn transform(&mut self, v: &weston_vector) -> weston_vector {
	let mut r = weston_vector { f: [ 0.0; 4 ] };
	for i in 0..4_usize {
	    r.f[i] = (0..4_usize).map(|j| v.f[j] * self.d[i + j * 4]).sum()
	}

	r
    }
}

impl From<f32> for wl_fixed_t {
    fn from(item: f32) -> Self {
        wl_fixed_t((item * 256.0) as i32)
    }
}

impl From<wl_fixed_t> for f32 {
    fn from(item: wl_fixed_t) -> Self {
        item.0 as f32 / 256.0
    }
}

impl From<i32> for wl_fixed_t {
    fn from(item: i32) -> Self {
        wl_fixed_t(item << 8)
    }
}

impl From<wl_fixed_t> for i32 {
    fn from(item: wl_fixed_t) -> Self {
        item.0 >> 8
    }
}

impl ops::Add<wl_fixed_t> for wl_fixed_t {
    type Output = wl_fixed_t;

    fn add(self, rhs: wl_fixed_t) -> wl_fixed_t {
	wl_fixed_t(self.0 + rhs.0)
    }
}

impl ops::Sub<wl_fixed_t> for wl_fixed_t {
    type Output = wl_fixed_t;

    fn sub(self, rhs: wl_fixed_t) -> wl_fixed_t {
	wl_fixed_t(self.0 - rhs.0)
    }
}

impl weston_pointer {
    pub fn r#move(&mut self, event: *mut weston_pointer_motion_event) {
	unsafe { weston_pointer_move(self, event) }
    }
}

pub mod evdev {
    include!(concat!(env!("OUT_DIR"), "/input.rs"));
}
