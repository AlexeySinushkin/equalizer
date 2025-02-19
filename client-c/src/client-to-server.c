#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <errno.h>
#include "common.h"
#include "packet.h"
#include "pipe.h"
#include <sys/socket.h>



struct Header create_data_header(int packet_size)
{
    struct Header header;
    header.packet_type = TYPE_DATA;
    header.packet_size = packet_size;
    return header;
}

void header_to_buf(struct Header *header, u8* header_buf)
{
    header_buf[0] = FIRST_BYTE;
    header_buf[TYPE_BYTE_INDEX] = header->packet_type;
    header_buf[LENGTH_BYTE_LSB_INDEX] = (u8)(header->packet_size & 0xFF);
    header_buf[LENGTH_BYTE_MSB_INDEX] = (u8)((header->packet_size >> 8) & 0xFF);
}

int write_packet(struct Pipe* pipe){ //-> EXIT_FAILURE | EXIT_SUCCESS

    while (pipe->offset < pipe->size)
    {
        //тело идет сразу за заголовком и выровнено по 4 байта - отправляем одним буфером
        int sent = send(pipe->dst_fd, pipe->header_buf + pipe->offset, pipe->size - pipe->offset, MSG_NOSIGNAL);
        if (sent == -1) {
            if (errno == EWOULDBLOCK || errno == EAGAIN) {
                //printf("Socket is not ready for writing, try again later.\n");
                return EXIT_SUCCESS;  // Not an error, just need to retry later
            } else {
                perror("Send to server error");
                pipe->state = ERROR;
                return EXIT_FAILURE;  // Real error
            }
        }
        //printf("--> Forwarded to server %d\n", sent);
        pipe->offset += sent;
    }
    if (pipe->offset == pipe->size)
    {
        pipe->state = IDLE;
    }
    return EXIT_SUCCESS;
}


int on_client_rw_state_changed(struct Pipe *pipe){ //-> EXIT_FAILURE | EXIT_SUCCESS
    //printf("on_client_rdata_available %d\n", pipe->state);
    if (pipe->state == WRITING)
    {
        int write_result = write_packet(pipe);
        if (write_result == EXIT_FAILURE)
        {
            pipe->state = ERROR;
            return EXIT_FAILURE;
        }
    }
    if (pipe->state == READING || pipe->state == IDLE){
        pipe->state = READING;
        int from_client_bytes_read = read(pipe->src_fd, pipe->body_buf, MAX_BODY_SIZE);

        if (from_client_bytes_read==0){
            fprintf(stderr, "Nothing was read %d %d\n", from_client_bytes_read, errno);
            pipe->state = ERROR;
            return EXIT_FAILURE;
        }
        if (from_client_bytes_read == -1) {
            if (errno == EWOULDBLOCK || errno == EAGAIN) {
                //printf("Socket is not ready for reading, try again later.\n");
                return EXIT_SUCCESS;  // Not an error, just need to retry later
            } else {
                fprintf(stderr, "Read error %d\n", errno);
                pipe->state = ERROR;
                return EXIT_FAILURE;  // Real error
            }
        }   
        //printf("--> Received from client %d\n", from_client_bytes_read);
        struct Header header = create_data_header(from_client_bytes_read);
        header_to_buf(&header, pipe->header_buf);
        pipe->offset = 0;
        pipe->size = HEADER_SIZE + from_client_bytes_read;
        pipe->state = WRITING;
        return write_packet(pipe);
    }
    if (pipe->state == ERROR)
    {
        pipe->state = ERROR;
        return EXIT_FAILURE;
    }
    return EXIT_SUCCESS;
}