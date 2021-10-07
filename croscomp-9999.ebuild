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
	dev-libs/libevdev:=
	dev-libs/libinput:=
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
	emesonargs+=(
		-Dexecmem=false
		-Dbackend-default=drm
		-Dbackend-drm=true
		-Dbackend-drm-screencast-vaapi=false
		-Dbackend-fbdev=false
		-Dbackend-headless=true
		-Dbackend-rdp=false
		-Dbackend-wayland=true
		-Dbackend-x11=false
		-Dcolor-management-colord=false
		-Dcolor-management-lcms=true
		-Ddemo-clients=true
		-Ddesktop-shell-client-default=weston-desktop-shell
		-Ddoc=false
		-Dimage-jpeg=false
		-Dimage-webp=false
		-Dimage-png=true
		-Dlauncher-logind=false
		-Dpipewire=false
		-Dremoting=false
		-Drenderer-gl=true
		-Dresize-pool=true
		-Dscreenshare=true
		-Dshell-desktop=true
		-Dshell-fullscreen=false
		-Dshell-ivi=false
		-Dshell-kiosk=false
		-Dsystemd=false
		-Dtest-gl-renderer=true
		-Dtest-junit-xml=false
		-Dwcap-decode=false
		-Dweston-launch=false
	)
	export MESON_BUILD_DIR="${WORKDIR}/${P}-build"
	EMESON_SOURCE=${S}/weston
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
	# cargo doesn't know how to install cross-compiled binaries.  It will
	# always install native binaries for the host system.  Manually install
	# crosvm instead.
	local build_dir="$(cros-rust_get_build_dir)"
	dobin "${build_dir}/croscomp"

	insinto /etc/init
	doins croscomp.conf
	insinto /usr/bin
	doins croscomp-terminal

	meson_src_install
}
