#!/bin/sh
make clean
echo $1
ssh ot "rm -rf  ~/equalizer/client-c/build && rm -rf ~/equalizer/client-c/obj"
rsync -avr ./ ot:~/equalizer/client-c
ssh ot "~/equalizer/client-c/build.sh ~/equalizer/client-c/$1 && scp ~/equalizer/client-c/build/equalizer-client jump:~/"