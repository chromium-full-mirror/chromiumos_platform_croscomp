/*
 * Copyright © 2021 Google, Inc
 *
 * Permission is hereby granted, free of charge, to any person obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice (including the next
 * paragraph) shall be included in all copies or substantial portions of the
 * Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
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
#include <libweston/zalloc.h>
#include "shared/helpers.h"
#include "shared/timespec-util.h"
#include "libweston-desktop.h"
#include "internal.h"

#include "croscomp-server-protocol.h"

static int
splash_get_label(struct weston_surface *surface, char *buf, size_t len)
{
	return snprintf(buf, len, "splash for output %s",
			(surface->output ? surface->output->name : "NULL"));
}

static void
splash_committed(struct weston_surface *es, int32_t sx, int32_t sy)
{
	struct weston_view *ev =
		container_of(es->views.next, struct weston_view, surface_link);
	struct weston_compositor *ec = ev->surface->compositor;
	struct weston_layer *layer = &ec->fade_layer;

	weston_view_set_position(ev, ev->output->x, ev->output->y);
	ev->surface->is_mapped = true;
	ev->is_mapped = true;

	if (wl_list_empty(&ev->layer_link.link)) {
		weston_layer_entry_insert(&layer->view_list, &ev->layer_link);
		weston_compositor_schedule_repaint(ev->surface->compositor);
	}
}

static void
handle_splash_surface_destroy(struct wl_listener *listener, void *data)
{
	struct shell_output *output =
	    container_of(listener, struct shell_output, splash_surface_listener);

	weston_log("splash surface gone\n");
	wl_list_remove(&output->splash_surface_listener.link);
	output->splash_surface = NULL;
}

static void
splash_set(struct wl_client *client,
	   struct wl_resource *resource,
	   struct wl_resource *output_resource,
	   struct wl_resource *surface_resource)
{
	struct desktop_shell *shell = wl_resource_get_user_data(resource);
	struct weston_surface *surface =
		wl_resource_get_user_data(surface_resource);
	struct shell_output *sh_output;
	struct weston_view *view, *next;

	if (surface->committed) {
		wl_resource_post_error(surface_resource,
				       WL_DISPLAY_ERROR_INVALID_OBJECT,
				       "surface role already assigned");
		return;
	}

	wl_list_for_each_safe(view, next, &surface->views, surface_link)
		weston_view_destroy(view);
	view = weston_view_create(surface);

	surface->committed = splash_committed;
	surface->committed_private = shell;
	weston_surface_set_label_func(surface, splash_get_label);
	surface->output = weston_head_from_resource(output_resource)->output;
	weston_view_set_output(view, surface->output);

	sh_output = find_shell_output_from_weston_output(shell, surface->output);
	sh_output->splash_surface = surface;

	sh_output->splash_surface_listener.notify =
		handle_splash_surface_destroy;
	wl_signal_add(&surface->destroy_signal,
		      &sh_output->splash_surface_listener);
}

static const struct croscomp_splash_interface croscomp_splash_implementation = {
	.set = splash_set
};

static void
unbind_splash(struct wl_resource *resource)
{
}

static void
bind_splash(struct wl_client *client,
		   void *data, uint32_t version, uint32_t id)
{
	struct desktop_shell *shell = data;
	struct wl_resource *resource;

	resource = wl_resource_create(client, &croscomp_splash_interface,
				      1, id);

	wl_resource_set_implementation(resource,
				       &croscomp_splash_implementation,
				       shell, unbind_splash);
}

static void
launch_splash_process(void *data)
{
	struct desktop_shell *shell = data;

	weston_client_start(shell->compositor, "/usr/bin/weston-simple-splash");
}

int
init_splash(struct desktop_shell *shell)
{
	struct weston_compositor *ec = shell->compositor;
	struct wl_event_loop *loop;

	if (wl_global_create(ec->wl_display,
			     &croscomp_splash_interface, 1,
			     shell, bind_splash) == NULL)
		return -1;

	loop = wl_display_get_event_loop(ec->wl_display);
	wl_event_loop_add_idle(loop, launch_splash_process, shell);

	return 0;
}
