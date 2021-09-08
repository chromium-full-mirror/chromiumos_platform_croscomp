# Copyright 1999-2010 Gentoo Foundation
# Distributed under the terms of the GNU General Public License v2

EAPI=7

CROS_WORKON_PROJECT="chromiumos/platform/croscomp"
CROS_WORKON_LOCALNAME="platform/croscomp"
CROS_WORKON_INCREMENTAL_BUILD=1

KEYWORDS="~*"

inherit base meson cros-rust flag-o-matic cros-workon

DESCRIPTION="ChromeOS System Compositor"

LICENSE="MIT SGI-B-2.0"

IUSE="debug vulkan"

COMMON_DEPEND="
	x11-libs/cairo:=
	sys-libs/mtdev:=
	x11-libs/libxkbcommon:=
	x11-libs/pixman:=
	x11-libs/pango:=
	dev-libs/wayland:=
	dev-libs/expat:=
	media-libs/lcms:=
	>=x11-libs/libdrm-2.4.94:=
"

RDEPEND="${COMMON_DEPEND}
"

DEPEND="${COMMON_DEPEND}
	virtual/bindgen:=
	>=dev-rust/libc-0.2.44:= <dev-rust/libc-0.3.0
	=dev-rust/cfg-if-1.0.0:=
	>=dev-rust/parking_lot-0.7.1:=
	>=dev-rust/scoped-tls-0.1.0:=
"

BDEPEND="
	virtual/pkgconfig
	dev-libs/wayland-protocols:=
"

src_unpack() {
	# Unpack both the project and dependency source code
	cros-workon_src_unpack
	cros-rust_src_unpack
}

src_prepare() {
	cros-rust_src_prepare
	default
}

src_configure() {
	export MESON_BUILD_DIR="${WORKDIR}/${P}-build"
	meson_src_configure
	cros-rust_src_configure
}

src_compile() {
	export MESON_BUILD_DIR="${WORKDIR}/${P}-build"
	meson_src_compile
	env | grep build
	ecargo_build -v \
		-p croscomp \
		|| die "cargo build failed"
}

src_install() {
	meson_src_install
}
