//владеет всеми инстансами VpnProxy
//собирает статистику по ним и отправляет в анализатор изменения скорости

use crate::core::vpn_proxy::{Proxy, VpnProxy};
use crate::objects::Pair;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::speed::speed_correction::SpeedCorrector;
use crate::statistic::{StatisticCollector, Summary};
use log::info;
use std::ops::DerefMut;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

pub const SPEED_CORRECTION_INVOKE_PERIOD: Duration = Duration::from_millis(100);
pub struct Orchestrator {
    new_proxy_receiver: Receiver<Pair>,
    pairs: Vec<Box<dyn Proxy>>,
    stat: Box<dyn StatisticCollector>,
    speed_corrector: SpeedCorrector,
    last_speed_correction_invoke: Instant,
}

impl Orchestrator {
    pub fn new_stat(
        new_proxy_receiver: Receiver<Pair>,
        stat: Box<dyn StatisticCollector>,
    ) -> Orchestrator {
        let pair: Vec<Box<dyn Proxy>> = vec![];
        Self {
            new_proxy_receiver,
            pairs: pair,
            stat,
            speed_corrector: SpeedCorrector::new(),
            last_speed_correction_invoke: Instant::now(),
        }
    }

    pub fn invoke(&mut self) {
        loop {
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
                if let Some(commands) = self.speed_corrector.append_and_get(summary) {
                    for command in commands.into_iter() {
                        if let Some(proxy) = self.get_by_key(&command.key) {
                            proxy
                                .try_send_command(RuntimeCommand::SetSpeed(command.speed))
                                .unwrap()
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
                return Some(&mut self.pairs[i]);
            }
        }
        None
    }

    fn check_new_connections(&mut self) -> bool {
        if let Ok(main_channel) = self.new_proxy_receiver.try_recv() {
            let proxy = VpnProxy::new(main_channel);
            self.pairs.push(Box::new(proxy));
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use crate::core::vpn_proxy::Proxy;
    use crate::objects::{HotPotatoInfo, ProxyState, RuntimeCommand, SentPacket};
    
    
    use crate::speed::INITIAL_SPEED;
    
    
    use std::ops::Sub;
    use std::sync::mpsc::{SendError, TryRecvError};
    use std::time::Duration;
    use std::time::Instant;

    struct TestProxy {
        key: String,
    }

    impl TestProxy {
        pub fn new() -> TestProxy {
            Self {
                key: "1".to_string(),
            }
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
            collected_info.data_packets[0] = Some(SentPacket {
                sent_date: fifty_ms_ago,
                sent_size: 10_000,
            });
            collected_info.filler_packets[0] = Some(SentPacket {
                sent_date: fifty_ms_ago,
                sent_size: 5_000,
            });
            Ok(ProxyState::Info(collected_info))
        }

        fn try_send_command(
            &mut self,
            _command: RuntimeCommand,
        ) -> Result<(), SendError<RuntimeCommand>> {
            Ok(())
        }
    }

    /*
    Проверяем что данные от Прокси сквозь оркестратор поступают в сборщик статистики

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
    */
}
