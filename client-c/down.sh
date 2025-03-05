#!/bin/sh
PATH=/usr/sbin:/usr/bin:/sbin:/bin
source /root/env.sh
WAN_DEVICE=$(uci get network.wan.device)
nft flush ruleset
ip rule del fwmark 0x2 table 200
ip route del default table 200
ip route del default dev tun1
iptables -t mangle -D PREROUTING -i br-lan -p udp --dport 1025:65535 -j MARK --set-mark 0x2
iptables -t nat -D POSTROUTING -o tun1 -j MASQUERADE
ip route del $VPN_BYPASS_IP table main
ip route del $CONTROL_PANEL_IP table main
if [[ -f /tmp/default_gateway.txt ]]; then
    # Load the gateway IP from the file
    WAN_GATEWAY=$(cat /tmp/default_gateway.txt)
    logger "Environment variables: WAN_GATEWAY=$WAN_GATEWAY WAN_DEVICE=$WAN_DEVICE"
    ip route add default via $WAN_GATEWAY dev $WAN_DEVICE table main
fi

