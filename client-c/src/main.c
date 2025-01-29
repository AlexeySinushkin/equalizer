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
        communication_session();
        printf("Sleep 10 sec\n");
        sleep(10);
    }   

}