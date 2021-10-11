// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use log::debug;
use std::cell::RefCell;
use std::rc::Rc;

pub fn init(rc: &Rc<RefCell<crate::Shell>>) {
    let shell = &rc.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    use super::protocol::zcr_alpha_compositing_v1::ZcrAlphaCompositingV1;

    let global = compositor
        .server
        .create_global::<ZcrAlphaCompositingV1>(1)
        .on_request(|_resource, request| {
            debug!("{:?}", request);
        });
    Box::into_raw(Box::new(global));

    use super::protocol::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;
    let global = compositor
        .server
        .create_global::<ZwpPointerGesturesV1>(2)
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_gaming_input_v2::ZcrGamingInputV2;
    let global = compositor
        .server
        .create_global::<ZcrGamingInputV2>(2)
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_keyboard_configuration_v1::ZcrKeyboardConfigurationV1;
    let global = compositor
        .server
        .create_global::<ZcrKeyboardConfigurationV1>(2)
        .on_request(|_resource, request| {
            // xxx: handle: get_keyboard_device_configuration request
            debug!("{:?}", request)
        });
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_keyboard_extension_v1::ZcrKeyboardExtensionV1;
    let global = compositor
        .server
        .create_global::<ZcrKeyboardExtensionV1>(1)
        .on_request(|_resource, request| {
            // xxx: handle: get_extended_keyboard request
            debug!("{:?}", request)
        });
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_cursor_shapes_v1::ZcrCursorShapesV1;
    let global = compositor
        .server
        .create_global::<ZcrCursorShapesV1>(1)
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_secure_output_v1::ZcrSecureOutputV1;
    let global = compositor
        .server
        .create_global::<ZcrSecureOutputV1>(1)
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_stylus_v2::ZcrStylusV2;
    let global = compositor
        .server
        .create_global::<ZcrStylusV2>(2)
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_stylus_tools_v1::ZcrStylusToolsV1;
    let global = compositor
        .server
        .create_global::<ZcrStylusToolsV1>(1)
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_vsync_feedback_v1::ZcrVsyncFeedbackV1;
    let global = compositor
        .server
        .create_global::<ZcrVsyncFeedbackV1>(1)
        .on_request(|_resource, request| debug!("{:?}", request));
    Box::into_raw(Box::new(global));
}
