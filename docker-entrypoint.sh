#!/bin/sh
set -e

PUID=${PUID:-1000}
PGID=${PGID:-1000}
USERNAME=appuser
GROUPNAME=appgroup

# Check if group with PGID exists
existing_group=$(getent group "$PGID" | cut -d: -f1)
if [ -z "$existing_group" ]; then
    addgroup -g "$PGID" "$GROUPNAME"
else
    GROUPNAME="$existing_group"
fi

# Check if user exists, if not, create with correct UID and group
if ! id "$USERNAME" >/dev/null 2>&1; then
    adduser -D -H -u "$PUID" -G "$GROUPNAME" "$USERNAME"
fi

echo "Switching to UID=$PUID, GID=$PGID ($USERNAME:$GROUPNAME)"
exec su-exec "$USERNAME" "$@"
