#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_macros)]

// Put these uglies here

macro_rules! offset_of {
    ($container:ty, $field:ident) => {
        &(*(0usize as *const $container)).$field as *const _ as usize
    }
}

macro_rules! container_of {
    ($ptr:ident, $container:ty, $field:ident) => ({
        (($ptr as usize) - offset_of!($container, $field)) as *const $container
    })
}

macro_rules! container_of_mut {
    ($ptr:ident, $container:ty, $field:ident) => ({
        (($ptr as usize) - offset_of!($container, $field)) as *mut $container
    })
}


include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
