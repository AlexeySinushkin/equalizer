source $1
echo $STAGING_DIR
echo $TOOLCHAIN_DIR
echo $TOOLCHAIN_DIR
echo $TARGET_INCLUDE
echo $TARGET_LIBS
echo $CC
echo $LD
export PATH=$TOOLCHAIN_DIR/bin:$PATH
export SYSROOT=$TOOLCHAIN_DIR
export CFLAGS="--sysroot=$SYSROOT -I$TARGET_INCLUDE -D__MUSL__"
export LDFLAGS="-L$TARGET_LIBS -lubox"

cd ~/equalizer/client-c
make clean
make CC=$CC CFLAGS="$CFLAGS" LDFLAGS="$LDFLAGS" LD=$LD
