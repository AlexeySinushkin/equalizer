Установить все, что нужно для сборки под Linux
```
sudo apt-get install build-essential
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