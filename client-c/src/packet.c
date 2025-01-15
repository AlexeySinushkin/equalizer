typedef u_int8_t u8;

const int HEADER_SIZE = 4;
const int MAX_PACKET_SIZE = 10 * 1024;
const int BUFFER_SIZE = MAX_PACKET_SIZE + HEADER_SIZE;
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

int exchange(int clientFd, int serverFd) {
    u8 header[HEADER_SIZE];
    u8 body[MAX_PACKET_SIZE];
    return -1;
}