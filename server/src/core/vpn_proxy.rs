use std::collections::VecDeque;
use crate::core::filler::Filler;
use crate::core::throttler::ThrottlerAnalyzer;
use crate::objects::Pair;
use crate::objects::ONE_PACKET_MAX_SIZE;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::speed::INITIAL_SPEED;
use log::{error, info, trace};
use std::fs::File;
use std::io::{Read, Write};
use std::sync::mpsc::{channel, Receiver, SendError, Sender, TryRecvError};
use std::thread;
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, Instant};
use easy_error::{bail, Error, ResultExt};

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
            ct_state: ct_state.clone(),
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
        thread::Builder::new()
            .name(instance.key.clone())
            .spawn(move || {
                //цикл который использует заполнитель
                let mut filler = Filler::new(INITIAL_SPEED);
                let mut throttler = ThrottlerAnalyzer::new(INITIAL_SPEED);
                instance.ct_state.send(ProxyState::SetupComplete).unwrap();
                info!("Client thread started");
                loop {
                    match instance.main_loop(&mut filler, &mut throttler) {
                        Err(e) => {
                            error!("{} {}", e.ctx, e.location);
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
            .expect("client_stream thread started")
    }

    fn main_loop(
        &mut self,
        filler: &mut Filler,
        throttler: &mut ThrottlerAnalyzer,
    ) -> Result<(), Error> {
        //GET запрос на чтение нового видоса
        let size= self.pair.client_stream.read(&mut self.buf[..])?;
        if size > 0 {
            //перенаправляем его VPN серверу
            //trace!("->> {}", size);
            self.pair.up_stream.write_all(&self.buf[..size])?;
        }
        //если есть место
        let mut available_space = throttler.get_available_space();
        if available_space > A_FEW_SPACE {
            if available_space > ONE_PACKET_MAX_SIZE {
                available_space = ONE_PACKET_MAX_SIZE;
            }
            let vpn_incoming_data_size = self.pair.up_stream.read(&mut self.buf[..available_space])?;
            if vpn_incoming_data_size > 0  {
                //trace!("=>> {}", vpn_incoming_data_size);
                self.pair.client_stream.write_all(&self.buf[..vpn_incoming_data_size])?;
                throttler.data_was_sent(vpn_incoming_data_size);
                filler.data_was_sent(vpn_incoming_data_size);
            } else if let Some(packet) = filler.get_fill_bytes() {
                //trace!("=>> filler {}", packet.size);
                //FIXME: временный хак
                let size = packet.size/4;
                self.pair.filler_stream.write_all(&packet.buf[..size])?;
                filler.filler_was_sent(size);
            } else {
                sleep(BURNOUT_DELAY);
            }
        } else {
            sleep(BURNOUT_DELAY);
        }
        if let Some(collected_info) = filler.clean_almost_full() {
            let start = Instant::now();
            self.ct_state.send(ProxyState::Info(collected_info))
                .context("Send hot statistic info")?;
            if start.elapsed() > Duration::from_millis(3){
                bail!("Долгая отправка данных по статистике");
            }
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
#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;
    use log::info;
    use crate::tests::test_init::initialize_logger;

    #[test]
    fn mpsc_test() {
        initialize_logger();
        let (tx, rx) = mpsc::channel();
        let join  = thread::spawn(move || {
            tx.send(1).unwrap();
            tx.send(2).unwrap();
            tx.send(3).unwrap();
            tx.send(4).unwrap();
            info!("send done");
            sleep(Duration::from_millis(300))
        });
        sleep(Duration::from_millis(100));
        assert_eq!(1, rx.recv().unwrap());
        info!("received 1");
        assert_eq!(2, rx.recv().unwrap());
        info!("received 2");
        assert_eq!(3, rx.recv().unwrap());
        info!("received 3");
        assert_eq!(4, rx.recv().unwrap());
        info!("received 4");
        join.join().unwrap();
    }
}