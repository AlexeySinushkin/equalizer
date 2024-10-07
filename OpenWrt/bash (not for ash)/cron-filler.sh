#!/bin/sh
run="$(ps | grep filler.sh | grep -v grep)"

if [ -z "${run}" ] ; then
    echo "starting filler"
    /root/filler.sh &
fi

