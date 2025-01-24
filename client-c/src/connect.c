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

int acceptVpnClient(int* vpnClientFd){
    int sockfd, vpnClientFd, len;
    struct sockaddr_in servaddr, cli;

    // socket create and verification
    sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd == -1)
    {
        printf("socket creation failed...\n");
        return 1;
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
        return 2;
    }
    else
    {
        printf("Socket successfully binded..\n");
    }

    // Now server is ready to listen and verification
    if ((listen(sockfd, 5)) != 0)
    {
        printf("Listen failed...\n");
        return 3;
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
        return 4;
    }
    else {
        printf("server accept the client...\n");
    }    
    return 0;
}

int connectToVpnServer(int* vpnServerFd) {
    int sockfd, connfd;
    struct sockaddr_in servaddr, cli;

    // socket create and verification
    sockfd = socket(AF_INET, SOCK_STREAM, 0);
    if (sockfd == -1) {
        printf("socket creation failed...\n");
        return 5;
    }
    else{
        printf("Socket successfully created..\n");
    }
        
    bzero(&servaddr, sizeof(servaddr));

    // assign IP, PORT
    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = inet_addr("127.0.0.1");
    servaddr.sin_port = htons(PORT);

    // connect the client socket to server socket
    if (connect(sockfd, (SA*)&servaddr, sizeof(servaddr))!= 0) {
        printf("connection with the server failed...\n");
        return 6;
    }
    else{
        printf("connected to the server..\n");
    }
    return 0;
}

int acceptAndConnect(int* vpnClientFd, int* vpnServerFd) {
    if (acceptVpnClient(vpnClientFd)==0 && connectToVpnServer(vpnServerFd)==0) {
        return 0;
    }
    return 7;
}