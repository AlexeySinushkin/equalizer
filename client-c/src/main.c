#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <sys/types.h>



#include "connect.h"
#include "communicate.h"


int main(int argc, char *argv[])
{
    while (1)
    {
        int result = communication_session();
        printf("Session end with result %d Sleep 10 sec\n", result);
        sleep(10);
    }   
}