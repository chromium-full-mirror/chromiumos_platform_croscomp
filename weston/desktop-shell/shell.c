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

#define DEFAULT_NUM_WORKSPACES 1

static struct desktop_shell *
shell_surface_get_shell(struct shell_surface *shsurf);

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

void
get_output_work_area(struct desktop_shell *shell,
		     struct weston_output *output,
		     pixman_rectangle32_t *area)
{
	if (!output) {
		area->x = 0;
		area->y = 0;
		area->width = 0;
		area->height = 0;

		return;
	}

	area->x = output->x;
	area->y = output->y;
	area->width = output->width;
	area->height = output->height;
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

static void
surface_keyboard_focus_lost(struct weston_surface *surface)
{
	struct weston_compositor *compositor = surface->compositor;
	struct weston_seat *seat;
	struct weston_surface *focus;

	wl_list_for_each(seat, &compositor->seat_list, link) {
		struct weston_keyboard *keyboard =
			weston_seat_get_keyboard(seat);

		if (!keyboard)
			continue;

		focus = weston_surface_get_main_surface(keyboard->focus);
		if (focus == surface)
			weston_keyboard_set_focus(keyboard, NULL);
	}
}

void
shell_surface_set_minimized(struct shell_surface *shsurf)
{
	struct weston_surface *surface = shsurf->surface;
	struct workspace *current_ws;
	struct weston_view *view;

	view = get_default_view(surface);
	if (!view)
		return;

	assert(weston_surface_get_main_surface(view->surface) == view->surface);

	shsurf = get_shell_surface(surface);
	current_ws = get_current_workspace(shsurf->shell);

	weston_layer_entry_remove(&view->layer_link);
	weston_layer_entry_insert(&shsurf->shell->minimized_layer.view_list, &view->layer_link);

	drop_focus_state(shsurf->shell, current_ws, view->surface);
	surface_keyboard_focus_lost(surface);

	shell_surface_update_child_surface_layers(shsurf);
	weston_view_damage_below(view);
}


static struct desktop_shell *
shell_surface_get_shell(struct shell_surface *shsurf)
{
	return shsurf->shell;
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

static struct weston_output *
get_focused_output(struct weston_compositor *compositor)
{
	struct weston_seat *seat;
	struct weston_output *output = NULL;

	wl_list_for_each(seat, &compositor->seat_list, link) {
		struct weston_touch *touch = weston_seat_get_touch(seat);
		struct weston_pointer *pointer = weston_seat_get_pointer(seat);
		struct weston_keyboard *keyboard =
			weston_seat_get_keyboard(seat);

		/* Priority has touch focus, then pointer and
		 * then keyboard focus. We should probably have
		 * three for loops and check first for touch,
		 * then for pointer, etc. but unless somebody has some
		 * objections, I think this is sufficient. */
		if (touch && touch->focus)
			output = touch->focus->output;
		else if (pointer && pointer->focus)
			output = pointer->focus->output;
		else if (keyboard && keyboard->focus)
			output = keyboard->focus->output;

		if (output)
			break;
	}

	return output;
}

static void
get_maximized_size(struct shell_surface *shsurf, int32_t *width, int32_t *height)
{
	struct desktop_shell *shell;
	pixman_rectangle32_t area;

	shell = shell_surface_get_shell(shsurf);
	get_output_work_area(shell, shsurf->output, &area);

	*width = area.width;
	*height = area.height;
}

struct weston_size
shell_surface_set_fullscreen(struct shell_surface *shsurf, bool fullscreen,
			     struct weston_output *output)
{
	struct weston_surface *surface = shsurf->surface;
	struct weston_size size = { 0, 0 };

	if (fullscreen) {
		/* handle clients launching in fullscreen */
		if (output == NULL && !weston_surface_is_mapped(surface)) {
			/* Set the output to the one that has focus currently. */
			output = get_focused_output(surface->compositor);
		}

		shell_surface_set_output(shsurf, output);
		shsurf->fullscreen_output = shsurf->output;

		if (shsurf->output) {
			size.width = shsurf->output->width;
			size.height = shsurf->output->height;
		}
	} else if (shsurf->state.maximized) {
		get_maximized_size(shsurf, &size.width, &size.height);
	}

	return size;
}

struct weston_size
shell_surface_set_maximized(struct shell_surface *shsurf, bool maximized)
{
	struct weston_surface *surface = shsurf->surface;
	struct weston_size size = { 0, 0 };

	if (maximized) {
		struct weston_output *output;

		if (!weston_surface_is_mapped(surface))
			output = get_focused_output(surface->compositor);
		else
			output = surface->output;

		shell_surface_set_output(shsurf, output);

		get_maximized_size(shsurf, &size.width, &size.height);
	}

	return size;
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

static void
shell_reposition_view_on_output_change(struct weston_view *view)
{
	struct weston_output *output, *first_output;
	struct weston_compositor *ec = view->surface->compositor;
	struct shell_surface *shsurf;
	float x, y;
	int visible;

	if (wl_list_empty(&ec->output_list))
		return;

	x = view->geometry.x;
	y = view->geometry.y;

	/* At this point the destroyed output is not in the list anymore.
	 * If the view is still visible somewhere, we leave where it is,
	 * otherwise, move it to the first output. */
	visible = 0;
	wl_list_for_each(output, &ec->output_list, link) {
		if (pixman_region32_contains_point(&output->region,
						   x, y, NULL)) {
			visible = 1;
			break;
		}
	}

	if (!visible) {
		first_output = container_of(ec->output_list.next,
					    struct weston_output, link);

		x = first_output->x + first_output->width / 4;
		y = first_output->y + first_output->height / 4;

		weston_view_set_position(view, x, y);
	} else {
		weston_view_geometry_dirty(view);
	}


	shsurf = get_shell_surface(view->surface);
	if (!shsurf)
		return;

	shsurf->saved_position_valid = false;
	shell_surface_set_maximized(shsurf, false);
	shell_surface_set_fullscreen(shsurf, false, NULL);
}

void
shell_for_each_layer(struct desktop_shell *shell,
		     shell_for_each_layer_func_t func, void *data)
{
	struct workspace **ws;

	func(shell, &shell->fullscreen_layer, data);
	func(shell, &shell->background_layer, data);

	wl_array_for_each(ws, &shell->workspaces.array)
		func(shell, &(*ws)->layer, data);
}

static void
shell_output_changed_move_layer(struct desktop_shell *shell,
				struct weston_layer *layer,
				void *data)
{
	struct weston_view *view;

	wl_list_for_each(view, &layer->view_list.link, layer_link.link)
		shell_reposition_view_on_output_change(view);

}

static void
handle_output_destroy(struct wl_listener *listener, void *data)
{
	struct shell_output *output_listener =
		container_of(listener, struct shell_output, destroy_listener);
	struct desktop_shell *shell = output_listener->shell;

	shell_for_each_layer(shell, shell_output_changed_move_layer, NULL);

	if (output_listener->panel_surface)
		wl_list_remove(&output_listener->panel_surface_listener.link);
	if (output_listener->background_surface)
		wl_list_remove(&output_listener->background_surface_listener.link);
	wl_list_remove(&output_listener->destroy_listener.link);
	wl_list_remove(&output_listener->link);
	free(output_listener);
}

static void
create_shell_output(struct desktop_shell *shell,
					struct weston_output *output)
{
	struct shell_output *shell_output;

	shell_output = zalloc(sizeof *shell_output);
	if (shell_output == NULL)
		return;

	shell_output->output = output;
	shell_output->shell = shell;
	shell_output->destroy_listener.notify = handle_output_destroy;
	wl_signal_add(&output->destroy_signal,
		      &shell_output->destroy_listener);
	wl_list_insert(shell->output_list.prev, &shell_output->link);

	if (wl_list_length(&shell->output_list) == 1)
		shell_for_each_layer(shell,
				     shell_output_changed_move_layer, NULL);
}

static void
handle_output_create(struct wl_listener *listener, void *data)
{
	struct desktop_shell *shell =
		container_of(listener, struct desktop_shell, output_create_listener);
	struct weston_output *output = (struct weston_output *)data;

	create_shell_output(shell, output);
}

static void
handle_output_move_layer(struct desktop_shell *shell,
			 struct weston_layer *layer, void *data)
{
	struct weston_output *output = data;
	struct weston_view *view;
	float x, y;

	wl_list_for_each(view, &layer->view_list.link, layer_link.link) {
		if (view->output != output)
			continue;

		x = view->geometry.x + output->move_x;
		y = view->geometry.y + output->move_y;
		weston_view_set_position(view, x, y);
	}
}

static void
handle_output_move(struct wl_listener *listener, void *data)
{
	struct desktop_shell *shell;

	shell = container_of(listener, struct desktop_shell,
			     output_move_listener);

	shell_for_each_layer(shell, handle_output_move_layer, data);
}

static void
setup_output_destroy_handler(struct weston_compositor *ec,
							struct desktop_shell *shell)
{
	struct weston_output *output;

	wl_list_init(&shell->output_list);
	wl_list_for_each(output, &ec->output_list, link)
		create_shell_output(shell, output);

	shell->output_create_listener.notify = handle_output_create;
	wl_signal_add(&ec->output_created_signal,
				&shell->output_create_listener);

	shell->output_move_listener.notify = handle_output_move;
	wl_signal_add(&ec->output_moved_signal, &shell->output_move_listener);
}

static void
shell_destroy(struct wl_listener *listener, void *data)
{
	struct desktop_shell *shell =
		container_of(listener, struct desktop_shell, destroy_listener);
	struct shell_output *shell_output, *tmp;

	wl_list_remove(&shell->destroy_listener.link);

	wl_list_for_each_safe(shell_output, tmp, &shell->output_list, link) {
		wl_list_remove(&shell_output->destroy_listener.link);
		wl_list_remove(&shell_output->link);
		free(shell_output);
	}

	wl_list_remove(&shell->output_create_listener.link);
	wl_list_remove(&shell->output_move_listener.link);

	destroy_workspaces(shell);

	free(shell->client);
	free(shell);
}

int
desktop_shell_init(struct desktop_shell *shell,
		   struct weston_compositor *ec)
{
	if (!weston_compositor_add_destroy_listener_once(ec,
							 &shell->destroy_listener,
							 shell_destroy))
		return -1;

	if (init_workspaces(shell) == -1) {
		weston_log("init_workspaces failed\n");
		return -1;
	}

	setup_output_destroy_handler(ec, shell);

	clock_gettime(CLOCK_MONOTONIC, &shell->startup_time);

	return 0;
}
