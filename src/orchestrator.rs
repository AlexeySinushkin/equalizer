
//владеет всеми инстансами VpnProxy
//собирает статистику по ним и отправляет в анализатор изменения скорости

use std::net::TcpStream;
use std::ops::{DerefMut};
use std::sync::mpsc::{Receiver};
use log::info;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::statistic::{StatisticCollector, NoStatistic, ClientInfo};
use crate::vpn_proxy::VpnProxy;



pub struct Orchestrator {
    new_proxy_receiver: Receiver<VpnProxy>,
    new_filler_receiver: Receiver<TcpStream>,
    proxy_only: Vec<VpnProxy>,
    filler_only: Vec<TcpStream>,
    pairs: Vec<VpnProxy>,
    stat: Box<dyn StatisticCollector>
}

impl Orchestrator {
    pub fn new(new_proxy_receiver: Receiver<VpnProxy>, new_filler_receiver: Receiver<TcpStream>) -> Orchestrator {
        let stat  =  Box::new(NoStatistic::default());
        Orchestrator::new_stat(new_proxy_receiver, new_filler_receiver, stat)
    }

    pub fn new_stat(new_proxy_receiver: Receiver<VpnProxy>,
                    new_filler_receiver: Receiver<TcpStream>,
                    stat: Box<dyn StatisticCollector>) -> Orchestrator {
        let proxy_only: Vec<VpnProxy> = vec![];
        let filler_only: Vec<TcpStream> = vec![];
        let pair: Vec<VpnProxy> = vec![];
        Self { new_proxy_receiver, new_filler_receiver, proxy_only, filler_only, pairs: pair, stat }
    }


    pub fn invoke(&mut self){
        while let received = self.check_new_connections() {
            if !received {
                break;
            }
        }

        self.receive_proxy_state();
    }

    pub fn calculate_and_get(&mut self) -> Option<Vec<ClientInfo>> {
        self.stat.calculate_and_get()
    }

    fn receive_proxy_state(&mut self) {
        for i in 0..self.pairs.len() {
            let proxy = &self.pairs[i];
            if let Ok(state) = proxy.cr_state.try_recv() {
                let stat = self.stat.deref_mut();
                match state {
                    ProxyState::SetupComplete => {
                        info!("SetupComplete {}", &proxy.key);
                    }
                    ProxyState::Info(collected_info) => {
                        stat.append_info(&proxy.key, collected_info);
                    }
                    ProxyState::Broken => {
                        info!("Broken {}", &proxy.key);
                        stat.clear_info(&proxy.key);
                    }
                }
            }
        }
    }


    fn check_new_connections(&mut self) -> bool{
        if let Ok(proxy) = self.new_proxy_receiver.try_recv(){
            //ищем пару
            for i in 0..self.filler_only.len() {
                let filler = &self.filler_only[i];
                let filler_key=VpnProxy::get_key(&filler);
                if proxy.key.eq(&filler_key){
                    info!("Pair {} case1", &proxy.key);
                    let filler = self.filler_only.remove(i);
                    self.ensure_previous_session_is_destroyed(&filler_key);
                    proxy.ct_command.send(RuntimeCommand::SetFiller(filler)).unwrap();
                    self.pairs.push(proxy);
                    return true;
                }
            }
            //пары еще нет - добавляем в одиночки
            info!("Proxy {} was received ", &proxy.key);
            self.proxy_only.push(proxy);
            return true;
        }
        if let Ok(filler) = self.new_filler_receiver.try_recv(){
            //ищем пару
            for i in 0..self.proxy_only.len() {
                let proxy = &self.proxy_only[i];
                let filler_key=VpnProxy::get_key(&filler);
                if proxy.key.eq(&filler_key) {
                    info!("Pair {} case2", &proxy.key);
                    //self.ensure_previous_session_is_destroyed(&filler_key);
                    proxy.ct_command.send(RuntimeCommand::SetFiller(filler)).unwrap();
                    let proxy = self.proxy_only.remove(i);
                    self.pairs.push(proxy);
                    return true;
                }
            }
            //пары еще нет - добавляем в одиночки
            info!("Filler {} was received ", VpnProxy::get_key(&filler));
            self.filler_only.push(filler);
            return true;
        }
        return false;
    }

    fn ensure_previous_session_is_destroyed(&mut self, key: &String) {
        for i in 0..self.pairs.len() {
            if self.pairs[i].key.eq(key){
                self.pairs.remove(i);
                //TODO завершить поток, если он еще живой
            }
        }
    }



}