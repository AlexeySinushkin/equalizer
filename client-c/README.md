Установить все, что нужно для сборки под Linux
```
sudo apt-get install build-essential
git submodule add https://git.openwrt.org/project/libubox.git
```

В проекте используется рекомендумая OpenWrt библиотека **libubox**

На девелоперской машине
```
git submodule add https://git.openwrt.org/project/libubox.git
cd libubox
mkdir build
cd build
cmake -DBUILD_LUA=OFF -DBUILD_EXAMPLES=OFF ..
make
sudo make install
sudo ldconfig

Install the project...
-- Install configuration: ""
-- Installing: /usr/local/include/libubox/assert.h
-- Installing: /usr/local/include/libubox/avl-cmp.h
-- Installing: /usr/local/include/libubox/avl.h
-- Installing: /usr/local/include/libubox/blob.h
-- Installing: /usr/local/include/libubox/blobmsg.h
-- Installing: /usr/local/include/libubox/blobmsg_json.h
-- Installing: /usr/local/include/libubox/json_script.h
-- Installing: /usr/local/include/libubox/kvlist.h
-- Installing: /usr/local/include/libubox/list.h
-- Installing: /usr/local/include/libubox/md5.h
-- Installing: /usr/local/include/libubox/runqueue.h
-- Installing: /usr/local/include/libubox/safe_list.h
-- Installing: /usr/local/include/libubox/udebug-proto.h
-- Installing: /usr/local/include/libubox/udebug.h
-- Installing: /usr/local/include/libubox/ulog.h
-- Installing: /usr/local/include/libubox/uloop.h
-- Installing: /usr/local/include/libubox/usock.h
-- Installing: /usr/local/include/libubox/ustream.h
-- Installing: /usr/local/include/libubox/utils.h
-- Installing: /usr/local/include/libubox/vlist.h
-- Installing: /usr/local/lib/libubox.so
-- Installing: /usr/local/lib/libubox.a

```


If needed, manually copy the compiled library:
```
sudo cp libubox.so /usr/local/lib/
sudo cp *.h /usr/local/include/libubox/
sudo ldconfig  # Refresh shared libraries
```

Собрать (для локальных тестов)
```
make
```

### Для кроссплатформенной сборки (чтоб работало на OpenWRT) изучить
https://openwrt.org/docs/guide-developer/toolchain/start
- выяснить какой процессор у вашего роутера
- какая версия openwrt установлена
- собрать туллчейн https://openwrt.org/docs/guide-developer/toolchain/use-buildsystem
- вызвать make
- скопировать бинарник на роутер scp

```

cd ~/openwrt
make distclean
./scripts/feeds update -a
./scripts/feeds install -a
make menuconfig  # Reconfigure if needed
make -j4 V=s
make world
make package/libs/libubox/compile V=s
mkdir -p staging_dir/target-*/usr/lib/
cp build_dir/target-*/libubox*/libubox.so.* staging_dir/target-*/usr/lib/

```


На роутере
```
opkg update
opkg remove kmod-usb2 kmod-usb-ledtrig-usbport kmod-usb-ehci kmod-phy-ath79-usb kmod-usb-core
opkg install libubox
opkg install sshtunnel --nodeps
opkg install luci-app-sshtunnel
dropbearkey -t rsa -f id_rsa -s 2048
dropbearkey -y -f id_rsa | grep "ssh-rsa" > id_rsa.pub
opkg install openvpn-mbedtls
opkg install luci-app-openvpn
```

записать в /etc/init.d/qualizer 
```
#!/bin/sh /etc/rc.common

START=89
STOP=11

start() {        
        echo start
        # commands to launch application
        /root/equalizer-client &
}                 

stop() {          
        echo stop
        # commands to kill application 
}

```

Делаем исполняемым и ссылку чтобы загружался перед openvpn 89<90
```
chmod +x equalizer
cd ../rc.d
ln -s ../init.d/equalizer S89equalizer

```
