use std::env;
use log::LevelFilter;
use simplelog::{Config, SimpleLogger};
use crate::entry_point::listen;

mod throttler;
mod packet;
mod r#const;
mod bank;
mod vpn_proxy;
mod entry_point;
mod filler;
mod tests;

fn main() {
    SimpleLogger::init(LevelFilter::Info, Config::default()).expect("Логгер проинициализирован");
    let args: Vec<String> = env::args().collect();
    if args.len()<4 {
        println!("Example usage: ./equalizer 11194 1194 11196");
        println!("11194 - to accept vpn clients");
        println!("1194 - OpenVPN listening address (tcp)");
        println!("11196 - to accept filler clients");
        println!("Filler simple example");
        print!("
On client side
ssh -NT -L 11196:127.0.0.1:11196 11194:127.0.0.1:11194  vpn_server
then establish vpn connection to 11194:127.0.0.1
and filler connection to 11196:127.0.0.1
(both inside one ssh session)

#!/bin/sh
while true
do
 nc 127.0.0.1 11196 > /dev/null
 sleep 5
done
");
        return;
    }
    let proxy_listen_port: u16 = *&args.get(1).unwrap().parse().unwrap();
    let vpn_listen_port: u16 = *&args.get(2).unwrap().parse().unwrap();
    let filler_listen_port: u16 = *&args.get(3).unwrap().parse().unwrap();

    listen(proxy_listen_port, vpn_listen_port, filler_listen_port).unwrap();
}
