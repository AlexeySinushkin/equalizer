#include "common.h"
#include "packet.h"
#include <stdbool.h>

#ifndef _PIPE_H
#define _PIPE_H
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
	//смещение буфера в процессе отправки
	int offset;
	//целевая длинна буфера	
	int size;
	u8 header_buf[HEADER_SIZE];
	u8 body_buf[MAX_BODY_SIZE];	
};
#endif