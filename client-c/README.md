Установить все, что нужно для сборки под Linux
```
sudo apt-get install build-essential
git submodule add https://git.openwrt.org/project/libubox.git
```

В проекте используется рекомендумая OpenWrt библиотека
```
git submodule add https://git.openwrt.org/project/libubox.git
```

Собрать (для локальных тестов)
```
make
```

Для кроссплатформенной сборки (чтоб работало на OpenWRT) изучить
https://openwrt.org/docs/guide-developer/toolchain/start
- выяснить какой процессор у вашего роутера
- какая версия openwrt установлена
- собрать туллчейн https://openwrt.org/docs/guide-developer/toolchain/use-buildsystem
- вызвать make
- скопировать бинарник на роутер scp

```
make CC=mipsel-openwrt-linux-musl-gcc LD=mipsel-openwrt-linux-musl-ld
```

