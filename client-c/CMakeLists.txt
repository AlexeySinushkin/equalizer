cmake_minimum_required(VERSION 3.10)
project(VPNProject C)

set(CMAKE_C_STANDARD 99)

include_directories(${CMAKE_SOURCE_DIR}/src)
# Set the output directory for the executable
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_SOURCE_DIR}/build)

add_executable(VPNProject
    src/main.c
    src/communicate.c
    src/connect.c
    src/packet.c
)

target_link_libraries(VPNProject
    PRIVATE    
    pthread
    rt
)