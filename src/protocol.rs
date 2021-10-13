// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

#![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
#![allow(non_upper_case_globals, non_snake_case, unused_imports)]
#![allow(missing_docs, clippy::all)]

// Use our own version of these
use crate::server::{Main, Resource, ResourceMap};

pub(crate) use wayland_commons::map::*;
pub(crate) use wayland_commons::wire::*;
pub(crate) use wayland_commons::*;
pub(crate) use wayland_server::sys;
pub(crate) use wayland_server::AnonymousObject;

include!(concat!(env!("OUT_DIR"), "/wayland.rs"));
include!(concat!(env!("OUT_DIR"), "/weston-screenshooter.rs"));
include!(concat!(env!("OUT_DIR"), "/xdg-shell.rs"));
include!(concat!(env!("OUT_DIR"), "/croscomp.rs"));
include!(concat!(env!("OUT_DIR"), "/aura-shell.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/alpha-compositing-unstable-v1.rs"
));
include!(concat!(env!("OUT_DIR"), "/cursor-shapes-unstable-v1.rs"));
include!(concat!(env!("OUT_DIR"), "/pointer-gestures-unstable-v1.rs"));
include!(concat!(env!("OUT_DIR"), "/gaming-input-unstable-v2.rs"));
include!(concat!(env!("OUT_DIR"), "/input.rs"));
include!(concat!(
    env!("OUT_DIR"),
    "/keyboard-configuration-unstable-v1.rs"
));
include!(concat!(
    env!("OUT_DIR"),
    "/keyboard-extension-unstable-v1.rs"
));
include!(concat!(env!("OUT_DIR"), "/linux-dmabuf-unstable-v1.rs"));
include!(concat!(env!("OUT_DIR"), "/remote-shell-unstable-v1.rs"));
include!(concat!(env!("OUT_DIR"), "/secure-output-unstable-v1.rs"));
include!(concat!(env!("OUT_DIR"), "/stylus-tools-unstable-v1.rs"));
include!(concat!(env!("OUT_DIR"), "/stylus-unstable-v2.rs"));
include!(concat!(env!("OUT_DIR"), "/vsync-feedback-unstable-v1.rs"));
