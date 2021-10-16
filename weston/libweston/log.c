/*
 * Copyright © 2012 Martin Minarik
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

#include <stdio.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>
#include <ctype.h>

#include <wayland-util.h>

#include <libweston/libweston.h>
#include "weston-log-internal.h"

static void (*log_handler)(const char *);

WL_EXPORT void
weston_set_log_handler(void (*handler)(const char *))
{
	log_handler = handler;
}

/** weston_vlog calls log_handler
 * \ingroup wlog
 */
WL_EXPORT int
weston_vlog(const char *fmt, va_list ap)
{
	char *msg;
	int len, skip;

	if (vasprintf(&msg, fmt, ap) == -1)
		return -1;

	len = strlen(msg);

	if (msg[len - 1] == '\n')
		msg[len - 1] = '\0';

	for (skip = 0; isspace(msg[skip]); skip++)
		;

	log_handler(msg + skip);

	free(msg);

	return 0;
}

/** printf() equivalent in weston compositor.
 *
 * \rststar
 * 	.. note::
 *
 * 		Needs :var:`log_handler` to be set-up!
 * \endrststar
 *
 * \ingroup wlog
 */
WL_EXPORT int
weston_log(const char *fmt, ...)
{
	int l;
	va_list argp;

	va_start(argp, fmt);
	l = weston_vlog(fmt, argp);
	va_end(argp);

	return l;
}

/** weston_vlog_continue calls log_continue_handler
 *
 * \ingroup wlog
 */
WL_EXPORT int
weston_vlog_continue(const char *fmt, va_list argp)
{
	return weston_vlog(fmt, argp);
}

/** weston_log_continue
 *
 * \ingroup wlog
 */
WL_EXPORT int
weston_log_continue(const char *fmt, ...)
{
	int l;
	va_list argp;

	va_start(argp, fmt);
	l = weston_vlog_continue(fmt, argp);
	va_end(argp);

	return l;
}
