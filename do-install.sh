#!/bin/sh

# Take this from environment but default to hatch and my IP of the day
DUT=${DUT:-192.168.86.226}

d=~/trunk/src/platform/croscomp

rsync -av --progress $d/target/x86_64-cros-linux-gnu/debug/croscomp $DUT:

rsync -av --progress $d/x86_64/subprojects/weston/libweston/libweston-9.so* \
      $DUT:/usr/lib64
rsync -av --progress $d/x86_64/subprojects/weston/libweston/renderer-gl/gl-renderer.so \
      $DUT:/usr/lib64/libweston-9


