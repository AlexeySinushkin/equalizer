#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <signal.h> 
#include <pthread.h>
#include <poll.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>  // Include for timeval
#include <netinet/in.h>
#include <unistd.h>
#include <errno.h>
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

int clientFd = 0;
int listenSocketFd = 0;
int serverFd = 0;  
// для блокировки операций с серверным сокетом
pthread_mutex_t  rw_server = PTHREAD_MUTEX_INITIALIZER;
// для блокировки операций с клиентским сокетом
pthread_mutex_t  rw_client = PTHREAD_MUTEX_INITIALIZER;
pthread_t childThreadId;
pthread_t serverThreadId;
int serverReadOffset = 0;

enum ReadResult{
    READ_INCOMPLETE,
    READ_COMPLETE,
    READ_ERROR
};

struct Header create_data_header(int packet_size)
{
    struct Header header;
    header.packet_type = TYPE_DATA;
    header.packet_size = packet_size;
    return header;
}


int write_header(int fd, struct Header *header)
{
    u8 header_buf[HEADER_SIZE];
    header_buf[0] = FIRST_BYTE;
    header_buf[TYPE_BYTE_INDEX] = header->packet_type;
    header_buf[LENGTH_BYTE_LSB_INDEX] = (u8)(header->packet_size & 0xFF);
    header_buf[LENGTH_BYTE_MSB_INDEX] = (u8)((header->packet_size >> 8) & 0xFF);

    int offset = 0;
    while (offset < HEADER_SIZE)
    {
        int written = write(fd, header_buf + offset, HEADER_SIZE - offset);
        if (written == -1)
        {            
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

enum ReadResult read_header(int fd, u8 *buffer, struct Header *header)
{
    if (serverReadOffset < HEADER_SIZE)
    {
        int bytes_read = read(fd, buffer + serverReadOffset, HEADER_SIZE - serverReadOffset);
        if (bytes_read == 0)
        {            
            return READ_ERROR;
        } else if (bytes_read == -1){
            // Read failed, check errno
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                return READ_INCOMPLETE;
            } else {
                return READ_ERROR;
            }
        }
        serverReadOffset += bytes_read;
    }
    if (serverReadOffset == HEADER_SIZE)
    {
        if (buffer[0] != FIRST_BYTE)
        {
            return READ_ERROR;
        }
        int packet_size = (buffer[LENGTH_BYTE_MSB_INDEX] << 8) + buffer[LENGTH_BYTE_LSB_INDEX];
        if (packet_size > MAX_BODY_SIZE)
        {
            return READ_ERROR;
        }
        header->packet_type = buffer[TYPE_BYTE_INDEX];
        header->packet_size = packet_size;
        return READ_COMPLETE;
    }
    return READ_INCOMPLETE;
}


enum ReadResult read_packet(int fd, u8 *buffer, struct Header *header)
{
    //еще не прочитали заголовок
    if (serverReadOffset<HEADER_SIZE){
        enum ReadResult read_header_result = read_header(fd, buffer, header);
        if (read_header_result == READ_ERROR || read_header_result == READ_INCOMPLETE){
            return read_header_result;
        } 
    }

    int rightOffset = HEADER_SIZE + header->packet_size;
    if (serverReadOffset < rightOffset)
    {
        int bytes_read = read(fd, buffer + serverReadOffset, rightOffset - serverReadOffset);
        if (bytes_read == 0)
        {            
            return READ_ERROR;
        } else if (bytes_read == -1){
            // Read failed, check errno
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                return READ_INCOMPLETE;
            } else {
                return READ_ERROR;
            }
        }
        serverReadOffset += bytes_read;
    }
    if (serverReadOffset == rightOffset){
        serverReadOffset = 0;
        return READ_COMPLETE;            
    }
}



void *serverToClientProcess(void *arg)
{
    printf("Starting server->client %d-%d\n", serverFd, clientFd);   
    u8 buffer[HEADER_SIZE+MAX_BODY_SIZE];
    struct Header header;
    while (1)
    {
        pthread_mutex_lock(&rw_server);
        enum ReadResult read_packet_result = read_packet(serverFd, buffer, &header);
        pthread_mutex_unlock(&rw_server);
        if (read_packet_result == READ_ERROR)
        {            
            printf("read server packet error\n");
            pthread_exit(NULL); 
        }else if (read_packet_result == READ_INCOMPLETE){
            continue;
        }
        printf("<-- Received from server 0x%02x  %d\n", header.packet_type, header.packet_size);
        if (header.packet_type == TYPE_DATA)
        {
            int offset = 0;
            u8* body_offset = buffer + HEADER_SIZE;
            pthread_mutex_lock(&rw_client);
            while (offset < header.packet_size)
            {                
                int written = write(clientFd, body_offset + offset, header.packet_size - offset);
                printf("<-- Forwarded to client %d\n", written);
                if (written == -1)
                {                    
                    pthread_mutex_unlock(&rw_client);
                    printf("return %d\n", 221);
                    pthread_exit(NULL); 
                }
                offset += written;
            }
            pthread_mutex_unlock(&rw_client);
        }else{
            printf("<-- no forward for  0x%02x\n", header.packet_type);
        }
    }
    printf("return %d\n", 222);
    pthread_exit(NULL); 
}

void *clientToServerProcess(void *arg)
{
    printf("Starting client->server %d-%d\n", serverFd, clientFd);  
    u8 packet_body[MAX_BODY_SIZE];
    
    while (1)
    {
        pthread_mutex_lock(&rw_client);
        int bytes_read = read(clientFd, packet_body, MAX_BODY_SIZE);
        pthread_mutex_unlock(&rw_client);
        if (bytes_read == 0)
        {            
            printf("return %d\n", 301);
            pthread_exit(NULL); 
        } else if (bytes_read == -1){
            // Read failed, check errno
            if (errno == EAGAIN || errno == EWOULDBLOCK) {
                //Read timed out (no data received within 2ms
                continue;
            } else {
                printf("Read error: %s\n", strerror(errno));
                pthread_exit(NULL); 
            }
        }

        
        printf("--> Received from client %d\n", bytes_read);

        struct Header header = create_data_header(bytes_read);
        pthread_mutex_lock(&rw_server);
        int write_header_result = write_header(serverFd, &header);
        if (write_header_result != 0)
        {            
            pthread_mutex_unlock(&rw_server);
            printf("return %d\n", write_header_result);
            pthread_exit(NULL); 
        }
        
        int offset = 0;
        while (offset < header.packet_size)
        {
            int written = write(serverFd, packet_body + offset, header.packet_size - offset);
            printf("--> Forwarded to server %d\n", written);
            if (written == -1)
            {                
                pthread_mutex_unlock(&rw_server);
                printf("return %d\n", 302);
                pthread_exit(NULL); 
            }
            offset += written;
        }
        pthread_mutex_unlock(&rw_server);
    }
    printf("return %d\n", 333);
    pthread_exit(NULL); 
}



  
// Handler for SIGINT, triggered by 
// Ctrl-C at the keyboard 
void handle_sigint(int sig)  { 
    printf("Caught signal %d\n", sig); 
    pthread_cancel(childThreadId);

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
    
    if (sig == SIGINT || sig == SIGQUIT)
    {
        exit(sig);
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
    serverReadOffset = 0;

    signal(SIGINT, handle_sigint); 
    signal(SIGQUIT, handle_sigint); 
    int result = acceptAndConnect(&clientFd, &listenSocketFd, &serverFd);
    if (result == 0)
    {
        printf("Two links established\n");     
        struct timeval timeout1;      
        timeout1.tv_sec = 0;
        timeout1.tv_usec = 2000;    
        if (setsockopt (clientFd, SOL_SOCKET, SO_RCVTIMEO, &timeout1, sizeof timeout1) < 0){
            perror("setsockopt failed\n");  
            return 502;
        }
        struct timeval timeout2;      
        timeout2.tv_sec = 0;
        timeout2.tv_usec = 2000;
        if (setsockopt (serverFd, SOL_SOCKET, SO_RCVTIMEO, &timeout2, sizeof timeout2) < 0){
            perror("setsockopt failed\n");  
            return 504;
        }
        

        if (pthread_create(&childThreadId, NULL, clientToServerProcess, NULL) != 0) {
            perror("pthread_create error");
            return 501;
        }
        if (pthread_create(&serverThreadId, NULL, serverToClientProcess, NULL) != 0) {
            perror("pthread_create error");
            return 503;
        }
        pthread_join(childThreadId, NULL); 
        pthread_join(serverThreadId, NULL); 

    }
    handle_sigint(result);
    return result;
}
