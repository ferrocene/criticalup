# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: The Ferrocene Developers

# This is built based on stock ubuntu

ARG TARGET_UBUNTU_VERSION=24.04

#
# As a multiplatform container we support all these: https://docs.docker.com/reference/dockerfile/#automatic-platform-args-in-the-global-scope
FROM ubuntu:$TARGET_UBUNTU_VERSION AS ferrocene_builder

ARG TARGET_UBUNTU_VERSION=20.04
ARG CRITICALUP_RELEASE=1.6.0
ARG FERROCENE_RELEASE

USER root

SHELL [ "bash", "-c" ]

RUN <<-EOF
    set -xe
    echo 'debconf debconf/frontend select Noninteractive' | debconf-set-selections

    apt-get update
    # Update all the dependencies
    apt-get upgrade -y
    # Install required packages
    apt-get install -yq --no-install-recommends --option Dpkg::Options::=--force-confnew \
        curl \
        unzip \
        xz-utils \
        ca-certificates
    rm -rf /var/lib/apt/lists/*
EOF

# Tell programs relying on the system language (like Python) to use UTF-8.
ENV LANG=C.UTF-8

ENV PATH="root/.cargo/bin:$PATH"

# install ferrocene

RUN --mount=type=secret,id=criticalup_token,env=CRITICALUP_TOKEN <<-EOF
  set -xe

  curl --proto '=https' --tlsv1.2 -LsSf https://github.com/ferrocene/criticalup/releases/download/v${CRITICALUP_RELEASE}/criticalup-installer.sh | sh

  # If no criticalup.toml is added to the build, init criticalup.toml with the default configuration
  if [[ ! -e criticalup.toml ]]; then
    criticalup init --release $FERROCENE_RELEASE
  fi

  cat criticalup.toml

  criticalup auth set $CRITICALUP_TOKEN
  criticalup install
  criticalup auth remove
EOF

CMD ["sh", "-c", "echo 'This image is intended for multi-stage builds only.' >&2; exit 1"]

FROM ubuntu:$TARGET_UBUNTU_VERSION AS example

COPY --from=0 /root/.cache/criticalup/artifacts artifacts
CMD ["/usr/bin/ls", "-R", "artifacts"]
