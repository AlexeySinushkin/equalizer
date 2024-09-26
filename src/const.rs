
//размер одного tcp пакета (как правило не больше 1024)
pub const ONE_PACKET_MAX_SIZE: usize = 1500;

//10 Мбит/с = 1МБ/с = 1048 байт/мс
pub const INITIAL_SPEED: usize = 1*1024*1024/1000;