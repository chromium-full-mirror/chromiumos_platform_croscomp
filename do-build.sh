#!/bin/bash

set -xe

# Take board from environment but default to hatch
export BOARD=${BOARD:-hatch}

d=$(dirname $(readlink -f $0))

shopt -s lastpipe
emerge-$BOARD --info --verbose 2>/dev/null | while IFS=\= read var val; do
    case $var in
	CHOST|CBUILD|ABI|SYSROOT|CFLAGS|LDFLAGS|CPPFLAGS|AR|AS|CC|CXX|STRIP|LIBDIR_*)
	    eval export "$var=$val";;
    esac
done
eval export LIBDIR="\$LIBDIR_$ABI"

create_cross_file() {
    ECLASS_PATH=~/trunk/src/third_party/portage-stable/eclass

    inherit() { :; }
    EXPORT_FUNCTIONS() { :; }
    EAPI=7

    # meson.eclass has a read call that returns non-0 when it
    # encounters EOF
    set +e

    source ${ECLASS_PATH}/toolchain-funcs.eclass
    source ${ECLASS_PATH}/meson.eclass

    T=$1
    mkdir -p $T

    _meson_create_cross_file
}

create_cargo_config() {
    CROS_RUST_REGISTRY_BASE="/usr/lib/cros_rust_registry"
    CROS_RUST_REGISTRY_DIR="${CROS_RUST_REGISTRY_BASE}/store"
    CROS_RUST_REGISTRY_INST_DIR="${CROS_RUST_REGISTRY_BASE}/registry"

    cat <<- EOF > "${CARGO_HOME}/config"
[source.chromeos]
directory = "${SYSROOT}${CROS_RUST_REGISTRY_INST_DIR}"

[source.crates-io]
replace-with = "chromeos"
local-registry = "/nonexistent"

[target.${CHOST}]
linker = "${CHOST}-clang"

[target.${CBUILD}]
linker = "${CBUILD}-clang"
EOF
}

out=$d/target/$CHOST/debug

cross_file="${out}/meson.${CHOST}.${ABI}"
if test ! -e "${cross_file}"; then
    # Run this in a subshell to avoid all the eclass variable spilling
    # into out namespace.
    (create_cross_file $out)
fi

if test ! -d "${out}/meson"; then
    meson --cross-file $cross_file \
	   "${out}/meson" --buildtype debug -D debug=true --prefix=/usr --libdir=$LIBDIR

fi

export PKG_CONFIG_ALLOW_CROSS=1
export CARGO_HOME="$d/cargo_home"
mkdir -p ${CARGO_HOME}

create_cargo_config

ninja -j 80 -C $out/meson

# Print this message to help emacs compile mode figure out where the
# source is
echo "cargo: Entering directory '$d'"

cargo build --manifest-path $d/Cargo.toml --target="${CHOST}"

# Take this from environment but default to hatch and my IP of the day
DUT=${DUT:-192.168.86.226}

rsync -av --progress $out/croscomp $DUT:

rsync -av --progress $out/meson/subprojects/weston/libweston/libweston-9.so{,*.0} \
      $DUT:/usr/lib64
rsync -av --progress $out/meson/subprojects/weston/libweston/renderer-gl/gl-renderer.so \
      $DUT:/usr/lib64/libweston-9
rsync -av --progress $out/meson/subprojects/weston/clients/weston-{simple-splash,terminal} \
      $DUT:/usr/bin
rsync -av --progress $d/croscomp-terminal $DUT:/usr/bin

