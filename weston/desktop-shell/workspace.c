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

#include "shell.h"
#include <libweston/config-parser.h>
#include <libweston/libweston.h>
#include <libweston/libweston.h>
#include <libweston/weston-log.h>
#include "shared/zalloc.h"
#include "shared/helpers.h"
#include "shared/timespec-util.h"

#define DEFAULT_WORKSPACE_CHANGE_ANIMATION_LENGTH 200

struct focus_state {
	struct desktop_shell *shell;
	struct weston_seat *seat;
	struct workspace *ws;
	struct weston_surface *keyboard_focus;
	struct wl_list link;
	struct wl_listener seat_destroy_listener;
	struct wl_listener surface_destroy_listener;
};

static void
focus_state_destroy(struct focus_state *state)
{
	wl_list_remove(&state->seat_destroy_listener.link);
	wl_list_remove(&state->surface_destroy_listener.link);
	free(state);
}

static void
focus_state_seat_destroy(struct wl_listener *listener, void *data)
{
	struct focus_state *state = container_of(listener,
						 struct focus_state,
						 seat_destroy_listener);

	wl_list_remove(&state->link);
	focus_state_destroy(state);
}

static void
focus_state_surface_destroy(struct wl_listener *listener, void *data)
{
	struct focus_state *state = container_of(listener,
						 struct focus_state,
						 surface_destroy_listener);
	struct weston_surface *main_surface;
	struct weston_view *next;
	struct weston_view *view;

	main_surface = weston_surface_get_main_surface(state->keyboard_focus);

	next = NULL;
	wl_list_for_each(view,
			 &state->ws->layer.view_list.link, layer_link.link) {
		if (view->surface == main_surface)
			continue;
		if (!get_shell_surface(view->surface))
			continue;

		next = view;
		break;
	}

	/* if the focus was a sub-surface, activate its main surface */
	if (main_surface != state->keyboard_focus)
		next = get_default_view(main_surface);

	if (next) {
		if (state->keyboard_focus) {
			wl_list_remove(&state->surface_destroy_listener.link);
			wl_list_init(&state->surface_destroy_listener.link);
		}
		state->keyboard_focus = NULL;
		weston_view_activate(next, state->seat,
				     WESTON_ACTIVATE_FLAG_CONFIGURE);
	} else {
		wl_list_remove(&state->link);
		focus_state_destroy(state);
	}
}

static struct focus_state *
focus_state_create(struct desktop_shell *shell, struct weston_seat *seat,
		   struct workspace *ws)
{
	struct focus_state *state;

	state = malloc(sizeof *state);
	if (state == NULL)
		return NULL;

	state->shell = shell;
	state->keyboard_focus = NULL;
	state->ws = ws;
	state->seat = seat;
	wl_list_insert(&ws->focus_list, &state->link);

	state->seat_destroy_listener.notify = focus_state_seat_destroy;
	state->surface_destroy_listener.notify = focus_state_surface_destroy;
	wl_signal_add(&seat->destroy_signal,
		      &state->seat_destroy_listener);
	wl_list_init(&state->surface_destroy_listener.link);

	return state;
}

struct focus_state *
ensure_focus_state(struct desktop_shell *shell, struct weston_seat *seat)
{
	struct workspace *ws = get_current_workspace(shell);
	struct focus_state *state;

	wl_list_for_each(state, &ws->focus_list, link)
		if (state->seat == seat)
			break;

	if (&state->link == &ws->focus_list)
		state = focus_state_create(shell, seat, ws);

	return state;
}

struct weston_surface *
focus_state_set_focus(struct focus_state *state,
		      struct weston_surface *surface)
{
	struct weston_surface *prev = state->keyboard_focus;
	
	if (prev) {
		wl_list_remove(&state->surface_destroy_listener.link);
		wl_list_init(&state->surface_destroy_listener.link);
	}

	state->keyboard_focus = surface;
	if (surface)
		wl_signal_add(&surface->destroy_signal,
			      &state->surface_destroy_listener);

	return prev;
}

static void
restore_focus_state(struct desktop_shell *shell, struct workspace *ws)
{
	struct focus_state *state, *next;
	struct weston_surface *surface;
	struct wl_list pending_seat_list;
	struct weston_seat *seat, *next_seat;

	/* Temporarily steal the list of seats so that we can keep
	 * track of the seats we've already processed */
	wl_list_init(&pending_seat_list);
	wl_list_insert_list(&pending_seat_list, &shell->compositor->seat_list);
	wl_list_init(&shell->compositor->seat_list);

	wl_list_for_each_safe(state, next, &ws->focus_list, link) {
		struct weston_keyboard *keyboard =
			weston_seat_get_keyboard(state->seat);

		wl_list_remove(&state->seat->link);
		wl_list_insert(&shell->compositor->seat_list,
			       &state->seat->link);

		if (!keyboard)
			continue;

		surface = state->keyboard_focus;

		weston_keyboard_set_focus(keyboard, surface);
	}

	/* For any remaining seats that we don't have a focus state
	 * for we'll reset the keyboard focus to NULL */
	wl_list_for_each_safe(seat, next_seat, &pending_seat_list, link) {
		struct weston_keyboard *keyboard =
			weston_seat_get_keyboard(seat);

		wl_list_insert(&shell->compositor->seat_list, &seat->link);

		if (!keyboard)
			continue;

		weston_keyboard_set_focus(keyboard, NULL);
	}
}

static void
replace_focus_state(struct desktop_shell *shell, struct workspace *ws,
		    struct weston_seat *seat)
{
	struct weston_keyboard *keyboard = weston_seat_get_keyboard(seat);
	struct focus_state *state;

	wl_list_for_each(state, &ws->focus_list, link) {
		if (state->seat == seat) {
			focus_state_set_focus(state, keyboard->focus);
			return;
		}
	}
}

void
drop_focus_state(struct desktop_shell *shell, struct workspace *ws,
		 struct weston_surface *surface)
{
	struct focus_state *state;

	wl_list_for_each(state, &ws->focus_list, link)
		if (state->keyboard_focus == surface)
			focus_state_set_focus(state, NULL);
}

static void
workspace_destroy(struct workspace *ws)
{
	struct focus_state *state, *next;

	wl_list_for_each_safe(state, next, &ws->focus_list, link)
		focus_state_destroy(state);
	free(ws);
}

static void
seat_destroyed(struct wl_listener *listener, void *data)
{
	struct weston_seat *seat = data;
	struct focus_state *state, *next;
	struct workspace *ws = container_of(listener,
					    struct workspace,
					    seat_destroyed_listener);

	wl_list_for_each_safe(state, next, &ws->focus_list, link)
		if (state->seat == seat)
			wl_list_remove(&state->link);
}

static struct workspace *
workspace_create(struct desktop_shell *shell)
{
	struct workspace *ws = malloc(sizeof *ws);
	if (ws == NULL)
		return NULL;

	weston_layer_init(&ws->layer, shell->compositor);

	wl_list_init(&ws->focus_list);
	wl_list_init(&ws->seat_destroyed_listener.link);
	ws->seat_destroyed_listener.notify = seat_destroyed;
	ws->focus_animation = NULL;

	return ws;
}

static int
workspace_is_empty(struct workspace *ws)
{
	return wl_list_empty(&ws->layer.view_list.link);
}

static struct workspace *
get_workspace(struct desktop_shell *shell, unsigned int index)
{
	struct workspace **pws = shell->workspaces.array.data;
	assert(index < shell->workspaces.num);
	pws += index;
	return *pws;
}

struct workspace *
get_current_workspace(struct desktop_shell *shell)
{
	return get_workspace(shell, shell->workspaces.current);
}

static void
activate_workspace(struct desktop_shell *shell, unsigned int index)
{
	struct workspace *ws;

	ws = get_workspace(shell, index);
	weston_layer_set_position(&ws->layer, WESTON_LAYER_POSITION_NORMAL);

	shell->workspaces.current = index;
}

static unsigned int
get_output_height(struct weston_output *output)
{
	return abs(output->region.extents.y1 - output->region.extents.y2);
}


static struct weston_transform *
view_get_transform(struct weston_view *view)
{
	struct shell_surface *shsurf = NULL;

	shsurf = get_shell_surface(view->surface);
	if (shsurf)
		return &shsurf->workspace_transform;

	return NULL;
}

static void
view_translate(struct workspace *ws, struct weston_view *view, double d)
{
	struct weston_transform *transform = view_get_transform(view);

	if (!transform)
		return;

	if (wl_list_empty(&transform->link))
		wl_list_insert(view->geometry.transformation_list.prev,
			       &transform->link);

	weston_matrix_init(&transform->matrix);
	weston_matrix_translate(&transform->matrix,
				0.0, d, 0.0);
	weston_view_geometry_dirty(view);
}

static void
workspace_translate_out(struct workspace *ws, double fraction)
{
	struct weston_view *view;
	unsigned int height;
	double d;

	wl_list_for_each(view, &ws->layer.view_list.link, layer_link.link) {
		height = get_output_height(view->surface->output);
		d = height * fraction;

		view_translate(ws, view, d);
	}
}

static void
workspace_translate_in(struct workspace *ws, double fraction)
{
	struct weston_view *view;
	unsigned int height;
	double d;

	wl_list_for_each(view, &ws->layer.view_list.link, layer_link.link) {
		height = get_output_height(view->surface->output);

		if (fraction > 0)
			d = -(height - height * fraction);
		else
			d = height + height * fraction;

		view_translate(ws, view, d);
	}
}

static void
reverse_workspace_change_animation(struct desktop_shell *shell,
				   unsigned int index,
				   struct workspace *from,
				   struct workspace *to)
{
	shell->workspaces.current = index;

	shell->workspaces.anim_to = to;
	shell->workspaces.anim_from = from;
	shell->workspaces.anim_dir = -1 * shell->workspaces.anim_dir;
	shell->workspaces.anim_timestamp = (struct timespec) { 0 };

	weston_layer_set_position(&to->layer, WESTON_LAYER_POSITION_NORMAL);
	weston_layer_set_position(&from->layer, WESTON_LAYER_POSITION_NORMAL - 1);

	weston_compositor_schedule_repaint(shell->compositor);
}

static void
workspace_deactivate_transforms(struct workspace *ws)
{
	struct weston_view *view;
	struct weston_transform *transform;

	wl_list_for_each(view, &ws->layer.view_list.link, layer_link.link) {
		transform = view_get_transform(view);
		if (!transform)
			continue;

		if (!wl_list_empty(&transform->link)) {
			wl_list_remove(&transform->link);
			wl_list_init(&transform->link);
		}
		weston_view_geometry_dirty(view);
	}
}

static void
finish_workspace_change_animation(struct desktop_shell *shell,
				  struct workspace *from,
				  struct workspace *to)
{
	struct weston_view *view;

	weston_compositor_schedule_repaint(shell->compositor);

	/* Views that extend past the bottom of the output are still
	 * visible after the workspace animation ends but before its layer
	 * is hidden. In that case, we need to damage below those views so
	 * that the screen is properly repainted. */
	wl_list_for_each(view, &from->layer.view_list.link, layer_link.link)
		weston_view_damage_below(view);

	wl_list_remove(&shell->workspaces.animation.link);
	workspace_deactivate_transforms(from);
	workspace_deactivate_transforms(to);
	shell->workspaces.anim_to = NULL;

	weston_layer_unset_position(&shell->workspaces.anim_from->layer);
}

static void
animate_workspace_change_frame(struct weston_animation *animation,
			       struct weston_output *output,
			       const struct timespec *time)
{
	struct desktop_shell *shell =
		container_of(animation, struct desktop_shell,
			     workspaces.animation);
	struct workspace *from = shell->workspaces.anim_from;
	struct workspace *to = shell->workspaces.anim_to;
	int64_t t;
	double x, y;

	if (workspace_is_empty(from) && workspace_is_empty(to)) {
		finish_workspace_change_animation(shell, from, to);
		return;
	}

	if (timespec_is_zero(&shell->workspaces.anim_timestamp)) {
		if (shell->workspaces.anim_current == 0.0)
			shell->workspaces.anim_timestamp = *time;
		else
			timespec_add_msec(&shell->workspaces.anim_timestamp,
				time,
				/* Inverse of movement function 'y' below. */
				-(asin(1.0 - shell->workspaces.anim_current) *
				  DEFAULT_WORKSPACE_CHANGE_ANIMATION_LENGTH *
				  M_2_PI));
	}

	t = timespec_sub_to_msec(time, &shell->workspaces.anim_timestamp);

	/*
	 * x = [0, π/2]
	 * y(x) = sin(x)
	 */
	x = t * (1.0/DEFAULT_WORKSPACE_CHANGE_ANIMATION_LENGTH) * M_PI_2;
	y = sin(x);

	if (t < DEFAULT_WORKSPACE_CHANGE_ANIMATION_LENGTH) {
		weston_compositor_schedule_repaint(shell->compositor);

		workspace_translate_out(from, shell->workspaces.anim_dir * y);
		workspace_translate_in(to, shell->workspaces.anim_dir * y);
		shell->workspaces.anim_current = y;

		weston_compositor_schedule_repaint(shell->compositor);
	}
	else
		finish_workspace_change_animation(shell, from, to);
}

static void
animate_workspace_change(struct desktop_shell *shell,
			 unsigned int index,
			 struct workspace *from,
			 struct workspace *to)
{
	struct weston_output *output;

	int dir;

	if (index > shell->workspaces.current)
		dir = -1;
	else
		dir = 1;

	shell->workspaces.current = index;

	shell->workspaces.anim_dir = dir;
	shell->workspaces.anim_from = from;
	shell->workspaces.anim_to = to;
	shell->workspaces.anim_current = 0.0;
	shell->workspaces.anim_timestamp = (struct timespec) { 0 };

	output = container_of(shell->compositor->output_list.next,
			      struct weston_output, link);
	wl_list_insert(&output->animation_list,
		       &shell->workspaces.animation.link);

	weston_layer_set_position(&to->layer, WESTON_LAYER_POSITION_NORMAL);
	weston_layer_set_position(&from->layer, WESTON_LAYER_POSITION_NORMAL - 1);

	workspace_translate_in(to, 0);

	restore_focus_state(shell, to);

	weston_compositor_schedule_repaint(shell->compositor);
}

static void
update_workspace(struct desktop_shell *shell, unsigned int index,
		 struct workspace *from, struct workspace *to)
{
	shell->workspaces.current = index;
	weston_layer_set_position(&to->layer, WESTON_LAYER_POSITION_NORMAL);
	weston_layer_unset_position(&from->layer);
}

static void
change_workspace(struct desktop_shell *shell, unsigned int index)
{
	struct workspace *from;
	struct workspace *to;

	if (index == shell->workspaces.current)
		return;

	/* Don't change workspace when there is any fullscreen surfaces. */
	if (!wl_list_empty(&shell->fullscreen_layer.view_list.link))
		return;

	from = get_current_workspace(shell);
	to = get_workspace(shell, index);

	if (shell->workspaces.anim_from == to &&
	    shell->workspaces.anim_to == from) {
		restore_focus_state(shell, to);
		reverse_workspace_change_animation(shell, index, from, to);
		return;
	}

	if (shell->workspaces.anim_to != NULL)
		finish_workspace_change_animation(shell,
						  shell->workspaces.anim_from,
						  shell->workspaces.anim_to);

	restore_focus_state(shell, to);

	if (workspace_is_empty(to) && workspace_is_empty(from))
		update_workspace(shell, index, from, to);
	else
		animate_workspace_change(shell, index, from, to);
}

static bool
workspace_has_only(struct workspace *ws, struct weston_surface *surface)
{
	struct wl_list *list = &ws->layer.view_list.link;
	struct wl_list *e;

	if (wl_list_empty(list))
		return false;

	e = list->next;

	if (e->next != list)
		return false;

	return container_of(e, struct weston_view, layer_link.link)->surface == surface;
}

static void
take_surface_to_workspace_by_seat(struct desktop_shell *shell,
				  struct weston_seat *seat,
				  unsigned int index)
{
	struct weston_keyboard *keyboard = weston_seat_get_keyboard(seat);
	struct weston_surface *surface;
	struct weston_view *view;
	struct shell_surface *shsurf;
	struct workspace *from;
	struct workspace *to;
	struct focus_state *state;

	surface = weston_surface_get_main_surface(keyboard->focus);
	view = get_default_view(surface);
	if (view == NULL ||
	    index == shell->workspaces.current)
		return;

	from = get_current_workspace(shell);
	to = get_workspace(shell, index);

	weston_layer_entry_remove(&view->layer_link);
	weston_layer_entry_insert(&to->layer.view_list, &view->layer_link);

	shsurf = get_shell_surface(surface);
	if (shsurf != NULL)
		shell_surface_update_child_surface_layers(shsurf);

	replace_focus_state(shell, to, seat);
	drop_focus_state(shell, from, surface);

	if (shell->workspaces.anim_from == to &&
	    shell->workspaces.anim_to == from) {
		reverse_workspace_change_animation(shell, index, from, to);

		return;
	}

	if (shell->workspaces.anim_to != NULL)
		finish_workspace_change_animation(shell,
						  shell->workspaces.anim_from,
						  shell->workspaces.anim_to);

	if (workspace_is_empty(from) &&
	    workspace_has_only(to, surface))
		update_workspace(shell, index, from, to);
	else {
		if (shsurf != NULL &&
		    wl_list_empty(&shsurf->workspace_transform.link))
			wl_list_insert(&shell->workspaces.anim_sticky_list,
				       &shsurf->workspace_transform.link);

		animate_workspace_change(shell, index, from, to);
	}

	state = ensure_focus_state(shell, seat);
	if (state != NULL)
		focus_state_set_focus(state, surface);
}

static void
workspace_up_binding(struct weston_keyboard *keyboard,
		     const struct timespec *time, uint32_t key, void *data)
{
	struct desktop_shell *shell = data;
	unsigned int new_index = shell->workspaces.current;

	if (new_index != 0)
		new_index--;

	change_workspace(shell, new_index);
}

static void
workspace_down_binding(struct weston_keyboard *keyboard,
		       const struct timespec *time, uint32_t key, void *data)
{
	struct desktop_shell *shell = data;
	unsigned int new_index = shell->workspaces.current;

	if (new_index < shell->workspaces.num - 1)
		new_index++;

	change_workspace(shell, new_index);
}

static void
workspace_f_binding(struct weston_keyboard *keyboard,
		    const struct timespec *time, uint32_t key, void *data)
{
	struct desktop_shell *shell = data;
	unsigned int new_index;

	new_index = key - KEY_F1;
	if (new_index >= shell->workspaces.num)
		new_index = shell->workspaces.num - 1;

	change_workspace(shell, new_index);
}

static void
workspace_move_surface_up_binding(struct weston_keyboard *keyboard,
				  const struct timespec *time, uint32_t key,
				  void *data)
{
	struct desktop_shell *shell = data;
	unsigned int new_index = shell->workspaces.current;

	if (new_index != 0)
		new_index--;

	take_surface_to_workspace_by_seat(shell, keyboard->seat, new_index);
}

static void
workspace_move_surface_down_binding(struct weston_keyboard *keyboard,
				    const struct timespec *time, uint32_t key,
				    void *data)
{
	struct desktop_shell *shell = data;
	unsigned int new_index = shell->workspaces.current;

	if (new_index < shell->workspaces.num - 1)
		new_index++;

	take_surface_to_workspace_by_seat(shell, keyboard->seat, new_index);
}

void
destroy_workspaces(struct desktop_shell *shell)
{
	struct workspace **ws;

	wl_array_for_each(ws, &shell->workspaces.array)
		workspace_destroy(*ws);
	wl_array_release(&shell->workspaces.array);
}

int
init_workspaces(struct desktop_shell *shell)
{
	struct weston_compositor *ec = shell->compositor;
	uint32_t mod, i, num_workspace_bindings;
	struct workspace **pws;

	for (i = 0; i < shell->workspaces.num; i++) {
		pws = wl_array_add(&shell->workspaces.array, sizeof *pws);
		if (pws == NULL)
			return -1;

		*pws = workspace_create(shell);
		if (*pws == NULL)
			return -1;
	}
	activate_workspace(shell, 0);

	wl_list_init(&shell->workspaces.anim_sticky_list);
	wl_list_init(&shell->workspaces.animation.link);
	shell->workspaces.animation.frame = animate_workspace_change_frame;

	mod = shell->binding_modifier;
	if (!mod)
		/* No binding modifier just means no bindings, not
		 * failure. */
		return 0;

	weston_compositor_add_key_binding(ec, KEY_UP, mod,
					  workspace_up_binding, shell);
	weston_compositor_add_key_binding(ec, KEY_DOWN, mod,
					  workspace_down_binding, shell);
	weston_compositor_add_key_binding(ec, KEY_UP, mod | MODIFIER_SHIFT,
					  workspace_move_surface_up_binding,
					  shell);
	weston_compositor_add_key_binding(ec, KEY_DOWN, mod | MODIFIER_SHIFT,
					  workspace_move_surface_down_binding,
					  shell);

	/* Add bindings for mod+F[1-6] for workspace 1 to 6. */
	if (shell->workspaces.num > 1) {
		num_workspace_bindings = shell->workspaces.num;
		if (num_workspace_bindings > 6)
			num_workspace_bindings = 6;
		for (i = 0; i < num_workspace_bindings; i++)
			weston_compositor_add_key_binding(ec, KEY_F1 + i, mod,
							  workspace_f_binding,
							  shell);
	}

	return 0;
}
