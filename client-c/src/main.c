#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <strings.h> // bzero()
#include <errno.h>
#include <socket.h>
#include <arpa/inet.h> // inet_addr()
#include <netdb.h>


#include "connect.h"
#include "communicate.h"


int main(int argc, char *argv[])
{
    while (1)
    {
        communication_session();
    }   

}