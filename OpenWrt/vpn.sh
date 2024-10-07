#!/bin/sh
#crontab -e -> */2 * * * * /root/vpn.sh
ssh -NT -L 11196:127.0.0.1:11196 -L 11194:127.0.0.1:11194 doisp -o ExitOnForwardFailure=yes && nc 127.0.0.1 11196 > /dev/null &