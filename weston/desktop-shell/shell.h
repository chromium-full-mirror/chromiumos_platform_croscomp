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

#include <stdbool.h>
#include <stdint.h>
#include <time.h>

#include <libweston/libweston.h>
#include <pixman.h>
#include <stdbool.h>

enum animation_type {
	ANIMATION_NONE,
	ANIMATION_ZOOM,
	ANIMATION_FADE,
	ANIMATION_DIM_LAYER,
};

enum fade_type {
	FADE_IN,
	FADE_OUT
};

struct workspace {
	struct weston_layer layer;

	struct wl_list focus_list;
	struct wl_listener seat_destroyed_listener;

	struct weston_view_animation *focus_animation;
};

struct shell_output {
	struct desktop_shell  *shell;
	struct weston_output  *output;
	struct wl_listener    destroy_listener;
	struct wl_list        link;

	struct weston_surface *panel_surface;
	struct wl_listener panel_surface_listener;

	struct weston_surface *background_surface;
	struct wl_listener background_surface_listener;

	struct {
		struct weston_view *view;
		struct weston_view_animation *animation;
		enum fade_type type;
		struct wl_event_source *startup_timer;
	} fade;
};

enum shell_resize_edge {
	RESIZE_EDGE_NONE = 0,
	RESIZE_EDGE_TOP = 1,
	RESIZE_EDGE_BOTTOM = 2,
	RESIZE_EDGE_LEFT = 4,
	RESIZE_EDGE_TOP_LEFT = 5,
	RESIZE_EDGE_BOTTOM_LEFT = 6,
	RESIZE_EDGE_RIGHT = 8,
	RESIZE_EDGE_TOP_RIGHT = 9,
	RESIZE_EDGE_BOTTOM_RIGHT = 10,
};

struct shell_surface {
	struct wl_signal destroy_signal;

	struct wl_signal state_change_signal;

	struct weston_view *view;
	struct weston_surface *surface;
	int32_t last_width, last_height;

	struct desktop_shell *shell;

	struct wl_list children_list;
	struct wl_list children_link;

	int32_t saved_x, saved_y;
	bool saved_position_valid;
	bool unresponsive, grabbed;
	uint32_t resize_edges;

	struct {
		struct weston_transform transform; /* matrix from x, y */
		struct weston_view *black_view;
	} fullscreen;

	struct weston_transform workspace_transform;

	struct weston_output *fullscreen_output;
	struct weston_output *output;
	struct wl_listener output_destroy_listener;

	struct surface_state {
		bool fullscreen;
		bool maximized;
		bool lowered;
		bool activated;
	} pending_state, state;

	struct weston_size pending_size;

	struct weston_geometry geometry;

	int focus_count;

	bool destroying;
};

struct desktop_shell {
	struct weston_compositor *compositor;

	struct wl_listener destroy_listener;

	struct weston_layer fullscreen_layer;
	struct weston_layer background_layer;

	struct {
		struct wl_array array;
		unsigned int current;
		unsigned int num;

		struct wl_list client_list;

		struct weston_animation animation;
		struct wl_list anim_sticky_list;
		int anim_dir;
		struct timespec anim_timestamp;
		double anim_current;
		struct workspace *anim_from;
		struct workspace *anim_to;
	} workspaces;

	uint32_t binding_modifier;

	struct weston_layer minimized_layer;

	struct wl_listener output_create_listener;
	struct wl_listener output_move_listener;
	struct wl_list output_list;

	char *client;

	struct timespec startup_time;
};

struct weston_output *
get_default_output(struct weston_compositor *compositor);

struct weston_view *
get_default_view(struct weston_surface *surface);

void
shell_surface_set_output(struct shell_surface *shsurf,
                         struct weston_output *output);

struct weston_size
shell_surface_set_maximized(struct shell_surface *shsurf, bool maximized);

struct weston_size
shell_surface_set_fullscreen(struct shell_surface *shsurf, bool fullscreen,
			     struct weston_output *output);

void
shell_surface_set_minimized(struct shell_surface *shsurf);

struct shell_surface *
get_shell_surface(struct weston_surface *surface);

int
shell_surface_move(struct shell_surface *shsurf, struct weston_pointer *pointer,
		   bool client_initiated);
int
shell_surface_resize(struct shell_surface *shsurf,
		     struct weston_pointer *pointer, uint32_t edges);

void
shell_configure_fullscreen(struct shell_surface *shsurf);

void
shell_surface_update_layer(struct shell_surface *shsurf);

void
shell_surface_update_child_surface_layers (struct shell_surface *shsurf);

struct workspace *
get_current_workspace(struct desktop_shell *shell);

void
drop_focus_state(struct desktop_shell *shell, struct workspace *ws,
		 struct weston_surface *surface);
struct focus_state *
ensure_focus_state(struct desktop_shell *shell, struct weston_seat *seat);
struct weston_surface *
focus_state_set_focus(struct focus_state *state,
		      struct weston_surface *surface);
void
animate_focus_change(struct desktop_shell *shell, struct workspace *ws,
		     struct weston_view *from, struct weston_view *to);
int
init_workspaces(struct desktop_shell *shell);
void
destroy_workspaces(struct desktop_shell *shell);


void
get_output_work_area(struct desktop_shell *shell,
		     struct weston_output *output,
		     pixman_rectangle32_t *area);

void
lower_fullscreen_layer(struct desktop_shell *shell,
		       struct weston_output *lowering_output);

int
input_panel_setup(struct desktop_shell *shell);
void
input_panel_destroy(struct desktop_shell *shell);

typedef void (*shell_for_each_layer_func_t)(struct desktop_shell *,
					    struct weston_layer *, void *);

void
shell_for_each_layer(struct desktop_shell *shell,
		     shell_for_each_layer_func_t func,
		     void *data);

static inline struct shell_output *
find_shell_output_from_weston_output(struct desktop_shell *shell,
				     struct weston_output *output)
{
	struct shell_output *shell_output;

	wl_list_for_each(shell_output, &shell->output_list, link) {
		if (shell_output->output == output)
			return shell_output;
	}

	return NULL;
}

int desktop_shell_init(struct desktop_shell *shell,
		       struct weston_compositor *ec);

void
weston_seat_break_desktop_grabs(struct weston_seat *seat);


void
shell_ping_timeout(struct desktop_shell *shell,
		   struct wl_client *client);
void
shell_pong(struct desktop_shell *shell,
	   struct wl_client *client);

struct weston_desktop_seat *
weston_desktop_seat_from_seat(struct weston_seat *wseat);
