srcs_libdesktop = [
	'libweston-desktop.c',
	'client.c',
	'seat.c',
	'surface.c',
	'xwayland.c',
	'wl-shell.c',
	'xdg-shell.c',
	'xdg-shell-v6.c',
	'exo-extensions.c',
	alpha_compositing_unstable_v1_server_protocol_h,
	alpha_compositing_unstable_v1_protocol_c,
	aura_shell_server_protocol_h,
	aura_shell_protocol_c,
	cursor_shapes_unstable_v1_server_protocol_h,
	cursor_shapes_unstable_v1_protocol_c,
	gaming_input_unstable_v2_server_protocol_h,
	gaming_input_unstable_v2_protocol_c,
	keyboard_configuration_unstable_v1_server_protocol_h,
	keyboard_configuration_unstable_v1_protocol_c,
	keyboard_extension_unstable_v1_server_protocol_h,
	keyboard_extension_unstable_v1_protocol_c,
	pointer_gestures_unstable_v1_server_protocol_h,
	pointer_gestures_unstable_v1_protocol_c,
	remote_shell_unstable_v1_server_protocol_h,
	remote_shell_unstable_v1_protocol_c,
	secure_output_unstable_v1_server_protocol_h,
	secure_output_unstable_v1_protocol_c,
	stylus_tools_unstable_v1_server_protocol_h,
	stylus_tools_unstable_v1_protocol_c,
	stylus_unstable_v2_server_protocol_h,
	stylus_unstable_v2_protocol_c,
	vsync_feedback_unstable_v1_server_protocol_h,
	vsync_feedback_unstable_v1_protocol_c,
	xdg_shell_unstable_v6_server_protocol_h,
	xdg_shell_unstable_v6_protocol_c,
	xdg_shell_server_protocol_h,
	xdg_shell_protocol_c,
]
lib_desktop = shared_library(
	'weston-desktop-@0@'.format(libweston_major),
	srcs_libdesktop,
	include_directories: common_inc,
	install: true,
	version: '0.0.@0@'.format(libweston_revision),
	dependencies: dep_libweston_public
)
dep_lib_desktop = declare_dependency(
	link_with: lib_desktop,
	dependencies: dep_libweston_public
)

pkgconfig.generate(
	lib_desktop,
	filebase: 'libweston-desktop-@0@'.format(libweston_major),
	name: 'libweston-desktop',
	version: version_weston,
	description: 'Desktop shells abstraction library for libweston compositors',
	requires_private: [ lib_weston, dep_wayland_server ],
	subdirs: dir_include_libweston
)
