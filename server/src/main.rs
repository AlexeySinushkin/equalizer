use std::{env, thread};
use std::io::Write;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;

use log::LevelFilter;
use simplelog::{Config, SimpleLogger};
use entry::entry_point::start_listen;


use crate::orchestrator::Orchestrator;
use crate::speed::native_to_regular;
use crate::statistic::{SimpleStatisticCollector, Summary};

mod objects;
mod orchestrator;
mod statistic;
mod speed;
mod entry;
mod core;

fn main() {
    SimpleLogger::init(LevelFilter::Info, Config::default()).expect("Логгер проинициализирован");
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("Example usage: ./equalizer 11194 1194");
        println!("11194 - to accept vpn clients");
        println!("1194 - OpenVPN listening port (tcp)");
        print!("
On client side
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

To run as service /absolute_path/equalizer 11194 1194 11196 --service
");
        return;
    }
    let proxy_listen_port: u16 = *&args.get(1).unwrap().parse().unwrap();
    let vpn_listen_port: u16 = *&args.get(2).unwrap().parse().unwrap();
    let service_mode: bool = args.len()>3 && args.get(3).unwrap().eq("--service"); //TODO to use some lib

    let (ct_pair, cr_pair) = channel();
    let (_ct_stop, cr_stop) = channel();
    let join = start_listen(proxy_listen_port, vpn_listen_port, ct_pair, cr_stop).unwrap();
    thread::spawn(move || {
        let pause = Duration::from_millis(50);
        let mut orchestrator = Orchestrator::new_stat(cr_pair,
                                                      Box::new(SimpleStatisticCollector::default()));
        loop{
            orchestrator.invoke();
            sleep(pause);
            orchestrator.invoke();
            sleep(pause);
            if !service_mode {
                orchestrator.invoke();
                if let Some(collected_info) = orchestrator.calculate_and_get() {
                    print_client_info(collected_info);
                }
            }
        }
    });
    join.join().unwrap();
}

fn print_client_info(collected_info: Vec<Summary>) {
    if !collected_info.is_empty() {
        let mut result : String = "".to_string();
        for client in collected_info.iter() {
            let calculated_speed = native_to_regular(client.calculated_speed);
            let target_speed = native_to_regular(client.target_speed);
            let stat_line = format!("\r{}- {:03}% / {:03}% {} / {}\t",
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
    let vec= vec![Summary::default()];
    print_client_info(vec);
}


