
//владеет всеми инстансами VpnProxy
//собирает статистику по ним и отправляет в анализатор изменения скорости

use std::net::TcpStream;
use std::sync::mpsc::{channel, Receiver};
use log::info;
use crate::objects::{ProxyState, RuntimeCommand};
use crate::vpn_proxy::VpnProxy;



pub struct Orchestrator {
    new_proxy_receiver: Receiver<VpnProxy>,
    new_filler_receiver: Receiver<TcpStream>,
    proxy_only: Vec<VpnProxy>,
    filler_only: Vec<TcpStream>,
    pair: Vec<VpnProxy>
}

pub struct StatisticCollector {
    receiver: Receiver<ProxyState>
}

impl Orchestrator {
    pub fn new(new_proxy_receiver: Receiver<VpnProxy>, new_filler_receiver: Receiver<TcpStream>) -> Orchestrator {
        let proxy_only: Vec<VpnProxy> = vec![];
        let filler_only: Vec<TcpStream> = vec![];
        let pair: Vec<VpnProxy> = vec![];
        Self { new_proxy_receiver, new_filler_receiver, proxy_only, filler_only, pair}
    }

    pub fn invoke(&mut self){
        loop {
            self.check_new_connections();
        }
    }

    fn check_new_connections(&mut self){
        if let Ok(proxy) = self.new_proxy_receiver.try_recv(){
            //ищем пару
            for i in 0..self.filler_only.len() {
                let filler = &self.filler_only[i];
                let filler_key=VpnProxy::get_key(&filler);
                if &proxy.key.eq(&filler_key){
                    info!("Pair {} case1", &proxy.key);
                    let filler = self.filler_only.remove(i);
                    proxy.ct_command.send(RuntimeCommand::SetFiller(filler)).unwrap();
                    self.pair.push(proxy);
                    return;
                }
            }
            //пары еще нет - добавляем в одиночки
            self.proxy_only.push(proxy);
        }
        if let Ok(filler) = self.new_filler_receiver.try_recv(){
            //ищем пару
            for i in 0..self.proxy_only.len() {
                let proxy = &self.proxy_only[i];
                let filler_key=VpnProxy::get_key(&filler);
                if &proxy.key.eq(&filler_key){
                    info!("Pair {} case2", &proxy.key);
                    proxy.ct_command.send(RuntimeCommand::SetFiller(filler)).unwrap();
                    let proxy = self.proxy_only.remove(i);
                    self.pair.push(proxy);
                    return;
                }
            }
            //пары еще нет - добавляем в одиночки
            self.filler_only.push(filler);
        }
    }

}