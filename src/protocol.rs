// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
#![allow(non_upper_case_globals, non_snake_case, unused_imports)]
#![allow(missing_docs, clippy::all)]

pub(crate) use wayland_sys as sys;
pub(crate) use wayland_server::*;
pub(crate) use wayland_commons::*;
pub(crate) use wayland_commons::wire::*;
pub(crate) use wayland_commons::map::*;

use wayland_server::{
    protocol::{wl_compositor, wl_shell, wl_output, wl_surface},
    Display,
};

include!(concat!(env!("OUT_DIR"), "/croscomp_api.rs"));
