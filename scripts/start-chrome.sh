#!/bin/sh

export XDG_RUNTIME_DIR=/run/chrome
export WAYLAND_DISPLAY=wayland-0

sudo -E -u chronos /opt/google/chrome/chrome --use-gl=egl --enable-native-gpu-memory-buffers \
	--no-sandbox \
	--enable-logging  --user-data-dir=/home/chronos \
	--login-profile=user   --enable-crashpad \
	--login-manager  --ozone-platform=wayland "$@"
