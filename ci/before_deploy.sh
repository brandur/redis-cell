# This script takes care of building your crate and packaging it for release

set -ex

main() {
    local src=$(pwd) \
          stage=

    case $TRAVIS_OS_NAME in
        linux)
            stage=$(mktemp -d)
            ;;
        osx)
            stage=$(mktemp -d -t tmp)
            ;;
    esac

    test -f Cargo.lock || cargo generate-lockfile

    # The template uses `-- -C lto`, but this only applies to executables,
    # cdylibs, and static libraries.
    cross rustc --target $TARGET --release

    # Naming will be .dylib on Mac OS and .so elsewhere.
    cp target/$TARGET/release/libredis_cell.* $stage/

    cd $stage
    tar czf $src/$CRATE_NAME-$TRAVIS_TAG-$TARGET.tar.gz *
    cd $src

    rm -rf $stage
}

main
