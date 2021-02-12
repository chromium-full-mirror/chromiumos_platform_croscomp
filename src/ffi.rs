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

pub mod evdev {
    include!(concat!(env!("OUT_DIR"), "/input.rs"));
}
