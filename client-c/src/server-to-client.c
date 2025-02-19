#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <errno.h>
#include "common.h"
#include "packet.h"
#include "pipe.h"
#include <sys/socket.h>

enum ReadResult{
    READ_INCOMPLETE,
    READ_COMPLETE,
    READ_ERROR
};


enum ReadResult build_header(u8 *buffer, struct Header *header){
    if (buffer[0] != FIRST_BYTE)
    {
        printf("Error 52\n");
        return READ_ERROR;
    }
    int packet_size = (buffer[LENGTH_BYTE_MSB_INDEX] << 8) + buffer[LENGTH_BYTE_LSB_INDEX];
    if (packet_size > MAX_BODY_SIZE)
    {
        printf("Error 53\n");
        return READ_ERROR;
    }
    header->packet_type = buffer[TYPE_BYTE_INDEX];
    header->packet_size = packet_size;
    return READ_COMPLETE;
}

enum ReadResult read_header(struct Pipe *pipe, struct Header *header)
{
    if (pipe->offset < HEADER_SIZE)
    {
        //printf("Attempting to read from fd %d\n", pipe->src_fd);
        int from_server_bytes_read = read(pipe->src_fd, pipe->header_buf + pipe->offset, HEADER_SIZE - pipe->offset);        
        if (from_server_bytes_read==0){
            printf("read_header %d %d\n", from_server_bytes_read, errno);
            return READ_ERROR;
        }
        if (from_server_bytes_read == -1) {
            if (errno == EWOULDBLOCK || errno == EAGAIN) {
                return READ_INCOMPLETE;  // Not an error, just need to retry later
            } else {
                perror("send");
                return READ_ERROR;  // Real error
            }
        }        
        pipe->offset += from_server_bytes_read;
    }
    if (pipe->offset == HEADER_SIZE)
    {
        return build_header(pipe->header_buf, header);
    }
    return READ_INCOMPLETE;
}




enum ReadResult read_packet(struct Pipe *pipe){
    //еще не прочитали заголовок
    if (pipe->offset<HEADER_SIZE){
        struct Header header;
        int read_header_result = read_header(pipe, &header);
        if (read_header_result == READ_ERROR || read_header_result == READ_INCOMPLETE){
            return read_header_result;
        }
        pipe->size = HEADER_SIZE + header.packet_size;
    }

    if (pipe->offset >= HEADER_SIZE){        
        while (pipe->offset < pipe->size)
        {
            int from_server_bytes_read = read(pipe->src_fd, pipe->body_buf + pipe->offset - HEADER_SIZE, pipe->size - pipe->offset);
            if (from_server_bytes_read==0){
                printf("read_packet %d %d\n", from_server_bytes_read, errno);
                return READ_ERROR;
            }
            if (from_server_bytes_read == -1) {
                if (errno == EWOULDBLOCK || errno == EAGAIN) {
                    //printf("Socket is not ready for reading, try again later.\n");
                    return EXIT_SUCCESS;  // Not an error, just need to retry later
                } else {
                    perror("Read error");
                    pipe->state = ERROR;
                    return EXIT_FAILURE;  // Real error
                }
            }
            pipe->offset += from_server_bytes_read;
        }
    }

    if (pipe->offset == pipe->size){
        return READ_COMPLETE;            
    }
    return READ_INCOMPLETE;
}

int write_packet_body(struct Pipe* pipe){ //-> EXIT_FAILURE | EXIT_SUCCESS
    while (pipe->offset < pipe->size)
    {        
        int sent = send(pipe->dst_fd, pipe->body_buf + pipe->offset, pipe->size - pipe->offset, MSG_NOSIGNAL);
        if (sent == -1) {
            if (errno == EWOULDBLOCK || errno == EAGAIN) {
                //printf("Socket is not ready for writing, try again later.\n");
                return EXIT_SUCCESS;  // Not an error, just need to retry later
            } else {
                perror("Send to client error");
                pipe->state = ERROR;
                return EXIT_FAILURE;  // Real error
            }
        }
        //printf("<-- Forwarded to client %d\n", sent);
        pipe->offset += sent;
    }
    if (pipe->offset == pipe->size)
    {
        pipe->state = IDLE;
        pipe->offset = 0;
    }
    return EXIT_SUCCESS;
}


int on_server_rw_state_changed(struct Pipe *pipe){//-> EXIT_FAILURE | EXIT_SUCCESS
    //printf("on_server_rdata_available %d %d %d\n", pipe->state, pipe->offset, pipe->size);
    if (pipe->state == WRITING)
    {
        int write_result = write_packet_body(pipe);
        if (write_result == EXIT_FAILURE)
        {
            return EXIT_FAILURE;
        }
    }

    if (pipe->state == READING || pipe->state == IDLE){
        pipe->state = READING;
        enum ReadResult read_result = read_packet(pipe);
        if (read_result == READ_ERROR){
            return EXIT_FAILURE;
        }else if (read_result == READ_INCOMPLETE){
            return EXIT_SUCCESS;
        }else if (read_result == READ_COMPLETE){
            struct Header header;
            build_header(pipe->header_buf, &header);
            //printf("<-- Received from server 0x%02x  %d\n", header.packet_type, header.packet_size);
            if (header.packet_type == TYPE_DATA){                
                pipe->offset = 0;
                pipe->size = header.packet_size;
                pipe->state = WRITING;
                return write_packet_body(pipe);                
            }else{
                pipe->state = IDLE;     
                pipe->offset = 0;           
            }
        }
    }
    if (pipe->state == ERROR)
    {
        return EXIT_FAILURE;
    }
    return EXIT_SUCCESS;
}