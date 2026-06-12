FROM archlinux:multilib-devel

RUN pacman -Sy --noconfirm \
    rust \
    clang \
    pkgconf \
    git \
 && pacman -Scc --noconfirm
