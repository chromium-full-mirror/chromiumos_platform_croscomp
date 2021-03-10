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

    pub fn remove(&mut self) {
	unsafe { wl_list_remove(self) }
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

    pub fn zeroed() -> Self {
	unsafe { std::mem::zeroed() }
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

impl weston_keyboard_modifier {
    pub fn is_set(&self, bit: weston_keyboard_modifier) -> bool {
	(*self & bit) == bit
   }
}

impl wl_signal {
    extern "C" fn destroy_notify<T>(listener: *mut wl_listener,
				    _data: *mut ::std::os::raw::c_void)
    where
	T: From<*mut wl_listener>
    {
	println!("destroy notify");
	unsafe { (*listener).link.remove() };
	let _ = T::from(listener);
    }

    pub fn add<'a, T>(&mut self, item: &'a mut T)
    where
	T: From<*mut wl_listener>,
	&'a mut T: Into<&'a mut wl_listener>
    {
	let listener: &'a mut wl_listener = item.into();
	listener.notify = Some(Self::destroy_notify::<T>);
	unsafe {
	    wl_list_insert(self.listener_list.prev,
			   &mut listener.link as *mut _);
	}
    }

    pub fn get<T>(&mut self) -> Option<T>
    where
	T: From<*mut wl_listener>,
    {
	wl_list_iter_mut!(&mut self.listener_list, wl_listener, link)
	    .find(|listener| listener.notify == Some(Self::destroy_notify::<T>))
	    .map(|listener| T::from(listener))
    }

    pub fn remove<T>(&mut self) -> Option<T>
    where
	T: From<*mut wl_listener>,
    {
	wl_list_iter_mut!(&mut self.listener_list, wl_listener, link)
	    .find(|listener| listener.notify == Some(Self::destroy_notify::<T>))
	    .map(|listener| {
		listener.link.remove();
		T::from(listener)
	    })
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

    pub fn scale(mut self, x: f32, y: f32, z: f32) -> Self {
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

    pub fn rotate(mut self, cos: f32, sin: f32) -> Self {
	let m = weston_matrix {
	    d: [  cos, sin, 0.0, 0.0,
		 -sin, cos, 0.0, 0.0,
		  0.0, 0.0, 1.0, 0.0,
		  0.0, 0.0, 0.0, 1.0 ],
	    type_: weston_matrix_transform_type::WESTON_MATRIX_TRANSFORM_ROTATE as u32
	};

	self.multiply(&m);

	self
    }

    pub fn translate(mut self, x: f32, y: f32, z: f32) -> Self {
	let m = weston_matrix {
	    d: [  1.0, 0.0, 0.0, 0.0,
		  0.0, 1.0, 0.0, 0.0,
		  0.0, 0.0, 1.0, 0.0,
		  x,   y,   z,   1.0 ],
	    type_: weston_matrix_transform_type::WESTON_MATRIX_TRANSFORM_TRANSLATE as u32
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
