use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::core::filler::Filler;
use crate::objects::Pair;
use crate::objects::ONE_PACKET_MAX_SIZE;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::speed::{SpeedCorrectorCommand, SHUTDOWN_SPEED};
use log::{debug, error, info};
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
    running: Arc<AtomicBool>,
    join_handle: Option<JoinHandle<()>>,
    //подразумеваем что от одного VPN клиента может устанавливаться только одно подключение
    //будем использовать IP tun интерфейса
    pub key: String,
}
pub trait Proxy {
    fn get_key(&self) -> &String;
    fn try_recv_state(&mut self) -> Result<ProxyState, TryRecvError>;
    fn try_send_command(
        &mut self,
        command: RuntimeCommand,
    ) -> Result<(), SendError<RuntimeCommand>>;
}
impl Proxy for VpnProxy {
    fn get_key(&self) -> &String {
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
    running: Arc<AtomicBool>,
    //without throttler & filler
    free_mode: bool,
    //временный буфер
    buf: [u8; ONE_PACKET_MAX_SIZE],
}

impl VpnProxy {
    pub fn new(pair: Pair) -> VpnProxy {
        let (ct_command, cr_command) = channel();
        let (ct_state, cr_state) = channel();
        let key = pair.key.clone();
        let running = Arc::new(AtomicBool::new(true));
        let thread_working_set = ThreadWorkingSet {
            key: key.clone(),
            cr_command,
            ct_state: ct_state.clone(),
            running: running.clone(),
            free_mode: true,
            pair,
            buf: [0; ONE_PACKET_MAX_SIZE],
        };

        let join_handle = ThreadWorkingSet::thread_start(thread_working_set);


        Self {
            ct_command,
            cr_state,
            running,
            join_handle: Some(join_handle),
            key: key.clone(),
        }
    }
}

impl Drop for VpnProxy {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.join_handle.take() {
            if let Err(e) = handle.join() {
                error!("Failed to join thread: {:?}", e);
            }
        }
        info!("Dropping VpnProxy. Thread {}.",self.key);
    }
}

impl ThreadWorkingSet {
    pub fn thread_start(mut instance: ThreadWorkingSet) -> JoinHandle<()> {
        thread::Builder::new()
            .name(instance.key.clone())
            .spawn(move || {
                //цикл который использует заполнитель
                let mut filler = Filler::new(SHUTDOWN_SPEED);
                instance.ct_state.send(ProxyState::SetupComplete).unwrap();
                info!("Client thread started");
                loop {
                    if !instance.running.load(Ordering::Relaxed) {
                        instance.ct_state.send(ProxyState::Broken).unwrap();
                        let _ = instance.pair.client_stream.shutdown();
                        let _ = instance.pair.up_stream.shutdown();
                        break;
                    }
                    let result = if instance.free_mode {
                        instance.free_loop(&mut filler)
                    } else {
                        instance.main_loop(&mut filler)
                    };
                    match result {
                        Err(e) => {
                            instance.running.store(false, Ordering::Relaxed);
                            error!("{:?} {} {}", e.cause, e.ctx, e.location);
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
    ) -> Result<(), Error> {
        let mut some_work = false;
        //GET запрос на чтение нового видоса
        let size= self.pair.client_stream.read(&mut self.buf[..])?;
        if size > 0 {
            //перенаправляем его VPN серверу
            //trace!("->> {}", size);
            self.pair.up_stream.write_all(&self.buf[..size])?;
            some_work = true;
        }
        //если есть место
        let available_space = filler.get_available_space();
        if available_space > A_FEW_SPACE {
            let vpn_incoming_data_size = self.pair.up_stream.read(&mut self.buf[..])?;
            if vpn_incoming_data_size > 0  {
                //trace!("=>> {}", vpn_incoming_data_size);
                self.pair.client_stream.write_all(&self.buf[..vpn_incoming_data_size])?;
                filler.data_was_sent(vpn_incoming_data_size);
                some_work = true;
            }else if let Some(packet) = filler.get_filler_packet(){
                //trace!("=>> filler {}", packet.size);
                self.pair.filler_stream.write_all(&packet.buf[..packet.size])?;
                filler.filler_was_sent(packet.size);
                some_work = true;
            }
        }
        if let Some(collected_info) = filler.clean_almost_full() {
            let start = Instant::now();
            self.ct_state.send(ProxyState::Info(collected_info))
                .context("Send hot statistic info")?;
            some_work = true;
            if start.elapsed() > Duration::from_millis(3){
                bail!("Долгая отправка данных по статистике");
            }
        }
        if let Ok(command) = self.cr_command.try_recv() {
            match command {
                RuntimeCommand::SetSpeed(speed_command) => {
                    if let SpeedCorrectorCommand::SetSpeed(speed) = speed_command {
                        debug!("speed was updated {speed}");
                        filler.set_speed(speed);
                    }else if let SpeedCorrectorCommand::SwitchOff = speed_command {
                        debug!("free mode enter");
                        self.free_mode = true;
                    }
                }
            }
        }
        if !some_work {
            sleep(BURNOUT_DELAY);
        }
        Ok(())
    }

    fn free_loop(
        &mut self,
        filler: &mut Filler,
    ) -> Result<(), Error> {
        let mut some_work = false;
        let size= self.pair.client_stream.read(&mut self.buf[..])?;
        if size > 0 {
            self.pair.up_stream.write_all(&self.buf[..size])?;
            some_work = true;
        }
        let vpn_incoming_data_size = self.pair.up_stream.read(&mut self.buf[..])?;
        if vpn_incoming_data_size > 0  {
            self.pair.client_stream.write_all(&self.buf[..vpn_incoming_data_size])?;
            filler.data_was_sent(vpn_incoming_data_size);
            some_work = true;
        }

        if let Some(collected_info) = filler.clean_almost_full() {
            let start = Instant::now();
            self.ct_state.send(ProxyState::Info(collected_info))
                .context("Send hot statistic info")?;
            some_work = true;
            if start.elapsed() > Duration::from_millis(3){
                bail!("Долгая отправка данных по статистике");
            }
        }
        if let Ok(command) = self.cr_command.try_recv() {
            match command {
                RuntimeCommand::SetSpeed(speed_command) => {
                    if let SpeedCorrectorCommand::SetSpeed(speed) = speed_command {
                        debug!("speed was set {speed}");
                        filler.set_speed(speed);
                        self.free_mode = false;
                    }
                }
            }
        }
        if !some_work {
            sleep(BURNOUT_DELAY);
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