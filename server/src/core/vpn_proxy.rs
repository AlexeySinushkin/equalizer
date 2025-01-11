use crate::core::filler::Filler;
use crate::core::throttler::ThrottlerAnalyzer;
use crate::objects::Pair;
use crate::objects::ONE_PACKET_MAX_SIZE;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::speed::INITIAL_SPEED;
use log::{error, info, trace, warn};
use std::fs::File;
use std::io::Write;
use std::sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::Duration;
use easy_error::{Error, ResultExt};

const A_FEW_SPACE: usize = 100;
const BURNOUT_DELAY: Duration = Duration::from_micros(500);

pub struct VpnProxy {
    ct_command: Sender<RuntimeCommand>,
    cr_state: Receiver<ProxyState>,
    //подразумеваем что от одного VPN клиента может устанавливаться только одно подключение
    //будем использовать IP tun интерфейса
    pub key: String,
}
pub trait Proxy {
    fn get_key(&mut self) -> &String;
    fn try_recv_state(&mut self) -> Result<ProxyState, TryRecvError>;
    fn try_send_command(
        &mut self,
        command: RuntimeCommand,
    ) -> Result<(), SendError<RuntimeCommand>>;
}
impl Proxy for VpnProxy {
    fn get_key(&mut self) -> &String {
        &self.key
    }
    fn try_recv_state(&mut self) -> Result<ProxyState, TryRecvError> {
        self.cr_state.try_recv()
    }
    fn try_send_command(
        &mut self,
        command: RuntimeCommand,
    ) -> Result<(), SendError<RuntimeCommand>> {
        self.ct_command.send(command)
    }
}
struct ThreadWorkingSet {
    key: String,
    cr_command: Receiver<RuntimeCommand>,
    ct_state: Sender<ProxyState>,
    pair: Pair,
    //временный буфер
    buf: [u8; ONE_PACKET_MAX_SIZE],
}

impl VpnProxy {
    pub fn new(pair: Pair) -> VpnProxy {
        let (ct_command, cr_command) = channel();
        let (ct_state, cr_state) = channel();
        let key = pair.key.clone();
        let thread_working_set = ThreadWorkingSet {
            key: key.clone(),
            cr_command,
            ct_state,
            pair,
            buf: [0; ONE_PACKET_MAX_SIZE],
        };

        ThreadWorkingSet::thread_start(thread_working_set);

        Self {
            ct_command,
            cr_state,
            key: key.clone(),
        }
    }
}

impl ThreadWorkingSet {
    pub fn thread_start(mut instance: ThreadWorkingSet) -> JoinHandle<()> {
        return thread::Builder::new()
            .name("client_stream".to_string())
            .spawn(move || {
                //цикл который использует заполнитель
                let mut filler = Filler::new(INITIAL_SPEED);
                let mut throttler = ThrottlerAnalyzer::new(INITIAL_SPEED);
                instance.ct_state.send(ProxyState::SetupComplete).unwrap();
                let mut to_server = File::create("target/proxy-to-server.bin").unwrap();
                info!("Client thread started");
                loop {
                    match instance.exchange_with_filler(&mut filler, &mut throttler, &mut to_server) {
                        Err(e) => {
                            error!("{}", e);
                            let _ = instance.ct_state.send(ProxyState::Broken);
                            let _ = instance.pair.client_stream.shutdown();
                            let _ = instance.pair.up_stream.shutdown();
                            break;
                        }
                        Ok(_) => {}
                    }
                }
                info!("Exit from thread {}", instance.key)
            })
            .expect("client_stream");
    }

    fn exchange_with_filler(
        &mut self,
        filler: &mut Filler,
        throttler: &mut ThrottlerAnalyzer,
        to_server: &mut File,
    ) -> Result<(), Error> {
        //GET запрос на чтение нового видоса
        let size= self.pair.client_stream.read(&mut self.buf[..])
            .context("Read data from client stream")?;
        if size > 0 {
            //перенаправляем его VPN серверу
            //trace!("->> {}", size);
            to_server.write_all(&self.buf[..size])
                .context("Binary log")?;
            self.pair.up_stream.write_all(&self.buf[..size])
                .context("Send data through vpn stream")?;
        }else{
            warn!("READ ZERO")
        }
        //если есть место
        let mut available_space = throttler.get_available_space();
        if available_space > A_FEW_SPACE {
            if available_space > ONE_PACKET_MAX_SIZE {
                available_space = ONE_PACKET_MAX_SIZE;
            }
            if let Ok(size) = self.pair.up_stream.read(&mut self.buf[..available_space]) {
                //если есть, добавляем в дроссель
                if size > 0 {
                    //trace!("=>> {}", size);
                    self.pair.client_stream.write_all(&self.buf[..size])
                        .context("Send data through client stream")?;
                    throttler.data_was_sent(size);
                    filler.data_was_sent(size);
                }
            } else {
                if let Some(packet) = filler.get_fill_bytes() {
                    //trace!("=>> filler {}", packet.size);
                    self.pair.filler_stream.write_all(&packet.buf[..packet.size])
                        .context("Send data through filler stream")?;
                    filler.filler_was_sent(packet.size);
                }
            }
        } else {
            sleep(BURNOUT_DELAY);
        }
        if let Some(collected_info) = filler.clean_almost_full() {
            self.ct_state.send(ProxyState::Info(collected_info))
                .context("Send hot statistic info")?;
        }
        if let Ok(command) = self.cr_command.try_recv() {
            match command {
                RuntimeCommand::SetSpeed(speed) => {
                    filler.set_speed(speed);
                    throttler.set_speed(speed);
                }
            }
        }
        Ok(())
    }
}
