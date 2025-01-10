#[cfg(test)]
pub mod test_init {
    use std::sync::Once;
    use log::LevelFilter;
    use simplelog::{format_description, ConfigBuilder, SimpleLogger};

    static INIT: Once = Once::new();
    pub fn initialize_logger() {
        INIT.call_once(|| {
            let config = ConfigBuilder::new()
                .set_time_format_custom(format_description!("[hour]:[minute]:[second].[subsecond]"))
                .build();
            SimpleLogger::init(LevelFilter::Trace, config).expect("Логгер проинициализирован");
        });
    }
}


#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;
    use log::info;
    use crate::client_side_split::split_client_stream;
    use crate::packet::{create_packet_header, TYPE_DATA};
    use crate::server_side_split::split_server_stream;
    use crate::tests::test_init::initialize_logger;

    /**
    Сервер после подключения к нему шлет
    12345 в канал данных
    54321 в канал заполнителя
    23456 в канал заполнителя
    65432 в канал данных
    Все должно быть получено клиентом
     */
    #[test]
    fn all_received_test() {
        initialize_logger();
        info!("ALL_RECEIVED_TEST");
        let client_listener = TcpListener::bind(format!("127.0.0.1:{}", 11111))
            .expect("bind to client port");
        thread::spawn(move || {
            let stream = client_listener.accept().expect("client connected");
            let mut split = split_server_stream(stream.0);
            split.data_stream.write_all(b"11111").expect("write data");
            split.filler_stream.write_all(b"22222").expect("write filler");
            split.filler_stream.write_all(b"33333").expect("write filler");
            split.data_stream.write_all(b"44444").expect("write data");
        });
        thread::sleep(std::time::Duration::from_millis(100));

        let client_stream = TcpStream::connect(format!("127.0.0.1:{}", 11111)).unwrap();
        let mut split = split_client_stream(client_stream);
        let mut buf = [0; 10];
        split.data_stream.read(&mut buf).expect("read data");
        assert_eq!(b"11111", &buf[..5]);
        split.filler_stream.read(&mut buf).expect("read filler");
        assert_eq!(b"22222", &buf[..5]);

        let no_date_size = split.data_stream.read(&mut buf).expect("read data");
        assert_eq!(0, no_date_size, "Не должно быть данных");

        split.filler_stream.read(&mut buf).expect("read filler");
        assert_eq!(b"33333", &buf[..5]);
        split.data_stream.read(&mut buf).expect("read data");
        assert_eq!(b"44444", &buf[..5]);
    }

    #[test]
    fn slow_receive_test() {
        initialize_logger();
        info!("ALL_RECEIVED_TEST");
        let client_listener = TcpListener::bind(format!("127.0.0.1:{}", 11112))
            .expect("bind to client port");
        thread::spawn(move || {
            let mut stream = client_listener.accept().expect("client connected").0;
            let head_buf = create_packet_header(TYPE_DATA, 1000);
            stream.write_all(&head_buf[..2]).unwrap();
            stream.flush().unwrap();
            sleep(Duration::from_millis(10));
            stream.write_all(&head_buf[2..]).unwrap();
            let mut buf = [0; 100];
            buf[0] = 1;
            for _i in 0..10 {
                stream.write_all(&buf).unwrap();
                sleep(Duration::from_millis(3));
            }
        });
        sleep(Duration::from_millis(100));

        let client_stream = TcpStream::connect(format!("127.0.0.1:{}", 11112)).unwrap();
        let mut split = split_client_stream(client_stream);
        let mut buf = [0; 1000];
        let _ = split.data_stream.read(&mut buf).expect("read data");
        assert_eq!(1u8, buf[900]);
    }
}