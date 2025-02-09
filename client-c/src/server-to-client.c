#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include "common.h"
#include "packet.h"
#include "pipe.h"


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

enum ReadResult read_header(int fd, u8 *buffer, struct Header *header)
{
    if (server_read_offset < HEADER_SIZE)
    {
        bytes_read = read(fd, buffer + server_read_offset, HEADER_SIZE - server_read_offset);
        if (bytes_read <= 0)
        {
            return READ_ERROR;
        }
        server_read_offset += bytes_read;
    }
    if (server_read_offset == HEADER_SIZE)
    {
        return build_header(buffer, header);
    }
    return READ_INCOMPLETE;
}




enum ReadResult read_packet(int fd, u8 *buffer, struct Header *header)
{
    //еще не прочитали заголовок
    if (server_read_offset<HEADER_SIZE){
        enum ReadResult read_header_result = read_header(fd, buffer, header);
        if (read_header_result == READ_ERROR || read_header_result == READ_INCOMPLETE){
            return read_header_result;
        } 
    }else{
        //restore header from buffer
        build_header(buffer, header);
    }

    int right_offset = HEADER_SIZE + header->packet_size;
    if (server_read_offset < right_offset)
    {
        bytes_read = read(fd, buffer + server_read_offset, right_offset - server_read_offset);
        if (bytes_read <= 0)
        {            
            printf("Error 60\n");
            return READ_ERROR;
        }
        server_read_offset += bytes_read;
    }
    if (server_read_offset == right_offset){
        server_read_offset = 0;
        return READ_COMPLETE;            
    }
    return READ_INCOMPLETE;
}

int on_server_rdata_available(struct Pipe *pipe){
	struct Header header;
    enum ReadResult read_packet_result = read_packet(src_fd, buffer_server_to_client, &header);
    if (read_packet_result == READ_COMPLETE)
    {            
		printf("<-- Received from server 0x%02x  %d\n", header.packet_type, header.packet_size);
		if (header.packet_type == TYPE_DATA)
		{
			int offset = 0;
			u8* body_offset = buffer_server_to_client + HEADER_SIZE;
			while (offset < header.packet_size)
			{                
				int written = write(dst_fd, body_offset + offset, header.packet_size - offset);
				printf("<-- Forwarded to client %d\n", written);
				if (written <=0)
				{                    
					return written;
				}
				offset += written;
			}
		}
    }

	return bytes_read;
}