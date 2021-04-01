#!/bin/sh

export SYSROOT=/build/hatch

d=~/trunk/src/platform/croscomp
dut=192.168.86.140

rsync -av --progress $d/target/debug/croscomp $dut:

rsync -av --progress $d/x86_64/subprojects/weston/libweston/libweston-9.so* \
      $dut:/usr/lib64
rsync -av --progress $d/x86_64/subprojects/weston/libweston/renderer-gl/gl-renderer.so \
      $dut:/usr/lib64/libweston-9


