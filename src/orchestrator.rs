//владеет всеми инстансами VpnProxy
//собирает статистику по ним и отправляет в анализатор изменения скорости

use std::io::Write;
use std::net::{Shutdown, TcpStream};
use std::ops::{Deref, DerefMut};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};
use log::{info, warn};
use crate::objects::{ProxyState, RuntimeCommand};
use crate::speed::speed_correction::SpeedCorrector;
use crate::statistic::{StatisticCollector, Summary};
use crate::core::vpn_proxy::{Proxy, VpnProxy};
use crate::entry::entry_point::{FillerChannel, MainChannel};

pub const SPEED_CORRECTION_INVOKE_PERIOD: Duration = Duration::from_millis(100);
const FAILED_TO_CONNECT : &[u8] = "Already connected...".as_bytes();

pub struct Orchestrator {
    new_proxy_receiver: Receiver<MainChannel>,
    new_filler_receiver: Receiver<FillerChannel>,
    proxy_only: Vec<VpnProxy>,
    filler_only: Vec<FillerChannel>,
    pairs: Vec<Box<dyn Proxy>>,
    stat: Box<dyn StatisticCollector>,
    speed_corrector: SpeedCorrector,
    last_speed_correction_invoke: Instant,
}

impl Orchestrator {
    pub fn new_stat(new_proxy_receiver: Receiver<MainChannel>,
                    new_filler_receiver: Receiver<FillerChannel>,
                    stat: Box<dyn StatisticCollector>) -> Orchestrator {
        let proxy_only: Vec<VpnProxy> = vec![];
        let filler_only: Vec<FillerChannel> = vec![];
        let pair: Vec<Box<dyn Proxy>> = vec![];
        Self {
            new_proxy_receiver,
            new_filler_receiver,
            proxy_only,
            filler_only,
            pairs: pair,
            stat,
            speed_corrector: SpeedCorrector::new(),
            last_speed_correction_invoke: Instant::now(),
        }
    }


    pub fn invoke(&mut self) {
        loop  {
            if !self.check_new_connections() {
                break;
            }
        }
        self.receive_proxy_state();
        self.send_statistic_to_speed_correction();
    }

    pub fn get_pairs_count(&mut self) -> usize {
        self.pairs.len()
    }

    pub fn calculate_and_get(&mut self) -> Option<Vec<Summary>> {
        self.stat.calculate_and_get()
    }

    fn receive_proxy_state(&mut self) {
        for i in 0..self.pairs.len() {
            let proxy = self.pairs[i].deref_mut();
            if let Ok(state) = proxy.try_recv_state() {
                let stat = self.stat.deref_mut();
                match state {
                    ProxyState::SetupComplete => {
                        info!("SetupComplete {}", &proxy.get_key());
                    }
                    ProxyState::Info(collected_info) => {
                        stat.append_info(&proxy.get_key(), collected_info);
                    }
                    ProxyState::Broken => {
                        info!("Broken {}", &proxy.get_key());
                        stat.clear_info(&proxy.get_key());
                        //TODO вынести
                        self.pairs.remove(i);
                        break;
                    }
                }
            }
        }
    }

    fn send_statistic_to_speed_correction(&mut self) {
        if self.last_speed_correction_invoke.elapsed() >= SPEED_CORRECTION_INVOKE_PERIOD {
            if let Some(summary) = self.stat.calculate_and_get() {
                if let Some(commands) = self.speed_corrector.append_and_get(summary){
                    for command in commands.into_iter() {
                        if let Some(proxy) = self.get_by_key(&command.key){
                            proxy.try_send_command(RuntimeCommand::SetSpeed(command.speed)).unwrap()
                        }
                    }
                }
            }
            self.last_speed_correction_invoke = Instant::now();
        }
    }

    fn get_by_key(&mut self, key: &String) -> Option<&mut Box<dyn Proxy>> {
        for i in 0..self.pairs.len() {
            let proxy = self.pairs[i].deref_mut();
            if proxy.get_key().eq(key) {
                return Some(&mut self.pairs[i])
            }
        }
        None
    }

    fn check_new_connections(&mut self) -> bool {
        if let Ok(main_channel) = self.new_proxy_receiver.try_recv() {
            let proxy = VpnProxy::new(main_channel.client_stream, main_channel.up_stream);
            //ищем пару
            for i in 0..self.filler_only.len() {
                let filler = &self.filler_only[i];
                if proxy.key.eq(&filler.key) {
                    info!("Pair {} case1", &proxy.key);
                    let filler = self.filler_only.remove(i);
                    self.ensure_previous_session_is_destroyed(&filler.key);
                    let mut proxy = Box::new(proxy);
                    proxy.deref_mut().try_send_command(RuntimeCommand::SetFiller(filler.client_stream)).unwrap();
                    self.pairs.push(proxy);
                    return true;
                }
            }
            //пары еще нет - добавляем в одиночки
            info!("Proxy {} was received ", &proxy.key);
            self.proxy_only.push(proxy);
            return true;
        }
        if let Ok(mut filler) = self.new_filler_receiver.try_recv() {
            //ищем ему пару
            for i in 0..self.proxy_only.len() {
                let proxy = &self.proxy_only[i];
                if proxy.key.eq(&filler.key) {
                    info!("Pair {} case2", &proxy.key);
                    //self.ensure_previous_session_is_destroyed(&filler_key);
                    let proxy = self.proxy_only.remove(i);
                    let mut proxy = Box::new(proxy);
                    proxy.deref_mut().try_send_command(RuntimeCommand::SetFiller(filler.client_stream)).unwrap();
                    self.pairs.push(proxy);
                    return true;
                }
            }
            //проверяем по ключу не повторное ли это подключение
            for i in 0..self.pairs.len() {
                let pair = self.pairs[i].deref_mut();
                if pair.get_key().eq(&filler.key) {
                    warn!("{} client connection rejected (Already connected)", &filler.key);
                    let result = filler.client_stream.write(FAILED_TO_CONNECT);
                    if result.is_ok() {
                        let _ = filler.client_stream.shutdown(Shutdown::Both);
                    }
                    return false;
                }
            }

            //пары еще нет - добавляем в одиночки
            info!("Filler {} was received ", &filler.key);
            self.filler_only.push(filler);
            return true;
        }
        false
    }

    fn ensure_previous_session_is_destroyed(&mut self, key: &String) {
        for i in 0..self.pairs.len() {
            if self.pairs[i].get_key().eq(key) {
                self.pairs.remove(i);
                //TODO завершить поток, если он еще живой
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use std::ops::Sub;
    use std::sync::mpsc::{channel, SendError, TryRecvError};
    use std::time::Instant;
    use std::time::Duration;
    use log::info;
    use crate::objects::{HotPotatoInfo, ProxyState, RuntimeCommand, SentPacket};
    use crate::orchestrator::Orchestrator;
    use crate::speed::INITIAL_SPEED;
    use crate::speed::native_to_regular;
    use crate::statistic::SimpleStatisticCollector;
    use crate::core::vpn_proxy::Proxy;

    struct TestProxy {
        key: String,
    }

    impl TestProxy {
        pub fn new() -> TestProxy {
            Self { key: "1".to_string() }
        }
    }

    impl Proxy for TestProxy {
        fn get_key(&mut self) -> &String {
            &self.key
        }

        fn try_recv_state(&mut self) -> Result<ProxyState, TryRecvError> {
            let mut collected_info = HotPotatoInfo::default();
            //50мс назад отправили 2 пакета
            //один данных, один - заполнитель
            //данных 10Кб, заполнитель 5Кб
            let fifty_ms_ago = Instant::now().sub(Duration::from_millis(50));
            collected_info.target_speed = INITIAL_SPEED;
            collected_info.data_count = 1;
            collected_info.filler_count = 1;
            collected_info.data_packets[0] = Some(SentPacket { sent_date: fifty_ms_ago, sent_size: 10_000 });
            collected_info.filler_packets[0] = Some(SentPacket { sent_date: fifty_ms_ago, sent_size: 5_000 });
            Ok(ProxyState::Info(collected_info))
        }

        fn try_send_command(&mut self, _command: RuntimeCommand) -> Result<(), SendError<RuntimeCommand>> {
            Ok(())
        }
    }

    /**
    Проверяем что данные от Прокси сквозь оркестратор поступают в сборщик статистики
     */
    #[test]
    fn test_stat_collector_flow() {
        let (_, cr_vpn) = channel();
        let (_, cr_filler) = channel();
        let mut orchestrator = Orchestrator::new_stat(cr_vpn, cr_filler,
                                                      Box::new(SimpleStatisticCollector::default()));

        orchestrator.pairs.push(Box::new(TestProxy::new()));
        orchestrator.invoke();
        let stat = orchestrator.calculate_and_get().unwrap();
        let client = stat.into_iter().find(|_collected_info| true).unwrap();
        let calculated_speed = native_to_regular(client.calculated_speed);
        let target_speed = native_to_regular(client.target_speed);
        info!("\r{}\t\t\t {:03}%/{:03}% \t  {}/{}",
               client.key,
               client.percent_data,
               client.percent_filler,
               calculated_speed,
               target_speed);
    }
}