use std::{env, thread};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};


use std::sync::mpsc::{channel};
use std::thread::sleep;
use std::time::Duration;

use log::LevelFilter;
use serial_test::serial;
use simplelog::{Config, SimpleLogger};
use crate::entry_point::listen;


use crate::orchestrator::Orchestrator;
use crate::speed::native_to_regular;
use crate::statistic::{ClientInfo, SimpleStatisticCollector};

mod throttler;
mod objects;
mod r#const;
mod vpn_proxy;
mod entry_point;
mod filler;
mod tests;
mod orchestrator;
mod statistic;
mod speed_correction;
mod speed;

fn main() {
    SimpleLogger::init(LevelFilter::Info, Config::default()).expect("Логгер проинициализирован");
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("Example usage: ./equalizer 11194 1194 11196");
        println!("11194 - to accept vpn clients");
        println!("1194 - OpenVPN listening port (tcp)");
        println!("11196 - to accept filler clients");
        print!("
ssh -NT -L 11196:127.0.0.1:11196 -L 11194:127.0.0.1:11194  vpn_server
then establish vpn connection to 11194:127.0.0.1
and filler connection to 11196:127.0.0.1
(both inside one ssh session)

Filler simple example
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

    let (ct_vpn, cr_vpn) = channel();
    let (ct_filler, cr_filler) = channel();

    thread::spawn(|| {
        let pause = Duration::from_millis(50);
        let mut orchestrator = Orchestrator::new_stat(cr_vpn, cr_filler,
                                                      Box::new(SimpleStatisticCollector::default()));
        loop{
            orchestrator.invoke();
            sleep(pause);
            orchestrator.invoke();
            sleep(pause);
            orchestrator.invoke();
            if let Some(collected_info) = orchestrator.calculate_and_get() {
                print_client_info(collected_info);
            }
        }
    });
    listen(proxy_listen_port, vpn_listen_port, filler_listen_port, ct_vpn, ct_filler).unwrap();
}

fn print_client_info(collected_info: Vec<ClientInfo>) {
    if !collected_info.is_empty() {
        let mut result : String = "".to_string();
        for client in collected_info.iter() {
            let calculated_speed = native_to_regular(client.calculated_speed);
            let target_speed = native_to_regular(client.target_speed);
            let stat_line = format!("\r{}-{:03}%/{:03}% {} / {}\t",
                                    client.key,
                                    client.percent_data,
                                    client.percent_filler,
                                    calculated_speed,
                                    target_speed);
            result.push_str(&stat_line);
        }
        print!("{}", result);
        std::io::stdout().flush().unwrap();
    }
}


#[test]
fn print_client_info_it() {
    let vec= vec![ClientInfo::default()];
    print_client_info(vec);
}

/*
Проверка, что статистика доходит до мейна
*/
#[test]
#[serial]
fn stat_goes_to_main() {
    const PROXY_LISTEN_PORT: u16 = 11190;
    const VPN_LISTEN_PORT: u16 = 11193;
    const FILLER_LISTEN_PORT: u16 = 11196;
    let (ct_vpn, cr_vpn) = channel();
    let (ct_filler, cr_filler) = channel();
    let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT)).unwrap();
    thread::spawn(move || {
        let pause = Duration::from_millis(50);
        let mut orchestrator = Orchestrator::new_stat(cr_vpn, cr_filler,
                                                      Box::new(SimpleStatisticCollector::default()));
        let mut buf: [u8; 100] = [0; 100];
        sleep(Duration::from_millis(200));
        let mut client_stream = TcpStream::connect(format!("127.0.0.1:{}", PROXY_LISTEN_PORT)).unwrap();
        sleep(Duration::from_millis(200));
        let mut client_filler_stream = TcpStream::connect(format!("127.0.0.1:{}", FILLER_LISTEN_PORT)).unwrap();
        let mut vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();


        loop{
            orchestrator.invoke();
            sleep(pause);
            client_stream.write(&buf).unwrap();
            vpn_stream.write(&buf).unwrap();
            client_filler_stream.read(&mut buf);

            orchestrator.invoke();
            sleep(pause);
            client_stream.read(&mut buf).unwrap();
            vpn_stream.read(&mut buf).unwrap();

            orchestrator.invoke();
            if let Some(collected_info) = orchestrator.calculate_and_get() {
                print_client_info(collected_info);
            }
        }
    });
    listen(PROXY_LISTEN_PORT, VPN_LISTEN_PORT, FILLER_LISTEN_PORT, ct_vpn, ct_filler).unwrap();
}
