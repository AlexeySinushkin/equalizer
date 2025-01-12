#[cfg(test)]
pub mod test_init {
    use log::LevelFilter;
    use simplelog::{format_description, ConfigBuilder, SimpleLogger};
    use std::sync::Once;

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
    use crate::client_side_split::split_client_stream;
    use crate::packet::{create_packet_header, TYPE_DATA};
    use crate::server_side_split::split_server_stream;
    use crate::tests::test_init::initialize_logger;
    use log::{info};
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::thread::sleep;
    use std::time::Duration;
    use crate::READ_START_AWAIT_TIMEOUT;

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
        let client_listener =
            TcpListener::bind(format!("127.0.0.1:{}", 11111)).expect("bind to client port");

        thread::spawn(move || {
            let stream = client_listener.accept().expect("client connected");
            let mut split = split_server_stream(stream.0);
            split.data_stream.write_all(b"11111").expect("write data"); //0x49
            split
                .filler_stream
                .write_all(b"22222")
                .expect("write filler"); //0x50
            split
                .filler_stream
                .write_all(b"33333")
                .expect("write filler"); //0x51
            split.data_stream.write_all(b"44444").expect("write data"); //0x52
        });
        thread::sleep(std::time::Duration::from_millis(100));

        let client_stream = TcpStream::connect(format!("127.0.0.1:{}", 11111)).unwrap();
        let mut split = split_client_stream(client_stream);
        let mut buf = [0; 5];
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
        let client_listener =
            TcpListener::bind(format!("127.0.0.1:{}", 11112)).expect("bind to client port");
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

    /**
        Проверяем какой код ОШИБКИ возвращает Rust если нет данных
    */
    #[test]
    fn no_data_receive_test() {
        initialize_logger();
        let client_listener =
            TcpListener::bind(format!("127.0.0.1:{}", 51113)).expect("bind to client port");

        let join_handle = thread::spawn(move || {
            let client_stream = TcpStream::connect(format!("127.0.0.1:{}", 51113)).unwrap();
            client_stream
                .set_read_timeout(Some(READ_START_AWAIT_TIMEOUT))
                .expect("Должен быть не блокирующий метод чтения");
            let mut clone = client_stream.try_clone().unwrap();
            sleep(Duration::from_millis(500));
            let mut buf = [0; 10];
            let _ = clone.write(&mut buf);
        });

        let mut incoming_stream = client_listener.accept().expect("client connected").0;

        info!("Connected, begin read absent data");

        incoming_stream
            .set_read_timeout(Some(Duration::from_millis(10)))
            .expect("Должен быть не блокирующий метод чтения");

        info!("Begin read data");
        let mut buf = [0; 10];
        let result = incoming_stream.read(&mut buf[..]);
        //no data considering as error by Rust...
        let error = result.err().unwrap();
        //https://users.rust-lang.org/t/best-practices-for-handling-io-errors-in-libraries/115945/5
        assert_eq!(true, error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut);
        info!("{}", error);
        sleep(Duration::from_millis(600));

        let size = incoming_stream.read(&mut buf[..]).unwrap();
        assert_eq!(size, 10);
        join_handle.join().expect("join");
    }

    #[test]
    fn no_data_no_error() {
        initialize_logger();
        let client_listener =
            TcpListener::bind(format!("127.0.0.1:{}", 51114)).expect("bind to client port");

        let join_handle = thread::spawn(move || {
            let client_stream = TcpStream::connect(format!("127.0.0.1:{}", 51114)).unwrap();
            client_stream
                .set_read_timeout(Some(Duration::from_millis(10)))
                .expect("Должен быть не блокирующий метод чтения");
            let _clone = client_stream.try_clone().unwrap();
            sleep(Duration::from_secs(2));
        });

        let incoming_stream = client_listener.accept().expect("client connected").0;

        info!("Connected, begin read absent data");
        incoming_stream
            .set_read_timeout(Some(Duration::from_millis(10)))
            .expect("Должен быть не блокирующий метод чтения");
        let mut split = split_server_stream(incoming_stream);

        info!("Begin read data");
        let mut buf = [0; 10];
        //читаем 0 (количество прочитанного), если ничего не прочитали
        let size = split.data_stream.read(&mut buf[..]).unwrap();

        assert_eq!(size, 0);
        join_handle.join().expect("join");
    }
}
