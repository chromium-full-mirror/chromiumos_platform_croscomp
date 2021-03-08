// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

pub fn init(shell: &Rc<RefCell<crate::Shell>>) {
    let shell = &shell.borrow();
    let mut compositor = shell.compositor.borrow_mut();

    use super::protocol::zcr_alpha_compositing_v1::ZcrAlphaCompositingV1;

    let global = compositor.create_global::<ZcrAlphaCompositingV1>(1)
	.on_request(|_resource, request| {
	    println!("{:?}", request);
	});
    Box::into_raw(Box::new(global));


    use super::protocol::zaura_shell::ZauraShell;
    let global = compositor.create_global::<ZauraShell>(12)
	.on_request(|_resource, request| {
	    use super::protocol::zaura_shell::Request;
	    match request {
		Request::GetAuraSurface { .. } => {
		    // xxx resource.create().on_request(|request| { ... });
		    //
		    // returns (), but maybe have:
		    //  .with(|resource| ... _)
		    // if we want to do something with the resource...
		    println!("get aura surface");
		},

		Request::GetAuraOutput { .. } => {
		    println!("get aura output");
		},
	    }
	});
    Box::into_raw(Box::new(global));


    use super::protocol::zwp_pointer_gestures_v1::ZwpPointerGesturesV1;
    let global = compositor.create_global::<ZwpPointerGesturesV1>(2)
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_gaming_input_v2::ZcrGamingInputV2;
    let global = compositor.create_global::<ZcrGamingInputV2>(2)
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_keyboard_configuration_v1::ZcrKeyboardConfigurationV1;
    let global = compositor.create_global::<ZcrKeyboardConfigurationV1>(2)
	.on_request(|_resource, request| {
	    // xxx: handle: get_keyboard_device_configuration request
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_keyboard_extension_v1::ZcrKeyboardExtensionV1;
    let global = compositor.create_global::<ZcrKeyboardExtensionV1>(2)
	.on_request(|_resource, request| {
	    // xxx: handle: get_extended_keyboard request
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_cursor_shapes_v1::ZcrCursorShapesV1;
    let global = compositor.create_global::<ZcrCursorShapesV1>(2)
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_remote_shell_v1::ZcrRemoteShellV1;
    let global = compositor.create_global::<ZcrRemoteShellV1>(2)
	.on_bind(|_resource| {
	    // xxx create desktop client, send workspace info
	    println!("on_bind, send workspace info")

	    //zcr_remote_shell_v1_send_default_device_scale_factor(shell, 1 << 24);

	    //zcr_remote_shell_v1_send_configure(shell, ZCR_REMOTE_SHELL_V1_LAYOUT_MODE_WINDOWED);

	})
	.on_request(|_resource, request| {
	    // xxx: handle get_remote_surface
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_secure_output_v1::ZcrSecureOutputV1;
    let global = compositor.create_global::<ZcrSecureOutputV1>(2)
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_stylus_v2::ZcrStylusV2;
    let global = compositor.create_global::<ZcrStylusV2>(2)
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_stylus_tools_v1::ZcrStylusToolsV1;
    let global = compositor.create_global::<ZcrStylusToolsV1>(2)
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));

    use super::protocol::zcr_vsync_feedback_v1::ZcrVsyncFeedbackV1;
    let global = compositor.create_global::<ZcrVsyncFeedbackV1>(2)
	.on_request(|_resource, request| {
	    println!("{:?}", request)
	});
    Box::into_raw(Box::new(global));
}
