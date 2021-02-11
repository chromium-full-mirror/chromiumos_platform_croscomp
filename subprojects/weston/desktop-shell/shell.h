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

enum exposay_target_state {
	EXPOSAY_TARGET_OVERVIEW, /* show all windows */
	EXPOSAY_TARGET_CANCEL, /* return to normal, same focus */
	EXPOSAY_TARGET_SWITCH, /* return to normal, switch focus */
};

enum exposay_layout_state {
	EXPOSAY_LAYOUT_INACTIVE = 0, /* normal desktop */
	EXPOSAY_LAYOUT_ANIMATE_TO_INACTIVE, /* in transition to normal */
	EXPOSAY_LAYOUT_OVERVIEW, /* show all windows */
	EXPOSAY_LAYOUT_ANIMATE_TO_OVERVIEW, /* in transition to all windows */
};

struct exposay_output {
	int num_surfaces;
	int grid_size;
	int surface_size;
	int padding_inner;
};

struct exposay {
	/* XXX: Make these exposay_surfaces. */
	struct weston_view *focus_prev;
	struct weston_view *focus_current;
	struct weston_view *clicked;
	struct workspace *workspace;
	struct weston_seat *seat;

	struct wl_list surface_list;

	struct weston_keyboard_grab grab_kbd;
	struct weston_pointer_grab grab_ptr;

	enum exposay_target_state state_target;
	enum exposay_layout_state state_cur;
	int in_flight; /* number of animations still running */

	int row_current;
	int column_current;
	struct exposay_output *cur_output;

	bool mod_pressed;
	bool mod_invalid;
};

struct focus_surface {
	struct weston_surface *surface;
	struct weston_view *view;
	struct weston_transform workspace_transform;
};

struct workspace {
	struct weston_layer layer;

	struct wl_list focus_list;
	struct wl_listener seat_destroyed_listener;

	struct focus_surface *fsurf_front;
	struct focus_surface *fsurf_back;
	struct weston_view_animation *focus_animation;
};

struct shell_output {
	struct desktop_shell  *shell;
	struct weston_output  *output;
	struct exposay_output eoutput;
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

struct desktop_shell {
	struct weston_compositor *compositor;

	struct wl_listener idle_listener;
	struct wl_listener wake_listener;
	struct wl_listener resized_listener;
	struct wl_listener destroy_listener;
	struct wl_listener show_input_panel_listener;
	struct wl_listener hide_input_panel_listener;
	struct wl_listener update_input_panel_listener;

	struct weston_layer fullscreen_layer;
	struct weston_layer panel_layer;
	struct weston_layer background_layer;
	struct weston_layer lock_layer;
	struct weston_layer input_panel_layer;

	struct wl_listener pointer_focus_listener;
	struct weston_surface *grab_surface;

	struct wl_global *xdg_wm_base;	 /* Stable protocol xdg_shell replaces xdg_shell_unstable_v6 */
	struct wl_global *xdg_shell_v6;  /* Unstable xdg_shell_unstable_v6 protocol. */
	struct wl_global *wl_shell;

	struct {
		struct wl_client *client;
		struct wl_resource *desktop_shell;
		struct wl_listener client_destroy_listener;

		unsigned deathcount;
		struct timespec deathstamp;
	} child;

	bool locked;
	bool showing_input_panels;
	bool prepare_event_sent;

	struct text_backend *text_backend;

	struct {
		struct weston_surface *surface;
		pixman_box32_t cursor_rectangle;
	} text_input;

	struct weston_surface *lock_surface;
	struct wl_listener lock_surface_listener;

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

	struct {
		struct wl_resource *binding;
		struct wl_list surfaces;
	} input_panel;

	struct exposay exposay;

	bool allow_zap;
	uint32_t binding_modifier;
	uint32_t exposay_modifier;
	enum animation_type win_animation_type;
	enum animation_type win_close_animation_type;
	enum animation_type startup_animation_type;
	enum animation_type focus_animation_type;

	struct weston_layer minimized_layer;

	struct wl_listener seat_create_listener;
	struct wl_listener output_create_listener;
	struct wl_listener output_move_listener;
	struct wl_list output_list;

	char *client;

	struct timespec startup_time;
	bool initialized_inputs;
};

struct weston_output *
get_default_output(struct weston_compositor *compositor);

struct weston_view *
get_default_view(struct weston_surface *surface);

struct shell_surface *
get_shell_surface(struct weston_surface *surface);

struct workspace *
get_current_workspace(struct desktop_shell *shell);

void
get_output_work_area(struct desktop_shell *shell,
		     struct weston_output *output,
		     pixman_rectangle32_t *area);

void
lower_fullscreen_layer(struct desktop_shell *shell,
		       struct weston_output *lowering_output);

void
activate(struct desktop_shell *shell, struct weston_view *view,
	 struct weston_seat *seat, uint32_t flags);

void
exposay_binding(struct weston_keyboard *keyboard,
		enum weston_keyboard_modifier modifier,
		void *data);
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

int init_splash(struct desktop_shell *shell);

int init_croscomp_display(struct desktop_shell *shell);

void shell_finish_init(struct desktop_shell *shell);

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

int vlog(const char *fmt, va_list ap);
int vlog_continue(const char *fmt, va_list argp);

int desktop_shell_init(struct desktop_shell *shell,
		       struct weston_compositor *ec);

struct weston_compositor *
desktop_shell_get_compositor(struct desktop_shell *shell);
struct wl_display *
desktop_shell_get_display(struct desktop_shell *shell);


enum weston_desktop_surface_edge {
	WESTON_DESKTOP_SURFACE_EDGE_NONE = 0,
	WESTON_DESKTOP_SURFACE_EDGE_TOP = 1,
	WESTON_DESKTOP_SURFACE_EDGE_BOTTOM = 2,
	WESTON_DESKTOP_SURFACE_EDGE_LEFT = 4,
	WESTON_DESKTOP_SURFACE_EDGE_TOP_LEFT = 5,
	WESTON_DESKTOP_SURFACE_EDGE_BOTTOM_LEFT = 6,
	WESTON_DESKTOP_SURFACE_EDGE_RIGHT = 8,
	WESTON_DESKTOP_SURFACE_EDGE_TOP_RIGHT = 9,
	WESTON_DESKTOP_SURFACE_EDGE_BOTTOM_RIGHT = 10,
};

struct desktop_shell;
struct weston_desktop_client;
struct weston_desktop_surface;

void
weston_seat_break_desktop_grabs(struct weston_seat *seat);

struct wl_client *
weston_desktop_client_get_client(struct weston_desktop_client *client);
void
weston_desktop_client_for_each_surface(struct weston_desktop_client *client,
				       void (*callback)(struct weston_desktop_surface *surface, void *user_data),
				       void *user_data);
int
weston_desktop_client_ping(struct weston_desktop_client *client);

bool
weston_surface_is_desktop_surface(struct weston_surface *surface);
struct weston_desktop_surface *
weston_surface_get_desktop_surface(struct weston_surface *surface);

void
weston_desktop_surface_set_user_data(struct weston_desktop_surface *self,
				     void *user_data);
struct weston_view *
weston_desktop_surface_create_view(struct weston_desktop_surface *surface);
void
weston_desktop_surface_unlink_view(struct weston_view *view);
void
weston_desktop_surface_propagate_layer(struct weston_desktop_surface *surface);
void
weston_desktop_surface_set_activated(struct weston_desktop_surface *surface,
				     bool activated);
void
weston_desktop_surface_set_fullscreen(struct weston_desktop_surface *surface,
				      bool fullscreen);
void
weston_desktop_surface_set_maximized(struct weston_desktop_surface *surface,
				     bool maximized);
void
weston_desktop_surface_set_resizing(struct weston_desktop_surface *surface,
				    bool resized);
void
weston_desktop_surface_set_size(struct weston_desktop_surface *surface,
				int32_t width, int32_t height);
void
weston_desktop_surface_close(struct weston_desktop_surface *surface);
void
weston_desktop_surface_add_metadata_listener(struct weston_desktop_surface *surface,
					     struct wl_listener *listener);

void *
weston_desktop_surface_get_user_data(struct weston_desktop_surface *surface);
struct weston_desktop_client *
weston_desktop_surface_get_client(struct weston_desktop_surface *surface);
struct weston_surface *
weston_desktop_surface_get_surface(struct weston_desktop_surface *surface);
const char *
weston_desktop_surface_get_title(struct weston_desktop_surface *surface);
const char *
weston_desktop_surface_get_app_id(struct weston_desktop_surface *surface);
pid_t
weston_desktop_surface_get_pid(struct weston_desktop_surface *surface);
bool
weston_desktop_surface_get_activated(struct weston_desktop_surface *surface);
bool
weston_desktop_surface_get_maximized(struct weston_desktop_surface *surface);
bool
weston_desktop_surface_get_fullscreen(struct weston_desktop_surface *surface);
bool
weston_desktop_surface_get_resizing(struct weston_desktop_surface *surface);
struct weston_geometry
weston_desktop_surface_get_geometry(struct weston_desktop_surface *surface);
struct weston_size
weston_desktop_surface_get_max_size(struct weston_desktop_surface *surface);
struct weston_size
weston_desktop_surface_get_min_size(struct weston_desktop_surface *surface);


struct weston_desktop_client;
struct weston_desktop_surface;

void
weston_desktop_api_ping_timeout(struct desktop_shell *shell,
				struct weston_desktop_client *client);
void
weston_desktop_api_pong(struct desktop_shell *shell,
			struct weston_desktop_client *client);
void
weston_desktop_api_surface_added(struct desktop_shell *shell,
				 struct weston_desktop_surface *surface);
void
weston_desktop_api_surface_removed(struct desktop_shell *shell,
				   struct weston_desktop_surface *surface);
void
weston_desktop_api_committed(struct desktop_shell *shell,
			     struct weston_desktop_surface *surface,
			     int32_t sx, int32_t sy);
void
weston_desktop_api_show_window_menu(struct desktop_shell *shell,
				    struct weston_desktop_surface *surface,
				    struct weston_seat *seat,
				    int32_t x, int32_t y);
void
weston_desktop_api_set_parent(struct desktop_shell *shell,
			      struct weston_desktop_surface *surface,
			      struct weston_desktop_surface *parent);
void
weston_desktop_api_move(struct desktop_shell *shell,
			struct weston_desktop_surface *surface,
			struct weston_seat *seat, uint32_t serial);
void
weston_desktop_api_resize(struct desktop_shell *shell,
			  struct weston_desktop_surface *surface,
			  struct weston_seat *seat, uint32_t serial,
			  enum weston_desktop_surface_edge edges);
void
weston_desktop_api_fullscreen_requested(struct desktop_shell *shell,
					struct weston_desktop_surface *surface,
					bool fullscreen,
					struct weston_output *output);
void
weston_desktop_api_maximized_requested(struct desktop_shell *shell,
				       struct weston_desktop_surface *surface,
				       bool maximized);
void
weston_desktop_api_minimized_requested(struct desktop_shell *shell,
				       struct weston_desktop_surface *surface);

void
weston_desktop_api_set_xwayland_position(struct desktop_shell *shell,
					 struct weston_desktop_surface *surface,
					 int32_t x, int32_t y);

struct weston_desktop_seat *
weston_desktop_seat_from_seat(struct weston_seat *wseat);

struct weston_desktop_surface_implementation {
	void (*set_activated)(struct weston_desktop_surface *surface,
			      void *user_data, bool activated);
	void (*set_fullscreen)(struct weston_desktop_surface *surface,
			       void *user_data, bool fullscreen);
	void (*set_maximized)(struct weston_desktop_surface *surface,
			      void *user_data, bool maximized);
	void (*set_resizing)(struct weston_desktop_surface *surface,
			     void *user_data, bool resizing);
	void (*set_size)(struct weston_desktop_surface *surface,
			 void *user_data, int32_t width, int32_t height);
	void (*committed)(struct weston_desktop_surface *surface, void *user_data,
		          int32_t sx, int32_t sy);
	void (*update_position)(struct weston_desktop_surface *surface,
				void *user_data);
	void (*ping)(struct weston_desktop_surface *surface, uint32_t serial,
		     void *user_data);
	void (*close)(struct weston_desktop_surface *surface, void *user_data);

	bool (*get_activated)(struct weston_desktop_surface *surface,
			      void *user_data);
	bool (*get_fullscreen)(struct weston_desktop_surface *surface,
			       void *user_data);
	bool (*get_maximized)(struct weston_desktop_surface *surface,
			      void *user_data);
	bool (*get_resizing)(struct weston_desktop_surface *surface,
			     void *user_data);
	struct weston_size
	(*get_max_size)(struct weston_desktop_surface *surface,
			void *user_data);
	struct weston_size
	(*get_min_size)(struct weston_desktop_surface *surface,
			void *user_data);

	void (*destroy)(struct weston_desktop_surface *surface,
			void *user_data);
};

struct weston_desktop_client *
weston_desktop_client_create(struct desktop_shell *shell,
			     struct wl_client *client,
			     wl_dispatcher_func_t dispatcher,
			     const struct wl_interface *interface,
			     const void *implementation, uint32_t version,
			     uint32_t id);

void
weston_desktop_client_add_destroy_listener(struct weston_desktop_client *client,
					   struct wl_listener *listener);
struct desktop_shell *
weston_desktop_client_get_shell(struct weston_desktop_client *client);
struct wl_resource *
weston_desktop_client_get_resource(struct weston_desktop_client *client);
struct wl_list *
weston_desktop_client_get_surface_list(struct weston_desktop_client *client);

void
weston_desktop_client_pong(struct weston_desktop_client *client,
			   uint32_t serial);

struct weston_desktop_surface *
weston_desktop_surface_create(struct desktop_shell *shell,
			      struct weston_desktop_client *client,
			      struct weston_surface *surface,
			      const struct weston_desktop_surface_implementation *implementation,
			      void *implementation_data);
void
weston_desktop_surface_destroy(struct weston_desktop_surface *surface);
void
weston_desktop_surface_resource_destroy(struct wl_resource *resource);
struct wl_resource *
weston_desktop_surface_add_resource(struct weston_desktop_surface *surface,
				    const struct wl_interface *interface,
				    const void *implementation, uint32_t id,
				    wl_resource_destroy_func_t destroy);
struct weston_desktop_surface *
weston_desktop_surface_from_grab_link(struct wl_list *grab_link);

struct wl_list *
weston_desktop_surface_get_client_link(struct weston_desktop_surface *surface);
struct weston_desktop_surface *
weston_desktop_surface_from_client_link(struct wl_list *link);
bool
weston_desktop_surface_has_implementation(struct weston_desktop_surface *surface,
					  const struct weston_desktop_surface_implementation *implementation);
const struct weston_desktop_surface_implementation *
weston_desktop_surface_get_implementation(struct weston_desktop_surface *surface);
void *
weston_desktop_surface_get_implementation_data(struct weston_desktop_surface *surface);
struct weston_desktop_surface *
weston_desktop_surface_get_parent(struct weston_desktop_surface *surface);
bool
weston_desktop_surface_get_grab(struct weston_desktop_surface *surface);

void
weston_desktop_surface_set_title(struct weston_desktop_surface *surface,
				 const char *title);
void
weston_desktop_surface_set_app_id(struct weston_desktop_surface *surface,
				  const char *app_id);
void
weston_desktop_surface_set_pid(struct weston_desktop_surface *surface,
			       pid_t pid);
void
weston_desktop_surface_set_geometry(struct weston_desktop_surface *surface,
				    struct weston_geometry geometry);
void
weston_desktop_surface_set_relative_to(struct weston_desktop_surface *surface,
				       struct weston_desktop_surface *parent,
				       int32_t x, int32_t y, bool use_geometry);
void
weston_desktop_surface_unset_relative_to(struct weston_desktop_surface *surface);
void
weston_desktop_surface_popup_grab(struct weston_desktop_surface *popup,
				  struct weston_desktop_seat *seat,
				  uint32_t serial);
void
weston_desktop_surface_popup_ungrab(struct weston_desktop_surface *popup,
				    struct weston_desktop_seat *seat);
void
weston_desktop_surface_popup_dismiss(struct weston_desktop_surface *surface);

struct weston_desktop_surface *
weston_desktop_seat_popup_grab_get_topmost_surface(struct weston_desktop_seat *seat);
bool
weston_desktop_seat_popup_grab_start(struct weston_desktop_seat *seat,
				     struct wl_client *client, uint32_t serial);
void
weston_desktop_seat_popup_grab_add_surface(struct weston_desktop_seat *seat,
					   struct wl_list *link);
void
weston_desktop_seat_popup_grab_remove_surface(struct weston_desktop_seat *seat,
					      struct wl_list *link);

void
weston_desktop_destroy_request(struct wl_client *client,
			       struct wl_resource *resource);
struct wl_global *
weston_desktop_xdg_wm_base_create(struct desktop_shell *shell,
				  struct wl_display *display);
struct wl_global *
weston_desktop_xdg_shell_v6_create(struct desktop_shell *shell,
				   struct wl_display *display);
struct wl_global *
weston_desktop_wl_shell_create(struct desktop_shell *shell,
			       struct wl_display *display);

int
exo_extensions_init(struct desktop_shell *shell);
