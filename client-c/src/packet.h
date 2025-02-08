#include "common.h"

#ifndef _PACKET_H
#define _PACKET_H

const int HEADER_SIZE = 4;
const int MAX_BODY_SIZE = 10 * 1024;

const u8 FIRST_BYTE = 0x54;
const u8 TYPE_DATA = 0x55;
const u8 TYPE_FILLER = 0x56;
const int TYPE_BYTE_INDEX = 1;
const int LENGTH_BYTE_LSB_INDEX = 2;
const int LENGTH_BYTE_MSB_INDEX = 3;

struct Header
{
    u8 packet_type;
    int packet_size;
};
enum ReadResult{
    READ_INCOMPLETE,
    READ_COMPLETE,
    READ_ERROR
};

#endif