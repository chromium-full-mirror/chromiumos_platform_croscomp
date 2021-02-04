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

#include "alpha-compositing-unstable-v1-server-protocol.h"
#include "aura-shell-server-protocol.h"
#include "cursor-shapes-unstable-v1-server-protocol.h"
#include "gaming-input-unstable-v2-server-protocol.h"
#include "keyboard-configuration-unstable-v1-server-protocol.h"
#include "keyboard-extension-unstable-v1-server-protocol.h"
#include "pointer-gestures-unstable-v1-server-protocol.h"
#include "remote-shell-unstable-v1-server-protocol.h"
#include "secure-output-unstable-v1-server-protocol.h"
#include "stylus-tools-unstable-v1-server-protocol.h"
#include "stylus-unstable-v2-server-protocol.h"
#include "vsync-feedback-unstable-v1-server-protocol.h"

#include <libweston/libweston.h>
#include "shared/zalloc.h"
#include "libweston-desktop.h"
#include "internal.h"
#include "shell.h"


struct exo_surface {
	struct wl_resource *resource;
	struct desktop_shell *shell;
	struct wl_display *display;
	struct weston_desktop_surface *surface;
	struct wl_listener destroy_listener;
	bool added;
};

static void
exo_surface_set_size(struct weston_desktop_surface *dsurface,
		     void *user_data,
		     int32_t width, int32_t height)
{
}

static void
exo_surface_committed(struct weston_desktop_surface *dsurface,
		      void *user_data,
		      int32_t sx, int32_t sy)
{
	struct exo_surface *surface = user_data;

	weston_desktop_api_committed(surface->shell, surface->surface, sx, sy);
}

static void
exo_surface_destroy(struct weston_desktop_surface *dsurface,
					void *user_data)
{
	struct exo_surface *surface = user_data;

	wl_list_remove(&surface->destroy_listener.link);

	if (surface->added)
		weston_desktop_api_surface_removed(surface->shell,
						   surface->surface);

	free(surface);
}

static const struct weston_desktop_surface_implementation exo_surface_internal_implementation = {
	.set_size = exo_surface_set_size,
	.committed = exo_surface_committed,
	.destroy = exo_surface_destroy
};

static void nop_handler(void)
{
}

static const struct zcr_blending_v1_interface zcr_blending_v1_implementation = {
	.destroy = (void *) nop_handler,
	.set_blending = (void *) nop_handler,
	.set_alpha = (void *) nop_handler,
};

static void
alpha_compositing_destroy(struct wl_client *client,
			  struct wl_resource *resource)
{
}

static void
alpha_compositing_get_blending(struct wl_client *client,
			       struct wl_resource *desktop_resource,
			       uint32_t id,
			       struct wl_resource *surface)
{
	struct wl_resource *resource;

	resource = wl_resource_create(client, &zcr_blending_v1_interface, 1, id);
	wl_resource_set_implementation(resource,
				       &zcr_blending_v1_implementation,
				       NULL, NULL);

}

static const struct zcr_alpha_compositing_v1_interface zcr_alpha_compositing_v1_implementation = {
	.destroy = alpha_compositing_destroy,
	.get_blending = alpha_compositing_get_blending
};

static struct wl_resource *
do_bind(struct wl_client *client, struct desktop_shell *shell,
	uint32_t version, uint32_t id,
	const struct wl_interface *interface,
	const void *implementation)
{
	struct wl_resource *resource;

	weston_log("exo: bind %s\n", interface->name);

	resource = wl_resource_create(client, interface, version, id);

	wl_resource_set_implementation(resource, implementation, shell, NULL);

	return resource;
}

static void
bind_alpha_compositing(struct wl_client *client,
		       void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zcr_alpha_compositing_v1_interface,
		&zcr_alpha_compositing_v1_implementation);
}

static void
zaura_surface_set_frame(struct wl_client *client,
			struct wl_resource *resource,
			uint32_t type)
{
}

static void
zaura_surface_set_parent(struct wl_client *client,
			 struct wl_resource *resource,
			 struct wl_resource *parent,
			 int32_t x,
			 int32_t y)
{
}

static void
zaura_surface_set_frame_colors(struct wl_client *client,
			       struct wl_resource *resource,
			       uint32_t active_color,
			       uint32_t inactive_color)
{
}

static void
zaura_surface_set_startup_id(struct wl_client *client,
			     struct wl_resource *resource,
			     const char *startup_id)
{
}

static void
zaura_surface_set_application_id(struct wl_client *client,
				 struct wl_resource *resource,
				 const char *application_id)
{
}

static void
zaura_surface_set_client_surface_id(struct wl_client *client,
				    struct wl_resource *resource,
				    int32_t client_surface_id)
{
}

static void
zaura_surface_set_occlusion_tracking(struct wl_client *client,
				     struct wl_resource *resource)
{
}

static void
zaura_surface_unset_occlusion_tracking(struct wl_client *client,
				       struct wl_resource *resource)
{
}

static void
zaura_surface_activate(struct wl_client *client,
		       struct wl_resource *resource)
{
}

static void
zaura_surface_draw_attention(struct wl_client *client,
			     struct wl_resource *resource)
{
}

static void
zaura_surface_set_fullscreen_mode(struct wl_client *client,
				  struct wl_resource *resource,
				  uint32_t mode)
{
	/* xxx */

	weston_log("exo: zaura_surface_set_fullscreen_mode %d\n", mode);
}

static void
zaura_surface_set_client_surface_str_id(struct wl_client *client,
					struct wl_resource *resource,
					const char *client_surface_id)
{
}

static const struct zaura_surface_interface zaura_surface_implementation = {
	.set_frame = zaura_surface_set_frame,
	.set_parent = zaura_surface_set_parent,
	.set_frame_colors = zaura_surface_set_frame_colors,
	.set_startup_id = zaura_surface_set_startup_id,
	.set_application_id = zaura_surface_set_application_id,
	.set_client_surface_id = zaura_surface_set_client_surface_id,
	.set_occlusion_tracking = zaura_surface_set_occlusion_tracking,
	.unset_occlusion_tracking = zaura_surface_unset_occlusion_tracking,
	.activate = zaura_surface_activate,
	.draw_attention = zaura_surface_draw_attention,
	.set_fullscreen_mode = zaura_surface_set_fullscreen_mode,
	.set_client_surface_str_id = zaura_surface_set_client_surface_str_id
};

static void
zaura_shell_get_aura_surface(struct wl_client *client,
			     struct wl_resource *resource,
			     uint32_t id,
			     struct wl_resource *surface)
{
	struct wl_resource *aura_surface;

	/* xxx */

	aura_surface = wl_resource_create(client, &zaura_surface_interface, 12, id);
	wl_resource_set_implementation(aura_surface,
				       &zaura_surface_implementation,
				       NULL, NULL);


}

static void
zaura_shell_get_aura_output(struct wl_client *client,
			    struct wl_resource *resource,
			    uint32_t id,
			    struct wl_resource *output)
{
	weston_log("exo: zaura_shell_get_aura_output\n");
}

struct zaura_shell_interface zaura_shell_implementation = {
	.get_aura_surface = zaura_shell_get_aura_surface,
	.get_aura_output = zaura_shell_get_aura_output
};

struct aura_client_data {
	struct wl_listener listener;
};

static void
client_gone_notify(struct wl_listener *listener, void *data)
{
	weston_log("exo: aura client gone\n");
}

static void
bind_aura_shell(struct wl_client *client,
		void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zaura_shell_interface,
		&zaura_shell_implementation);

	struct aura_client_data *client_data = malloc(sizeof(*client_data));

	client_data->listener.notify = client_gone_notify;
	wl_client_add_destroy_listener(client,
				       &client_data->listener);
}

static void
pointer_gestures_get_swipe_gesture(struct wl_client *client,
				   struct wl_resource *resource,
				   uint32_t id,
				   struct wl_resource *pointer)
{
	weston_log("exo: pointer_gestures_get_swipe_gesture\n");
}

static void
pointer_gestures_get_pinch_gesture(struct wl_client *client,
				   struct wl_resource *resource,
				   uint32_t id,
				   struct wl_resource *pointer)
{
	weston_log("exo: pointer_gestures_get_pinch_gesture\n");
}

static void
pointer_gestures_release(struct wl_client *client,
			 struct wl_resource *resource)
{
	weston_log("exo: pointer_gestures_release\n");
}

static const struct zwp_pointer_gestures_v1_interface zwp_pointer_gestures_v1_implementation = {
	.get_swipe_gesture = pointer_gestures_get_swipe_gesture,
	.get_pinch_gesture = pointer_gestures_get_pinch_gesture,
	.release = pointer_gestures_release
};

static void
bind_pointer_gestures(struct wl_client *client,
		      void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zwp_pointer_gestures_v1_interface,
		&zwp_pointer_gestures_v1_implementation);
}

static void
gaming_input_get_gaming_seat(struct wl_client *client,
			     struct wl_resource *resource,
			     uint32_t gaming_seat,
			     struct wl_resource *seat)
{
}

static void
gaming_input_destroy(struct wl_client *client,
		     struct wl_resource *resource)
{
}

static const struct zcr_gaming_input_v2_interface zcr_gaming_input_v2_implementation = {
	.get_gaming_seat = gaming_input_get_gaming_seat,
	.destroy = gaming_input_destroy
};

static void
bind_gaming_input(struct wl_client *client,
		  void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zcr_gaming_input_v2_interface,
		&zcr_gaming_input_v2_implementation);
}

static void
keyboard_configuration_get_keyboard_device_configuration(struct wl_client *client,
							 struct wl_resource *resource,
							 uint32_t id,
							 struct wl_resource *keyboard)
{
	/* xxx */

	wl_resource_create(client, &zcr_keyboard_device_configuration_v1_interface, 2, id);

}

static const struct zcr_keyboard_configuration_v1_interface zcr_keyboard_configuration_v1_implementation = {
	.get_keyboard_device_configuration = keyboard_configuration_get_keyboard_device_configuration
};

static void
bind_keyboard_configuration(struct wl_client *client,
			    void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zcr_keyboard_configuration_v1_interface,
		&zcr_keyboard_configuration_v1_implementation);
}

static void
extended_keyboard_destroy(struct wl_client *client,
			  struct wl_resource *resource)
{
}

static void
extended_keyboard_ack_key(struct wl_client *client,
			  struct wl_resource *resource,
			  uint32_t serial,
			  uint32_t handled)
{
}

static const struct zcr_extended_keyboard_v1_interface zcr_extended_keyboard_v1_implementation = {
	.destroy = extended_keyboard_destroy,
	.ack_key = extended_keyboard_ack_key
};

static void
keyboard_extension_get_extended_keyboard(struct wl_client *client,
					 struct wl_resource *resource,
					 uint32_t id,
					 struct wl_resource *wl_keyboard)
{
	struct wl_resource *keyboard;

	/* xxx */

	keyboard = wl_resource_create(client, &zcr_extended_keyboard_v1_interface, 1, id);
	wl_resource_set_implementation(keyboard,
				       &zcr_extended_keyboard_v1_implementation,
				       NULL, NULL);
}

static const struct zcr_keyboard_extension_v1_interface zcr_keyboard_extension_v1_implementation = {
	.get_extended_keyboard = keyboard_extension_get_extended_keyboard
};

static void
bind_keyboard_extension(struct wl_client *client,
			void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zcr_keyboard_extension_v1_interface,
		&zcr_keyboard_extension_v1_implementation);
}


static void
cursor_shapes_set_cursor_shape(struct wl_client *client,
			       struct wl_resource *resource,
			       struct wl_resource *pointer,
			       int32_t shape)
{
}

static const struct zcr_cursor_shapes_v1_interface zcr_cursor_shapes_v1_implementation = {
	.set_cursor_shape = cursor_shapes_set_cursor_shape
};

static void
bind_cursor_shapes(struct wl_client *client,
		   void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zcr_cursor_shapes_v1_interface,
		&zcr_cursor_shapes_v1_implementation);
}

static void
remote_shell_destroy(struct wl_client *client,
		     struct wl_resource *resource)
{
	weston_log("exo: remote_shell_destroy\n");
}

static void
remote_surface_destroy(struct wl_client *client,
		       struct wl_resource *resource)
{
	weston_log("exo: remote_surface_destroy\n");
}

static void
remote_surface_set_app_id(struct wl_client *client,
			  struct wl_resource *resource,
			  const char *app_id)
{
	weston_log("exo: remote_surface_set_app_id\n");
}

static void
remote_surface_set_window_geometry(struct wl_client *client,
				   struct wl_resource *resource,
				   int32_t x,
				   int32_t y,
				   int32_t width,
				   int32_t height)
{
	weston_log("exo: remote_surface_set_window_geometry\n");
}

static void
remote_surface_set_scale(struct wl_client *client,
			 struct wl_resource *resource,
			 wl_fixed_t scale)
{
	weston_log("exo: remote_surface_set_scale\n");
}

static void
remote_surface_set_rectangular_shadow(struct wl_client *client,
				      struct wl_resource *resource,
				      int32_t x,
				      int32_t y,
				      int32_t width,
				      int32_t height)
{
	weston_log("exo: remote_surface_set_rectangular_shadow\n");
}

static void
remote_surface_set_rectangular_shadow_background_opacity(struct wl_client *client,
							 struct wl_resource *resource,
							 wl_fixed_t opacity)
{
	weston_log("exo: remote_surface_set_rectangular_shadow_background_opacity\n");
}

static void
remote_surface_set_title(struct wl_client *client,
			 struct wl_resource *resource,
			 const char *title)
{
	weston_log("exo: remote_surface_set_title\n");
}

static void
remote_surface_set_top_inset(struct wl_client *client,
			     struct wl_resource *resource,
			     int32_t height)
{
	weston_log("exo: remote_surface_set_top_inset\n");
}

static void
remote_surface_activate(struct wl_client *client,
			struct wl_resource *resource,
			uint32_t serial)
{
	weston_log("exo: remote_surface_activate\n");
}

static void
remote_surface_maximize(struct wl_client *client,
			struct wl_resource *resource)
{
	struct weston_desktop_surface *dsurface =
		wl_resource_get_user_data(resource);
	struct exo_surface *surface =
		weston_desktop_surface_get_implementation_data(dsurface);

	weston_log("exo: remote_surface_maximize\n");

	surface->added = true;
	weston_desktop_api_surface_added(surface->shell,
					 surface->surface);
	weston_desktop_api_maximized_requested(surface->shell, dsurface, true);
}

static void
remote_surface_minimize(struct wl_client *client,
			struct wl_resource *resource)
{
	weston_log("exo: remote_surface_minimize\n");
}

static void
remote_surface_restore(struct wl_client *client,
		       struct wl_resource *resource)
{
	weston_log("exo: remote_surface_restore\n");
}

static void
remote_surface_fullscreen(struct wl_client *client,
			  struct wl_resource *resource)
{
	weston_log("exo: remote_surface_fullscreen\n");
}

static void
remote_surface_unfullscreen(struct wl_client *client,
			    struct wl_resource *resource)
{
	weston_log("exo: remote_surface_unfullscreen\n");
}

static void
remote_surface_pin(struct wl_client *client,
		   struct wl_resource *resource,
		   int32_t trusted)
{
	weston_log("exo: remote_surface_pin\n");
}

static void
remote_surface_unpin(struct wl_client *client,
		     struct wl_resource *resource)
{
	weston_log("exo: remote_surface_unpin\n");
}

static void
remote_surface_set_system_modal(struct wl_client *client,
				struct wl_resource *resource)
{
	weston_log("exo: remote_surface_set_system_modal\n");
}

static void
remote_surface_unset_system_modal(struct wl_client *client,
				  struct wl_resource *resource)
{
	weston_log("exo: remote_surface_unset_system_modal\n");
}

static void
remote_surface_set_rectangular_surface_shadow(struct wl_client *client,
					      struct wl_resource *resource,
					      int32_t x,
					      int32_t y,
					      int32_t width,
					      int32_t height)
{
	weston_log("exo: remote_surface_set_rectangular_surface_shadow\n");
}

static void
remote_surface_set_systemui_visibility(struct wl_client *client,
				       struct wl_resource *resource,
				       uint32_t visibility)
{
	weston_log("exo: remote_surface_set_systemui_visibility\n");
}

static void
remote_surface_set_always_on_top(struct wl_client *client,
				 struct wl_resource *resource)
{
	weston_log("exo: remote_surface_set_always_on_top\n");
}

static void
remote_surface_unset_always_on_top(struct wl_client *client,
				   struct wl_resource *resource)
{
	weston_log("exo: remote_surface_unset_always_on_top\n");
}

static void
remote_surface_ack_configure(struct wl_client *client,
			     struct wl_resource *resource,
			     uint32_t serial)
{
	weston_log("exo: remote_surface_ack_configure\n");
}

static void
remote_surface_move(struct wl_client *client,
		    struct wl_resource *resource)
{
	weston_log("exo: remote_surface_move\n");
}

static void
remote_surface_set_orientation(struct wl_client *client,
			       struct wl_resource *resource,
			       int32_t orientation)
{
	weston_log("exo: remote_surface_set_orientation\n");
}

static void
remote_surface_set_window_type(struct wl_client *client,
			       struct wl_resource *resource,
			       uint32_t type)
{
	weston_log("exo: remote_surface_set_window_type\n");
}

static void
remote_surface_resize(struct wl_client *client,
		      struct wl_resource *resource)
{
	weston_log("exo: remote_surface_resize\n");
}

static void
remote_surface_set_resize_outset(struct wl_client *client,
				 struct wl_resource *resource,
				 int32_t outset)
{
	weston_log("exo: remote_surface_set_resize_outset\n");
}

static void
remote_surface_start_move(struct wl_client *client,
			  struct wl_resource *resource,
			  int32_t x,
			  int32_t y)
{
	weston_log("exo: remote_surface_start_move\n");
}

static void
remote_surface_set_can_maximize(struct wl_client *client,
				struct wl_resource *resource)
{
	weston_log("exo: remote_surface_set_can_maximize\n");
}

static void
remote_surface_unset_can_maximize(struct wl_client *client,
				  struct wl_resource *resource)
{
	weston_log("exo: remote_surface_unset_can_maximize\n");
}

static void
remote_surface_set_min_size(struct wl_client *client,
			    struct wl_resource *resource,
			    int32_t width,
			    int32_t height)
{
	weston_log("exo: remote_surface_set_min_size\n");
}

static  void
remote_surface_set_max_size(struct wl_client *client,
			    struct wl_resource *resource,
			    int32_t width,
			    int32_t height)
{
	weston_log("exo: remote_surface_set_max_size\n");
}

static void
remote_surface_set_snapped_to_left(struct wl_client *client,
				   struct wl_resource *resource)
{
	weston_log("exo: remote_surface_set_snapped_to_left\n");
}

static void
remote_surface_set_snapped_to_right(struct wl_client *client,
				    struct wl_resource *resource)
{
	weston_log("exo: remote_surface_set_snapped_to_right\n");
}

static void
remote_surface_start_resize(struct wl_client *client,
			    struct wl_resource *resource,
			    uint32_t resize_direction,
			    int32_t x,
			    int32_t y)
{
	weston_log("exo: remote_surface_start_resize\n");
}

static void
remote_surface_set_frame(struct wl_client *client,
			 struct wl_resource *resource,
			 uint32_t type)
{
	weston_log("exo: remote_surface_set_frame\n");
}

static void
remote_surface_set_frame_buttons(struct wl_client *client,
				 struct wl_resource *resource,
				 uint32_t visible_buttons,
				 uint32_t enabled_buttons)
{
	weston_log("exo: remote_surface_set_frame_buttons\n");
}

static void
remote_surface_set_extra_title(struct wl_client *client,
			       struct wl_resource *resource,
			       const char *extra_title)
{
	weston_log("exo: remote_surface_set_extra_title\n");
}

static void
remote_surface_set_orientation_lock(struct wl_client *client,
				    struct wl_resource *resource,
				    uint32_t orientation_lock)
{
	weston_log("exo: remote_surface_set_orientation_lock\n");
}

static void
remote_surface_pip(struct wl_client *client,
		   struct wl_resource *resource)
{
	weston_log("exo: remote_surface_pip\n");
}

static void
remote_surface_set_bounds(struct wl_client *client,
			  struct wl_resource *resource,
			  uint32_t display_id_hi,
			  uint32_t display_id_lo,
			  int32_t x,
			  int32_t y,
			  int32_t width,
			  int32_t height)
{
	weston_log("exo: remote_surface_set_bounds\n");
}

static void
remote_surface_set_aspect_ratio(struct wl_client *client,
				struct wl_resource *resource,
				int32_t width,
				int32_t height)
{
	weston_log("exo: remote_surface_set_aspect_ratio\n");
}

static void
remote_surface_block_ime(struct wl_client *client,
			 struct wl_resource *resource)
{
	weston_log("exo: remote_surface_block_ime\n");
}

static void
remote_surface_unblock_ime(struct wl_client *client,
			   struct wl_resource *resource)
{
	weston_log("exo: remote_surface_unblock_ime\n");
}

static void
remote_surface_set_accessibility_id(struct wl_client *client,
				    struct wl_resource *resource,
				    int32_t id)
{
	weston_log("exo: remote_surface_set_accessibility_id\n");
}

static void
remote_surface_set_pip_original_window(struct wl_client *client,
				       struct wl_resource *resource)
{
	weston_log("exo: remote_surface_set_pip_original_window\n");
}

static void
remote_surface_unset_pip_original_window(struct wl_client *client,
					 struct wl_resource *resource)
{
	weston_log("exo: remote_surface_unset_pip_original_window\n");
}

static void
remote_surface_set_system_gesture_exclusion(struct wl_client *client,
					    struct wl_resource *resource,
					    struct wl_resource *region)
{
	weston_log("exo: remote_surface_set_system_gesture_exclusion\n");
}

static const struct zcr_remote_surface_v1_interface zcr_remote_surface_v1_implementation = {
	.destroy = remote_surface_destroy,
	.set_app_id = remote_surface_set_app_id,
	.set_window_geometry = remote_surface_set_window_geometry,
	.set_scale = remote_surface_set_scale,
	.set_rectangular_shadow = remote_surface_set_rectangular_shadow,
	.set_rectangular_shadow_background_opacity = remote_surface_set_rectangular_shadow_background_opacity,
	.set_title = remote_surface_set_title,
	.set_top_inset = remote_surface_set_top_inset,
	.activate = remote_surface_activate,
	.maximize = remote_surface_maximize,
	.minimize = remote_surface_minimize,
	.restore = remote_surface_restore,
	.fullscreen = remote_surface_fullscreen,
	.unfullscreen = remote_surface_unfullscreen,
	.pin = remote_surface_pin,
	.unpin = remote_surface_unpin,
	.set_system_modal = remote_surface_set_system_modal,
	.unset_system_modal = remote_surface_unset_system_modal,
	.set_rectangular_surface_shadow = remote_surface_set_rectangular_surface_shadow,
	.set_systemui_visibility = remote_surface_set_systemui_visibility,
	.set_always_on_top = remote_surface_set_always_on_top,
	.unset_always_on_top = remote_surface_unset_always_on_top,
	.ack_configure = remote_surface_ack_configure,
	.move = remote_surface_move,
	.set_orientation = remote_surface_set_orientation,
	.set_window_type = remote_surface_set_window_type,
	.resize = remote_surface_resize,
	.set_resize_outset = remote_surface_set_resize_outset,
	.start_move = remote_surface_start_move,
	.set_can_maximize = remote_surface_set_can_maximize,
	.unset_can_maximize = remote_surface_unset_can_maximize,
	.set_min_size = remote_surface_set_min_size,
	.set_max_size = remote_surface_set_max_size,
	.set_snapped_to_left = remote_surface_set_snapped_to_left,
	.set_snapped_to_right = remote_surface_set_snapped_to_right,
	.start_resize = remote_surface_start_resize,
	.set_frame = remote_surface_set_frame,
	.set_frame_buttons = remote_surface_set_frame_buttons,
	.set_extra_title = remote_surface_set_extra_title,
	.set_orientation_lock = remote_surface_set_orientation_lock,
	.pip = remote_surface_pip,
	.set_bounds = remote_surface_set_bounds,
	.set_aspect_ratio = remote_surface_set_aspect_ratio,
	.block_ime = remote_surface_block_ime,
	.unblock_ime = remote_surface_unblock_ime,
	.set_accessibility_id = remote_surface_set_accessibility_id,
	.set_pip_original_window = remote_surface_set_pip_original_window,
	.unset_pip_original_window = remote_surface_unset_pip_original_window,
	.set_system_gesture_exclusion = remote_surface_set_system_gesture_exclusion,
};

static void
remote_surface_resource_destroyed(struct wl_listener *listener,
				  void *data)
{
	struct exo_surface *surface =
		wl_container_of(listener, surface, destroy_listener);

	/* the wl_shell_surface spec says that wl_shell_surfaces are to be
	 * destroyed automatically when the wl_surface is destroyed. */
	weston_desktop_surface_destroy(surface->surface);
}

static void
remote_shell_get_remote_surface(struct wl_client *wl_client,
				struct wl_resource *resource,
				uint32_t id,
				struct wl_resource *surface_resource,
				uint32_t container)
{
	struct weston_desktop_client *client = wl_resource_get_user_data(resource);
	struct weston_surface *wsurface = wl_resource_get_user_data(surface_resource);
	struct exo_surface *surface;

	weston_log("exo: remote_shell_get_remote_surface, enter\n");

	if (weston_surface_set_role(wsurface, "exo_surface",
				    resource, ZCR_REMOTE_SHELL_V1_ERROR_ROLE) < 0)
		return;

	surface = zalloc(sizeof(struct exo_surface));
	if (surface == NULL) {
		wl_client_post_no_memory(wl_client);
		return;
	}

	surface->shell = weston_desktop_client_get_shell(client);
	surface->display = desktop_shell_get_display(surface->shell);
	surface->added = false;

	surface->surface =
		weston_desktop_surface_create(surface->shell, client, wsurface,
					      &exo_surface_internal_implementation,
					      surface);
	if (surface->surface == NULL) {
		free(surface);
		return;
	}

	surface->destroy_listener.notify = remote_surface_resource_destroyed;
	wl_resource_add_destroy_listener(wsurface->resource,
					 &surface->destroy_listener);


	surface->resource =
		weston_desktop_surface_add_resource(surface->surface,
						    &zcr_remote_surface_v1_interface,
						    &zcr_remote_surface_v1_implementation,
						    id, NULL);

	weston_log("exo: remote_shell_get_remote_surface, weston surface %p, desktop surface %p\n",
		   wsurface, surface->surface);
}

static void
remote_shell_get_notification_surface(struct wl_client *client,
				      struct wl_resource *resource,
				      uint32_t id,
				      struct wl_resource *surface,
				      const char *notification_key)
{
	weston_log("exo: remote_shell_get_notification_surface\n");
}

static void
remote_shell_get_input_method_surface(struct wl_client *client,
				      struct wl_resource *resource,
				      uint32_t id,
				      struct wl_resource *surface)
{
	weston_log("exo: remote_shell_get_input_method_surface\n");
}

static void
remote_shell_get_toast_surface(struct wl_client *client,
			       struct wl_resource *resource,
			       uint32_t id,
			       struct wl_resource *surface)
{
	weston_log("exo: remote_shell_get_toast_surface\n");
}

static void
remote_shell_get_remote_output(struct wl_client *client,
			       struct wl_resource *resource,
			       uint32_t id,
			       struct wl_resource *output)
{
	weston_log("exo: remote_shell_get_remote_output\n");
}

static void
remote_shell_set_use_default_device_scale_cancellation(struct wl_client *client,
						       struct wl_resource *resource,
						       int32_t use_default_device_scale_factor)
{
}

static const struct zcr_remote_shell_v1_interface zcr_remote_shell_v1_implementation = {
	.destroy = remote_shell_destroy,
	.get_remote_surface = remote_shell_get_remote_surface,
	.get_notification_surface = remote_shell_get_notification_surface,
	.get_input_method_surface = remote_shell_get_input_method_surface,
	.get_toast_surface = remote_shell_get_toast_surface,
	.get_remote_output = remote_shell_get_remote_output,
	.set_use_default_device_scale_cancellation = remote_shell_set_use_default_device_scale_cancellation
};

static void
bind_remote_shell(struct wl_client *wl_client,
		  void *data, uint32_t version, uint32_t id)
{
	struct desktop_shell *dshell = data;
	struct weston_compositor *compositor = desktop_shell_get_compositor(dshell);
	struct weston_desktop_client *client;

	client = weston_desktop_client_create(dshell, wl_client, NULL,
					      &zcr_remote_shell_v1_interface,
					      &zcr_remote_shell_v1_implementation,
					      version, id);

	struct weston_head *head;
	struct wl_resource *shell = weston_desktop_client_get_resource(client);
	wl_list_for_each(head, &compositor->head_list, compositor_link) {
		struct weston_output *output = head->output;
		if (output == NULL)
			continue;
		uint64_t id = (uint64_t) head;
		struct wl_array identification_data = { 0, 0, NULL };
		zcr_remote_shell_v1_send_workspace_info(shell,
							id >> 32, id,
							output->x, output->y, output->width, output->height,
							/* inset */ 0, 0, 0, 0,
							/* stable_inst */ 0, 0, 0, 0, 

							ZCR_REMOTE_SURFACE_V1_SYSTEMUI_VISIBILITY_STATE_VISIBLE,
							WL_OUTPUT_TRANSFORM_NORMAL,
							head->connection_internal,
							&identification_data);
		weston_log("exo: output %p\n", output);
	}

	zcr_remote_shell_v1_send_default_device_scale_factor(shell, 1 << 24);

	zcr_remote_shell_v1_send_configure(shell, ZCR_REMOTE_SHELL_V1_LAYOUT_MODE_WINDOWED);
}

static void
bind_secure_output(struct wl_client *client,
		   void *data, uint32_t version, uint32_t id)
{
	weston_log("exo: bind_secure_output\n");
}

static void
stylus_get_touch_stylus(struct wl_client *client,
			struct wl_resource *resource,
			uint32_t id,
			struct wl_resource *touch)
{
	weston_log("exo: stylus_get_touch_stylus\n");
}

static void
stylus_get_pointer_stylus(struct wl_client *client,
			  struct wl_resource *resource,
			  uint32_t id,
			  struct wl_resource *pointer)
{
	weston_log("exo: stylus_get_pointer_stylus\n");
}

static const struct zcr_stylus_v2_interface zcr_stylus_v2_implementation = {
	.get_touch_stylus = stylus_get_touch_stylus,
	.get_pointer_stylus = stylus_get_pointer_stylus
};

static void
bind_stylus(struct wl_client *client,
	    void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zcr_stylus_v2_interface,
		&zcr_stylus_v2_implementation);
}

static void
bind_stylus_tools(struct wl_client *client,
		  void *data, uint32_t version, uint32_t id)
{
	weston_log("exo: bind_stylus_tools\n");
}

static void
vsync_feedback_destroy(struct wl_client *client,
		       struct wl_resource *resource)
{
	weston_log("exo: vsync_feedback_destroy\n");
}

static void
vsync_feedback_get_vsync_timing(struct wl_client *client,
				struct wl_resource *resource,
				uint32_t id,
				struct wl_resource *output)
{
	weston_log("exo: vsync_feedback_get_vsync_timing\n");
}

struct zcr_vsync_feedback_v1_interface zcr_vsync_feedback_v1_implementation = {
	.destroy = vsync_feedback_destroy,
	.get_vsync_timing = vsync_feedback_get_vsync_timing
};

static void
bind_vsync_feedback(struct wl_client *client,
		    void *data, uint32_t version, uint32_t id)
{
	do_bind(client, data, version, id,
		&zcr_vsync_feedback_v1_interface,
		&zcr_vsync_feedback_v1_implementation);
}

int
exo_extensions_init(struct desktop_shell *shell)
{
	struct wl_display *display = desktop_shell_get_display(shell);

	if (wl_global_create(display,
			     &zcr_alpha_compositing_v1_interface, 1,
			     shell, bind_alpha_compositing) == NULL)
		return -1;
	
	if (wl_global_create(display,
			     &zaura_shell_interface, 12,
			     shell, bind_aura_shell) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zwp_pointer_gestures_v1_interface, 2,
			     shell, bind_pointer_gestures) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_gaming_input_v2_interface, 2,
			     shell, bind_gaming_input) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_keyboard_configuration_v1_interface, 2,
			     shell, bind_keyboard_configuration) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_keyboard_extension_v1_interface, 1,
			     shell, bind_keyboard_extension) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_cursor_shapes_v1_interface, 1,
			     shell, bind_cursor_shapes) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_remote_shell_v1_interface, 30,
			     shell, bind_remote_shell) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_secure_output_v1_interface, 1,
			     shell, bind_secure_output) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_stylus_v2_interface, 2,
			     shell, bind_stylus) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_stylus_tools_v1_interface, 1,
			     shell, bind_stylus_tools) == NULL)
		return -1;

	if (wl_global_create(display,
			     &zcr_vsync_feedback_v1_interface, 1,
			     shell, bind_vsync_feedback) == NULL)
		return -1;

	weston_log("exo: init done\n");

	return 0;
}
