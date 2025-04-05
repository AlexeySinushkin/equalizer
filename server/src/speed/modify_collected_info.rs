use std::time::{Instant, Duration};
use log::{trace};
use crate::objects::HotPotatoInfo;
use crate::speed::{Info, TimeSpanSentDataInfo};


pub fn append_new_data(hp: &HotPotatoInfo, info: &mut Info) -> u64 {
    let data_size: usize = match hp.data_count > 0 {
        true => {
            let mut result = 0;
            for i in 0..hp.data_count {
                let sent_packet = hp.data_packets[i].unwrap();
                result += sent_packet.sent_size;
            }
            result
        }
        false => 0,
    };
    let filler_size: usize = match hp.filler_count > 0 {
        true => {
            let mut result = 0;
            for i in 0..hp.filler_count {
                let sent_packet = hp.filler_packets[i].unwrap();
                result += sent_packet.sent_size;
            }
            result
        }
        false => 0,
    };

    let data = TimeSpanSentDataInfo {
        id: info.next_sequence_data(),
        from: Instant::now(),
        data_size,
        filler_size,
    };

    if let Some(log) = info.speed_logging.as_mut() {
        log.append_new_data_log(&data);
    }
    let new_id = data.id;
    info.sent_data.push_back(data);
    new_id
}

pub fn clear_old_data(info: &mut Info, window: Duration) {
    dump_queue_info(info, "before");

    if let Some(back) = info.sent_data.back() {
        //удаляем данные старше 5 секунд
        let old_threshold = back.from - window;

        while let Some(first) = info.sent_data.front() {
            if first.from < old_threshold {
                let data = info.sent_data.pop_front().unwrap();
                if let Some(log) = info.speed_logging.as_mut() {
                    log.clear_old_data_log(&data);
                }
            } else {
                break;
            }
        }
    }

    dump_queue_info(info, "after");
}

fn dump_queue_info(info: &Info, tag: &str){
    if let Some(front) = info.sent_data.front() {
        if let Some(back) = info.sent_data.back() {
            trace!("clear data {}: #{}-#{} {:?}", tag, back.id, front.id, back.from - front.from);
        }
    }
}


#[cfg(test)]
mod tests {
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use log::trace;
    use crate::speed::{Info};
    use crate::speed::{TimeSpanSentDataInfo};
    use crate::speed::modify_collected_info::clear_old_data;
    use crate::tests::test_init::initialize_logger;

    /**
    Имея окно размером в 2с в течении 10 секунд добавляем элементы через равные промежутки времени
    и проверяем что количество элементов для подсчета не падает и не растет больше необходимого
     */
    #[test]
    fn clear_data_test() {
        initialize_logger();
        let mut id = 0;
        let mut info = Info::default();
        let window = Duration::from_secs(2);
        let mut queue_size = 0;
        for _i in 0..50 {
            let data = TimeSpanSentDataInfo {
                id,
                from: Instant::now(),
                data_size: 0,
                filler_size: 0,
            };
            info.sent_data.push_back(data);
            id += 1;
            clear_old_data(&mut info, window);

            let new_size = info.sent_data.len();
            if new_size > queue_size {
                queue_size = new_size;
            }
            trace!("{new_size}/{queue_size}");
            assert!(new_size >= queue_size);
            assert!(queue_size <= 21);
            sleep(Duration::from_millis(200));
        }
    }
}