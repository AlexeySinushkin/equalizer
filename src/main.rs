use std::{env, thread};
use std::io::Write;



use std::sync::mpsc::{channel};
use std::thread::sleep;
use std::time::Duration;

use log::LevelFilter;
use simplelog::{Config, SimpleLogger};
use crate::entry_point::listen;


use crate::orchestrator::Orchestrator;
use crate::statistic::SimpleStatisticCollector;

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

fn main() {
    SimpleLogger::init(LevelFilter::Info, Config::default()).expect("Логгер проинициализирован");
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        println!("Example usage: ./equalizer 11194 1194 11196");
        println!("11194 - to accept vpn clients");
        println!("1194 - OpenVPN listening port (tcp)");
        println!("11196 - to accept filler clients");
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
                if !collected_info.is_empty() {
                    let mut result : String = "".to_string();
                    for client in collected_info.iter() {
                        let stat_line = format!("\r{}\t\t\t {:03}%/{:03}% \t  {}/{}\n",
                                                      client.key,
                                                      client.percent_data,
                                                      client.percent_filler,
                                                      client.calculated_speed / 1000,
                                                      client.target_speed / 1000);
                        result.push_str(&stat_line);
                    }
                    print!("{}", result);
                    for _lines in 0..collected_info.len() {
                        print!("\033[1A")
                    }
                    std::io::stdout().flush().unwrap();
                }
            }
        }
    });
    listen(proxy_listen_port, vpn_listen_port, filler_listen_port, ct_vpn, ct_filler).unwrap();
}
