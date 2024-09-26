use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use crate::packet::Packet;
use crate::throttler::{Throttler, ThrottlerCommand};
use log::{info, trace};
use crate::filler::Filler;
use crate::r#const::INITIAL_SPEED;

pub struct VpnProxy {
    //Клиент к нам подключенный
    //client_stream: TcpStream,
    //Мы к VPN серверу подключены
    //up_stream: TcpStream,
    pub client_to_proxy_join: JoinHandle<()>,
    pub vpn_to_proxy_join: JoinHandle<()>,
    pub throttler_join: JoinHandle<()>,
    ct_filler: Sender<TcpStream>,
}


impl VpnProxy {
    /**
    Создаем поток по чтению запросов от клиента
    и поток внутри дросселя для отправки данных (лимитированных по скорости)
     */
    pub fn new(mut client_stream: TcpStream, mut up_stream: TcpStream) -> VpnProxy {

        //Правило именования каналов
        // ct_иточник_получатель, cr_иточник_получатель
        //канал в дроссель
        let (ct_vpn_throttler, cr_vpn_throttler) = channel();
        //дросселированные данные
        let (ct_throttler_client, cr_throttler_client) = channel();
        let throttler = Throttler::new(cr_vpn_throttler, ct_throttler_client);
        let (ct_client_vpn, cr_client_vpn) = channel();
        let (ct_filler, cr_filler) = channel();
        let timeout = Duration::from_millis(1);

        let client_to_proxy_join = thread::Builder::new()
            .name("client_stream".to_string()).spawn(move || {
            client_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");
            let filler_stream: Option<TcpStream>;
            //цикл который не использует заполнитель, а работает в режиме ожидания его появления
            loop {
                let mut possible_packet = Packet::new();//TODO брать из банки - не нагружать ОС

                //GET запрос на чтение нового видоса
                if let Ok(size) = client_stream.read(&mut possible_packet.buf) {
                    if size > 0 {
                        trace!("<< {size}");
                        possible_packet.size = size;
                        //перенаправляем его VPN серверу
                        ct_client_vpn.send(possible_packet).unwrap()
                    }
                }
                //вычитываем дросселированные пакеты (если есть)
                while let Ok(packet) = cr_throttler_client.try_recv() {
                    trace!(">> {}", packet.size);
                    client_stream.write_all(&packet.buf[..packet.size]).unwrap();
                }
                //проверяем не появился ли заполнитель
                if let Ok(filler) = cr_filler.try_recv() {
                    filler_stream = Some(filler);
                    break;
                }
            }
            //цикл который использует заполнитель
            let mut filler_stream = filler_stream.unwrap();
            let mut filler = Filler::new(INITIAL_SPEED, Duration::from_millis(2));
            info!("Filler stream is attached");
            loop {
                let mut possible_packet = Packet::new();

                //GET запрос на чтение нового видоса
                if let Ok(size) = client_stream.read(&mut possible_packet.buf) {
                    if size > 0 {
                        trace!("<< {size}");
                        possible_packet.size = size;
                        //перенаправляем его VPN серверу
                        ct_client_vpn.send(possible_packet).unwrap()
                    }
                }
                //вычитываем дросселированные пакеты (если есть)
                while let Ok(packet) = cr_throttler_client.try_recv() {
                    trace!(">> {}", packet.size);
                    client_stream.write_all(&packet.buf[..packet.size]).unwrap();
                    filler.data_was_sent(packet.size)
                }
                if let Some(packet) = filler.get_fill_bytes() {
                    trace!(">> filler {}", packet.size);
                    if let Ok(written) = filler_stream.write(&packet.buf[..packet.size]) {
                        filler.filler_was_sent(written);
                    }
                }else{
                    trace!("no filler");
                }
                filler.clean();//TODO передавать в модуль сбора статистики
            }
        }).expect("client_stream");


        let vpn_to_proxy_join = thread::Builder::new()
            .name("vpn_stream".to_string()).spawn(move || {
            up_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");

            loop {
                //проверяем нет ли фреймов видоса
                let mut possible_packet = Packet::new();//TODO брать из банки - не нагружать ОС

                if let Ok(size) = up_stream.read(&mut possible_packet.buf) {
                    //если есть, добавляем в дроссель
                    if size > 0 {
                        trace!("<< {size}");
                        possible_packet.size = size;
                        ct_vpn_throttler.send(ThrottlerCommand::Enqueue(possible_packet)).unwrap()
                    }
                }
                //отправляем все накопившиеся /GET запросы к VPN
                while let Ok(packet) = cr_client_vpn.try_recv() {
                    trace!(">> {}", packet.size);
                    up_stream.write_all(&packet.buf[..packet.size]).unwrap();
                }
            }
        }).expect("vpn_stream");

        Self {
            client_to_proxy_join,
            vpn_to_proxy_join,
            throttler_join: throttler.join_handle,
            ct_filler,
        }
    }

    pub fn attach(&self, filler_stream: TcpStream) {
        self.ct_filler.send(filler_stream).unwrap()
    }
}
