#!/bin/sh
#Get the WAN gateway dynamically
PATH=/usr/sbin:/usr/bin:/sbin:/bin
source /root/env.sh
WAN_DEVICE=$(uci get network.wan.device)
WAN_GATEWAY=$(ip route list | grep default | awk '{print $3}')
logger "Writing to temp $WAN_GATEWAY"
echo "$WAN_GATEWAY" > /tmp/default_gateway.txt
TUN_PTP_GATEWAY=$(ip -4 addr show tun1 | awk '/peer/ {print $4}' | cut -d'/' -f1)
logger "Environment variables: VPN_BYPASS_IP=$VPN_BYPASS_IP WAN_GATEWAY=$WAN_GATEWAY WAN_DEVICE=$WAN_DEVICE TUN_PTP_GATEWAY=$TUN_PTP_GATEWAY"
# Ensure valid WAN gateway
if [ -n "$WAN_GATEWAY" ]; then    
    nft flush ruleset
	# Add route to bypass tun1 for openvpn server ip
    logger "step 1"
    ip route add $VPN_BYPASS_IP via $WAN_GATEWAY dev $WAN_DEVICE table main
    ip route add $CONTROL_PANEL_IP via $WAN_GATEWAY dev $WAN_DEVICE table main
    logger "step 2"
    nft add table ip nat
    nft add chain ip nat postrouting { type nat hook postrouting priority srcnat \; }
    nft add rule ip nat postrouting oif tun1 masquerade
    logger "step 3"
    #iptables -t mangle -A PREROUTING -i br-lan -p udp --dport 1025:65535 -j MARK --set-mark 0x2
    nft add table ip mangle
    nft add chain ip mangle prerouting { type filter hook prerouting priority -150 \; }
    nft add rule ip mangle prerouting iifname "br-lan" udp dport 1025-65535 mark set 0x2
    ip rule add fwmark 0x2 table 200  # eth0.2 Table
    nft add table ip nat
    nft add chain ip nat postrouting { type nat hook postrouting priority srcnat \; }
    nft add rule ip nat postrouting mark 2 oif $WAN_DEVICE masquerade
    logger "step 4"
    ip route add default via $WAN_GATEWAY dev $WAN_DEVICE table 200
    ip route del default table main
    ip route add default via $TUN_PTP_GATEWAY dev tun1
    logger "Bypassing VPN for $VPN_BYPASS_IP via $WAN_DEVICE"
else
    logger "Failed to set bypass route!"
fi
