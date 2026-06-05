# Maintainer: slatkin@gmail.com
pkgname=mby
pkgver=0.1.0
pkgrel=1
pkgdesc="Terminal client for Emby media server"
arch=('x86_64')
url="https://github.com/slatkin/mby"
license=('MIT')
depends=('mpv' 'openssl')
source=("${pkgname}-${pkgver}-linux-x86_64.tar.gz::https://github.com/slatkin/${pkgname}/releases/download/v${pkgver}/${pkgname}-${pkgver}-linux-x86_64.tar.gz")
sha256sums=('55527ad705f0ee03c69f1a83ed05269ac92300ac50c1bb2ec353429ea3a2d798')

package() {
    cd "${pkgname}-${pkgver}"
    install -Dm755 "${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
    install -Dm644 "mby.lua" "${pkgdir}/usr/share/${pkgname}/scripts/mby.lua"
    install -Dm644 "Material-Design-Iconic-Font.ttf" \
        "${pkgdir}/usr/share/${pkgname}/fonts/Material-Design-Iconic-Font.ttf"
    install -Dm644 "config.toml" "${pkgdir}/usr/share/${pkgname}/config.toml"
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE" 2>/dev/null || true
}
