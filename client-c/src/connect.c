#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <strings.h> // bzero()
#include <errno.h>
#include <arpa/inet.h> // inet_addr()
#include <netdb.h>

#define LISTEN_PORT 12005
#define SERVER_PORT 12010
#define SA struct sockaddr

int acceptVpnClient(int* vpnClientFd, int* listenSocketFd){
    int sockfd;
    socklen_t len;
    struct sockaddr_in servaddr, cli;

    // socket create and verification
    sockfd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (sockfd == -1)
    {
        printf("socket creation failed...\n");
        return 1;
    }
    else
    {
        *listenSocketFd = sockfd;
        printf("Socket successfully created..\n");
    }

	/* Set socket to reuse address, otherwise bind() could return error,
	 * when server is restarted. */
    int ret, flag;    
	flag = 1;
	ret = setsockopt(sockfd, SOL_SOCKET, SO_REUSEADDR, &flag, sizeof(flag));
	if(ret == -1) {
		perror("setsockopt()");
		return EXIT_FAILURE;
	}

    bzero(&servaddr, sizeof(servaddr));

    // assign IP, PORT
    servaddr.sin_family = AF_INET;
    servaddr.sin_addr.s_addr = htonl(INADDR_ANY);
    servaddr.sin_port = htons(LISTEN_PORT);

    // Binding newly created socket to given IP and verification
    if ((bind(sockfd, (SA *)&servaddr, sizeof(servaddr))) != 0)
    {
        printf("socket bind failed...\n");
        close(sockfd);
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
        close(sockfd);
        return 3;
    }
    else
    {
        printf("Server listening..\n");
    }
        
    len = sizeof(cli);

    // Accept the data packet from client and verification
    int clientFd = accept(sockfd, (SA *)&cli, &len);
    if (clientFd < 0)
    {
        printf("server accept failed...\n");
        return 4;
    }
    else {
        printf("server accept the client...\n");
        *vpnClientFd = clientFd;
    }    
    return 0;
}

int connectToVpnServer(int* vpnServerFd) {
    int sockfd;
    struct sockaddr_in servaddr;

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
    servaddr.sin_port = htons(SERVER_PORT);

    // connect the client socket to server socket
    if (connect(sockfd, (SA*)&servaddr, sizeof(servaddr))!= 0) {
        printf("connection with the server failed...\n");
        return 6;
    }
    else{
        printf("connected to the server..\n");
        *vpnServerFd = sockfd;
    }
    return 0;
}

int acceptAndConnect(int* vpnClientFd, int* listenSocketFd, int* vpnServerFd) {
    if (acceptVpnClient(vpnClientFd, listenSocketFd)==0 && connectToVpnServer(vpnServerFd)==0) {
        return 0;
    }
    return 7;
}