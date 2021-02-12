#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_macros)]

use std::marker::PhantomData;

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

const NULL_LIST: wl_list = wl_list {
	prev: 0 as *mut _,
	next: 0 as *mut _
};

impl wl_listener {
    pub fn new(notify: NotifyFunc) -> Self {
	wl_listener {
	    link: NULL_LIST,
	    notify: Some(notify)
	}
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

impl weston_surface {
    pub fn create_view(&mut self) -> &mut weston_view {
	unsafe { &mut *weston_view_create(self) }
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

impl weston_view {
    pub fn set_position(&mut self, x: f32, y: f32) {
	unsafe { weston_view_set_position(self, x, y) }
    }

    pub fn geometry_dirty(&mut self) {
	unsafe { weston_view_geometry_dirty(self) }
    }
}

impl weston_keyboard {
    pub fn start_grab(&mut self, grab: *mut weston_keyboard_grab) {
	unsafe {
	    weston_keyboard_start_grab(self, grab)
	}
    }

    pub fn set_focus(&mut self, surface: Option<*mut weston_surface>) {
	unsafe {
	    weston_keyboard_set_focus(self, surface.unwrap_or(0 as *mut _))
	}
    }
}

pub mod evdev {
    include!(concat!(env!("OUT_DIR"), "/input.rs"));
}
