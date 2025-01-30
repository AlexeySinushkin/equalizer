#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <signal.h> 
#include <pthread.h>
#include <poll.h>
#include "common.h"
#include "connect.h"


#define SIGHUP  1   /* Hang up the process */ 
#define SIGINT  2   /* Interrupt the process */ 
#define SIGQUIT 3   /* Quit the process */ 
#define SIGILL  4   /* Illegal instruction. */ 
#define SIGTRAP 5   /* Trace trap. */ 
#define SIGABRT 6   /* Abort. */

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
int clientFd = 0;
int listenSocketFd = 0;
int serverFd = 0;  
// для блокировки операций с серверным сокетом
//pthread_mutex_t  rw_server = PTHREAD_MUTEX_INITIALIZER;
// для блокировки операций с клиентским сокетом
//pthread_mutex_t  rw_client = PTHREAD_MUTEX_INITIALIZER;
pthread_t childThreadId;

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
    header_buf[LENGTH_BYTE_LSB_INDEX] = (u8)(header->packet_size & 0xFF);
    header_buf[LENGTH_BYTE_MSB_INDEX] = (u8)((header->packet_size >> 8) & 0xFF);
    if (header->packet_size==10240) {
        printf("--> 10240 0x%02x 0x%02x 0x%02x 0x%02x\n", header_buf[0], header_buf[1], header_buf[2], header_buf[3]);
    }
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



void *serverToClientProcess(void *arg)
{
    printf("Starting server->client %d-%d\n", serverFd, clientFd);   
    u8 packet_body[MAX_BODY_SIZE];
    struct Header header;
    while (1)
    {
        //pthread_mutex_lock(&rw_server);
        int result = read_packet(serverFd, packet_body, &header);
        //pthread_mutex_unlock(&rw_server);
        if (result != 0)
        {
            shouldWork = 0;
            printf("return %d\n", result);
            pthread_exit(NULL); 
        }
        //printf("<-- Received from server 0x%02x  %d\n", header.packet_type, header.packet_size);
        if (header.packet_type == TYPE_DATA)
        {
            int offset = 0;
            //pthread_mutex_lock(&rw_client);
            while (offset < header.packet_size && shouldWork)
            {                
                int written = write(clientFd, packet_body + offset, header.packet_size - offset);
                //printf("<-- Forwarded to client %d\n", written);
                if (written == -1)
                {
                    shouldWork = 0;
                    //pthread_mutex_unlock(&rw_client);
                    printf("return %d\n", 221);
                    pthread_exit(NULL); 
                }
                offset += written;
            }
            //pthread_mutex_unlock(&rw_client);
        }else{
            //printf("<-- no forward for  0x%02x\n", header.packet_type);
        }
    }
    printf("return %d\n", 222);
    pthread_exit(NULL); 
}


int clientToServerProcess()
{
    printf("Starting client->server %d-%d\n", serverFd, clientFd);  
    u8 packet_body[MAX_BODY_SIZE];
    
    while (1)
    {
        //pthread_mutex_lock(&rw_client);
        int bytes_read = read(clientFd, packet_body, MAX_BODY_SIZE);
        //pthread_mutex_unlock(&rw_client);
        if (bytes_read == -1 )
        {
            shouldWork = 0;
            return 301;
        }
        printf("--> Received from client %d\n", bytes_read);

        struct Header header = create_data_header(bytes_read);
        //pthread_mutex_lock(&rw_server);
        int write_header_result = write_header(serverFd, &header);
        if (write_header_result != 0)
        {
            shouldWork = 0;
            //pthread_mutex_unlock(&rw_server);
            return write_header_result;
        }
        
        int offset = 0;
        while (offset < header.packet_size && shouldWork)
        {
            int written = write(serverFd, packet_body + offset, header.packet_size - offset);
            printf("--> Forwarded to server %d\n", written);
            if (written <= 0)
            {
                shouldWork = 0;
                //pthread_mutex_unlock(&rw_server);
                return 302;
            }
            offset += written;
        }
        //pthread_mutex_unlock(&rw_server);
    }
    return 333;
}



  
// Handler for SIGINT, triggered by 
// Ctrl-C at the keyboard 
void handle_sigint(int sig)  { 
    printf("Caught signal %d\n", sig); 
    if (clientFd!=0){
        close(clientFd);
        clientFd = 0;
    }
    if (serverFd!=0){
        close(serverFd);
        serverFd = 0;
    }    
    if (listenSocketFd!=0){
        close(listenSocketFd);
        listenSocketFd = 0;
    } 
    
    pthread_exit(&childThreadId);
    
    if (sig == SIGINT || sig == SIGQUIT)
    {
        exit(0);
    }
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
    clientFd = 0;
    listenSocketFd = 0;
    serverFd = 0;
    shouldWork = 1;

    signal(SIGINT, handle_sigint); 
    signal(SIGQUIT, handle_sigint); 
    int connectResult = acceptAndConnect(&clientFd, &listenSocketFd, &serverFd);
    if (connectResult == 0)
    {
        printf("Two links established\n");       

        if (pthread_create(&childThreadId, NULL, serverToClientProcess, NULL) != 0) {
            perror("pthread_create error");
            return 500;
        }
         
        int processResult = clientToServerProcess();
        printf("Exit client->server with error code  %d\n", processResult);   

    }
    handle_sigint(0);
    return connectResult;
}
