use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crate::core::throttler::ThrottlerAnalyzer;
use log::{info, trace};
use crate::core::filler::Filler;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::objects::ONE_PACKET_MAX_SIZE;
use crate::speed::INITIAL_SPEED;

const A_FEW_SPACE : usize = 100;
const BURNOUT_DELAY: Duration = Duration::from_micros(500);

pub struct VpnProxy {
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

struct ThreadWorkingSet{
    key: String,
    cr_command: Receiver<RuntimeCommand>,
    ct_state: Sender<ProxyState>,
    //входящее подклчюение от клиента
    client_stream: TcpStream,
    //подключение к ВПН серверу
    up_stream: TcpStream,
    //временный буфер
    buf: [u8; ONE_PACKET_MAX_SIZE],
}

impl VpnProxy {

    pub fn new(client_stream: TcpStream, up_stream: TcpStream) -> VpnProxy {
        let key = VpnProxy::get_key(&client_stream);
        let (ct_command, cr_command) = channel();
        let (ct_state, cr_state) = channel();

        let timeout = Duration::from_millis(1);
        client_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");
        up_stream.set_read_timeout(Some(timeout)).expect("Архитектура подразумевает не блокирующий метод чтения");

        let thread_working_set = ThreadWorkingSet {
            key: key.clone(),
            cr_command,
            ct_state,
            client_stream,
            up_stream,
            buf: [0; ONE_PACKET_MAX_SIZE],
        };

        ThreadWorkingSet::thread_start(thread_working_set);

        Self {
            ct_command,
            cr_state,
            key
        }
    }
    pub fn get_key(stream: &TcpStream) -> String {
        stream.peer_addr().unwrap().ip().to_string()
    }

}

impl ThreadWorkingSet{
    pub fn thread_start(mut instance: ThreadWorkingSet) -> JoinHandle<()> {
        return thread::Builder::new()
            .name("client_stream".to_string()).spawn(move || {
            let mut filler_stream: TcpStream;

            //цикл, который не использует заполнитель, а работает в режиме ожидания его появления
            info!("Filler-less mode start");
            loop {
                match instance.exchange_loop_pre_filler() {
                    Ok(filler) => {
                        match filler {
                            Some(filler) => {
                                filler_stream = filler;
                                break;
                            },
                            _ =>{}
                        }
                    }
                    Err(_) => {
                        let _ = instance.ct_state.send(ProxyState::Broken).unwrap();
                        let _ = instance.client_stream.shutdown(Shutdown::Both);
                        return;
                    }
                }
            }

            //цикл который использует заполнитель
            let mut filler = Filler::new(INITIAL_SPEED);
            let mut throttler = ThrottlerAnalyzer::new(INITIAL_SPEED);
            instance.ct_state.send(ProxyState::SetupComplete).unwrap();
            info!("Filler stream is attached");
            loop {
                match instance.exchange_with_filler(&mut filler_stream, &mut filler, &mut throttler) {
                    Err(_) => {
                        let _ = instance.ct_state.send(ProxyState::Broken);
                        let _ = instance.client_stream.shutdown(Shutdown::Both);
                        let _ = filler_stream.shutdown(Shutdown::Both);
                        break;
                    }
                    Ok(_) =>{}
                }
            }
            info!("Exit from thread {}", instance.key)
        }).expect("client_stream");
    }
    
    fn exchange_loop_pre_filler(&mut self) -> Result<Option<TcpStream>, Box<dyn Error>> {
        //GET запрос на чтение нового видоса
        if let Ok(size) = self.client_stream.read(&mut self.buf) {
            if size > 0 {
                //перенаправляем его VPN серверу
                trace!("->> {}", size);
                self.up_stream.write_all(&self.buf[..size])?;
            }
        }
        if let Ok(size) = self.up_stream.read(&mut self.buf) {
            //если есть, добавляем в дроссель
            if size > 0 {
                trace!("=>> {}", size);
                self.client_stream.write_all(&self.buf[..size])?;
            }
        }
        //проверяем не появился ли заполнитель
        if let Ok(command) = self.cr_command.try_recv() {
            match command {
                RuntimeCommand::SetFiller(filler) => {
                    trace!("Leaving simplified mode");
                    return Ok(Some(filler));
                },
                _=>{}
            }
        }
        Ok(None)
    }

    fn exchange_with_filler(&mut self, filler_stream: &mut TcpStream, filler: &mut Filler, throttler: &mut ThrottlerAnalyzer)
        -> Result<(), Box<dyn Error>> {
        //GET запрос на чтение нового видоса
        if let Ok(size) = self.client_stream.read(&mut self.buf) {
            if size > 0 {
                //перенаправляем его VPN серверу
                trace!("->> {}", size);
                self.up_stream.write_all(&self.buf[..size])?;
            }
        }
        //если есть место
        let mut available_space = throttler.get_available_space();
        if available_space > A_FEW_SPACE {
            if available_space>ONE_PACKET_MAX_SIZE {
                available_space= ONE_PACKET_MAX_SIZE;
            }
            if let Ok(size) = self.up_stream.read(&mut self.buf[..available_space]) {
                //если есть, добавляем в дроссель
                if size > 0 {
                    trace!("=>> {}", size);
                    self.client_stream.write_all(&self.buf[..size])?;
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
        }else{
            sleep(BURNOUT_DELAY);
        }
        if let Some(collected_info) = filler.clean_almost_full() {
            self.ct_state.send(ProxyState::Info(collected_info))?;
        }
        if let Ok(command) = self.cr_command.try_recv() {
            match command {
                RuntimeCommand::SetSpeed(speed) => {
                    filler.set_speed(speed);
                    throttler.set_speed(speed);
                },
                _=>{}
            }
        }
        Ok(())
    }

}
