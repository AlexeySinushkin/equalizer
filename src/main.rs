use std::{env, thread};
use std::io::Write;
use std::ops::Sub;
use std::sync::mpsc::{channel};
use std::time::{Duration, Instant};
use log::LevelFilter;
use simplelog::{Config, SimpleLogger};
use crate::entry_point::listen;
use objects::CollectedInfo;
use crate::objects::SentPacket;

mod throttler;
mod objects;
mod r#const;
mod vpn_proxy;
mod entry_point;
mod filler;
mod tests;
mod instance_lifecycle;

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

    let (ct_stat, cr_stat) = channel::<CollectedInfo>();
    thread::spawn(move || {
        let pbstr = " ".repeat(20).to_string();
        let mut data : Vec<SentPacket> = vec![];
        let mut filler : Vec<SentPacket> = vec![];
        let half_secs = Duration::from_millis(500);

        while let Ok(stat) = cr_stat.recv() {
            for i in 0..stat.data_count {
                data.push(stat.data_packets[i].unwrap());
            }
            for i in 0..stat.filler_count {
                filler.push(stat.filler_packets[i].unwrap());
            }
            let old_packets = Instant::now().sub(half_secs);
            while let Some(first) = data.first() {
                if first.sent_date<old_packets{
                    data.remove(0);
                }else{
                    break;
                }
            }
            while let Some(first) = filler.first() {
                if first.sent_date<old_packets{
                    filler.remove(0);
                }else{
                    break;
                }
            }

            let data_bytes: usize = data.iter()
                .map(|sp| { sp.sent_size })
                .reduce(|acc_size: usize, size| {
                    acc_size + size
            }).unwrap_or_else(|| {0});
            let filler_bytes: usize = filler.iter()
                .map(|sp| { sp.sent_size })
                .reduce(|acc_size: usize, size| {
                    acc_size + size
                }).unwrap_or_else(|| {0});



            let total_size = data_bytes + filler_bytes;
            if total_size==0{
                continue;
            }
            let percent_data = data_bytes * 100 / total_size;
            let percent_filler = filler_bytes * 100 / total_size;
            let avg_data_size: usize = match data.len() {
                0 => 0,
                _ => data_bytes / data.len()
            };
            print!("\r\t\t\t {:03}%/{:03}% \tavg data size {}{}", percent_data, percent_filler, avg_data_size, &pbstr);
            std::io::stdout().flush().unwrap();
        }
    });
    listen(proxy_listen_port, vpn_listen_port, filler_listen_port, ct_stat).unwrap();
}
