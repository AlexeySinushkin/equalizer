#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>
#include <errno.h>
#include <arpa/inet.h>
#include <fcntl.h>
#include <string.h>
#include <libubox/uloop.h>
#include "client-to-server.h"
#include "server-to-client.h"

#define VPN_SERVER_IP   "127.0.0.1"
#define VPN_SERVER_PORT 12010
#define SERVER_PORT 12005
#define BUFFER_SIZE 1024

struct uloop_fd server_fd;
//для взаимодействия с клиентом
struct uloop_fd *vpn_client_ufd;
//для взаимодействия с сервером
struct uloop_fd *vpn_server_ufd;

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

// Callback for handling client connections
void receive_data_handler(struct uloop_fd *ufd, unsigned int events) {
    if (events & ULOOP_READ) {
        int bytes_read = 0; 
        if (ufd == vpn_client_ufd) {
            bytes_read = on_client_rdata_available(ufd->fd, vpn_server_ufd->fd);
        } else if (ufd == vpn_server_ufd) {
            bytes_read = on_server_rdata_available(ufd->fd, vpn_client_ufd->fd);
        } else {
            printf("Unknown file descriptor\n");
            return;
        }
        //read(ufd->fd, buffer, sizeof(buffer) - 1);
        if (bytes_read > 0) {
            printf("Received: %d\n", bytes_read);
        } else if (bytes_read == 0) {
            printf("Client disconnected.\n");
            uloop_fd_delete(ufd);
            close(ufd->fd);
            free(ufd);
        } else {
            if (errno != EWOULDBLOCK && errno != EAGAIN) {
                perror("Read error");
                struct uloop_fd* ufd2 = ufd==vpn_client_ufd ? vpn_server_ufd : vpn_client_ufd;
                uloop_fd_delete(ufd);
                close(ufd->fd);
                free(ufd);         
                if (ufd2 != NULL){
                    uloop_fd_delete(ufd2);
                    close(ufd2->fd);
                    free(ufd2);
                }
                vpn_client_ufd = NULL;
                vpn_server_ufd = NULL;
            }
        }
    }
}
// сразу после подключения VPN клиента, мы подключаемся к в VPN server-у
void connect_to_server(){
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
    server_addr.sin_port = htons(SERVER_PORT);
    inet_pton(AF_INET, VPN_SERVER_IP, &server_addr.sin_addr);

    if (connect(sock_fd, (struct sockaddr *)&server_addr, sizeof(server_addr)) < 0) {
        if (errno != EINPROGRESS) {
            perror("connect");
            close(sock_fd);
            return EXIT_FAILURE;
        }
    }

    // Register socket with uloop
    struct uloop_fd *client_ufd = calloc(1, sizeof(struct uloop_fd));
    client_ufd->fd = sock_fd;
    client_ufd->cb = receive_data_handler;
    vpn_server_ufd = client_ufd;
    uloop_fd_add(client_ufd, ULOOP_READ);

    printf("Connected to server, waiting for data...\n");
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
        set_nonblocking(client_fd);

        struct uloop_fd *client_ufd = calloc(1, sizeof(struct uloop_fd));
        client_ufd->fd = client_fd;
        client_ufd->cb = receive_data_handler;
        vpn_client_ufd = client_ufd;
        uloop_fd_add(client_ufd, ULOOP_READ); 
        connect_to_server();
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


