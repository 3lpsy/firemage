#!/bin/sh
set -eu
finish() {
    status=$?
    trap - EXIT
    set +e
    mkdir -p /firemage/output
    printf '%s\n' "$status" > /firemage/output/exit-code
    sync
    reboot -f
    while :; do sleep 3600; done
}
trap finish EXIT
if [ ! -r /proc/mounts ]; then
    mount -t proc proc /proc
fi
ensure_mount() {
    while read -r device target rest; do
        if [ "$target" = "$2" ]; then return 0; fi
    done < /proc/mounts
    mount -t "$1" "$1" "$2"
}
ensure_mount sysfs /sys
ensure_mount devtmpfs /dev
mkdir -p /firemage/input /firemage/output
mount -o ro /dev/vdb /firemage/input
if [ -f /firemage/input/firemage/network.sh ]; then
    /bin/sh /firemage/input/firemage/network.sh
fi
if [ -f /firemage/input/firemage/environment.sh ]; then
    . /firemage/input/firemage/environment.sh
fi
if [ -f /firemage/input/firemage/setup.sh ]; then
    /bin/sh /firemage/input/firemage/setup.sh
fi
if [ -f /firemage/input/user-data ]; then
    /bin/sh /firemage/input/user-data
fi
set +e
/bin/sh /firemage/input/run.sh
exit $?
