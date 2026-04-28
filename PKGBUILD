# Maintainer: Jayson Lennon <jayson@jaysonlennon.dev>
pkgname=superscrub
pkgver=0.1.1
pkgrel=1
pkgdesc="Programmatic video editor"
arch=('x86_64')
url="https://github.com/jayson-lennon/superscrub"
license=('AGPL-3.0')
makedepends=('cargo' 'git')
provides=('superscrub-render' 'superscrub-editor')
source=("git+$url.git")
sha256sums=('SKIP')

build() {
    cd "$srcdir/superscrub"
    cargo build --release --bin superscrub-render --bin superscrub-editor --config profile.release.debug=false
}

package() {
    cd "$srcdir/superscrub"
    install -Dm755 "target/release/superscrub-render" "$pkgdir/usr/bin/superscrub-render"
    install -Dm755 "target/release/superscrub-editor" "$pkgdir/usr/bin/superscrub-editor"
}
