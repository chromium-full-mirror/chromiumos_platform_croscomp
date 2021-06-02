croscomp

Compositor for ChromeOS


Building

Building in the CrOS SDK

$ export BOARD=strongbad

$ setup_board --board=$BOARD

C deps:

$ emerge-$BOARD libxkbcommon wayland pixman libdrm mtdev wayland-protocols cairo mesa-freedreno minigbm lcms pango opengles-headers

Rust deps:

$ emerge-$BOARD bitflags bindgen nix smallvec xml-rs downcast-rs parking_lot_core lock_api owning_ref parking_lot scoped-tls pkg-config

$ ./do-build.sh

$ cros deploy $DUT mtdev
