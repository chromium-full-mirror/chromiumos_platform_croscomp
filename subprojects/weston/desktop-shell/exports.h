/* Exports to rust */

#include <stdarg.h>

struct weston_compositor;

int vlog(const char *fmt, va_list ap);
int vlog_continue(const char *fmt, va_list argp);

int desktop_shell_init(struct weston_compositor *ec);
