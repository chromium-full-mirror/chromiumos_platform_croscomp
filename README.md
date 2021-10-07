croscomp

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

	$ emerge-$BOARD bitflags bindgen nix smallvec xml-rs downcast-rs parking_lot_core lock_api owning_ref parking_lot scoped-tls pkg-config

	$ ./do-build.sh

	$ cros deploy $DUT mtdev


Running chrome under croscomp
-----------------------------

Using the CLs from

	https://chromium-review.googlesource.com/c/chromium/src/+/3211500,

build and deploy chrome to the device.  You need

	target_os = ["chromeos"]
	ozone_platform_wayland = "true"
	ozone_platform = "wayland"
	use_wayland_egl = "false"'
	use_wayland_gbm = "true"

in the gn args and then

	$ ninja -C out_$BOARD/Relase chrome

followed by

	$ deploy_chrome --build-dir=out_strongbad/Release --device=$DUT --nostartui

Then start croscomp on the device and use scripts/start-chrome.sh to
start the chrome login screen on croscomp.
