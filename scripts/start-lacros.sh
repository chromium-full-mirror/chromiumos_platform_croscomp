#!/bin/sh

export XDG_RUNTIME_DIR=/run/chrome
export WAYLAND_DISPLAY=wayland-0
export EGL_PLATFORM=surfaceless

sudo -E -u chronos python /root/mojo_connection_lacros_launcher.py -s /tmp/lacros.sock \
	/usr/local/lacros-chrome/chrome --ozone-platform=wayland \
	--user-data-dir=/usr/local/lacros-chrome/user_data \
	--enable-gpu-rasterization --enable-oop-rasterization \
	--enable-webgl-image-chromium --lang=en-US \
	--breakpad-dump-location=/usr/local/lacros-chrome/
