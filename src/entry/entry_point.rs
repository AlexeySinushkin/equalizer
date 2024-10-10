use std::{io, thread};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{sleep, JoinHandle};
use log::{error, info};

/**
Главный канал данных
 */
pub struct MainChannel {
    pub up_stream: TcpStream,
    pub client_stream: TcpStream,
    pub key: String,
}

impl MainChannel {
    pub fn new(up_stream: TcpStream, client_stream: TcpStream) -> MainChannel {
        let key = MainChannel::get_key(&client_stream);
        Self {up_stream, client_stream, key}
    }

    fn get_key(stream: &TcpStream) -> String {
        stream.peer_addr().unwrap().ip().to_string()
    }
}

pub struct FillerChannel {
    pub client_stream: TcpStream,
    pub key: String,
}

impl FillerChannel {
    pub fn new(client_stream: TcpStream) -> FillerChannel {
        let key = FillerChannel::get_key(&client_stream);
        Self {client_stream, key}
    }

    fn get_key(stream: &TcpStream) -> String {
        stream.peer_addr().unwrap().ip().to_string()
    }
}

pub fn start_listen(client_accept_port: u16, vpn_server_port: u16, filler_port: u16,
                    ct_vpn: Sender<MainChannel>,
                    ct_filler: Sender<FillerChannel>,
                    stop: Receiver<bool>) -> std::thread::Result<JoinHandle<()>> {
    let join = thread::spawn(move || {
        let client_listener = TcpListener::bind(format!("127.0.0.1:{}", client_accept_port)).expect("bind to client port");
        client_listener.set_nonblocking(true).expect("TODO: panic message");
        let filler_listener = TcpListener::bind(format!("127.0.0.1:{}", filler_port)).expect("bind to client filler port");
        filler_listener.set_nonblocking(true).expect("TODO: panic message");
        let sleep_ms = std::time::Duration::from_millis(50);
        loop {
            match client_listener.accept() {
                Ok((stream, _addr)) => {
                    if let Ok(vpn_proxy) = handle_client(stream, vpn_server_port) {
                        let result = ct_vpn.send(vpn_proxy);
                        if result.is_err() {
                            error!("VPN pipe is broken");
                        }
                    }
                }
                _=>{}
            }
            match filler_listener.accept() {
                Ok((stream, _addr)) => {
                    let result = ct_filler.send(FillerChannel::new(stream));
                    if result.is_err() {
                        error!("Filler's pipe is broken");
                    }
                }
                _=>{}
            }
            if let Ok(_) = stop.try_recv(){
                break;
            }
            sleep(sleep_ms);
        }
    });
    Ok(join)
}

fn handle_client(client_stream: TcpStream, vpn_server_port: u16) -> io::Result<MainChannel> {
    println!("Client connected. Theirs address {:?}", client_stream.peer_addr().unwrap());
    let result = TcpStream::connect(format!("127.0.0.1:{}", vpn_server_port));
    if result.is_ok() {
        info!("Connected to the VPN server!");
        Ok(MainChannel::new(result.unwrap(), client_stream))
    } else {
        error!("Couldn't connect to VPN server...");
        client_stream.shutdown(Shutdown::Both)?;
        Err(result.err().unwrap())
    }
}


