/*
 * Copyright © 2008-2011 Kristian Høgsberg
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files (the
 * "Software"), to deal in the Software without restriction, including
 * without limitation the rights to use, copy, modify, merge, publish,
 * distribute, sublicense, and/or sell copies of the Software, and to
 * permit persons to whom the Software is furnished to do so, subject to
 * the following conditions:
 *
 * The above copyright notice and this permission notice (including the
 * next paragraph) shall be included in all copies or substantial
 * portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
 * EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
 * MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
 * NONINFRINGEMENT.  IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
 * BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
 * ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
 * CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

#include "config.h"

#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <linux/input.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <sys/uio.h>

#include <libweston/libweston.h>
#include "shared/helpers.h"
#include "shared/zalloc.h"
#include "shared/timespec-util.h"
#include "backend.h"
#include "libweston-internal.h"

struct screenshooter_frame_listener {
	struct wl_listener listener;
	struct weston_buffer *buffer;
	struct weston_output *output;
	weston_screenshooter_done_func_t done;
	void *data;
};

static void
copy_bgra_yflip(uint8_t *dst, uint8_t *src, int height, int stride)
{
	uint8_t *end;

	end = dst + height * stride;
	while (dst < end) {
		memcpy(dst, src, stride);
		dst += stride;
		src -= stride;
	}
}

static void
copy_bgra(uint8_t *dst, uint8_t *src, int height, int stride)
{
	/* TODO: optimize this out */
	memcpy(dst, src, height * stride);
}

static void
copy_row_swap_RB(void *vdst, void *vsrc, int bytes)
{
	uint32_t *dst = vdst;
	uint32_t *src = vsrc;
	uint32_t *end = dst + bytes / 4;

	while (dst < end) {
		uint32_t v = *src++;
		/*                    A R G B */
		uint32_t tmp = v & 0xff00ff00;
		tmp |= (v >> 16) & 0x000000ff;
		tmp |= (v << 16) & 0x00ff0000;
		*dst++ = tmp;
	}
}

static void
copy_rgba_yflip(uint8_t *dst, uint8_t *src, int height, int stride)
{
	uint8_t *end;

	end = dst + height * stride;
	while (dst < end) {
		copy_row_swap_RB(dst, src, stride);
		dst += stride;
		src -= stride;
	}
}

static void
copy_rgba(uint8_t *dst, uint8_t *src, int height, int stride)
{
	uint8_t *end;

	end = dst + height * stride;
	while (dst < end) {
		copy_row_swap_RB(dst, src, stride);
		dst += stride;
		src += stride;
	}
}

static void
screenshooter_frame_notify(struct wl_listener *listener, void *data)
{
	struct screenshooter_frame_listener *l =
		container_of(listener,
			     struct screenshooter_frame_listener, listener);
	struct weston_output *output = l->output;
	struct weston_compositor *compositor = output->compositor;
	int32_t stride;
	uint8_t *pixels, *d, *s;

	weston_output_disable_planes_decr(output);
	wl_list_remove(&listener->link);
	stride = l->buffer->width * (PIXMAN_FORMAT_BPP(compositor->read_format) / 8);
	pixels = malloc(stride * l->buffer->height);

	if (pixels == NULL) {
		l->done(l->data, WESTON_SCREENSHOOTER_NO_MEMORY);
		free(l);
		return;
	}

	compositor->renderer->read_pixels(output,
			     compositor->read_format, pixels,
			     0, 0, output->current_mode->width,
			     output->current_mode->height);

	stride = wl_shm_buffer_get_stride(l->buffer->shm_buffer);

	d = wl_shm_buffer_get_data(l->buffer->shm_buffer);
	s = pixels + stride * (l->buffer->height - 1);

	wl_shm_buffer_begin_access(l->buffer->shm_buffer);

	switch (compositor->read_format) {
	case PIXMAN_a8r8g8b8:
	case PIXMAN_x8r8g8b8:
		if (compositor->capabilities & WESTON_CAP_CAPTURE_YFLIP)
			copy_bgra_yflip(d, s, output->current_mode->height, stride);
		else
			copy_bgra(d, pixels, output->current_mode->height, stride);
		break;
	case PIXMAN_x8b8g8r8:
	case PIXMAN_a8b8g8r8:
		if (compositor->capabilities & WESTON_CAP_CAPTURE_YFLIP)
			copy_rgba_yflip(d, s, output->current_mode->height, stride);
		else
			copy_rgba(d, pixels, output->current_mode->height, stride);
		break;
	default:
		break;
	}

	wl_shm_buffer_end_access(l->buffer->shm_buffer);

	l->done(l->data, WESTON_SCREENSHOOTER_SUCCESS);
	free(pixels);
	free(l);
}

WL_EXPORT int
weston_screenshooter_shoot(struct weston_output *output,
			   struct weston_buffer *buffer,
			   weston_screenshooter_done_func_t done, void *data)
{
	struct screenshooter_frame_listener *l;

	if (!wl_shm_buffer_get(buffer->resource)) {
		done(data, WESTON_SCREENSHOOTER_BAD_BUFFER);
		return -1;
	}

	buffer->shm_buffer = wl_shm_buffer_get(buffer->resource);
	buffer->width = wl_shm_buffer_get_width(buffer->shm_buffer);
	buffer->height = wl_shm_buffer_get_height(buffer->shm_buffer);

	if (buffer->width < output->current_mode->width ||
	    buffer->height < output->current_mode->height) {
		done(data, WESTON_SCREENSHOOTER_BAD_BUFFER);
		return -1;
	}

	l = malloc(sizeof *l);
	if (l == NULL) {
		done(data, WESTON_SCREENSHOOTER_NO_MEMORY);
		return -1;
	}

	l->buffer = buffer;
	l->output = output;
	l->done = done;
	l->data = data;
	l->listener.notify = screenshooter_frame_notify;
	wl_signal_add(&output->frame_signal, &l->listener);
	weston_output_disable_planes_incr(output);
	weston_output_damage(output);

	return 0;
}
