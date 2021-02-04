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
#include "shared/zalloc.h"
#include "shared/helpers.h"
#include "shared/timespec-util.h"
#include "libweston-desktop.h"
#include "internal.h"

#include "croscomp-server-protocol.h"

struct splash {
	struct desktop_shell *shell;
	struct wl_listener surface_create_listener;
};

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
splash_set(struct wl_client *client,
	   struct wl_resource *resource,
	   struct wl_resource *output_resource,
	   struct wl_resource *surface_resource)
{
	struct desktop_shell *shell = wl_resource_get_user_data(resource);
	struct weston_surface *surface =
		wl_resource_get_user_data(surface_resource);
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

	/* Hold on to last frame after client exits. */
	surface->ref_count++;
}

static const struct croscomp_splash_interface croscomp_splash_implementation = {
	.set = splash_set
};

static void
fade_out_boot_splash_done(struct weston_view_animation *animation, void *data)
{
	struct weston_view *view = data;

	weston_surface_destroy(view->surface);
}

static void
surface_create_notify(struct wl_listener *listener, void *data)
{
	struct splash *s =
		container_of(listener, struct splash,
			     surface_create_listener);
	struct weston_compositor *ec = s->shell->compositor;
	struct weston_layer *layer = &ec->fade_layer;
	struct weston_view *view;

	wl_list_for_each(view, &layer->view_list.link, layer_link.link)
		weston_fade_run(view, view->alpha, 0.0, 100,
				fade_out_boot_splash_done, view);

	wl_list_remove(&listener->link);
	free(s);
}

static void
unbind_splash(struct wl_resource *resource)
{
	struct splash *s = wl_resource_get_user_data(resource);
	struct weston_compositor *ec = s->shell->compositor;

	shell_finish_init(s->shell);

	/* Once the splash screen exits, the animation is
	 * complete. Now set up a surface create listener to fade out
	 * the splash screen surfaces once the first client creates a
	 * surface.
	 */
	s->surface_create_listener.notify = surface_create_notify;
	wl_signal_add(&ec->create_surface_signal,
		      &s->surface_create_listener);
}

static void
bind_splash(struct wl_client *client,
		   void *data, uint32_t version, uint32_t id)
{
	struct splash *s = data;
	struct wl_resource *resource;

	resource = wl_resource_create(client, &croscomp_splash_interface,
				      1, id);

	wl_resource_set_implementation(resource,
				       &croscomp_splash_implementation,
				       s, unbind_splash);
}

static void
launch_splash_process(void *data)
{
	struct splash *s = data;

	weston_client_start(s->shell->compositor,
			    "/usr/bin/weston-simple-splash");
}

int
init_splash(struct desktop_shell *shell)
{
	struct splash *s;
	struct weston_compositor *ec = shell->compositor;
	struct wl_event_loop *loop;

	s = zalloc(sizeof(*s));
	if (s == NULL) {
		weston_log("allocation failed for splash\n");
		return -1;
	}

	s->shell = shell;
	if (wl_global_create(ec->wl_display,
			     &croscomp_splash_interface, 1,
			     s, bind_splash) == NULL)
		return -1;

	/* xxx We probably want a signal for this for when the display
	 * is up and done with initial modeset.
	 */
	loop = wl_display_get_event_loop(ec->wl_display);
	wl_event_loop_add_idle(loop, launch_splash_process, s);

	return 0;
}
