make clean
ssh ot "rm -rf  ~/equalizer/client-c/build && rm -rf ~/equalizer/client-c/obj"
rsync -avr ./ ot:~/equalizer/client-c
ssh ot "~/equalizer/client-c/build.sh && scp ~/equalizer/client-c/build/equalizer-client o2:~/"