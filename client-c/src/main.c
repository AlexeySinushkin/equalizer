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
#include "packet.h"

typedef u_int8_t u8;

#define PORT 12007
#define VPN_SERVER_PORT 12008
#define SA struct sockaddr


int acceptVpnClient(){
    int sockfd, vpnClientFd, len;
    struct sockaddr_in servaddr, cli;

    // socket create and verification
    sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd == -1)
    {
        printf("socket creation failed...\n");
        exit(1);
    }
    else
    {
        printf("Socket successfully created..\n");
    }

    bzero(&servaddr, sizeof(servaddr));

    // assign IP, PORT
    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
    servaddr.sin_port = htons(PORT);

    // Binding newly created socket to given IP and verification
    if ((bind(sockfd, (SA *)&servaddr, sizeof(servaddr))) != 0)
    {
        printf("socket bind failed...\n");
        exit(2);
    }
    else
    {
        printf("Socket successfully binded..\n");
    }

    // Now server is ready to listen and verification
    if ((listen(sockfd, 5)) != 0)
    {
        printf("Listen failed...\n");
        exit(3);
    }
    else
    {
        printf("Server listening..\n");
    }
        
    len = sizeof(cli);

    // Accept the data packet from client and verification
    vpnClientFd = accept(sockfd, (SA *)&cli, &len);
    if (vpnClientFd < 0)
    {
        printf("server accept failed...\n");
        exit(4);
    }
    else {
        printf("server accept the client...\n");
    }    
    return vpnClientFd;
}

int connectToVpnServer() {
    int sockfd;
    struct sockaddr_in servaddr, cli;

    // socket create and verification
    sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd == -1) {
        printf("socket creation failed...\n");
        return -1;
    }
    else{
        printf("Socket successfully created..\n");
    }
        
    bzero(&servaddr, sizeof(servaddr));

    // assign IP, PORT
    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = inet_addr("127.0.0.1");
    servaddr.sin_port = htons(VPN_SERVER_PORT);

    // connect the client socket to server socket
    if (connect(sockfd, (SA*)&servaddr, sizeof(servaddr))!= 0) {
        printf("connection with the server failed...\n");
        return -2;
    }
    else{
        printf("connected to the server..\n");
    }
    return sockfd;
}

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
    int clientFd = acceptVPNClient();
    if (clientFd<0){
        return -10;
    }
    int serverFd = connectToVpnServer();
    if (serverFd<0){
        return -20;
    }

    return exchange(clientFd, serverFd);
}


int main(int argc, char *argv[])
{
    while (1)
    {
        communication_session();
    }   
}