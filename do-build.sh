#!/bin/bash

set -e

# Take board from environment but default to hatch
export BOARD=${BOARD:-hatch}

d=$(dirname $(readlink -f $0))

shopt -s lastpipe
emerge-$BOARD --info --verbose 2>/dev/null | while IFS=\= read var val; do
    case $var in
	PKG_CONFIG|CHOST|CBUILD|ABI|SYSROOT|CFLAGS|LDFLAGS|CPPFLAGS|AR|AS|CC|CXX|STRIP|LIBDIR_*)
	    eval export "$var=$val";;
    esac
done
eval export LIBDIR="\$LIBDIR_$ABI"

create_cross_file() {
    inherit() { :; }
    EXPORT_FUNCTIONS() { :; }
    EAPI=7
    get_libdir() { echo ${LIBDIR}; }

    # meson.eclass has a read call that returns non-0 when it
    # encounters EOF
    set +e

    source ~/trunk/src/third_party/portage-stable/eclass/toolchain-funcs.eclass
    source ~/trunk/src/third_party/eclass-overlay/eclass/meson.eclass

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

cross_file="${out}/meson.${CHOST}.${ABI}.ini"
if test ! -e "${cross_file}"; then
    # Run this in a subshell to avoid all the eclass variables
    # spilling into our namespace.
    (create_cross_file $out)
fi

if test ! -d "${out}/weston"; then
    meson --cross-file $cross_file \
	--buildtype debug \
	-Ddebug=true \
	--prefix=/usr \
	--libdir=$LIBDIR \
	"${out}/weston" "${d}/weston"
fi

export PKG_CONFIG_ALLOW_CROSS=1
export CARGO_HOME="$d/target/${CHOST}/cargo_home"
mkdir -p ${CARGO_HOME}

if test ! -e "${CARGO_HOME}/config"; then
    echo create cargo config
    create_cargo_config
fi

ninja -j 80 -C $out/weston
export MESON_BUILD_DIR=$out/weston

# Print this message to help emacs compile mode figure out where the
# source is
echo "cargo: Entering directory '$d'"

echo "PKG_CONFIG: ${PKG_CONFIG}"
echo "SYSROOT: ${SYSROOT}"

cargo fmt
cargo build -v --manifest-path $d/Cargo.toml --target="${CHOST}"

# Take this from environment but default to my IP of the day
DUT=${DUT:-192.168.86.226}

ssh $DUT mkdir -p /usr/$LIBDIR/libweston-9 /usr/share/weston

rsync -lav --progress \
    $out/weston/libweston/libweston-9.so{,*[0-9]} \
    $DUT:/usr/$LIBDIR

rsync -lav --progress \
      $d/weston/data/*.png \
      $DUT:/usr/share/weston

rsync -lav --progress $out/croscomp \
      $out/weston/clients/weston-{simple-splash,terminal} \
      $DUT:/usr/bin

rsync -av --progress $d/croscomp-terminal $DUT:/usr/bin

