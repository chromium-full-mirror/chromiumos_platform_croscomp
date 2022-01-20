/* This file includes all the header files we want to ffi to and is
 * what we run bindgen on.  Having this one header include all the
 * things means that we avoid duplicate and clashing binding code when
 * some of these files end up including the same header file.
 */

#include "include/libweston/backend-drm.h"
#include "include/libweston/backend-wayland.h"
#include "include/libweston/windowed-output-api.h"
