if get_option('shell-desktop')
	config_h.set_quoted('WESTON_SHELL_CLIENT', get_option('desktop-shell-client-default'))

	srcs_shell_desktop = [
		'shell.c',
		'exposay.c',
		'client.c',
		'seat.c',
		'surface.c',
		'xdg-shell.c',
		'workspace.c',
		xdg_shell_server_protocol_h,
		xdg_shell_protocol_c,
	]
	deps_shell_desktop = [
		dep_libm,
		dep_libshared,
		dep_libweston_public,
	]
	plugin_shell_desktop = static_library(
		'desktop-shell',
		srcs_shell_desktop,
		include_directories: common_inc,
		dependencies: deps_shell_desktop,
		install: true,
		install_dir: dir_module_weston,
		install_rpath: '$ORIGIN'
	)
	env_modmap += 'desktop-shell.so=@0@;'.format(plugin_shell_desktop.full_path())
endif
