#!/bin/sh

set -xe

BOARD=hatch
export SYSROOT=/build/${BOARD}

d=~/trunk/src/platform/croscomp
ECARGO_HOME=$d/cargo_home
CROS_RUST_REGISTRY_BASE="/usr/lib/cros_rust_registry"
CROS_RUST_REGISTRY_DIR="${CROS_RUST_REGISTRY_BASE}/store"
CROS_RUST_REGISTRY_INST_DIR="${CROS_RUST_REGISTRY_BASE}/registry"

CBUILD=$(portageq-${BOARD} envvar CBUILD)
CHOST=$(portageq-${BOARD} envvar CHOST)
export CARGO_HOME="${ECARGO_HOME}"
export HOST="${CBUILD}"
export HOST_CC="$HOST}-clang"

mkdir -p ${ECARGO_HOME}
cat <<- EOF > "${ECARGO_HOME}/config"
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



ninja -j 80 -C $d/x86_64

echo "cargo: Entering directory '$d'"
touch --no-create $d/src/main.rs
cargo build --manifest-path $d/Cargo.toml --target="${CHOST}"
