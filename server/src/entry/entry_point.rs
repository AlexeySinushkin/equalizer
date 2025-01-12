use splitter::server_side_split::{split_server_stream, ClientDataStream, FillerDataStream};
use crate::objects::{Pair};
use log::{error, info};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{sleep, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{io, thread};
use easy_error::{Error, ResultExt};
use splitter::{DataStream, Split};
use splitter::server_side_vpn_stream::VpnDataStream;

pub fn start_listen(
    client_accept_port: u16,
    vpn_server_port: u16,
    ct_pair: Sender<Pair>,
    stop_application_request: Receiver<bool>,
) -> thread::Result<JoinHandle<()>> {
    let join = thread::spawn(move || {
        let client_listener = TcpListener::bind(format!("127.0.0.1:{}", client_accept_port))
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
    });
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
        let split = split_server_stream(client_stream);
        Pair {
            up_stream: Box::new(VpnDataStream::new(up_stream)),
            client_stream: split.data_stream,
            filler_stream: split.filler_stream,
            key: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string()
                .into(),
        }
    }
}


