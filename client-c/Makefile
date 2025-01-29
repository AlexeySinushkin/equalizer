CC=gcc
CFLAGS= -I/usr/include/x86_64-linux-gnu -I/usr/include/x86_64-linux-gnu/bits  -Wall -std=gnu99 -lpthread -g -D_POSIX_C_SOURCE -O0
LDFLAGS=-pthread -lrt
SRC_DIR=src
OBJ_DIR=obj
BUILD_DIR=build
TARGET=$(BUILD_DIR)/equalizer-client


SRCS=$(wildcard $(SRC_DIR)/*.c)
OBJS=$(SRCS:$(SRC_DIR)/%.c=$(OBJ_DIR)/%.o)

all: $(TARGET)

$(TARGET): $(OBJS)
	@mkdir -p $(BUILD_DIR)
	$(CC) $(CFLAGS) -o $@ $^ $(LDFLAGS)

$(OBJ_DIR)/%.o: $(SRC_DIR)/%.c
	@mkdir -p $(OBJ_DIR)
	$(CC) $(CFLAGS) -c -o $@ $<

clean:
	rm -rf $(OBJ_DIR) $(BUILD_DIR)

run: $(TARGET)
	./$(TARGET)