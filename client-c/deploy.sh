#!/bin/sh
make clean
echo $1
ssh ot "rm -rf  ~/equalizer/client-c/build && rm -rf ~/equalizer/client-c/obj"
rsync -avr ./ ot:~/equalizer/client-c
ssh ot "cd ~/equalizer/client-c && make clean && ssh jump \"/etc/init.d/equalizer-client stop\""
ssh ot "~/equalizer/client-c/build.sh ~/equalizer/client-c/$1 && scp -O ~/equalizer/client-c/build/equalizer-client jump:~/"