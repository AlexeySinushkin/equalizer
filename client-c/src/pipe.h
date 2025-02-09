#include "common.h"
#include "packet.h"
#include <stdbool.h>

enum PipeState{
    IDLE,
    READING,
    WRITING,
	ERROR
};

struct Pipe
{
	enum PipeState state;
	//канал для чтения
    int src_fd;
	//канал для записиыц
    int dst_fd;	
	//если в процессе отправки данных - появились данные для чтения
	//устанавливаем этот флаг и сразу же читаем данные
	bool read_pending;
	//смещение буфера в процессе отправки
	int offset;
	//целевая длинна буфера	
	int size;
	u8 header_buf[HEADER_SIZE];
	u8 body_buf[MAX_BODY_SIZE];	
};