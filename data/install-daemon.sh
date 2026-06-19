#!/bin/sh
# Install (or uninstall) the myasusd privileged D-Bus daemon system-side.
#
# This is the MANUAL install path (no distro package, no meson). It mirrors what
# the meson build installs, rendering the same .in service templates so the unit
# and the D-Bus activation file point at the real binary location. Files default
# under /usr/local; set LIBEXEC=/usr/libexec for a /usr layout. Everything is
# reversible: `./install-daemon.sh uninstall` removes every file it placed.
#
# Run as root.
set -eu

LIBEXEC=${LIBEXEC:-/usr/local/libexec}
BIN="$LIBEXEC/myasusd"
DBUS_CONF=/usr/share/dbus-1/system.d/io.github.berkmuftuoglu.MyAsus4Linux.Helper.conf
DBUS_SVC=/usr/share/dbus-1/system-services/io.github.berkmuftuoglu.MyAsus4Linux.Helper.service
POLKIT=/usr/share/polkit-1/actions/io.github.berkmuftuoglu.MyAsus4Linux.Helper.policy
UNIT=/etc/systemd/system/myasusd.service
UDEV=/usr/lib/udev/rules.d/99-myasus4linux-backlight.rules

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)

# Render an @libexecdir@ template into place with the chosen libexec dir.
install_rendered() {
    src="$1"
    dest="$2"
    tmp=$(mktemp)
    sed "s|@libexecdir@|$LIBEXEC|g" "$src" >"$tmp"
    install -Dm644 "$tmp" "$dest"
    rm -f "$tmp"
}

uninstall() {
    systemctl stop myasusd.service 2>/dev/null || true
    rm -f "$BIN" "$DBUS_CONF" "$DBUS_SVC" "$POLKIT" "$UNIT" "$UDEV"
    systemctl daemon-reload 2>/dev/null || true
    echo "uninstalled myasusd"
}

do_install() {
    install -Dm755 "$root/target/release/myasusd" "$BIN"
    install -Dm644 "$here/io.github.berkmuftuoglu.MyAsus4Linux.Helper.conf" "$DBUS_CONF"
    install_rendered "$here/io.github.berkmuftuoglu.MyAsus4Linux.Helper.service.in" "$DBUS_SVC"
    install -Dm644 "$root/polkit/io.github.berkmuftuoglu.MyAsus4Linux.Helper.policy" "$POLKIT"
    install_rendered "$here/myasusd.service.in" "$UNIT"
    install -Dm644 "$here/99-myasus4linux-backlight.rules" "$UDEV"
    systemctl daemon-reload
    echo "installed myasusd (D-Bus activated; starts on first call)"
}

case "${1:-install}" in
    uninstall) uninstall ;;
    install) do_install ;;
    *) echo "usage: $0 [install|uninstall]" >&2; exit 1 ;;
esac
