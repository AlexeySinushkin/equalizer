use std::io::Write;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::time::Duration;
use std::{env, thread};
use std::fs::File;
use entry::entry_point::start_listen;
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, Config, SimpleLogger, TermLogger, TerminalMode, WriteLogger};

use crate::orchestrator::Orchestrator;
use crate::speed::{native_to_regular};
use crate::statistic::{SimpleStatisticCollector, Summary};

mod core;
mod entry;
mod tests;
mod objects;
mod orchestrator;
mod speed;
mod statistic;
mod c_client_tests;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("Example usage: ./equalizer 12010 1194");
        println!("12010 - to accept vpn clients");
        println!("1194 - OpenVPN listening port (tcp)");
        print!(
            "
On client side
ssh -NT -L 12010:127.0.0.1:12010 -L vpn_server
To run as service /absolute_path/equalizer 12010 1194 --service
"
        );
        return;
    }
    let proxy_listen_port: u16 = *&args.get(1).unwrap().parse().unwrap();
    let vpn_listen_port: u16 = *&args.get(2).unwrap().parse().unwrap();
    let service_mode: bool = args.len() > 3 && args.get(3).unwrap().eq("--service"); //TODO to use some lib
    if service_mode {
        SimpleLogger::init(LevelFilter::Info, Config::default()).expect("Логгер проинициализирован");
    }else {
        // Initialize logging to both console and file
        CombinedLogger::init(vec![
            // Console logger with colors
            TermLogger::new(
                LevelFilter::Debug,  // Logs everything from Debug and above
                Config::default(),
                TerminalMode::Mixed, // Mixed mode: colored output when supported
                ColorChoice::Auto    // Automatically select color mode
            ),
            // File logger
            WriteLogger::new(
                LevelFilter::Trace,   // Logs Info and above to the file
                Config::default(),
                File::create("app.log").unwrap(),
            ),
        ]).expect("Логгер проинициализирован");
    }
    let (ct_pair, cr_pair) = channel();
    let (_ct_stop, cr_stop) = channel();
    let join = start_listen(proxy_listen_port, vpn_listen_port, ct_pair, cr_stop).unwrap();
    thread::Builder::new()
        .name("orchestrator".to_string()).spawn(move || {
        let pause = Duration::from_millis(50);
        let mut orchestrator =
            Orchestrator::new(cr_pair, Box::new(SimpleStatisticCollector::default()));
        loop {
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
    }).expect("orchestrator thread started");
    join.join().unwrap();
}

fn print_client_info(collected_info: Vec<Summary>) {
    if !collected_info.is_empty() {
        let mut result: String = "".to_string();
        for client in collected_info.iter() {
            let calculated_speed = native_to_regular(client.calculated_speed);
            let target_speed = native_to_regular(client.target_speed);
            let stat_line = format!(
                "\r{}- {:03}% / {:03}% {} / {}\t",
                client.key,
                client.percent_data,
                client.percent_filler,
                calculated_speed,
                target_speed
            );
            result.push_str(&stat_line);
        }
        print!("{}", result);
        std::io::stdout().flush().unwrap();
    }
}

#[test]
fn print_client_info_it() {
    let vec = vec![Summary::default()];
    print_client_info(vec);
}
