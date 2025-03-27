
#[cfg(test)]
pub mod test_init {
    use std::sync::Once;
    use log::LevelFilter;
    use simplelog::{format_description, ConfigBuilder, SimpleLogger};

    static INIT: Once = Once::new();
    pub fn initialize_logger() {
        INIT.call_once(|| {
            let config = ConfigBuilder::new()
                .set_time_format_custom(format_description!("[minute]:[second].[subsecond]"))
                .build();
            SimpleLogger::init(LevelFilter::Trace, config).expect("Логгер проинициализирован");
        });
    }
}

#[cfg(test)]
mod tests {
    
    use std::fs::File;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::ops::Deref;
    use std::rc::Rc;
    use std::sync::mpsc;
    use std::sync::mpsc::{channel, Sender};
    use std::thread;
    use std::thread::{sleep, JoinHandle};
    use std::time::{Duration, Instant};
    use log::{error, info, trace};
    use rand::Rng;
    use rand::rngs::ThreadRng;
    use serial_test::serial;
    use splitter::client_side_split::{split_client_stream, squash, DataStreamFiller, DataStreamVpn};
    use crate::orchestrator::Orchestrator;
    use crate::statistic::{NoStatistic, SimpleStatisticCollector, StatisticCollector};
    use crate::tests::test_init::initialize_logger;
    use crate::entry::entry_point::*;
    use crate::objects::{RuntimeCommand, ONE_PACKET_MAX_SIZE};
    use crate::speed::{to_native_speed, to_regular_speed, SpeedCorrectorCommand};

    const TEST_BUF_SIZE: usize = 100 * 1024;
    const PROXY_LISTEN_PORT: u16 = 11100;
    const VPN_LISTEN_PORT: u16 = 11200;
    const TEST_CLIENT_NAME: &str = "test_client";

    struct TestStreams {
        vpn_stream: TcpStream,
        //мок клиента
        client_data_stream: Rc<dyn DataStreamVpn>,
        //мок филлера (клиента)
        client_filler_stream: Rc<dyn DataStreamFiller>,
        orchestrator: Orchestrator,
        join_handle: (Sender<bool>, JoinHandle<()>)
    }

    fn create_test_streams(offset: u16, stat_collector: Option<Box<dyn StatisticCollector>>) -> TestStreams {
        //первым делом должен быть запущен наш OpenVPN (tcp mode)
        let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT+offset)).unwrap();

        let (ct_vpn, cr_vpn) = channel();
        let mut orchestrator = Orchestrator::new(cr_vpn, stat_collector.unwrap_or_else(||
            Box::new(NoStatistic::default())));
        //дальше готовимся принимать клиентов
        let (ct_stop, cr_stop) = channel();
        let join = start_listen(PROXY_LISTEN_PORT+offset, VPN_LISTEN_PORT+offset, ct_vpn, cr_stop).unwrap();
        sleep(Duration::from_millis(200));
        let client_stream = TcpStream::connect(format!("127.0.0.1:{}", PROXY_LISTEN_PORT+offset)).unwrap();
        let vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();
        vpn_stream.set_read_timeout(Some(Duration::from_millis(10))).expect("Должен быть не блокирующий метод чтения");
        let split = split_client_stream(client_stream);

        send_client_name_packet(split.filler_stream.clone());
        trace!("Ждем подключения Заполнителя");
        orchestrator.invoke();
        TestStreams {vpn_stream,
            client_data_stream: split.data_stream,
            client_filler_stream: split.filler_stream,
            orchestrator,
            join_handle: (ct_stop, join) }
    }

    fn send_client_name_packet(filler_stream: Rc<dyn DataStreamFiller>) {
        let mut buf: [u8; 16] = [0; 16];
        buf[0] = 0x01;
        let client_name = TEST_CLIENT_NAME.as_bytes();
        buf[1..client_name.len()+1].copy_from_slice(client_name);
        info!("Отправляем имя клиента");
        filler_stream.deref().write_all(&buf[..client_name.len()+1]).unwrap();
    }


    /**
    Создаем мок VPN сервер:11194 и мок клиент.
    Проверяем что все данные ходят от мок клиента к серверу (через прокси:11195)
    и другие данные ходят от сервера к клиенту
     */
    #[test]
    #[serial]
    fn all_received_test() {
        initialize_logger();
        info!("ALL_RECEIVED_TEST");
        //первым делом должен быть запущен наш OpenVPN (tcp mode)
        let mock_vpn_listener = TcpListener::bind(format!("127.0.0.1:{}", VPN_LISTEN_PORT)).unwrap();
        let (ct_vpn, cr_vpn) = channel();
        let mut orchestrator = Orchestrator::new(cr_vpn, Box::new(NoStatistic::default()));
        //дальше готовимся принимать клиентов
        let (ct_stop, cr_stop) = channel();
        start_listen(PROXY_LISTEN_PORT, VPN_LISTEN_PORT, ct_vpn, cr_stop).unwrap();
        sleep(Duration::from_millis(500));

        //Создаем 2 массива по 1MB заполняем случайными данными
        //А также 2 пустых массива по 1МБ для приемки данных
        let client_proxy_stream = TcpStream::connect(format!("127.0.0.1:{}", PROXY_LISTEN_PORT)).unwrap();
        sleep(Duration::from_millis(500));
        let client_to_proxy = get_random_buf();
        let vpn_to_proxy = get_random_buf();

        let proxy_vpn_stream = mock_vpn_listener.incoming().next().unwrap().unwrap();
        orchestrator.invoke();

        //в двух разных потоках отправляем данные случайными порциями от 10 до 2000 за раз.
        //и делая при этом паузы от 10 до 73мс
        let join_handle_client = thread::Builder::new()
            .name("test_client".to_string()).spawn(move || {
            let mut proxy_to_vpn: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
            let stream = client_side_write_and_read(client_proxy_stream, &client_to_proxy, &mut proxy_to_vpn,  "client");
            return (proxy_to_vpn, stream);
        }).unwrap();

        let join_handle_server = thread::Builder::new()
            .name("test_vpn".to_string()).spawn(move || {
            let mut proxy_to_client: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
            let stream = server_side_write_and_read(proxy_vpn_stream, &vpn_to_proxy, &mut proxy_to_client, "vpn   ");
            return (proxy_to_client, stream);
        }).unwrap();

        let proxy_to_vpn = join_handle_client.join().unwrap();
        let proxy_to_client = join_handle_server.join().unwrap();

        let compare_result1 = compare("client->vpn", &client_to_proxy, &proxy_to_client.0);
        let compare_result2 = compare("vpn->client", &proxy_to_vpn.0, &vpn_to_proxy);

        for i in 0..10 {
            info!("{:#02x} {:#02x} {:#02x} {:#02x}", client_to_proxy[i], proxy_to_client.0[i],
                proxy_to_vpn.0[i], vpn_to_proxy[i])
        }
        ct_stop.send(true).unwrap();
        assert!(compare_result1);
        assert!(compare_result2);
    }

    fn client_side_write_and_read(stream: TcpStream, out_buf: &[u8], in_buf: &mut [u8], name: &str) -> TcpStream {
        let mut out_file = File::create("target/client-out.bin").unwrap();
        let mut in_file = File::create("target/client-in.bin").unwrap();

        stream.set_read_timeout(Some(Duration::from_millis(10))).expect("Должен быть не блокирующий метод чтения");
        let split= split_client_stream(stream);
        let stream = split.data_stream.clone();
        let filler = split.filler_stream.clone();
        let mut rng = rand::rng();
        let mut write_left_size = TEST_BUF_SIZE; //сколько байт осталось записать из буфера
        let mut write_offset = 0; //смещение указателя
        let mut read_size = 0;
        let mut filler_buf: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];

        while write_left_size > 0 || read_size < TEST_BUF_SIZE {
            let filler_size = filler.read(&mut filler_buf).unwrap();
            if filler_size > 0 {
                error!("filler packet appear {filler_size}");
            }
            //отправляем в поток данные из out_buf
            if write_left_size > 0 {
                let mut to_send_size: usize = get_amount_to_send_size(&mut rng) as usize;
                if to_send_size > write_left_size {
                    to_send_size = write_left_size
                }
                trace!("{:?} Отправляем в сторону сервера {:?} ", name, to_send_size);
                stream.write_all(&out_buf[write_offset..write_offset + to_send_size]).unwrap();
                let _ = out_file.write_all(&out_buf[write_offset..write_offset + to_send_size]);

                write_offset += to_send_size;
                write_left_size -= to_send_size;

                let sleep_ms = Duration::from_millis(get_sleep_ms(&mut rng) as u64);
                trace!("{:?} >> Отправили, засыпаем на {:?}", name, sleep_ms);
                sleep(sleep_ms);
            }
            //заполняем из потока данные в in_buf
            if read_size < TEST_BUF_SIZE {
                //trace!("{:?} Собираемся читать", name);
                if let Ok(read) = stream.read(&mut in_buf[read_size..]) {
                    let _ = in_file.write_all(&in_buf[read_size..read_size + read]);
                    //trace!("{:?} Прочитали {:?} ", name, read);
                    if read==0 {
                        sleep(Duration::from_millis(50));
                        info!("{name} Осталось отправить {write_left_size}, получить {}", TEST_BUF_SIZE-read_size);
                    }
                    read_size += read;
                }
            }
        }
        trace!("Поток завершён {:?}", thread::current().name());
        squash(split)
    }

    fn server_side_write_and_read(mut stream: TcpStream, out_buf: &[u8], in_buf: &mut [u8], name: &str) -> TcpStream{
        let mut out_file = File::create("target/server-out.bin").unwrap();
        let mut in_file = File::create("target/server-in.bin").unwrap();

        stream.set_read_timeout(Some(Duration::from_millis(10))).expect("Должен быть не блокирующий метод чтения");
        let mut rng = rand::rng();
        let mut write_left_size = TEST_BUF_SIZE; //сколько байт осталось записать из буфера
        let mut write_offset = 0; //смещение указателя
        let mut read_size = 0;

        while write_left_size > 0 || read_size < TEST_BUF_SIZE {
            //отправляем в поток данные из out_buf
            if write_left_size > 0 {
                let mut to_send_size: usize = get_amount_to_send_size(&mut rng) as usize;
                if to_send_size > write_left_size {
                    to_send_size = write_left_size
                }
                trace!("{:?} Пытаемся отправить {:?} ", name, to_send_size);
                stream.write_all(&out_buf[write_offset..write_offset + to_send_size]).unwrap();
                let _ = out_file.write_all(&out_buf[write_offset..write_offset + to_send_size]);

                write_offset += to_send_size;
                write_left_size -= to_send_size;
                let sleep_ms = Duration::from_millis(get_sleep_ms(&mut rng) as u64);
                trace!("{:?} >> Отправили, засыпаем на {:?}", name, sleep_ms);
                sleep(sleep_ms);
            }
            //заполняем из потока данные в in_buf
            if read_size < TEST_BUF_SIZE {
                //trace!("{:?} Собираемся читать", name);
                if let Ok(read) = stream.read(&mut in_buf[read_size..]) {
                    let _ = in_file.write_all(&in_buf[read_size..read_size + read]);
                    //trace!("{:?} Прочитали {:?} ", name, read);
                    if read==0 {
                        sleep(Duration::from_millis(50));
                        info!("{name} Осталось отправить {write_left_size}, получить {}", TEST_BUF_SIZE-read_size)
                    }
                    read_size += read;
                }
            }
        }
        info!("Поток сервера завершён {:?}", thread::current().name());
        stream
    }

    fn compare(pair: &str, left: &[u8], right: &[u8]) -> bool {
        for i in 0..TEST_BUF_SIZE {
            if left[i] != right[i] {
                error!("Ошибка в паре {pair} по индексу {i} {} {}", left[i], right[i]);
                return false;
            }
        }
        return true;
    }

    fn get_amount_to_send_size(rng: &mut ThreadRng) -> u16 {
        let mut rnd: u16 = rng.random();
        rnd = (rnd + 10) & 0x7FF;
        rnd
    }

    fn get_sleep_ms(rng: &mut ThreadRng) -> u16 {
        let mut rnd: u16 = rng.random();
        rnd = rnd & 0x3F;
        if rnd < 10 {
            rnd = 10;
        }
        rnd
    }

    fn get_random_buf() -> [u8; TEST_BUF_SIZE] {
        let mut buf: [u8; TEST_BUF_SIZE] = [0; TEST_BUF_SIZE];
        let mut rng = rand::rng();
        for i in 0..TEST_BUF_SIZE {
            buf[i] = rng.random()
        }
        buf
    }

    /*

    Проверяем что после подключения к заполнителю, он начинает слать данные
    При скорости 10Мбит/с за 1.1с мы должны получить 1Мб полезных данных и 100кб заполнителя
     */
    #[test]
    #[inline(never)]
    fn filler_attach_and_fill() {
        initialize_logger();
        info!("FILLER_ATTACH_AND_FILL");
        const BUF_SIZE: usize = 12_000;
        const PACKETS_COUNT: usize = 50;
        const HALF_SECOND_MS: usize = 500;
        const DELAY_MS:usize = HALF_SECOND_MS /PACKETS_COUNT;
        //10 Мбит/с = 1МБ/с = 1048 байт/мс
        pub const INITIAL_SPEED: usize = 1024 * 1024 / 1000;
        const ONE_PACKET_SIZE: usize = INITIAL_SPEED * HALF_SECOND_MS / PACKETS_COUNT;
        //размер пакета примерно 10_000
        let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

        let TestStreams {
            mut vpn_stream,
            client_data_stream,
            client_filler_stream,
            mut orchestrator,
            join_handle
        } = create_test_streams(1, None);

        if let Ok(vpn_read) = client_data_stream.read(&mut buf) {
            info!("Полученных полезных байт должно быть 10 => {}", vpn_read);
        }

        let half_secs = Duration::from_millis((HALF_SECOND_MS) as u64);
        sleep(half_secs);
        orchestrator.invoke();
        let (tx, rx) = mpsc::channel();
        //отправляем пол секунды объем данных который должен уйти за пол секунды
        //ждем 100мс - ничего не отправляем
        //отправляем пол секунды объем данных который должен уйти за пол секунды
        let join_handle_2 = thread::Builder::new()
            .name("send".to_string()).spawn(move || {
            let value_data: [u8; ONE_PACKET_SIZE] = [0; ONE_PACKET_SIZE];
            tx.send(true).unwrap();
            trace!("---------Начали отправку");
            let start = Instant::now();
            //Забиваем таким количеством, которого должно хватить на пол секунды
            for _i in 0..PACKETS_COUNT {
                trace!(r"-->  {} мс. {ONE_PACKET_SIZE}", start.elapsed().as_millis());
                vpn_stream.write_all(&value_data[..]).expect("Отправка полезных данных от прокси");
                sleep(Duration::from_millis(DELAY_MS as u64));
            }

            info!("AWAITING HALF SECOND");
            while start.elapsed() <= half_secs {
                sleep(Duration::from_millis(1))
            }
            info!("FILLER SHOULD START NOW");
            sleep(Duration::from_millis(100));
            info!("NO FILLER SHOULD BE AFTER NOW");
            for _i in 0..PACKETS_COUNT {
                vpn_stream.write_all(&value_data[..]).expect("Отправка полезных данных от прокси");
            }
            return vpn_stream;
        }).unwrap();


        let receive_time = Duration::from_millis(1100);
        rx.recv().unwrap();
        trace!("---------Начали получение");
        let start = Instant::now();
        let mut data_offset = 0;
        let mut filler_offset = 0;
        orchestrator.send_command(&TEST_CLIENT_NAME.to_string(),
                                  RuntimeCommand::SetSpeed(SpeedCorrectorCommand::SetSpeed(INITIAL_SPEED))).unwrap();

        loop {
            let data_read = client_data_stream.read(&mut buf[..]).unwrap();
            let filler_read = client_filler_stream.read(&mut buf[..]).unwrap();
            data_offset += data_read;
            filler_offset += filler_read;
            if start.elapsed() > receive_time {
                info!("Прошло {}. Окончили ожидание.", start.elapsed().as_millis());
                break;
            }

            if data_read > 0 || filler_read > 0 {
                trace!(r"<-- {} мс. {data_offset} {filler_offset}", start.elapsed().as_millis());
            }
            sleep(Duration::from_millis(DELAY_MS as u64));
        }
        info!("Получено поезных данных {}, заполнителя {}", data_offset, filler_offset);
        let _ = join_handle_2.join().unwrap();
        info!("data_offset - {data_offset}, filler_offset - {filler_offset}");
        assert!(data_offset >= 95_000 && data_offset < 1_100_000);
        assert!(filler_offset > 35_000 && filler_offset < 120_000, "{}", filler_offset);
        join_handle.0.send(true).unwrap();
        join_handle.1.join().unwrap();
    }


    /*
    Проверяем что сервер в целом не падает, если отключился какой-то клиент
    и уничтожается поток для клиента
    */
    #[test]
    #[inline(never)]
    fn server_stable_test() {
        initialize_logger();

        const BUF_SIZE: usize = 12_000;
        let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

        let TestStreams {
            mut vpn_stream,
            client_data_stream,
            client_filler_stream,
            mut orchestrator,
            join_handle
        } = create_test_streams(2, None);

        let _ = client_data_stream.write_all(&buf[..10]).unwrap();
        vpn_stream.write_all(&buf[..10]).unwrap();
        client_data_stream.read(&mut buf[..]).unwrap();

        sleep(Duration::from_millis(1000));
        //закрываем чтение
        client_data_stream.shutdown();
        //пытаемся отправить в через прокси данные в закрытый канал
        for i in 0..100 {
            //получаем состояние Broken
            orchestrator.invoke();
            sleep(Duration::from_millis(10));
            let send_result = vpn_stream.write_all(&buf[..10]);
            if send_result.is_err() {
                info!("broken after {}", i);
                orchestrator.invoke();
                break;
            }
        }
        sleep(Duration::from_millis(1000));
        //проверяем что клиентов больше нет, а мы все еще не упали
        let pairs_count = orchestrator.get_pairs_count();
        assert_eq!(0, pairs_count);
        join_handle.0.send(true).unwrap();
        join_handle.1.join().unwrap();
        //TODO: дописать повторное подключение
    }

    /*
    Проверка, что статистика доходит до мейна
    */
    #[test]
    #[serial]
    fn stat_goes_to_main() {
        let TestStreams {
            mut vpn_stream,
            client_data_stream,
            client_filler_stream,
            mut orchestrator,
            join_handle
        } = create_test_streams(3, Some(Box::new(SimpleStatisticCollector::default())));

        let mut buf: [u8; ONE_PACKET_MAX_SIZE] = [0; ONE_PACKET_MAX_SIZE];

        loop {
            orchestrator.invoke();
            sleep(Duration::from_millis(100));
            let _ = client_data_stream.write_all(&buf[..100]).unwrap();
            let _ = vpn_stream.write_all(&buf[..100]).unwrap();
            let _ = vpn_stream.flush().unwrap();
            let _ = client_data_stream.read(&mut buf);
            let _ = vpn_stream.read(&mut buf);
            let _ = client_filler_stream.read(&mut buf);

            orchestrator.invoke();
            if let Some(collected_info) = orchestrator.calculate_and_get() {
                assert_eq!(1, collected_info.len());
                join_handle.0.send(true).unwrap();
                break;
            }
        }
        join_handle.1.join().unwrap();
    }

    /**
       Проверяем что начиная с минимальной скорости мы достигаем максимальной скорости (1Gbit/s)
     */
    #[test]
    #[serial]
    fn one_gb_test() {
        initialize_logger();
        info!("1GB_TEST");
        const BUF_SIZE: usize = 30_000;
        let target_speed: usize = to_native_speed(1024);

        //размер пакета примерно 10_000
        let mut buf: [u8; BUF_SIZE] = [0; BUF_SIZE];

        let TestStreams {
            mut vpn_stream,
            client_data_stream,
            client_filler_stream,
            mut orchestrator,
            join_handle
        } = create_test_streams(4, None);

        let half_secs = Duration::from_millis(500);
        sleep(half_secs);
        orchestrator.invoke();

        let timeout = Duration::from_secs(5);
        let (tx, rx) = mpsc::channel();
        let join_handle_2 = thread::Builder::new()
            .name("send".to_string()).spawn(move || {
            let value_data: [u8; ONE_PACKET_MAX_SIZE] = [0; ONE_PACKET_MAX_SIZE];
            tx.send(true).unwrap();
            info!("---------Начали отправку");
            let start = Instant::now();
            //Забиваем таким количеством, которого должно хватить на пол секунды
            while start.elapsed() < timeout {
                let result = vpn_stream.write_all(&value_data[..]);
                if result.is_err() {
                    break;
                }
            }
            return vpn_stream;
        }).unwrap();

        rx.recv().unwrap();
        info!("---------Начали получение");
        let start = Instant::now();
        let mut data_offset = 0;
        let mut filler_offset = 0;
        let mut counter = 0;

        let mut total_size = 0;
        let mut time_start = Instant::now();
        while start.elapsed() < timeout {
            let data_read = client_data_stream.read(&mut buf[..]).unwrap();
            let filler_read = client_filler_stream.read(&mut buf[..]).unwrap();
            data_offset += data_read;
            filler_offset += filler_read;
            if counter % 10 == 0 {
                orchestrator.invoke();
            }
            if counter % 30 == 0 {
                let new_total_size = data_offset + filler_offset;
                let delta = new_total_size - total_size;
                let duration = Instant::now() - time_start;
                let ms = duration.as_millis() as usize;
                if delta > 0 && ms > 0 {
                    let speed = delta / ms;
                    if speed >= target_speed {
                        info!("target speed: {speed} MBit/s");
                        break;
                    }
                    let speed = to_regular_speed(speed);
                    trace!("speed: {speed} MBit/s");
                }
                total_size = new_total_size;
                time_start = Instant::now();
            }
            counter += 1;
        }
        assert!(start.elapsed() < timeout);
        let _ = join_handle_2.join().unwrap();
        join_handle.0.send(true).unwrap();
        join_handle.1.join().unwrap();
    }
}