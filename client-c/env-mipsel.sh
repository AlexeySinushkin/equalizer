export STAGING_DIR=~/openwrt/staging_dir
export TOOLCHAIN_DIR=$STAGING_DIR/toolchain-mipsel_24kc_gcc-12.3.0_musl
export TARGET_INCLUDE=$STAGING_DIR/target-mipsel_24kc_musl/usr/include
export TARGET_LIBS=$STAGING_DIR/target-mipsel_24kc_musl/usr/lib
export CC=mipsel-openwrt-linux-musl-gcc
export LD=mipsel-openwrt-linux-musl-ld