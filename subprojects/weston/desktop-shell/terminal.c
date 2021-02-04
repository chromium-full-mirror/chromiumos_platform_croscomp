/* Copyright 2021 The Chromium OS Authors. All rights reserved.
 * Use of this source code is governed by a MIT-style license that can be
 * found in the COPYING file.
 */

#include "config.h"

#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#include <linux/input.h>
#include <assert.h>
#include <signal.h>
#include <math.h>
#include <sys/types.h>
#include <wayland-server.h>

#include "shell.h"
#include "compositor/weston.h"
#include <libweston/libweston.h>
#include "shared/zalloc.h"
#include "shared/helpers.h"
#include "libweston-desktop.h"
#include "internal.h"

struct terminal {
	struct desktop_shell *shell;
	struct wl_listener surface_create_listener;
	struct wl_client *client;
	struct wl_listener client_destroy_listener;
	struct weston_surface *client_surface;
};

static void
terminal_destroy(struct wl_listener *listener, void *data)
{
	struct terminal *t =
		container_of(listener, struct terminal,
			     client_destroy_listener);

	wl_list_remove(&t->client_destroy_listener.link);
	t->client_surface = NULL;
	t->client = NULL;
}

static const char terminal_client_path[] = "/usr/bin/croscomp-terminal";

static void
terminal_binding(struct weston_keyboard *keyboard,
		 const struct timespec *time, uint32_t key, void *data)
{
	struct terminal *t = data;
	struct weston_view *view;

	if (key == KEY_F1) {
		if (t->client_surface) {
			struct weston_desktop_surface *ds =
				weston_surface_get_desktop_surface(t->client_surface);

			weston_desktop_api_fullscreen_requested(t->shell,
								ds, false, NULL);
			weston_desktop_api_minimized_requested(t->shell, ds);
		}
		return;
	}

	if (t->client_surface) {
		struct weston_desktop_surface *ds =
			weston_surface_get_desktop_surface(t->client_surface);

		wl_list_for_each(view, &t->client_surface->views, surface_link)
			activate(t->shell, view, keyboard->seat,
				 WESTON_ACTIVATE_FLAG_CONFIGURE);
		weston_desktop_api_fullscreen_requested(t->shell,
							ds, true, t->client_surface->output);

		return;
	}

	t->client =
		weston_client_start(t->shell->compositor, terminal_client_path);

	if (!t->client) {
		weston_log("not able to start %s\n", terminal_client_path);
		return;
	}

	t->client_destroy_listener.notify = terminal_destroy;
	wl_client_add_destroy_listener(t->client,
				       &t->client_destroy_listener);
}

static void
surface_create_notify(struct wl_listener *listener, void *data)
{
	struct terminal *t =
		container_of(listener, struct terminal,
			     surface_create_listener);
	struct weston_surface *surface = data;
	struct wl_client *client = wl_resource_get_client(surface->resource);

	if (client != t->client)
		return;

	t->client_surface = surface;
}

int
init_terminal(struct desktop_shell *shell)
{
	struct weston_compositor *ec = shell->compositor;
	struct terminal *t;

	t = zalloc(sizeof(*t));
	if (t == NULL) {
		weston_log("allocation failed for terminal\n");
		return -1;
	}

	t->shell = shell;
	weston_compositor_add_key_binding(ec, KEY_F1, MODIFIER_CTRL | MODIFIER_ALT,
					  terminal_binding, t);
	weston_compositor_add_key_binding(ec, KEY_F2, MODIFIER_CTRL | MODIFIER_ALT,
					  terminal_binding, t);

	t->surface_create_listener.notify = surface_create_notify;
	wl_signal_add(&ec->create_surface_signal,
		      &t->surface_create_listener);

	return 0;
}
