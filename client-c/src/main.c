#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <errno.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <string.h>
#include <libubox/uloop.h>
#include "pipe.h"
#include "client-to-server.h"
#include "server-to-client.h"

#define VPN_SERVER_IP   "127.0.0.1"
#define VPN_SERVER_PORT 12010
#define SERVER_PORT 12005
#define BUFFER_SIZE 1024

struct uloop_fd server_fd;
//для взаимодействия с клиентом
struct uloop_fd *client_ufd;
struct Pipe *client_pipe;
//для взаимодействия с сервером
struct uloop_fd *server_ufd;
struct Pipe *server_pipe;

// Function to set a socket to non-blocking mode
void set_nonblocking(int sock) {
    int flags = fcntl(sock, F_GETFL, 0);
    if (flags == -1) {
        perror("fcntl F_GETFL");
        exit(1);
    }
    if (fcntl(sock, F_SETFL, flags | O_NONBLOCK) == -1) {
        perror("fcntl F_SETFL O_NONBLOCK");
        exit(1);
    }
}

void free_resources(){
    uloop_fd_delete(client_ufd);
    close(client_ufd->fd);
    free(client_ufd);
    client_ufd = NULL;
    free(client_pipe);
    client_pipe = NULL;

    if (server_ufd != NULL)
    {
        perror("clean server-pipe resources");
        uloop_fd_delete(server_ufd);
        close(server_ufd->fd);
        free(server_ufd);
        server_ufd = NULL;
        free(server_pipe);
        server_pipe = NULL;
    }
}
void uloop_fd_mod(struct uloop_fd *ufd, unsigned int flags)
{    
    if (ufd->registered) {
        uloop_fd_delete(ufd);
    }    
    uloop_fd_add(ufd, flags);    
    printf("...done\n");
}

struct uloop_fd* get_read_ufd(struct Pipe *pipe) {
    if (pipe == client_pipe){
        printf("<<-- ");
       return client_ufd;
    }
    else if (pipe == server_pipe){
        printf("-->> ");
        return server_ufd;
    }
    else{
        perror("Unknown file descriptor. We assume only one client at once\n");
        exit(1);
    }
}

void disable_read_event(struct Pipe *pipe){
    struct uloop_fd *ufd = get_read_ufd(pipe);
    printf("disable_read_event \n");
    uloop_fd_mod(ufd, ufd->flags & ~ULOOP_READ);
}

void enable_read_event(struct Pipe *pipe){
    struct uloop_fd *ufd = get_read_ufd(pipe);
    printf("enable_read_event \n");
    uloop_fd_mod(ufd, ufd->flags | ULOOP_READ);
}

struct uloop_fd* get_write_ufd(struct Pipe *pipe) {
    if (pipe == client_pipe){
        printf("<<-- ");
        return server_ufd;
    }
    else if (pipe == server_pipe){
        printf("-->> ");
        return client_ufd;
    }
    else{
        perror("Unknown file descriptor. We assume only one client at once\n");
        exit(1);
    }
}

void disable_write_event(struct Pipe *pipe){
    struct uloop_fd *ufd = get_write_ufd(pipe);
    printf("disable_write_event \n");
    uloop_fd_mod(ufd, ufd->flags & ~ULOOP_WRITE);
}

void enable_write_event(struct Pipe *pipe){
    struct uloop_fd *ufd = get_write_ufd(pipe);
    printf("enable_write_event \n");
    uloop_fd_mod(ufd, ufd->flags | ULOOP_WRITE);
}



// Callback for handling client connections
void receive_data_handler(struct uloop_fd *ufd, unsigned int events)
{
/*
    Изначальное состояние IDLE и готовность к чтению
    Если закончили читать, то отключаем ожидание эвента чтения и начинаем писать
    Если с первой попытки все разом не записали, то включаем ожидание эвента записи
    Если записали все, то отключаем ожидание эвента записи и включаем ожидание эвента чтения
*/
    int result = 0;
    struct Pipe* pipe = NULL;
    if (ufd == client_ufd){
        pipe = client_pipe;
    }
    else if (ufd == server_ufd){
        pipe = server_pipe;
    }
    else{
        perror("Unknown file descriptor. We assume only one client at once\n");
        exit(1);
    }

    if (pipe->state==ERROR){
        perror("pipe broken");
        free_resources();  
        return; 
    }


    if (events & ULOOP_READ)
    {       
        if (pipe->state==WRITING){
            disable_read_event(pipe);
            return;
        }
        result = pipe->read(pipe);
        if (result == EXIT_FAILURE)
        {
            perror("read from pipe error");
            free_resources();
            return;
        }
        if (pipe->write_pending){
            result = pipe->write(pipe);
            //если записали только часть данных 
            if (pipe->state==WRITING){
                enable_write_event(pipe);
                disable_read_event(pipe);
            }
        }
        if (result == EXIT_FAILURE || pipe->state==ERROR)
        {
            perror("error in read data available event\n");
            free_resources();
            return;
        }
    }
    if (events & ULOOP_WRITE)
    {
        if (pipe->state==READING){
            disable_write_event(pipe);
            return;
        }
        if (pipe->state==WRITING){
            result = pipe->write(pipe);
            //если записали все, готовимся читать
            if (pipe->state==IDLE){
                enable_read_event(pipe);
                disable_write_event(pipe);
            }
        }else if (pipe->state==IDLE){
            disable_write_event(pipe);
        }
        if (result == EXIT_FAILURE)
        {
            perror("error in write data available event\n");
            free_resources();
            return;
        }
    }
    if (!(events & ULOOP_EVENT_MASK)){
        perror("unknown event\n");
        free_resources();
        return;
    }
}

// сразу после подключения VPN клиента, мы подключаемся к в VPN server-у
int connect_to_server(){
    struct sockaddr_in server_addr;
    int sock_fd;
    sock_fd = socket(AF_INET, SOCK_STREAM, 0);
    if (sock_fd < 0) {
        perror("socket");
        return EXIT_FAILURE;
    }

    set_nonblocking(sock_fd);  // Set socket to non-blocking mode

    memset(&server_addr, 0, sizeof(server_addr));
    server_addr.sin_family = AF_INET;
    server_addr.sin_port = htons(VPN_SERVER_PORT);
    
    char* server_ip = getenv("VPN_SERVER_IP");
    if (server_ip != NULL){
        inet_pton(AF_INET, server_ip, &server_addr.sin_addr);
        printf("server ip was setup from env VPN_SERVER_IP: %s\n", server_ip);
    }
    else{
        inet_pton(AF_INET, VPN_SERVER_IP, &server_addr.sin_addr);
        printf("server ip was not setup from env VPN_SERVER_IP. 127.0.0.1 is used\n");
    }
    

    if (connect(sock_fd, (struct sockaddr *)&server_addr, sizeof(server_addr)) < 0) {
        if (errno != EINPROGRESS) {
            perror("connect");
            close(sock_fd);
            return EXIT_FAILURE;
        }
    }
    socklen_t errlen = sizeof(int);
    int err = 0;
    getsockopt(sock_fd, SOL_SOCKET, SO_ERROR, &err, &errlen);
    if (err != 0) {
        printf("connect() error: %s\n", strerror(err));
        close(sock_fd);
        return EXIT_FAILURE;
    }

    char *client_name = getenv("CLIENT_NAME");
    if (client_name != NULL){
        int len = strlen(client_name);
        if (len > 1 && len < 11){
            int body_size = len + 1;
            u8 buf[16] = {0};
            struct Header header = create_filler_header(body_size);
            header_to_buf(&header, buf);
            buf[HEADER_SIZE] = 0x01;
            u8* dst_offset = &buf[HEADER_SIZE + 1];
            for (int i=0; i<len; i++){
                printf("%02X ", (u8)client_name[i]);
                dst_offset[i] = (u8)client_name[i];
            }
            printf("\n");
            int sent = send(sock_fd, &buf[0], body_size+HEADER_SIZE, 0);
            if (sent!=body_size+HEADER_SIZE){
                printf("unable to send client name packet at once: %d\n", sent);
                close(sock_fd);
                return EXIT_FAILURE;
            }
        }
    }

    // Register socket with uloop
    server_ufd = calloc(1, sizeof(struct uloop_fd));
    server_ufd->fd = sock_fd;
    server_ufd->cb = receive_data_handler;
    int ret = uloop_fd_add(server_ufd, ULOOP_READ);
    if (ret < 0) {
        perror("uloop_fd_add failed");
        return EXIT_FAILURE;
    }
    printf("Connected to server, waiting for data...\n");
    client_pipe = calloc(1, sizeof(struct Pipe));
    client_pipe->state = IDLE;
    client_pipe->offset = 0;
    client_pipe->src_fd = client_ufd->fd;
    client_pipe->dst_fd = server_ufd->fd;
    client_pipe->read = read_from_client;
    client_pipe->write = write_to_server;
    server_pipe = calloc(1, sizeof(struct Pipe));
    server_pipe->state = IDLE;
    server_pipe->offset = 0;
    server_pipe->src_fd = server_ufd->fd;
    server_pipe->dst_fd = client_ufd->fd;
    server_pipe->read = read_from_server;
    server_pipe->write = write_to_client;
    return EXIT_SUCCESS;
}


// Callback for accepting new connections
void server_handler(struct uloop_fd *ufd, unsigned int events) {
    if (events & ULOOP_READ) {
        struct sockaddr_in client_addr;
        socklen_t addr_len = sizeof(client_addr);
        int client_fd = accept(ufd->fd, (struct sockaddr *)&client_addr, &addr_len);

        if (client_fd < 0) {
            perror("Accept failed");
            return;
        }

        printf("New connection from %s:%d\n", inet_ntoa(client_addr.sin_addr), ntohs(client_addr.sin_port));
        if (client_ufd != NULL) {
            printf("Already have a client, closing prev instance\n");
            free_resources();
        }
        set_nonblocking(client_fd);

        client_ufd = calloc(1, sizeof(struct uloop_fd));
        client_ufd->fd = client_fd;
        client_ufd->cb = receive_data_handler;
        uloop_fd_add(client_ufd, ULOOP_READ); 
        int result = connect_to_server();
        if (result == EXIT_FAILURE){
            perror("connect_to_server");
            perror("clean client-pipe resources");
            uloop_fd_delete(client_ufd);
            close(client_fd);
            free(client_ufd);
            client_ufd = NULL;
        }
    }
}

int main() {
    struct sockaddr_in server_addr;

    // Initialize uloop
    uloop_init();

    // Create a TCP socket
    int sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd < 0) {
        perror("Socket creation failed");
        return 1;
    }

    // Set socket to non-blocking mode
    set_nonblocking(sockfd);

    // Allow address reuse
    int opt = 1;
    setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

    // Bind the socket
    memset(&server_addr, 0, sizeof(server_addr));
    server_addr.sin_family = AF_INET;
    server_addr.sin_addr.s_addr = INADDR_ANY;
    server_addr.sin_port = htons(SERVER_PORT);

    if (bind(sockfd, (struct sockaddr *)&server_addr, sizeof(server_addr)) < 0) {
        perror("Bind failed");
        close(sockfd);
        return 1;
    }

    // Listen for incoming connections
    if (listen(sockfd, 5) < 0) {
        perror("Listen failed");
        close(sockfd);
        return 1;
    }

    printf("TCP server listening on port %d...\n", SERVER_PORT);

    // Register the server socket with uloop
    server_fd.fd = sockfd;
    server_fd.cb = server_handler;
    uloop_fd_add(&server_fd, ULOOP_READ);

    // Start the event loop
    uloop_run();

    // Cleanup
    uloop_done();
    close(sockfd);
    return 0;
}


