croscomp

Compositor for ChromeOS


Building

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


