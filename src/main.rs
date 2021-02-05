use std::env;
use std::fs::metadata;

fn verify_xdg_runtime_dir() {
    let path = env::var_os("XDG_RUNTIME_DIR")
	.expect("environment variable XDG_RUNTIME_DIR is not set.")
	.into_string()
	.expect("invalid value");

    let md = metadata(&path).unwrap();
    if !md.is_dir() {
	panic!("environment variable XDG_RUNTIME_DIR\nis set to \"{}\", \
		which is not a directory.", path);
    }

    // Verify permissions 0700 and owned by user?
}

mod ffi;

extern "C"
fn head_notify(_listener: *mut ffi::wl_listener, data: *mut ::std::os::raw::c_void) {
    let c = data as *mut ffi::weston_compositor;
    println!("listener");

    let null_head = 0 as *mut ffi::weston_head;
    let null_str = 0 as *mut ::std::os::raw::c_char;

    unsafe {
	let mut head = ffi::weston_compositor_iterate_heads(c, null_head);
	while head != null_head {
	    let connected = ffi::weston_head_is_connected(head);
	    let enabled = ffi::weston_head_is_enabled(head);
	    let changed = ffi::weston_head_is_device_changed(head);

	    println!("head {:?}, connected {}, enabled {}, changed {}",
		     head, connected, enabled, changed);

	    if connected && !enabled {
		let output = ffi::weston_compositor_create_output(c, "output\0".as_ptr() as *const i8);

		ffi::weston_output_attach_head(output, head);

		let api = ffi::weston_plugin_api_get(
		    c,
		    ffi::WESTON_DRM_OUTPUT_API_NAME.as_ptr() as *const i8,
		    std::mem::size_of::<ffi::weston_drm_output_api>() as u64)
		    as *mut ffi::weston_drm_output_api;

		let set_mode = (*api).set_mode.unwrap();

		set_mode(output, ffi::weston_drm_backend_output_mode::WESTON_DRM_BACKEND_OUTPUT_PREFERRED, null_str);

		ffi::weston_output_set_scale(output, 1);

		ffi::weston_output_set_transform(output,
						 ffi::wl_output_transform::WL_OUTPUT_TRANSFORM_NORMAL as u32);
		let set_gbm_format = (*api).set_gbm_format.unwrap();
		set_gbm_format(output, null_str);

		let set_seat = (*api).set_seat.unwrap();
		set_seat(output, "\0".as_ptr() as *const i8);

		if ffi::weston_output_enable(output) != 0 {
		    println!("weston_output_enable return non-0");
		}

		println!("attached to output {:?}", output);
	    }

	    head = ffi::weston_compositor_iterate_heads(c, head);
	}
    }
}

fn main() {
    verify_xdg_runtime_dir();

    let args: Vec<String> = env::args().collect();

    let ctx = unsafe {
	ffi::weston_log_ctx_create()
    };

    let display = unsafe {
	ffi::wl_display_create()
    };

    let compositor = unsafe {
	ffi::weston_compositor_create(display,
				      ctx, 0 as *mut ::std::os::raw::c_void)
    };

    println!("set log handler");
    unsafe {
	ffi::weston_log_set_handler(Some(ffi::vlog),
				    Some(ffi::vlog_continue))
    }

    println!("done set log handler");

    let null_list = ffi::wl_list {
	prev: 0 as *mut ffi::wl_list,
	next: 0 as *mut ffi::wl_list
    };

    let mut listener = ffi::wl_listener {
	link: null_list,
	notify: Some(head_notify)
    };

    unsafe {
	ffi::weston_compositor_add_heads_changed_listener(compositor,
							  &mut listener)
    }

    let null_str = 0 as *mut ::std::os::raw::c_char;

    let mut rules = ffi::xkb_rule_names {
	rules: null_str,
	model: null_str,
	layout: null_str,
	variant: null_str,
	options: null_str
    };

    unsafe {
	ffi::weston_compositor_set_xkb_rule_names(compositor, &mut rules);
    }

    let mut config = ffi::weston_drm_backend_config {
	base: ffi::weston_backend_config {
	    struct_version: ffi::WESTON_DRM_BACKEND_CONFIG_VERSION,
	    struct_size: std::mem::size_of::<ffi::weston_drm_backend_config>() as u64
	},
	tty: -1,
	use_pixman: false,
	seat_id: null_str,
	gbm_format: null_str,
	configure_device: None,
	pageflip_timeout: 0,
	specific_device: null_str,
	use_pixman_shadow: false,
	continue_without_input: true
    };

    let _ret = unsafe { ffi::weston_compositor_load_backend(
	compositor,
	ffi::weston_compositor_backend::WESTON_BACKEND_DRM,
	&mut config.base)
    };

    unsafe {
	ffi::weston_compositor_flush_heads_changed(compositor)
    };

    let socket_name = unsafe {
	ffi::wl_display_add_socket_auto(display)
    };
    if socket_name == null_str {
	panic!("failed to create listening socket")
    }

    unsafe {
	ffi::desktop_shell_init(compositor)
    };

    println!("{:?}", display);
    println!("{:?}", args);
    println!("{:?}", compositor);

    unsafe {
	ffi::weston_compositor_wake(compositor)
    };

    unsafe {
	ffi::wl_display_run(display)
    };

}
