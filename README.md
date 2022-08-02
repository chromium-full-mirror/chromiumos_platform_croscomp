croscomp
========

Compositor for ChromeOS


Building
--------

Build using portage

	$ mkdir ~/trunk/src/third_party/chromiumos-overlay/chromeos-base/croscomp

	$ ln -s  ../../../../platform/croscomp/croscomp-9999.ebuild \
		~/trunk/src/third_party/chromiumos-overlay/chromeos-base/croscomp

	$ emerge-$BOARD croscomp

Building in the CrOS SDK

	$ export BOARD=strongbad

	$ setup_board --board=$BOARD

C deps:

	$ emerge-$BOARD libxkbcommon wayland pixman libdrm mtdev wayland-protocols cairo mesa-freedreno minigbm lcms pango opengles-headers

Rust deps:

	$ emerge-$BOARD bitflags bindgen libloading nix smallvec xml-rs downcast-rs parking_lot_core lock_api owning_ref parking_lot scoped-tls pkg-config

	$ ./do-build.sh

	$ cros deploy $DUT mtdev

Troubleshooting
---------------

You may need to specify which mtdev to deploy

	$ cros deploy $DUT sys-libs/mtdev

If croscomp doesn't run smoothly you can get more logs with:
	$ export WAYLAND_DEBUG=1 RUST_BACKTRACE=1 RUST_LOG=debug

Croscomp may be missing libinput that may require libevdev

	$ cros deploy $DUT dev-libs/libevdev
	$ cros deploy $DUT libinput

When running the ./do-build.sh script if you get the following error
	$ ninja: error: loading 'build.ninja': No such file or directory`
It means menson generation failed to generate the build.ninja file, the easiest work around is to delete weston out folder and retry the script
	$ rm /mnt/host/source/src/platform/croscomp/target/x86_64-cros-linux-gnu/debug/weston 

	$ ./do-build.sh

Running chrome under croscomp
-----------------------------

Using the CLs from

	https://chromium-review.googlesource.com/c/chromium/src/+/3272706,

build and deploy chrome to the device.  You need

	target_os = ["chromeos"]
	ozone_platform_wayland = "true"
	ozone_platform = "wayland"
	use_wayland_egl = "false"
	use_wayland_gbm = "true"

in the gn args and then

	$ ninja -C out_$BOARD/Relase chrome

followed by

	$ deploy_chrome --build-dir=out_$BOARD/Release --device=$DUT --nostartui

Make sure both ui and frecon are not running, then start croscomp remotely on the device and use scripts/start-chrome.sh to
start the chrome login screen on croscomp.
