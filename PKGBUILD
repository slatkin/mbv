# Maintainer: slatkin@woims.net
pkgbase=mbv
pkgname=(mbv pinwin)
pkgver=0.21.8
pkgrel=1
pkgdesc="Terminal client for Emby media server"
arch=('x86_64')
url="https://github.com/slatkin/mbv"
license=('MIT')
depends=('mpv' 'openssl' 'pipewire')
source=("${pkgbase}-${pkgver}-linux-x86_64.tar.gz::https://github.com/slatkin/${pkgbase}/releases/download/v${pkgver}/${pkgbase}-${pkgver}-linux-x86_64.tar.gz")
sha256sums=('SKIP')

package_mbv() {
    optdepends=('pinwin: open pinned from the launcher' 'xdg-terminal-exec: open in a terminal from the launcher')
    cd "${pkgbase}-${pkgver}"
    install -Dm755 "${pkgname}" "${pkgdir}/usr/bin/${pkgname}"
    install -Dm755 "mbvd" "${pkgdir}/usr/bin/mbvd"
    install -Dm644 "mbv.desktop" "${pkgdir}/usr/share/applications/${pkgname}.desktop"
    install -Dm644 "icon.svg" "${pkgdir}/usr/share/icons/hicolor/scalable/apps/${pkgname}.svg"
    install -Dm644 "mbv.lua" "${pkgdir}/usr/share/${pkgname}/scripts/mbv.lua"
    for script in mbv_*.lua; do
        install -Dm644 "${script}" "${pkgdir}/usr/share/${pkgname}/scripts/${script}"
    done
    install -Dm644 "Material-Design-Iconic-Font.ttf" \
        "${pkgdir}/usr/share/${pkgname}/fonts/Material-Design-Iconic-Font.ttf"
    install -Dm644 "config.toml" "${pkgdir}/usr/share/${pkgname}/config.toml"
    install -Dm640 "mbvd.toml" "${pkgdir}/etc/mbv/config.toml"
    install -Dm644 "mbvd.service" "${pkgdir}/usr/lib/systemd/system/mbvd.service"
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE" 2>/dev/null || true
}

package_pinwin() {
    pkgdesc="Layer-shell panel that runs a command pinned beside tiled windows"
    depends=('gtk4' 'gtk4-layer-shell' 'libdbusmenu-glib' 'pango')
    cd "${pkgbase}-${pkgver}"
    install -Dm755 "pinwin" "${pkgdir}/usr/bin/pinwin"
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE" 2>/dev/null || true
}
