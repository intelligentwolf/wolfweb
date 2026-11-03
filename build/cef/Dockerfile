# Build environment for WolfWeb's own CEF (Chromium Embedded Framework) — CEF is developed and tested on Ubuntu
# (chromiumembedded.github.io/cef/branches_and_building: Linux "Ubuntu 20.04+"), so the build runs in Ubuntu
# whatever the host is. Chromium's own build dependencies are installed later from its checkout
# (src/build/install-build-deps.sh), so they always match the Chromium version.
FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      python3 python3-venv git curl ca-certificates xz-utils bzip2 file lsb-release sudo procps patch \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
