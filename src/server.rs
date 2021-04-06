// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::marker::PhantomData;
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use std::os::unix::net::{SocketAddr, UnixListener};

use wayland_server::MessageGroup;

use crate::ffi;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Resource<I>
where
    I: wayland_server::Interface,
{
    native: *mut ffi::wl_resource,
    phantom: PhantomData<I>,
}

impl<I> Resource<I>
where
    I: wayland_server::Interface,
{
    pub fn new<J>(&self, id: u32) -> Resource<J>
    where
        J: wayland_server::Interface + From<Resource<J>> + AsRef<Resource<J>>,
    {
        unsafe {
            let client = ffi::wl_resource_get_client(self.native);
            let version = ffi::wl_resource_get_version(self.native);

            Resource::from_c_ptr(ffi::wl_resource_create(
                client,
                J::c_interface() as *mut ffi::wl_interface,
                version,
                id,
            ))
        }
    }

    pub fn from_c_ptr(native: *mut ffi::wl_resource) -> Self {
        Resource::<I> {
            native,
            phantom: PhantomData,
        }
    }

    pub unsafe fn make_child_for<J>(&self, id: u32) -> Option<Main<J>>
    where
        J: wayland_server::Interface + From<Resource<J>> + AsRef<Resource<J>>,
    {
        Some(Main::<J> {
            resource: self.new(id),
        })
    }

    pub fn c_ptr(&self) -> *mut ffi::wl_resource {
        self.native
    }

    pub fn client(&self) -> *mut ffi::wl_client {
        unsafe { ffi::wl_resource_get_client(self.native) }
    }

    pub fn id(&self) -> u32 {
        unsafe { ffi::wl_resource_get_id(self.native) }
    }

    pub fn user_data<J>(&self) -> &mut J {
        unsafe { &mut *(ffi::wl_resource_get_user_data(self.native) as *mut J) }
    }

    pub fn send(&self, message: I::Event) {
        message.as_raw_c_in(|opcode, args| unsafe {
            ffi::wl_resource_post_event_array(
                self.native,
                opcode,
                args.as_mut_ptr() as *mut ffi::wl_argument,
            );
        })
    }

    pub fn on_request<F>(&self, f: F) -> &Self
    where
        F: 'static + Fn(&mut Resource<I>, &I::Request),
    {
        unsafe extern "C" fn call_dispatcher<I, F>(
            data: *const ::std::ffi::c_void,
            object: *mut ::std::ffi::c_void,
            opcode: u32,
            _message: *const ffi::wl_message,
            args: *mut ffi::wl_argument,
        ) -> ::std::os::raw::c_int
        where
            I: wayland_server::Interface,
            F: 'static + Fn(&mut Resource<I>, &I::Request),
        {
            let request = <I::Request as wayland_server::MessageGroup>::from_raw_c(
                object,
                opcode,
                args as *const wayland_sys::common::wl_argument,
            );

            let mut resource = Resource::<I>::from_c_ptr(object as *mut _);
            let f = &mut *(data as *mut F);

            if let Ok(request) = request {
                (*f)(&mut resource, &request)
            }
            0
        }

        println!("set dispatcher for {}: {:?}", I::NAME, &f as *const _);

        let dispatcher = Box::new(f);
        unsafe {
            ffi::wl_resource_set_dispatcher(
                self.native,
                Some(call_dispatcher::<I, F>),
                Box::into_raw(dispatcher) as *mut _,
                std::ptr::null_mut(),
                None,
            )
        }

        self
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Main<I>
where
    I: wayland_server::Interface,
{
    resource: Resource<I>,
}

impl<I> Main<I>
where
    I: wayland_server::Interface,
{
    pub fn on_request<F>(&self, f: F) -> &Self
    where
        F: 'static + Fn(&mut Resource<I>, &I::Request),
    {
        self.resource.on_request(f);
        self
    }
}

pub struct ResourceMap {}

impl ResourceMap {
    pub fn get_new<I>(&mut self, _id: u32) -> Option<Main<I>>
    where
        I: wayland_server::Interface,
    {
        panic!("ResourceMap::get_new called");
    }

    pub fn get<I: wayland_server::Interface>(&mut self, _id: u32) -> Option<Resource<I>> {
        panic!("ResourceMap::get called");
    }
}

struct GlobalHandlers<I>
where
    I: wayland_server::Interface,
{
    handle_bind: Option<Box<dyn Fn(&mut Resource<I>)>>,
    dispatcher: Option<Box<dyn Fn(&mut Resource<I>, &I::Request)>>,
    handle_destroy: Option<Box<dyn Fn(&mut Resource<I>)>>,
}

pub struct Global<I>
where
    I: wayland_server::Interface,
{
    native: *mut ffi::wl_global,
    handlers: Box<GlobalHandlers<I>>,
}

impl<I: wayland_server::Interface> Global<I> {
    pub fn on_bind<F>(mut self, f: F) -> Self
    where
        F: 'static + Fn(&mut Resource<I>),
    {
        // hook for handling bind time checks or allocations
        self.handlers.handle_bind = Some(Box::new(f));
        self
    }

    pub fn on_request<F>(mut self, f: F) -> Self
    where
        F: 'static + Fn(&mut Resource<I>, &I::Request),
    {
        println!("set dispatcher for {}: {:?}", I::NAME, &f as *const _);
        self.handlers.dispatcher = Some(Box::new(f));
        self
    }

    pub fn on_destroy<F>(mut self, f: F) -> Self
    where
        F: 'static + Fn(&mut Resource<I>),
    {
        self.handlers.handle_destroy = Some(Box::new(f));
        self
    }
}

#[derive(Debug)]
pub struct Server {
    pub native: *mut ffi::wl_display,
}

impl Server {
    pub fn new() -> Self {
        Server {
            native: unsafe { ffi::wl_display_create() },
        }
    }

    pub fn when_idle<F>(&mut self, f: F)
    where
        F: FnOnce(),
    {
        let event_loop = unsafe { &mut *ffi::wl_display_get_event_loop(self.native) };

        extern "C" fn call_closure<F>(data: *mut ::std::os::raw::c_void)
        where
            F: FnOnce(),
        {
            let f = unsafe { Box::from_raw(data as *mut F) };

            f()
        }

        let p = Box::into_raw(Box::new(f));

        unsafe {
            ffi::wl_event_loop_add_idle(event_loop, Some(call_closure::<F>), p as *mut _);
        }
    }

    pub fn on_connect<F>(&mut self, listener: UnixListener, f: F)
    where
        F: Fn(RawFd, SocketAddr),
    {
        let event_loop = unsafe { &mut *ffi::wl_display_get_event_loop(self.native) };

        extern "C" fn call_closure<F>(fd: i32, _: u32, data: *mut ::std::os::raw::c_void) -> i32
        where
            F: Fn(RawFd, SocketAddr),
        {
            let listener = unsafe { UnixListener::from_raw_fd(fd) };
            let f = unsafe { &*(data as *mut F) };
            match listener.accept() {
                Ok((socket, addr)) => f(socket.into_raw_fd(), addr),
                Err(_) => (),
            }

            1
        }

        let p = Box::into_raw(Box::new(f));

        unsafe {
            ffi::wl_event_loop_add_fd(
                event_loop,
                listener.into_raw_fd(),
                ffi::WL_EVENT_READABLE as u32,
                Some(call_closure::<F>),
                p as *mut _,
            );
        }
    }

    pub fn create_client(&self, fd: RawFd) -> *mut ffi::wl_client {
        unsafe { ffi::wl_client_create(self.native, fd) }
    }

    pub fn create_global<I>(&mut self, version: u32) -> Global<I>
    where
        I: wayland_server::Interface,
    {
        unsafe extern "C" fn call_dispatcher<I>(
            data: *const ::std::ffi::c_void,
            object: *mut ::std::ffi::c_void,
            opcode: u32,
            _message: *const ffi::wl_message,
            args: *mut ffi::wl_argument,
        ) -> ::std::os::raw::c_int
        where
            I: wayland_server::Interface,
        {
            let request = <I::Request as wayland_server::MessageGroup>::from_raw_c(
                object,
                opcode,
                args as *const wayland_sys::common::wl_argument,
            );

            let mut resource = Resource::<I> {
                native: object as *mut ffi::wl_resource,
                phantom: PhantomData,
            };

            let handlers = &mut *(data as *mut GlobalHandlers<I>);

            if let (Ok(request), Some(dispatcher)) = (request, handlers.dispatcher.as_ref()) {
                (*dispatcher)(&mut resource, &request)
            }
            0
        }

        unsafe extern "C" fn call_destroy_closure<I>(native: *mut ffi::wl_resource)
        where
            I: wayland_server::Interface,
        {
            let b =
                &*(ffi::wl_resource_get_user_data(native) as *mut Box<dyn Fn(&mut Resource<I>)>);

            let mut resource = Resource::<I> {
                native,
                phantom: PhantomData,
            };

            b(&mut resource)
        }

        unsafe extern "C" fn call_bind_closure<I>(
            client: *mut ffi::wl_client,
            data: *mut ::std::os::raw::c_void,
            version: u32,
            id: u32,
        ) where
            I: wayland_server::Interface,
        {
            let client = &mut *client;

            let native = client.create_resource(&mut *(I::c_interface() as *mut _), version, id);

            let mut resource = Resource::<I> {
                native,
                phantom: PhantomData,
            };

            let handlers = &mut *(data as *mut GlobalHandlers<I>);

            println!("handlers: {:?}", handlers as *const _);

            if let Some(ref handle_bind) = handlers.handle_bind {
                println!(
                    "calling handle_bind for {}: {:?}",
                    I::NAME,
                    &(*handle_bind) as *const _
                );
                (*handle_bind)(&mut resource);
                // xxx return value from handle_bind should indicate whether
                // or not bind was successful
            }

            if let Some(ref dispatcher) = handlers.dispatcher {
                println!(
                    "setting dispatcher for {}: {:?}",
                    I::NAME,
                    &(*dispatcher) as *const _
                );

                ffi::wl_resource_set_dispatcher(
                    resource.native,
                    Some(call_dispatcher::<I>),
                    handlers as *mut _ as *mut std::ffi::c_void,
                    std::ptr::null_mut(),
                    None,
                )
            }

            if let Some(ref mut handle_destroy) = handlers.handle_destroy {
                ffi::wl_resource_set_user_data(
                    resource.native,
                    handle_destroy as *mut _ as *mut std::ffi::c_void,
                );
                ffi::wl_resource_set_destructor(resource.native, Some(call_destroy_closure::<I>));
            }
        }

        let mut global = Global {
            native: std::ptr::null_mut(),
            handlers: Box::new(GlobalHandlers {
                handle_bind: None,
                dispatcher: None,
                handle_destroy: None,
            }),
        };

        global.native = unsafe {
            ffi::wl_global_create(
                self.native,
                I::c_interface() as *const _,
                version as i32,
                &mut *global.handlers as *mut _ as *mut std::ffi::c_void,
                Some(call_bind_closure::<I>),
            )
        };

        global
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        println!("drop display");
        unsafe { ffi::wl_display_destroy(self.native) }
    }
}
