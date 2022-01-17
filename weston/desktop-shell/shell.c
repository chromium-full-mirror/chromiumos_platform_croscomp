/*
 * Copyright © 2010-2012 Intel Corporation
 * Copyright © 2011-2012 Collabora, Ltd.
 * Copyright © 2013 Raspberry Pi Foundation
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
#include <errno.h>
#include <sys/socket.h>
#include <sys/wait.h>

#include "shell.h"
#include <libweston/config-parser.h>
#include <libweston/libweston.h>
#include <libweston/libweston.h>
#include <libweston/weston-log.h>
#include "shared/zalloc.h"
#include "shared/helpers.h"
#include "shared/timespec-util.h"
#include "shared/os-compatibility.h"

struct weston_view *
get_default_view(struct weston_surface *surface)
{
	struct shell_surface *shsurf;
	struct weston_view *view;

	if (!surface || wl_list_empty(&surface->views))
		return NULL;

	shsurf = get_shell_surface(surface);
	if (shsurf)
		return shsurf->view;

	wl_list_for_each(view, &surface->views, surface_link)
		if (weston_view_is_mapped(view))
			return view;

	return container_of(surface->views.next, struct weston_view, surface_link);
}

static void
center_on_output(struct weston_view *view,
		 struct weston_output *output);

struct weston_output *
get_default_output(struct weston_compositor *compositor)
{
	if (wl_list_empty(&compositor->output_list))
		return NULL;

	return container_of(compositor->output_list.next,
			    struct weston_output, link);
}

/*
 * Returns the bounding box of a surface and all its sub-surfaces,
 * in surface-local coordinates. */
static void
surface_subsurfaces_boundingbox(struct weston_surface *surface, int32_t *x,
				int32_t *y, int32_t *w, int32_t *h) {
	pixman_region32_t region;
	pixman_box32_t *box;
	struct weston_subsurface *subsurface;

	pixman_region32_init_rect(&region, 0, 0,
	                          surface->width,
	                          surface->height);

	wl_list_for_each(subsurface, &surface->subsurface_list, parent_link) {
		pixman_region32_union_rect(&region, &region,
		                           subsurface->position.x,
		                           subsurface->position.y,
		                           subsurface->surface->width,
		                           subsurface->surface->height);
	}

	box = pixman_region32_extents(&region);
	if (x)
		*x = box->x1;
	if (y)
		*y = box->y1;
	if (w)
		*w = box->x2 - box->x1;
	if (h)
		*h = box->y2 - box->y1;

	pixman_region32_fini(&region);
}

/* The surface will be inserted into the list immediately after the link
 * returned by this function (i.e. will be stacked immediately above the
 * returned link). */
static struct weston_layer_entry *
shell_surface_calculate_layer_link (struct shell_surface *shsurf)
{
	struct workspace *ws;

	if (shsurf->state.fullscreen && !shsurf->state.lowered) {
		return &shsurf->shell->fullscreen_layer.view_list;
	}

	/* Move the surface to a normal workspace layer so that surfaces
	 * which were previously fullscreen or transient are no longer
	 * rendered on top. */
	ws = get_current_workspace(shsurf->shell);
	return &ws->layer.view_list;
}

void
shell_surface_update_child_surface_layers (struct shell_surface *shsurf)
{
	// wtf is this?
	// weston_desktop_surface_propagate_layer(shsurf->desktop_surface);
}

/* Update the surface’s layer. Mark both the old and new views as having dirty
 * geometry to ensure the changes are redrawn.
 *
 * If any child surfaces exist and are mapped, ensure they’re in the same layer
 * as this surface. */
void
shell_surface_update_layer(struct shell_surface *shsurf)
{
	struct weston_surface *surface = shsurf->surface;
	struct weston_layer_entry *new_layer_link;

	new_layer_link = shell_surface_calculate_layer_link(shsurf);

	if (new_layer_link == NULL)
		return;
	if (new_layer_link == &shsurf->view->layer_link)
		return;

	weston_view_geometry_dirty(shsurf->view);
	weston_layer_entry_remove(&shsurf->view->layer_link);
	weston_layer_entry_insert(new_layer_link, &shsurf->view->layer_link);
	weston_view_geometry_dirty(shsurf->view);
	weston_surface_damage(surface);

	shell_surface_update_child_surface_layers(shsurf);
}

static void
notify_output_destroy(struct wl_listener *listener, void *data)
{
	struct shell_surface *shsurf =
		container_of(listener,
			     struct shell_surface, output_destroy_listener);

	shsurf->output = NULL;
	shsurf->output_destroy_listener.notify = NULL;
}

void
shell_surface_set_output(struct shell_surface *shsurf,
                         struct weston_output *output)
{
	struct weston_surface *es = shsurf->surface;

	/* get the default output, if the client set it as NULL
	   check whether the output is available */
	if (output)
		shsurf->output = output;
	else if (es->output)
		shsurf->output = es->output;
	else
		shsurf->output = get_default_output(es->compositor);

	if (shsurf->output_destroy_listener.notify) {
		wl_list_remove(&shsurf->output_destroy_listener.link);
		shsurf->output_destroy_listener.notify = NULL;
	}

	if (!shsurf->output)
		return;

	shsurf->output_destroy_listener.notify = notify_output_destroy;
	wl_signal_add(&shsurf->output->destroy_signal,
		      &shsurf->output_destroy_listener);
}

static int
black_surface_get_label(struct weston_surface *surface, char *buf, size_t len)
{
	struct weston_view *fs_view = surface->committed_private;
	struct weston_surface *fs_surface = fs_view->surface;
	int n;
	int rem;
	int ret;

	n = snprintf(buf, len, "black background surface for ");
	if (n < 0)
		return n;

	rem = (int)len - n;
	if (rem < 0)
		rem = 0;

	if (fs_surface->get_label)
		ret = fs_surface->get_label(fs_surface, buf + n, rem);
	else
		ret = snprintf(buf + n, rem, "<unknown>");

	if (ret < 0)
		return n;

	return n + ret;
}

static void
black_surface_committed(struct weston_surface *es, int32_t sx, int32_t sy);

static struct weston_view *
create_black_surface(struct weston_compositor *ec,
		     struct weston_view *fs_view,
		     float x, float y, int w, int h)
{
	struct weston_surface *surface = NULL;
	struct weston_view *view;

	surface = weston_surface_create(ec);
	if (surface == NULL) {
		weston_log("no memory\n");
		return NULL;
	}
	view = weston_view_create(surface);
	if (surface == NULL) {
		weston_log("no memory\n");
		weston_surface_destroy(surface);
		return NULL;
	}

	surface->committed = black_surface_committed;
	surface->committed_private = fs_view;
	weston_surface_set_label_func(surface, black_surface_get_label);
	weston_surface_set_color(surface, 0.0, 0.0, 0.0, 1);
	pixman_region32_fini(&surface->opaque);
	pixman_region32_init_rect(&surface->opaque, 0, 0, w, h);
	pixman_region32_fini(&surface->input);
	pixman_region32_init_rect(&surface->input, 0, 0, w, h);

	weston_surface_set_size(surface, w, h);
	weston_view_set_position(view, x, y);

	return view;
}

static void
shell_ensure_fullscreen_black_view(struct shell_surface *shsurf)
{
	struct weston_surface *surface = shsurf->surface;
	struct weston_output *output = shsurf->fullscreen_output;

	assert(shsurf->state.fullscreen);

	if (!shsurf->fullscreen.black_view)
		shsurf->fullscreen.black_view =
			create_black_surface(surface->compositor,
			                     shsurf->view,
			                     output->x, output->y,
			                     output->width,
			                     output->height);

	weston_view_geometry_dirty(shsurf->fullscreen.black_view);
	weston_layer_entry_remove(&shsurf->fullscreen.black_view->layer_link);
	weston_layer_entry_insert(&shsurf->view->layer_link,
				  &shsurf->fullscreen.black_view->layer_link);
	weston_view_geometry_dirty(shsurf->fullscreen.black_view);
	weston_surface_damage(surface);

	shsurf->fullscreen.black_view->is_mapped = true;
	shsurf->state.lowered = false;
}

/* Create black surface and append it to the associated fullscreen surface.
 * Handle size dismatch and positioning according to the method. */
void
shell_configure_fullscreen(struct shell_surface *shsurf)
{
	struct weston_surface *surface = shsurf->surface;
	int32_t surf_x, surf_y, surf_width, surf_height;

	/* Reverse the effect of lower_fullscreen_layer() */
	weston_layer_entry_remove(&shsurf->view->layer_link);
	weston_layer_entry_insert(&shsurf->shell->fullscreen_layer.view_list,
				  &shsurf->view->layer_link);

	if (!shsurf->fullscreen_output) {
		/* If there is no output, there's not much we can do.
		 * Position the window somewhere, whatever. */
		weston_view_set_position(shsurf->view, 0, 0);
		return;
	}

	shell_ensure_fullscreen_black_view(shsurf);

	surface_subsurfaces_boundingbox(surface, &surf_x, &surf_y,
	                                &surf_width, &surf_height);

	if (surface->buffer_ref.buffer)
		center_on_output(shsurf->view, shsurf->fullscreen_output);
}

/* Move all fullscreen layers down to the current workspace and hide their
 * black views. The surfaces' state is set to both fullscreen and lowered,
 * and this is reversed when such a surface is re-configured, see
 * shell_configure_fullscreen() and shell_ensure_fullscreen_black_view().
 *
 * lowering_output = NULL - Lower on all outputs, else only lower on the
 *                   specified output.
 *
 * This should be used when implementing shell-wide overlays, such as
 * the alt-tab switcher, which need to de-promote fullscreen layers. */
void
lower_fullscreen_layer(struct desktop_shell *shell,
		       struct weston_output *lowering_output)
{
	struct workspace *ws;
	struct weston_view *view, *prev;

	ws = get_current_workspace(shell);
	wl_list_for_each_reverse_safe(view, prev,
				      &shell->fullscreen_layer.view_list.link,
				      layer_link.link) {
		struct shell_surface *shsurf = get_shell_surface(view->surface);

		if (!shsurf)
			continue;

		/* Only lower surfaces which have lowering_output as their fullscreen
		 * output, unless a NULL output asks for lowering on all outputs.
		 */
		if (lowering_output && (shsurf->fullscreen_output != lowering_output))
			continue;

		/* We can have a non-fullscreen popup for a fullscreen surface
		 * in the fullscreen layer. */
		if (shsurf->state.fullscreen) {
			/* Hide the black view */
			weston_layer_entry_remove(&shsurf->fullscreen.black_view->layer_link);
			wl_list_init(&shsurf->fullscreen.black_view->layer_link.link);
			weston_view_damage_below(shsurf->fullscreen.black_view);

		}

		/* Lower the view to the workspace layer */
		weston_layer_entry_remove(&view->layer_link);
		weston_layer_entry_insert(&ws->layer.view_list, &view->layer_link);
		weston_view_damage_below(view);
		weston_surface_damage(view->surface);

		shsurf->state.lowered = true;
	}
}

/* no-op func for checking black surface */
static void
black_surface_committed(struct weston_surface *es, int32_t sx, int32_t sy)
{
}

static void
center_on_output(struct weston_view *view, struct weston_output *output)
{
	int32_t surf_x, surf_y, width, height;
	float x, y;

	if (!output) {
		weston_view_set_position(view, 0, 0);
		return;
	}

	surface_subsurfaces_boundingbox(view->surface, &surf_x, &surf_y, &width, &height);

	x = output->x + (output->width - width) / 2 - surf_x / 2;
	y = output->y + (output->height - height) / 2 - surf_y / 2;

	weston_view_set_position(view, x, y);
}
