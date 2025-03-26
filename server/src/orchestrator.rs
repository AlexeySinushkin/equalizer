//владеет всеми инстансами VpnProxy
//собирает статистику по ним и отправляет в анализатор изменения скорости

use crate::core::vpn_proxy::{Proxy, VpnProxy};
use crate::objects::Pair;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::speed::SpeedCorrector;
use crate::statistic::{StatisticCollector, Summary};
use log::{info, warn};
use std::ops::DerefMut;
use std::sync::mpsc::Receiver;
#[cfg(test)]
use easy_error::{bail, ResultExt, Error};

pub struct Orchestrator {
    new_proxy_receiver: Receiver<Pair>,
    pub(crate) pairs: Vec<Box<dyn Proxy>>,
    stat: Box<dyn StatisticCollector>,
    speed_corrector: SpeedCorrector
}

impl Orchestrator {
    pub fn new(
        new_proxy_receiver: Receiver<Pair>,
        stat: Box<dyn StatisticCollector>,
    ) -> Orchestrator {
        let pair: Vec<Box<dyn Proxy>> = vec![];
        Self {
            new_proxy_receiver,
            pairs: pair,
            stat,
            speed_corrector: SpeedCorrector::new()
        }
    }

    pub fn invoke(&mut self) {
        loop {
            if !self.check_new_connections() {
                break;
            }
        }
        self.receive_proxy_state();
    }


    pub fn calculate_and_get(&mut self) -> Option<Vec<Summary>> {
        self.stat.calculate_and_get()
    }

    fn receive_proxy_state(&mut self) {
        for i in 0..self.pairs.len() {
            let proxy = self.pairs[i].deref_mut();
            if let Ok(state) = proxy.try_recv_state() {
                let stat = self.stat.deref_mut();
                let sc = &mut self.speed_corrector;
                match state {
                    ProxyState::SetupComplete => {
                        info!("SetupComplete {}", &proxy.get_key());
                    }
                    ProxyState::Info(collected_info) => {
                        if let Some(command) =
                            sc.append_and_get(proxy.get_key(), &collected_info) {
                            if proxy.try_send_command(RuntimeCommand::SetSpeed(command)).is_err() {
                                warn!("Ошибка отправки команды изменения скорости для {}", proxy.get_key());
                            }
                        }
                        stat.append_info(proxy.get_key(), collected_info);
                    }
                    ProxyState::Broken => {
                        info!("Broken {}", proxy.get_key());
                        stat.clear_info(proxy.get_key());
                        sc.clear_info(proxy.get_key());
                        self.pairs.remove(i);
                        break;
                    }
                }
            }
        }
    }

    #[cfg(test)]
    pub fn send_command(&mut self, key: &String, command: RuntimeCommand) -> Result<(), Error> {
        if let Some(proxy) = self.get_by_key(key) {
            return proxy.try_send_command(command)
                .context("Failed to send runtime command");
        }
        self.pairs.iter().for_each(|pair| {
            info!("proxy: {}", pair.get_key());
        });
        bail!("Не найден прокси по ключу, {}", self.pairs.len());
    }
    #[cfg(test)]
    fn get_by_key(&mut self, key: &String) -> Option<&mut Box<dyn Proxy>> {
        for i in 0..self.pairs.len() {
            let proxy = self.pairs[i].deref_mut();
            if proxy.get_key().eq(key) {
                return Some(&mut self.pairs[i]);
            }
        }
        None
    }
    #[cfg(test)]
    pub fn get_pairs_count(&mut self) -> usize {
        self.pairs.len()
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

