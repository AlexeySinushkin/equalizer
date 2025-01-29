export STAGING_DIR=~/openwrt/staging_dir
export TOOLCHAIN_DIR=$STAGING_DIR/toolchain-mipsel_24kc_gcc-12.3.0_musl
export PATH=$TOOLCHAIN_DIR/bin:$PATH
export SYSROOT=$TOOLCHAIN_DIR
export CC=mipsel-openwrt-linux-musl-gcc
export LD=mipsel-openwrt-linux-musl-ld
export CFLAGS="--sysroot=$SYSROOT -I$SYSROOT/include -D__MUSL__"

cd ~/equalizer/client-c
make clean
make CC=$CC CFLAGS="$CFLAGS" LD=$LD
