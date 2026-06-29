#!/usr/bin/env bash
set -euo pipefail

print_usage() {
    echo "Usage: $0 [-t master|plugin|all]"
    echo "  master  - Baue den Rust Master (ignite-master)"
    echo "  plugin  - Baue das Paper Plugin (ignite-plugin)"
    echo "  all     - Baue beides (default)"
    exit 1
}

TARGET="${1:-all}"

build_master() {
    echo "=== Baue Rust Master ==="
    cd master
    cargo build --release
    echo "Binary: master/target/release/ignite-master"
    cd ..
}

build_plugin() {
    echo "=== Baue Paper Plugin ==="
    cd plugin
    if [ ! -f "gradlew" ]; then
        echo "Generiere Gradle Wrapper..."
        gradle wrapper --gradle-version 8.10
    fi
    chmod +x gradlew
    ./gradlew build
    echo "JAR: plugin/build/libs/ignite-plugin-1.0.0.jar"
    cd ..
}

case "${TARGET}" in
    master)
        build_master
        ;;
    plugin)
        build_plugin
        ;;
    all)
        build_master
        echo ""
        build_plugin
        ;;
    *)
        print_usage
        ;;
esac
