#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include "common.h"
#include "packet.h"

u8 packet_body_client_to_server[MAX_BODY_SIZE];


struct Header create_data_header(int packet_size)
{
    struct Header header;
    header.packet_type = TYPE_DATA;
    header.packet_size = packet_size;
    return header;
}


int write_header(int fd, struct Header *header)
{
    u8 header_buf[HEADER_SIZE];
    header_buf[0] = FIRST_BYTE;
    header_buf[TYPE_BYTE_INDEX] = header->packet_type;
    header_buf[LENGTH_BYTE_LSB_INDEX] = (u8)(header->packet_size & 0xFF);
    header_buf[LENGTH_BYTE_MSB_INDEX] = (u8)((header->packet_size >> 8) & 0xFF);

    int offset = 0;
    while (offset < HEADER_SIZE)
    {
        int written = write(fd, header_buf + offset, HEADER_SIZE - offset);
		if (written <=0)
		{                    
			return written;
		}
        offset += written;
    }
    if (offset < HEADER_SIZE)
    {
        return -1;
    }
    return 0;
}


int on_client_rdata_available(int src_fd, int dst_fd){
    int from_client_bytes_read = read(src_fd, packet_body_client_to_server, MAX_BODY_SIZE);

    if (from_client_bytes_read <= 0)
    {            
        return from_client_bytes_read;

    }    
    printf("--> Received from client %d\n", from_client_bytes_read);

    struct Header header = create_data_header(from_client_bytes_read);

    int write_header_result = write_header(dst_fd, &header);
    if (write_header_result != 0)
    {            
        return write_header_result;
    }
    
    int offset = 0;
    while (offset < header.packet_size)
    {
        int written = write(dst_fd, packet_body_client_to_server + offset, header.packet_size - offset);
        printf("--> Forwarded to server %d\n", written);
        if (written <= 0)
        {                
            return written;
        }
        offset += written;
    }
    return from_client_bytes_read;
}