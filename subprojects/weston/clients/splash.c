/*
 * Copyright © 2011 Benjamin Franzke
 * Copyright © 2010 Intel Corporation
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

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <assert.h>
#include <unistd.h>
#include <sys/mman.h>
#include <sys/ioctl.h>
#include <signal.h>
#include <errno.h>
#include <linux/dma-buf.h>
#include <linux/udmabuf.h>
#include <drm/drm_fourcc.h>
#include <fcntl.h>

#include <wayland-client.h>
#include "shared/os-compatibility.h"
#include "shared/helpers.h"
#include "shared/image-loader.h"
#include <libweston/zalloc.h>

#include "croscomp-client-protocol.h"
#include "linux-dmabuf-unstable-v1-client-protocol.h"

struct display {
	struct wl_display *display;
	struct wl_registry *registry;
	struct wl_compositor *compositor;
	struct zwp_linux_dmabuf_v1 *linux_dmabuf;
	struct croscomp_splash *splash;
	bool has_xrgb;

	int udmabuf_fd;

	pixman_image_t *frames[32];
	uint32_t frame_count;
	uint32_t current_frame;
	uint32_t width, height;

	bool terminate;
};

struct buffer {
	struct wl_buffer *buffer;
	uint32_t stride;
	void *data;
	int busy;
};

struct window {
	struct display *display;
	int width, height;
	struct wl_surface *surface;
	struct buffer buffers[3];
	struct buffer *prev_buffer;
	struct wl_callback *callback;
	bool wait_for_configure;
};

static void
redraw(void *data, struct wl_callback *callback, uint32_t time);

static void
buffer_release(void *data, struct wl_buffer *buffer)
{
	struct buffer *mybuf = data;

	mybuf->busy = 0;
}

static const struct wl_buffer_listener buffer_listener = {
	buffer_release
};

static int
create_dma_buf(struct display *display, struct buffer *buffer,
	       int width, int height, uint32_t format)
{
	int fd, ret, size, stride;
	void *data;

	/* We align the stride to 256, in best effort to allocate a
	 * linear layout dma-buf that can be scanned out.
	 */
	stride = (width * 4 + 255) & ~255;
	size = stride * height;

	fd = memfd_create("weston-shared", MFD_CLOEXEC | MFD_ALLOW_SEALING);
	if (fd < 0) {
		fprintf(stderr, "memfd_create failed: %s\n", strerror(errno));
		return -1;
	}

	/* We can add this seal before calling posix_fallocate(), as
	 * the file is currently zero-sized anyway.
	 *
	 * There is also no need to check for the return value, we
	 * couldn't do anything with it anyway.
	 */
	fcntl(fd, F_ADD_SEALS, F_SEAL_SHRINK);
	ret = ftruncate(fd, size);
	if (fd < 0) {
		fprintf(stderr, "ftruncate failed: %s\n", strerror(errno));
		return -1;
	}

	struct udmabuf_create create;
	create.memfd = fd;
	create.flags = UDMABUF_FLAGS_CLOEXEC;
	create.offset = 0;
	create.size = size;

	int dmabuf_fd = ioctl(display->udmabuf_fd, UDMABUF_CREATE, &create);
	if (dmabuf_fd < 0) {
		fprintf(stderr, "error creating udmabuf: %s", strerror(errno));
		return -1;
	}

	data = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
	if (data == MAP_FAILED) {
		fprintf(stderr, "mmap failed: %s\n", strerror(errno));
		close(fd);
		return -1;
	}

	struct zwp_linux_buffer_params_v1 *params =
		zwp_linux_dmabuf_v1_create_params(display->linux_dmabuf);

	zwp_linux_buffer_params_v1_add(params, dmabuf_fd, 0, 0, stride, 0, 0);

	buffer->buffer = zwp_linux_buffer_params_v1_create_immed(params, width, height, format, 0);
	buffer->stride = stride;
	wl_buffer_add_listener(buffer->buffer, &buffer_listener, buffer);
	close(fd);

	zwp_linux_buffer_params_v1_destroy(params);

	buffer->data = data;

	return 0;
}

static struct window *
create_window(struct display *display, struct wl_output *output, int width, int height)
{
	struct window *window;

	window = zalloc(sizeof *window);
	if (!window)
		return NULL;

	window->callback = NULL;
	window->display = display;
	window->width = width;
	window->height = height;
	window->surface = wl_compositor_create_surface(display->compositor);

	struct wl_region *region =
		wl_compositor_create_region(display->compositor);
	wl_region_add(region, 0, 0, width, height);
	wl_surface_set_opaque_region(window->surface, region);
	wl_region_destroy(region);

	croscomp_splash_set(display->splash, output, window->surface);
	redraw(window, NULL, 0);

	return window;
}

#if 0
static void
destroy_window(struct window *window)
{
	if (window->callback)
		wl_callback_destroy(window->callback);

	if (window->buffers[0].buffer)
		wl_buffer_destroy(window->buffers[0].buffer);
	if (window->buffers[1].buffer)
		wl_buffer_destroy(window->buffers[1].buffer);

	wl_surface_destroy(window->surface);
	free(window);
}
#endif

static struct buffer *
window_next_buffer(struct window *window)
{
	struct buffer *buffer;
	int ret = 0, i;

	for (i = 0; i < ARRAY_LENGTH(window->buffers); i++) {
		if (!window->buffers[i].busy)
			break;
	}

	if (i == ARRAY_LENGTH(window->buffers))
		return NULL;

	buffer = &window->buffers[i];
	if (!buffer->buffer) {
		ret = create_dma_buf(window->display, buffer,
				     window->width, window->height,
				     DRM_FORMAT_ARGB8888);

		if (ret < 0)
			return NULL;

		pixman_image_t *dst = pixman_image_create_bits(PIXMAN_a8r8g8b8,
							       window->width, window->height,
							       buffer->data,
							       buffer->stride);
		pixman_rectangle16_t rects = { 0, 0, window->width, window->height };
		pixman_color_t color = { 0xfefe, 0xfefe, 0xfefe, 0xffff };

		pixman_image_fill_rectangles(PIXMAN_OP_SRC, dst, &color, 1, &rects);
		pixman_image_unref(dst);
	}

	return buffer;
}

static const struct wl_callback_listener frame_listener;

static void
redraw(void *data, struct wl_callback *callback, uint32_t time)
{
	struct window *window = data;
	struct display *display = window->display;
	struct buffer *buffer;

	buffer = window_next_buffer(window);
	if (!buffer) {
		fprintf(stderr,
			!callback ? "Failed to create the first buffer.\n" :
			"Both buffers busy at redraw(). Server bug?\n");
		abort();
	}

	uint32_t frame = display->current_frame / 1;
	display->current_frame++;

	pixman_image_t *src = display->frames[frame];
	pixman_image_t *dst = pixman_image_create_bits(PIXMAN_a8r8g8b8,
						       window->width, window->height,
						       buffer->data,
						       buffer->stride);

	uint32_t width = pixman_image_get_width(src);
	uint32_t height = pixman_image_get_height(src);
	uint32_t x = (window->width - width) / 2;
	uint32_t y = (window->height - height) / 2;

	pixman_image_composite(PIXMAN_OP_SRC,
			       src,
			       NULL,
			       dst,
			       0, 0,
			       0, 0,
			       x, y,
			       width, height);

	pixman_image_unref(dst);

	wl_surface_attach(window->surface, buffer->buffer, 0, 0);
	wl_surface_damage(window->surface, x, y, width, height);

	if (callback)
		wl_callback_destroy(callback);

	if (frame < display->frame_count - 1) {
		window->callback = wl_surface_frame(window->surface);
		wl_callback_add_listener(window->callback, &frame_listener, window);
	} else {
		display->terminate = true;
	}

	wl_surface_commit(window->surface);
	buffer->busy = 1;
}

static const struct wl_callback_listener frame_listener = {
	redraw
};

static void
output_handle_geometry(void *data,
                       struct wl_output *wl_output,
                       int x, int y,
                       int physical_width,
                       int physical_height,
                       int subpixel,
                       const char *make,
                       const char *model,
                       int transform)
{
}

static void
output_handle_mode(void *data,
		   struct wl_output *wl_output,
		   uint32_t flags,
		   int width,
		   int height,
		   int refresh)
{
	struct display *display = data;

	if (flags & WL_OUTPUT_MODE_CURRENT)
		create_window(display, wl_output, width, height);
}

static void
output_handle_done(void *data,
                   struct wl_output *wl_output)
{
}

static void
output_handle_scale(void *data,
                    struct wl_output *wl_output,
                    int32_t scale)
{
}

static const struct wl_output_listener output_listener = {
	output_handle_geometry,
	output_handle_mode,
	output_handle_done,
	output_handle_scale
};

static void
registry_handle_global(void *data, struct wl_registry *registry,
		       uint32_t id, const char *interface, uint32_t version)
{
	struct display *d = data;

	if (strcmp(interface, "wl_compositor") == 0) {
		d->compositor =
			wl_registry_bind(registry,
					 id, &wl_compositor_interface, 1);
	} else if (strcmp(interface, "croscomp_splash") == 0) {
		d->splash = wl_registry_bind(registry,
					      id, &croscomp_splash_interface, 1);
	} else if (strcmp(interface, "zwp_linux_dmabuf_v1") == 0) {
		d->linux_dmabuf = wl_registry_bind(registry,
						   id, &zwp_linux_dmabuf_v1_interface, 3);
	} else if (strcmp(interface, "wl_output") == 0) {
		struct wl_output *output=
			wl_registry_bind(registry,
					 id, &wl_output_interface, 1);
		wl_output_add_listener(output, &output_listener, d);
	}
}

static void
registry_handle_global_remove(void *data, struct wl_registry *registry,
			      uint32_t name)
{
}

static const struct wl_registry_listener registry_listener = {
	registry_handle_global,
	registry_handle_global_remove
};

static struct display *
create_display(void)
{
	struct display *display;

	display = malloc(sizeof *display);
	if (display == NULL) {
		fprintf(stderr, "out of memory\n");
		exit(1);
	}
	display->display = wl_display_connect(NULL);
	assert(display->display);

	display->udmabuf_fd = open("/dev/udmabuf", O_RDWR);
	if (display->udmabuf_fd == -1) {
		fprintf(stderr, "could not open /dev/udmabuf: %s\n", strerror(errno));
		exit(1);
	}

	for (uint32_t i = 0; i < ARRAY_LENGTH(display->frames); i++) {
		char path[256];

		snprintf(path, sizeof(path),
			 "/usr/share/chromeos-assets/images_200_percent"
			 "/boot_splash_frame%02d.png", i + 1);

		display->frames[i] = load_image(path);
		if (display->frames[i] == NULL)
			break;
		display->frame_count = i + 1;
	}

	display->has_xrgb = false;
	display->registry = wl_display_get_registry(display->display);
	wl_registry_add_listener(display->registry,
				 &registry_listener, display);
	wl_display_roundtrip(display->display);

	if (display->linux_dmabuf == NULL) {
		fprintf(stderr, "No zwp_linux_dmabuf_v1 global\n");
		exit(1);
	}

	if (display->splash == NULL) {
		fprintf(stderr, "No croscomp_splash global\n");
		exit(1);
	}

	if (display->compositor == NULL) {
		fprintf(stderr, "No compositor global\n");
		exit(1);
	}

	wl_display_roundtrip(display->display);

	return display;
}

static void
destroy_display(struct display *display)
{
	zwp_linux_dmabuf_v1_destroy(display->linux_dmabuf);
	wl_compositor_destroy(display->compositor);
	wl_registry_destroy(display->registry);
	wl_display_flush(display->display);
	wl_display_disconnect(display->display);
	free(display);
}

int
main(int argc, char **argv)
{
	struct display *display;
	int ret = 0;

	display = create_display();

	while (!display->terminate && ret != -1)
		ret = wl_display_dispatch(display->display);

	destroy_display(display);

	return 0;
}
