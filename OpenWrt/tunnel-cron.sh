#!/bin/sh
run="$(ps | grep tunnel.sh | grep -v grep)"

if [ -z "${run}" ] ; then
    echo "starting tunnel"
    ./tunnel.sh &
fi

