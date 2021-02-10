#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_macros)]

use std::marker::PhantomData;

// Put these uglies here

macro_rules! offset_of {
    ($container:ty, $field:ident) => {
        &(*(0usize as *const $container)).$field as *const _ as usize
    }
}

macro_rules! container_of {
    ($ptr:expr, $container:ty, $field:ident) => ({
        (($ptr as usize) - offset_of!($container, $field)) as *const $container
    })
}

macro_rules! container_of_mut {
    ($ptr:expr, $container:ty, $field:ident) => ({
        (($ptr as usize) - offset_of!($container, $field)) as *mut $container
    })
}

pub struct WlListIter<'a, T: 'a> {
    offset: usize,
    head: *mut wl_list,
    next: *mut wl_list,
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

impl wl_list {
    pub fn iter<'a, T>(&mut self, offset: usize) -> WlListIter<'a, T> {
	WlListIter {
	    offset: offset,
	    head: self as *mut wl_list,
	    next: self.next as *mut wl_list,
	    phantom: PhantomData
	}
    }
}

macro_rules! wl_list_iter {
    ($ptr:expr, $container:ty, $field:ident) => ({
	let offset = unsafe { offset_of!($container, $field) };

	$ptr.iter::<$container>(offset)
    })
}

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
