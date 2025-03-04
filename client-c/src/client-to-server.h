#include "pipe.h"
struct Header create_filler_header(int body_size);
void header_to_buf(struct Header *header, u8* buf);
int write_to_server(struct Pipe *pipe);
int read_from_client(struct Pipe *pipe);