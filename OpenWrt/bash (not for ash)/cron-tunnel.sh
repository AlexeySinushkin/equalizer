#!/bin/sh
run="$(ps | grep tunnel.sh | grep -v grep)"

if [ -z "${run}" ] ; then
    echo "starting tunnel"
    /root/tunnel.sh &
fi

