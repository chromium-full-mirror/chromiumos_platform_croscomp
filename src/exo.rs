// Copyright 2021 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::cell::RefCell;
use std::rc::Rc;

pub fn init(rc: &Rc<RefCell<crate::Shell>>) {
    let shell = &rc.borrow();
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
		Request::GetAuraSurface { id, surface: _ } => {
		    println!("get aura surface");
		    id.on_request(|_, request| {
			println!("request for aura surface: {:?}", request)
		    });
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
    let rc = Rc::clone(rc);
    let global = compositor.create_global::<ZcrRemoteShellV1>(2)
	.on_bind(move |resource| {
	    use super::protocol::zcr_remote_shell_v1::Event::*;
	    use super::protocol::zcr_remote_shell_v1::LayoutMode;
	    use super::protocol::zcr_remote_surface_v1::SystemuiVisibilityState;
	    use super::protocol::wl_output;;

	    let shell = &rc.borrow();
	    let mut compositor = shell.compositor.borrow_mut();

	    println!("on_bind, send workspace info");

	    for head in compositor.head_iter_mut()
		.filter(|h| !h.output.is_null()) {
		    let output = unsafe { &*head.output };
		    resource.send(WorkspaceInfo {
			display_id_hi: 0,
			display_id_lo: 0,
			x: output.x,
			y: output.y,
			width: output.width,
			height: output.height,
			inset_left: 0,
			inset_top: 0,
			inset_right: 0,
			inset_bottom: 0,
			stable_inset_left: 0,
			stable_inset_top: 0,
			stable_inset_right: 0,
			stable_inset_bottom: 0,
			systemui_visibility: SystemuiVisibilityState::Visible.to_raw() as i32,
			transform: wl_output::Transform::Normal.to_raw() as i32,
			is_internal: head.connection_internal as u32,
			identification_data: Vec::new(),
		    });
		}

	    // xxx create desktop client

	    resource.send(DefaultDeviceScaleFactor { scale: 1 << 24 });
	    resource.send(Configure { layout_mode: LayoutMode::Windowed.to_raw() });
	})
	.on_request(|_resource, request| {
	    use super::protocol::zcr_remote_shell_v1::Request::*;
	    match request {
		Destroy => { },
		GetRemoteSurface { id, surface: _, container: _ } => {
		    id.on_request(|_resource, request| match request {
			_ => { println!("request: {:?}", request) }
		    });
		},
		GetNotificationSurface { id: _, surface: _, notification_key: _ } => { },
		GetInputMethodSurface { id: _, surface: _ } =>  { },
		GetToastSurface { id: _, surface: _ } => { },
		GetRemoteOutput { id: _, output: _ } => { },
		SetUseDefaultDeviceScaleCancellation { use_default_device_scale_factor: _ } => { },
	    }
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
