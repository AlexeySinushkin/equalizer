# equalizer
Маскировка Youtube трафика

Видео с объяснением здесь
https://www.youtube.com/watch?v=G_AqbmhKpG8

Код для одного клиента, который описан в видео, находится на ветке
release/one_client

Сейчас же вы ходитесь на ветке где используется один TCP поток, в который упаковываются пакеты OpenVPN + Filler

## Серверная часть
после покупки VPS
Устанавливаете и настраиваете OpenVPN (есть много инструкций)
порт 1194 наружу не открываете

Сборка эквалайзера
1. Устанавливаете Cargo (для сборки из исходников на вашем VPS) 
https://www.rust-lang.org/tools/install - после установки надо будет перезайти в терминал
2. git clone https://github.com/AlexeySinushkin/equalizer equalizer
3. cd equalizer
4. cargo build --release
5. cd target/release
6. ./equalizer 12010 1194

после этого эквалайзер готов принимать входящие подключения

## Клиентская часть
На вашем линукс клиенте (можно использовать OpenWRT)
устанавливаете ssh-tunnel с двумя каналами
```
ssh -NT -L 12010:127.0.0.1:12010 vpn_server
```
Запускаете client-c - он слушает по порту 12005 и упаковывает ваши данные
перенаправляя на порт 12010
подключаетесь OpenVPN к 127.0.0.1 12005


# testing
RUST_MIN_STACK=104857600 cargo test -- --nocapture

# run as service
```
sudo cp Service/equalizer.service /etc/systemd/system/equalizer-cs.service
sudo systemctl enable equalizer-cs
sudo systemctl start equalizer-cs
```
