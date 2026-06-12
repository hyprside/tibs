# Maintainer: Tiago <tiago@ardos.local>

pkgname=tibs
pkgver=0.1.0_alpha
_cargo_pkgver=0.1.0-alpha
pkgrel=1
pkgdesc="Ardos OS graphical login and boot interface"
arch=('x86_64')
license=('MIT')
depends=(
	'glibc'
	'libgcc'
	'libxkbcommon'
	'mesa'
)
makedepends=('rust' 'clang' 'pkgconf' 'git')
source=()
sha256sums=()

build() {
	export CARGO_TARGET_DIR="${BUILDDIR}/cargo-target"
	cargo build \
		--manifest-path "${startdir}/Cargo.toml" \
		--release
}

package() {
	install -Dm755 \
		"${BUILDDIR}/cargo-target/release/tibs" \
		"${pkgdir}/ardos/services/tibs/tibs"
}
