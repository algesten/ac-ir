Infrared control for MSZ-HR35VF
===============================

This is to control a MSZ-HR35VF from soldering into the
IR-receiver and drive it with a Raspberry Pi.

## Cross compile macOS -> musl (raspberry pi)

1. rustup target add aarch64-unknown-linux-musl
2. brew install FiloSottile/musl-cross/musl-cross
3. `.cargo/config.toml` (see below)
4. TARGET_CC=aarch64-linux-musl-gcc cargo build --release --target aarch64-unknown-linux-musl

```toml
[target.aarch64-unknown-linux-musl]
linker = "aarch64-linux-musl-gcc"
```
