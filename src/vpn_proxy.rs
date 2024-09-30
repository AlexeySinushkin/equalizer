use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use crate::throttler::ThrottlerAnalyzer;
use log::{info, trace};
use crate::filler::Filler;
use crate::objects::CollectedInfo;
use crate::r#const::{INITIAL_SPEED, ONE_PACKET_MAX_SIZE};

const A_FEW_SPACE : usize = 100;

pub struct VpnProxy {
    //Клиент к нам подключенный
    //client_stream: TcpStream,
    //Мы к VPN серверу подключены
    //up_stream: TcpStream,
    pub join_handle: JoinHandle<()>,
    ct_filler: Sender<TcpStream>
}



impl VpnProxy {
    /**
    Создаем поток по чтению запросов от клиента
    и поток внутри дросселя для отправки данных (лимитированных по скорости)
     */
    pub fn new(mut client_stream: TcpStream, mut up_stream: TcpStream, ct_stat: Sender<CollectedInfo>) -> VpnProxy {

        //Правило именования каналов
        // ct_иточник_получатель, cr_иточник_получатель

        let (ct_filler, cr_filler) = channel();
        let timeout = Duration::from_millis(1);
        client_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");
        up_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");

        let join_handle = thread::Builder::new()
            .name("client_stream".to_string()).spawn(move || {

            let filler_stream: Option<TcpStream>;
            let mut buf :[u8; ONE_PACKET_MAX_SIZE] = [0; ONE_PACKET_MAX_SIZE];
            //цикл, который не использует заполнитель, а работает в режиме ожидания его появления
            loop {
                //GET запрос на чтение нового видоса
                if let Ok(size) = client_stream.read(&mut buf) {
                    if size > 0 {
                        //перенаправляем его VPN серверу
                        trace!("->> {}", size);
                        up_stream.write_all(&buf[..size]).unwrap();
                    }
                }
                if let Ok(size) = up_stream.read(&mut buf) {
                    //если есть, добавляем в дроссель
                    if size > 0 {
                        trace!("=>> {}", size);
                        client_stream.write_all(&buf[..size]).unwrap();
                    }
                }
                //проверяем не появился ли заполнитель
                if let Ok(filler) = cr_filler.try_recv() {
                    filler_stream = Some(filler);
                    break;
                }
            }
            //цикл который использует заполнитель
            let mut filler_stream = filler_stream.unwrap();
            let mut filler = Filler::new(INITIAL_SPEED);
            let mut throttler = ThrottlerAnalyzer::new(INITIAL_SPEED);
            info!("Filler stream is attached");
            loop {
                //GET запрос на чтение нового видоса
                if let Ok(size) = client_stream.read(&mut buf) {
                    if size > 0 {
                        //перенаправляем его VPN серверу
                        trace!("->> {}", size);
                        up_stream.write_all(&buf[..size]).unwrap();
                    }
                }
                //если есть место
                let mut available_space = throttler.get_available_space();
                if available_space > A_FEW_SPACE {
                    if available_space>ONE_PACKET_MAX_SIZE {
                        available_space= ONE_PACKET_MAX_SIZE;
                    }
                    if let Ok(size) = up_stream.read(&mut buf[..available_space]) {
                        //если есть, добавляем в дроссель
                        if size > 0 {
                            trace!("=>> {}", size);
                            client_stream.write_all(&buf[..size]).unwrap();
                            throttler.data_was_sent(size);
                            filler.data_was_sent(size);
                        }
                    }else{
                        if let Some(packet) = filler.get_fill_bytes() {
                            trace!("=>> filler {}", packet.size);
                            if let Ok(written) = filler_stream.write(&packet.buf[..packet.size]) {
                                filler.filler_was_sent(written);
                            }
                        }
                    }
                }
                ct_stat.send(filler.clean()).unwrap();
                /*
                if let Some(command) = cr_command.try_recv() {
                    match command {
                        SetSpeed(speed) => {
                            throttler.set_speed(speed);
                            filler.set_speed(speed)
                        }
                    }
                }
                */
            }
        }).expect("client_stream");


        Self {
            join_handle,
            ct_filler
        }
    }

    pub fn attach(&self, filler_stream: TcpStream) {
        self.ct_filler.send(filler_stream).unwrap()
    }
}
