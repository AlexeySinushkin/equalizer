#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include "common.h"
#include "connect.h"

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
volatile int shouldWork = 1;

int read_header(int fd, struct Header *header)
{
    u8 header_buf[HEADER_SIZE];
    int offset = 0;
    while (offset < HEADER_SIZE && shouldWork)
    {
        int bytes_read = read(fd, header_buf + offset, HEADER_SIZE - offset);
        if (bytes_read == -1)
        {
            shouldWork = 0;
            return 200;
        }
        offset += bytes_read;
    }
    if (offset < HEADER_SIZE)
    {
        return 201;
    }
    if (header_buf[0] != FIRST_BYTE)
    {
        return 202;
    }
    int packet_size = (header_buf[LENGTH_BYTE_MSB_INDEX] << 8) + header_buf[LENGTH_BYTE_LSB_INDEX];
    if (packet_size > MAX_BODY_SIZE)
    {
        return 203;
    }
    header->packet_type = header_buf[TYPE_BYTE_INDEX];
    header->packet_size = packet_size;
    return 0;
}

int write_header(int fd, struct Header *header)
{
    u8 header_buf[HEADER_SIZE];
    header_buf[0] = FIRST_BYTE;
    header_buf[TYPE_BYTE_INDEX] = header->packet_type;
    header_buf[LENGTH_BYTE_LSB_INDEX] = header->packet_size & 0xFF;
    header_buf[LENGTH_BYTE_MSB_INDEX] = (header->packet_size >> 8) & 0xFF;
    int offset = 0;
    while (offset < HEADER_SIZE && shouldWork)
    {
        int written = write(fd, header_buf + offset, HEADER_SIZE - offset);
        if (written == -1)
        {
            shouldWork = 0;
            return 250;
        }
        offset += written;
    }
    if (offset < HEADER_SIZE)
    {
        return 251;
    }
    return 0;
}

struct Header create_data_header(int packet_size)
{
    struct Header header;
    header.packet_type = TYPE_DATA;
    header.packet_size = packet_size;
    return header;
}


int read_packet(int fd, u8 *buffer, struct Header *header)
{
    int head_header_result = read_header(fd, header);
    if (head_header_result != 0)
    {
        return head_header_result;
    }
    int offset = 0;
    while (offset < header->packet_size && shouldWork)
    {
        int bytes_read = read(fd, buffer + offset, header->packet_size - offset);
        if (bytes_read == -1)
        {
            return 210;
        }
        offset += bytes_read;
    }
    if (offset < header->packet_size)
    {
        return 211;
    }
    return 0;
}

int serverToClientProcess(int serverFd, int clientFd)
{
    u8 packet_body[MAX_BODY_SIZE];
    struct Header header;
    while (1)
    {
        int result = read_packet(serverFd, packet_body, &header);
        if (result != 0)
        {
            shouldWork = 0;
            return result;
        }
        if (header.packet_type == TYPE_DATA)
        {
            int offset = 0;
            while (offset < header.packet_size && shouldWork)
            {
                int written = write(clientFd, packet_body + offset, header.packet_size - offset);
                if (written == -1)
                {
                    shouldWork = 0;
                    return 221;
                }
                offset += written;
            }
        }
    }
    return 222;
}


int clientToServerProcess(int serverFd, int clientFd)
{
    u8 packet_body[MAX_BODY_SIZE];
    
    while (1)
    {
        int bytes_read = read(clientFd, &packet_body, MAX_BODY_SIZE);
        if (bytes_read == -1 )
        {
            shouldWork = 0;
            return 301;
        }
        struct Header header = create_data_header(bytes_read);
        int write_header_result = write_header(serverFd, &header);
        if (write_header_result != 0)
        {
            shouldWork = 0;
            return write_header_result;
        }
        int offset = 0;
        while (offset < header.packet_size && shouldWork)
        {
            int written = write(serverFd, packet_body + offset, header.packet_size - offset);
            if (written <= 0)
            {
                shouldWork = 0;
                return 302;
            }
            offset += written;
        }
    }
    return 333;
}



/**
    Принимаем входящее подключение от VPN клиента
    Пытаемся подключиться к VPN серверу
    Если подключиться удалось, создаем второй поток
    В одном потоке данные идут клиент->сервер, в другом сервер->клиент
    (все в блокирующем режиме)
    При поломке одного из каналов выходим и ожидаем нового подключения.
*/
int communication_session()
{
    int vpnClientFd = 0;
    int listenSocketFd = 0;
    int vpnServerFd = 0;
    int connectResult = acceptAndConnect(&vpnClientFd, &listenSocketFd, &vpnServerFd);
    if (connectResult == 0)
    {
        printf("Two links established\n");
        pid_t result = fork();
        /*
            Negative Value: The creation of a child process was unsuccessful.
            Zero: Returned to the newly created child process.
            Positive value: Returned to parent or caller. The value contains the process ID of the newly created child process.
        */
        if (result == -1)
        {
            fprintf(stderr, "Failed to fork!");
            return 20;
        }
        if (result == 0)
        {
            printf("Starting server->client  %d\n", result);   
            int processResult = serverToClientProcess(vpnServerFd, vpnClientFd);
            printf("Exit server->client with error code  %d\n", processResult);   
        } else {
            printf("Starting client->server  %d\n", result);   
            int processResult = clientToServerProcess(vpnServerFd, vpnClientFd);
            printf("Exit client->server with error code  %d\n", processResult);   
        }
        printf("Closing. Pid %d\n", result);
        close(vpnClientFd);
        close(vpnServerFd);
        close(listenSocketFd);
        return 0;
    }else {
        if (vpnClientFd!=0){
            close(vpnClientFd);
        }
        if (listenSocketFd!=0){
            close(listenSocketFd);
        }        
    }
    return connectResult;
}
