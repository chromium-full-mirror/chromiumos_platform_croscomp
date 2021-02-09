use std::env;
use std::fs::metadata;
use std::cell::RefCell;
use std::rc::Rc;

#[macro_use]
mod ffi;
mod terminal;

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

const NULL_HEAD: *mut ffi::weston_head = 0 as *mut ffi::weston_head;
const NULL_STR: *mut ::std::os::raw::c_char = 0 as *mut ::std::os::raw::c_char;
const NULL_VOID: *mut ::std::os::raw::c_void = 0 as *mut ::std::os::raw::c_void;
type Void = ::std::os::raw::c_void;

extern "C"
fn head_notify(_listener: *mut ffi::wl_listener, data: *mut ::std::os::raw::c_void) {
    let c = data as *mut ffi::weston_compositor;
    println!("listener");

    unsafe {
	let mut head = ffi::weston_compositor_iterate_heads(c, NULL_HEAD);
	while head != NULL_HEAD {
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

		set_mode(output, ffi::weston_drm_backend_output_mode::WESTON_DRM_BACKEND_OUTPUT_PREFERRED, NULL_STR);

		ffi::weston_output_set_scale(output, 1);

		ffi::weston_output_set_transform(output,
						 ffi::wl_output_transform::WL_OUTPUT_TRANSFORM_NORMAL as u32);
		let set_gbm_format = (*api).set_gbm_format.unwrap();
		set_gbm_format(output, NULL_STR);

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

#[derive(Debug)]
struct Display {
    native: *mut ffi::wl_display
}

impl Display {
    fn new() -> Self {
	Display { native: unsafe { ffi::wl_display_create() } }
    }

    fn run(&self) {
	unsafe { ffi::wl_display_run(self.native) }
    }

    fn add_socket_auto(&self) -> *const ::std::os::raw::c_char {
	unsafe { ffi::wl_display_add_socket_auto(self.native) }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
	println!("drop display");
	unsafe { ffi::wl_display_destroy(self.native) }
    }
}

#[derive(Debug)]
pub struct Compositor {
    native: *mut ffi::weston_compositor
}

impl Compositor {
    fn new(display: &Display, ctx: *mut ffi::weston_log_context) -> Self {
	Compositor { native: unsafe {
	    ffi::weston_compositor_create(display.native,
					  ctx, 0 as *mut ::std::os::raw::c_void)
	} }
    }

    fn add_heads_changed_listener(&mut self, listener: &mut ffi::wl_listener) {
	unsafe {
	    ffi::weston_compositor_add_heads_changed_listener(self.native,
							      listener)
	}
    }

    fn add_key_binding(&mut self, key: u32,
		       modifier: ffi::weston_keyboard_modifier,
		       binding: ffi::weston_key_binding_handler_t,
		       data: *mut ::std::os::raw::c_void) -> *mut ffi::weston_binding{
	unsafe {
	    ffi::weston_compositor_add_key_binding(
		self.native, key, modifier, binding, data)
	}
    }

    fn wake(&mut self) {
	unsafe { ffi::weston_compositor_wake(self.native) }
    }

    fn flush_heads_changed(&self) {
	unsafe {
	    ffi::weston_compositor_flush_heads_changed(self.native)
	}
    }

    fn set_xkb_rule_names(&self, names: &mut ffi::xkb_rule_names) {
	unsafe {
	    ffi::weston_compositor_set_xkb_rule_names(self.native, names);
	}
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
	println!("drop compositor");
	unsafe { ffi::weston_compositor_destroy(self.native) }
    }
}


#[derive(Debug)]
pub struct Shell {
    native: *mut ffi::desktop_shell,
    compositor: Rc<RefCell<Compositor>>
}

impl Shell {
    fn new(compositor: &Rc<RefCell<Compositor>>) -> Self {
	let native = compositor.borrow().native;
	Shell {
	    native: unsafe { ffi::desktop_shell_init(native) },
	    compositor: Rc::clone(compositor)
	}
    }

    fn finish_init(&self) {
	unsafe {
	    ffi::weston_compositor_init_input(self.compositor.borrow().native);
	    ffi::exo_extensions_init(self.native);
	}
    }
}

const NULL_LIST: ffi::wl_list = ffi::wl_list {
	prev: 0 as *mut ffi::wl_list,
	next: 0 as *mut ffi::wl_list
};

type NotifyFunc =
    unsafe extern "C" fn(listener: *mut ffi::wl_listener, data: *mut ::std::os::raw::c_void);

impl ffi::wl_listener {
    fn new(notify: NotifyFunc) -> Self {
	ffi::wl_listener {
	    link: NULL_LIST,
	    notify: Some(notify)
	}
    }
}

fn main() {
    verify_xdg_runtime_dir();

    let ctx = unsafe {
	ffi::weston_log_ctx_create()
    };

    let display = Display::new();

    let compositor =
	Rc::new(RefCell::new(Compositor::new(&display, ctx)));

    println!("set log handler");
    unsafe {
	ffi::weston_log_set_handler(Some(ffi::vlog),
				    Some(ffi::vlog_continue))
    }

    println!("done set log handler");

    let mut listener = ffi::wl_listener::new(head_notify);

    compositor.borrow_mut().add_heads_changed_listener(&mut listener);

    let mut rules = ffi::xkb_rule_names {
	rules: NULL_STR,
	model: NULL_STR,
	layout: NULL_STR,
	variant: NULL_STR,
	options: NULL_STR
    };

    compositor.borrow_mut().set_xkb_rule_names(&mut rules);

    let mut config = ffi::weston_drm_backend_config {
	base: ffi::weston_backend_config {
	    struct_version: ffi::WESTON_DRM_BACKEND_CONFIG_VERSION,
	    struct_size: std::mem::size_of::<ffi::weston_drm_backend_config>() as u64
	},
	tty: -1,
	use_pixman: false,
	seat_id: NULL_STR,
	gbm_format: NULL_STR,
	configure_device: None,
	pageflip_timeout: 0,
	specific_device: NULL_STR,
	use_pixman_shadow: false,
	continue_without_input: true
    };

    let _ret = unsafe { ffi::weston_compositor_load_backend(
	compositor.borrow().native,
	ffi::weston_compositor_backend::WESTON_BACKEND_DRM,
	&mut config.base)
    };

    compositor.borrow().flush_heads_changed();

    let socket_name = display.add_socket_auto();

    if socket_name == NULL_STR {
	panic!("failed to create listening socket")
    }

    let shell = Rc::new(RefCell::new(
	Shell::new(&compositor)));

    terminal::init(&shell);

    compositor.borrow_mut().wake();

    display.run();
}
