export STAGING_DIR=~/openwrt/staging_dir
export TOOLCHAIN_DIR=$STAGING_DIR/toolchain-mipsel_24kc_gcc-12.3.0_musl
export TARGET_INCLUDE=$STAGING_DIR/target-mipsel_24kc_musl/usr/include
export TARGET_LIBS=$STAGING_DIR/target-mipsel_24kc_musl/usr/lib
export PATH=$TOOLCHAIN_DIR/bin:$PATH
export SYSROOT=$TOOLCHAIN_DIR
export CC=mipsel-openwrt-linux-musl-gcc
export LD=mipsel-openwrt-linux-musl-ld
export CFLAGS="--sysroot=$SYSROOT -I$TARGET_INCLUDE -D__MUSL__"
export LDFLAGS="-L$TARGET_LIBS -lubox"

cd ~/equalizer/client-c
make clean
make CC=$CC CFLAGS="$CFLAGS" LDFLAGS="$LDFLAGS" LD=$LD
