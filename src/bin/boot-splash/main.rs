use wayland_client::{
    protocol::{wl_compositor, wl_shell},
    Display, GlobalManager,
};

pub mod protocol {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
    #![allow(missing_docs, clippy::all)]

    pub(crate) use wayland_client::{AnonymousObject, Attached, Main, Proxy, ProxyMap};
    pub(crate) use wayland_commons::map::{Object, ObjectMetadata};
    pub(crate) use wayland_commons::smallvec;
    pub(crate) use wayland_commons::wire::{Argument, ArgumentType, Message, MessageDesc};
    pub(crate) use wayland_commons::{Interface, MessageGroup};

    pub(crate) use wayland_client::protocol::wl_buffer;

    include!(concat!(env!("OUT_DIR"), "/linux_dmabuf_unstable_v1_api.rs"));
}

pub struct Card {
    file: std::fs::File
}

impl std::os::unix::io::AsRawFd for Card {
    fn as_raw_fd(&self) -> std::os::unix::io::RawFd {
        self.file.as_raw_fd()
    }
}

use drm::control::Device as ControlDevice;
use drm::buffer::Buffer;

impl drm::Device for Card {}
impl ControlDevice for Card {}

impl Card {
    pub fn open(path: &str) -> Self {
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        options.write(true);
        Card { file: options.open(path).unwrap() }
    }

    pub fn open_global() -> Self {
        Self::open("/dev/dri/card0")
    }
}

fn main() {
    let display = Display::connect_to_env().unwrap();

    let mut event_queue = display.create_event_queue();

    let attached_display = (*display).clone().attach(event_queue.token());

    let globals = GlobalManager::new(&attached_display);

    event_queue.sync_roundtrip(&mut (), |_, _, _| unreachable!()).unwrap();

    let dmabuf = globals.instantiate_exact::<protocol::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1>(3).
	unwrap();
    let fmt = drm::buffer::format::PixelFormat::ABGR8888;

    /*
     * Create a buffer with window contents
     */
    let card = Card::open_global();
    let width = 800;
    let height = 600;

    let mut db = card.create_dumb_buffer((width, height), fmt)
	    .expect("Could not create dumb buffer");

    {
	let mut map = card.map_dumb_buffer(&mut db).expect("Could not map dumbbuffer");

	let bg_color : [u8;4] = [128, 0, 0, 255];
	for dst in map.as_mut().chunks_exact_mut(4) {
	    dst.copy_from_slice(&bg_color)
	}
    }

    const DRM_CLOEXEC: u32 = 0o2000000;
    const DRM_RDWR: u32 = 0o2;

    let fd = card.buffer_to_prime_fd(db.handle(), DRM_CLOEXEC | DRM_RDWR).
	expect("Buffer to prime fd failed");

    /*
     * Init wayland objects
     */

    let params = dmabuf.create_params();
    params.add(fd, 0, 0, db.pitch(), 0, 0);
    let buffer = params.create_immed(width as i32, height as i32, fmt.as_raw(), 0);
    params.destroy();

    // The compositor allows us to creates surfaces
    let compositor = globals.instantiate_exact::<wl_compositor::WlCompositor>(1).unwrap();
    let surface = compositor.create_surface();

    // The shell allows us to define our surface as a "toplevel", meaning the
    // server will treat it as a window
    //
    // NOTE: the wl_shell interface is actually deprecated in favour of the xdg_shell
    // protocol, available in wayland-protocols. But this will do for this example.
    let shell = globals
        .instantiate_exact::<wl_shell::WlShell>(1)
        .expect("Compositor does not support wl_shell");
    let shell_surface = shell.get_shell_surface(&surface);
    shell_surface.quick_assign(|shell_surface, event, _| {
        use wayland_client::protocol::wl_shell_surface::Event;
        // This ping/pong mechanism is used by the wayland server to detect
        // unresponsive applications
        if let Event::Ping { serial } = event {
            shell_surface.pong(serial);
        }
    });

    // Set our surface as toplevel and define its contents
    shell_surface.set_toplevel();
    surface.attach(Some(&buffer), 0, 0);
    surface.commit();

    loop {
        event_queue.dispatch(&mut (), |_, _, _| { /* we ignore unfiltered messages */ }).unwrap();
    }
}
