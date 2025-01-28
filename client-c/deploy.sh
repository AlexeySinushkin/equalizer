make clean
ssh ot "rm -rf  ~/equalizer/client-c/build && rm -rf ~/equalizer/client-c/obj"
rsync -avr ./ ot:~/equalizer/client-c
ssh ot "cd ~/equalizer/client-c && \
STAGING_DIR=/home/user/openwrt/staging_dir/toolchain-mipsel_24kc_gcc-12.3.0_musl \
make CC=/home/user/openwrt/staging_dir/toolchain-mipsel_24kc_gcc-12.3.0_musl/bin/mipsel-openwrt-linux-musl-gcc \
LD=/home/user/openwrt/staging_dir/toolchain-mipsel_24kc_gcc-12.3.0_musl/bin/mipsel-openwrt-linux-musl-ld \
&& scp build/equalizer-client o2:~/"