/**
    Работает подготовленная пара Основного канала и Канал-заполнитель
    Если кто-то из них отваливается - завершаем работу инстанса
 */
pub mod vpn_proxy;
mod throttler;
mod filler;