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

#include "compositor/weston.h"
#include <libweston/libweston.h>
#include "shared/zalloc.h"
#include "shared/helpers.h"
#include "shell.h"

#include "croscomp-server-protocol.h"

static void
destroy_output(struct wl_resource *resource)
{
}

static void
send_output(struct wl_resource *display_resource, struct weston_head *head)
{
	struct weston_output *output = head->output;
	struct wl_resource *resource;
	struct weston_mode *mode;

	weston_log("send output\n");
	resource =
		wl_resource_create(wl_resource_get_client(display_resource),
				   &croscomp_output_interface,
				   wl_resource_get_version(display_resource), 0);
	weston_log("new resource %p\n", resource);

	if (resource == NULL)
		return;

	wl_resource_set_implementation(resource, &croscomp_output_interface,
				       NULL, destroy_output);
	croscomp_display_send_output(display_resource, resource);

	croscomp_output_send_geometry(resource,
				      head->mm_width,
				      head->mm_height,
				      head->subpixel,
				      head->make,
				      head->model,
				      head->transform);

	if (output && output->enabled) {
		wl_list_for_each(mode, &output->mode_list, link) {
			croscomp_output_send_mode(resource,
						  mode->flags,
						  mode->width,
						  mode->height,
						  mode->refresh);
		}
	}

	croscomp_output_send_done(resource);

}

static void display_destroy(struct wl_client *client,
			    struct wl_resource *resource)
{
}

static const struct croscomp_display_interface croscomp_display_implementation = {
	.destroy = display_destroy
};

static void
unbind_display(struct wl_resource *resource)
{
}

static void
bind_display(struct wl_client *client,
	     void *data, uint32_t version, uint32_t id)
{
	struct desktop_shell *shell = data;
	struct wl_resource *resource;
	struct weston_compositor *compositor = shell->compositor;
	struct weston_head *head;

	resource = wl_resource_create(client, &croscomp_display_interface,
				      1, id);

	wl_resource_set_implementation(resource,
				       &croscomp_display_implementation,
				       shell, unbind_display);

	wl_list_for_each(head, &compositor->head_list, compositor_link) {
		if (head->output && head->output->enabled)
			send_output(resource, head);
	}
}

int
init_croscomp_display(struct desktop_shell *shell)
{
	struct weston_compositor *ec = shell->compositor;

	if (wl_global_create(ec->wl_display,
			     &croscomp_display_interface, 1,
			     shell, bind_display) == NULL)
		return -1;

	return 0;
}
