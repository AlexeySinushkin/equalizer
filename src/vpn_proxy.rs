use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver, Sender, SendError, TryRecvError};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use crate::throttler::ThrottlerAnalyzer;
use log::{info, trace};
use crate::filler::Filler;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::r#const::{INITIAL_SPEED, ONE_PACKET_MAX_SIZE};

const A_FEW_SPACE : usize = 100;

pub struct VpnProxy {
    //Клиент к нам подключенный
    //client_stream: TcpStream,
    //Мы к VPN серверу подключены
    //up_stream: TcpStream,
    pub join_handle: JoinHandle<()>,
    ct_command: Sender<RuntimeCommand>,
    cr_state: Receiver<ProxyState>,
    //подразумеваем что от одного VPN клиента может устанавливаться только одно подключение
    //будем использовать IP tun интерфейса
    pub key: String
}

pub trait Proxy {
    fn get_key(&mut self) -> &String;
    fn try_recv_state(&mut self) -> Result<ProxyState, TryRecvError>;
    fn try_send_command(&mut self, command: RuntimeCommand) -> Result<(), SendError<RuntimeCommand>>;
}

impl Proxy for VpnProxy {
    fn get_key(&mut self) -> &String {
        &self.key
    }
    fn try_recv_state(&mut self) -> Result<ProxyState, TryRecvError> {
        self.cr_state.try_recv()
    }
    fn try_send_command(&mut self, command: RuntimeCommand) -> Result<(), SendError<RuntimeCommand>> {
        self.ct_command.send(command)
    }
}

impl VpnProxy {
    /**
    Создаем поток по чтению запросов от клиента
    и поток внутри дросселя для отправки данных (лимитированных по скорости)
     */
    pub fn new(mut client_stream: TcpStream, mut up_stream: TcpStream) -> VpnProxy {
        let key = VpnProxy::get_key(&client_stream);
        let (ct_command, cr_command) = channel();
        let (ct_state, cr_state) = channel();

        let timeout = Duration::from_millis(1);
        client_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");
        up_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");

        let join_handle = thread::Builder::new()
            .name("client_stream".to_string()).spawn(move || {

            let mut filler_stream: TcpStream;
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
                if let Ok(command) = cr_command.try_recv() {
                    match command {
                        RuntimeCommand::SetFiller(filler) => {
                            filler_stream = filler;
                            trace!("Leaving simplified mode");
                            break;
                        },
                        _=>{}
                    }
                }
            }
            //цикл который использует заполнитель
            let mut filler = Filler::new(INITIAL_SPEED);
            let mut throttler = ThrottlerAnalyzer::new(INITIAL_SPEED);
            ct_state.send(ProxyState::SetupComplete).unwrap();
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
                ct_state.send(ProxyState::Info(filler.clean())).unwrap();
                if let Ok(command) = cr_command.try_recv() {
                    match command {
                        RuntimeCommand::SetSpeed(speed) => {
                            filler.set_speed(speed);
                            throttler.set_speed(speed);
                        },
                        _=>{}
                    }
                    break;
                }
            }
        }).expect("client_stream");


        Self {
            join_handle,
            ct_command,
            cr_state,
            key
        }
    }

    pub fn get_key(stream: &TcpStream) -> String {
        stream.peer_addr().unwrap().ip().to_string()
    }

}
