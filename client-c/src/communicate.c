#include <sys/types.h>
#include <unistd.h>
#include <string.h>
#include <strings.h> // bzero()
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <socket.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h> // inet_addr()
#include <netdb.h>
#include "connect.h"
#include "common.h"

#define PORT 12007
#define SA struct sockaddr

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


/**
    Принимаем входящее подключение от VPN клиента
    Пытаемся подключиться к VPN серверу
    Если подключиться удалось, создаем второй поток
    В этом потоке продолжаем слать данные, в другом читать
    (все в блокирующем режиме)
    При поломке одного из каналов выходим и ожидаем нового подключения.
*/
int communication_session()
{
    int* vpnClientFd;
    int* vpnServerFd;
    if (acceptAndConnect(&vpnClientFd, &vpnServerFd) == 0 ){

    }
    u8 header[HEADER_SIZE];
    u8 body[MAX_PACKET_SIZE];
    return -1;
}