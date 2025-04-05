#include "common.h"

#ifndef _PACKET_H
#define _PACKET_H

#define HEADER_SIZE 4
#define MAX_BODY_SIZE 10240

#define FIRST_BYTE 0x54
#define TYPE_DATA 0x55
#define TYPE_FILLER 0x56
#define TYPE_BYTE_INDEX 1
#define LENGTH_BYTE_LSB_INDEX 2
#define LENGTH_BYTE_MSB_INDEX 3

struct Header
{
    u8 packet_type;
    int packet_size;
};

#endif