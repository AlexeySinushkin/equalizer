#!/bin/sh
PATH=/usr/sbin:/usr/bin:/sbin:/bin
nft flush ruleset
ip rule del fwmark 0x2 table 200
ip route del default table 200
ip route del default dev tun0
iptables -t mangle -D PREROUTING -i br-lan -p udp --dport 1025:65535 -j MARK --set-mark 0x2
iptables -t nat -D POSTROUTING -o tun0 -j MASQUERADE