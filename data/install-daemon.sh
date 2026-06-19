#!/bin/sh
# Install (or uninstall) the myasusd privileged D-Bus daemon system-side.
#
# The daemon is the only thing that writes the ASUS hardware controls. Installing
# it removes the need for the old world/group-writable udev rule. Everything here
# is reversible: `./install-daemon.sh uninstall` removes every file it placed.
#
# Run as root.
set -eu

BIN=/usr/local/libexec/myasusd
DBUS_CONF=/usr/share/dbus-1/system.d/io.github.berkmuftuoglu.MyAsus4Linux.Helper.conf
DBUS_SVC=/usr/share/dbus-1/system-services/io.github.berkmuftuoglu.MyAsus4Linux.Helper.service
POLKIT=/usr/share/polkit-1/actions/io.github.berkmuftuoglu.MyAsus4Linux.Helper.policy
UNIT=/etc/systemd/system/myasusd.service

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)

uninstall() {
    systemctl stop myasusd.service 2>/dev/null || true
    rm -f "$BIN" "$DBUS_CONF" "$DBUS_SVC" "$POLKIT" "$UNIT"
    systemctl daemon-reload 2>/dev/null || true
    echo "uninstalled myasusd"
}

install() {
    install -Dm755 "$root/target/release/myasusd" "$BIN"
    install -Dm644 "$here/io.github.berkmuftuoglu.MyAsus4Linux.Helper.conf" "$DBUS_CONF"
    install -Dm644 "$here/io.github.berkmuftuoglu.MyAsus4Linux.Helper.service" "$DBUS_SVC"
    install -Dm644 "$root/polkit/io.github.berkmuftuoglu.MyAsus4Linux.Helper.policy" "$POLKIT"
    install -Dm644 "$here/myasusd.service" "$UNIT"
    systemctl daemon-reload
    echo "installed myasusd (D-Bus activated; starts on first call)"
}

case "${1:-install}" in
    uninstall) uninstall ;;
    install) install ;;
    *) echo "usage: $0 [install|uninstall]" >&2; exit 1 ;;
esac
