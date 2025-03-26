use splitter::server_side_split::split_server_stream;
use crate::objects::{Pair, ONE_PACKET_MAX_SIZE};
use log::{error, info};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{io, thread};
use splitter::{DataStream};
use splitter::server_side_vpn_stream::VpnDataStream;

pub fn start_listen(
    client_accept_port: u16,
    vpn_server_port: u16,
    ct_pair: Sender<Pair>,
    stop_application_request: Receiver<bool>,
) -> thread::Result<JoinHandle<()>> {
    let join = thread::Builder::new()
        .name("server_listen".to_string()).spawn(move || {
        let client_listener = TcpListener::bind(format!("0.0.0.0:{}", client_accept_port))
            .expect("bind to client port");
        client_listener
            .set_nonblocking(true)
            .expect("TODO: panic message");
        let sleep_ms = std::time::Duration::from_millis(50);

        loop {
            match client_listener.accept() {
                Ok((stream, _addr)) => {
                    if let Ok(vpn_proxy) = handle_client(stream, vpn_server_port) {
                        let result = ct_pair.send(vpn_proxy);
                        if result.is_err() {
                            error!("VPN pipe is broken");
                        }
                    }
                }
                _ => {}
            }
            if let Ok(_) = stop_application_request.try_recv() {
                break;
            }
            sleep(sleep_ms);
        }
    }).expect("server_listen thread started");
    Ok(join)
}

fn handle_client(client_stream: TcpStream, vpn_server_port: u16) -> io::Result<Pair> {
    println!(
        "Client connected. Theirs address {:?}",
        client_stream.peer_addr()?
    );
    let result = TcpStream::connect(format!("127.0.0.1:{}", vpn_server_port));
    if result.is_ok() {
        info!("Connected to the VPN server!");
        Ok(Pair::new(result?, client_stream))
    } else {
        error!("Couldn't connect to VPN server...");
        client_stream.shutdown(Shutdown::Both)?;
        Err(result.err().unwrap())
    }
}

impl Pair {
    pub fn new(up_stream: TcpStream, client_stream: TcpStream) -> Pair {
        let mut split = split_server_stream(client_stream);
        let filler_stream = &mut split.filler_stream;
        //ожидаем пол секунды (нужно узнать имя клиента)
        sleep(Duration::from_millis(500));
        let key = Pair::get_key(filler_stream);
        Pair {
            up_stream: Box::new(VpnDataStream::new(up_stream)),
            client_stream: split.data_stream,
            filler_stream: split.filler_stream,
            key,
        }
    }

    fn get_key(filler_stream: &mut Box<dyn DataStream>) -> String {
        let mut buf: [u8; ONE_PACKET_MAX_SIZE] = [0; ONE_PACKET_MAX_SIZE];
        if let Ok(size) = filler_stream.read(&mut buf){
            if size > 1 && buf[0] == 0x01 {//TODO 0x01 - пакет авторизации добавить в какой-нибудь модуль обработку
                if let Ok(key) = std::str::from_utf8(&buf[1..size]){
                    return key.to_string();
                }
            }
        }
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string()
            .into()
    }
}


