project('libinput', 'c',
	version : '0.1',
	license : 'MIT/Expat',
	default_options : [ 'c_std=gnu99', 'warning_level=2' ],
	meson_version : '>= 0.47.0')

cc = meson.get_compiler('c')

global_args = []
global_args_maybe = [
	'-Wmissing-prototypes',
	'-Wno-unused-parameter',
	'-Wno-shift-negative-value', # required due to Pixman
	'-Wno-missing-field-initializers',
	'-Wno-pedantic',
	'-fvisibility=hidden',
]
foreach a : global_args_maybe
	if cc.has_argument(a)
		global_args += a
	endif
endforeach
add_global_arguments(global_args, language: 'c')

install_data(
	'croscomp.conf',
	install_dir : join_paths(get_option('sysconfdir'), 'init'))

proj_weston = subproject('weston', default_options: [
	'backend-default=drm',
	'backend-drm=true',
	'backend-drm-screencast-vaapi=false',
	'backend-fbdev=false',
	'backend-headless=true',
	'backend-rdp=false',
	'backend-wayland=true',
	'backend-x11=false',
	'color-management-colord=false',
	'color-management-lcms=true',
	'demo-clients=true',
	'desktop-shell-client-default=weston-desktop-shell',
	'doc=false',
	'image-jpeg=false',
	'image-webp=false',
	'image-png=true',
	'launcher-logind=false',
	'pipewire=false',
	'remoting=false',
	'renderer-gl=true',
	'resize-pool=true',
	'screenshare=true',
	'shell-desktop=true',
	'shell-fullscreen=false',
	'shell-ivi=false',
	'shell-kiosk=false',
	'systemd=false',
	'test-gl-renderer=true',
	'test-junit-xml=false',
	'wcap-decode=false',
	'weston-launch=false',
	'xwayland=false'])