# rio

Minimal Asynchronous Runtime for Rust.

> [!WARNING]
> This project is experimental and not intended for production use.

[![MIT Licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/hmunye/rio/blob/main/LICENSE)
[![Build Status](https://github.com/hmunye/rio/workflows/CI/badge.svg)](https://github.com/hmunye/rio/actions?query=workflow%3ACI+branch%3Amain)
[![Dependency Status](https://deps.rs/repo/github/hmunye/rio/status.svg)](https://deps.rs/repo/github/hmunye/rio)

## Quick Start

Add `rio` to your project as a dependency:

```bash
cargo add --git https://github.com/hmunye/rio.git rio --features full
```

Or in `Cargo.toml`:

```bash
[dependencies]
rio = { git = "https://github.com/hmunye/rio.git", version = "0.1.0", features = ["full"] }
```

Examples using this crate can be found [here](https://github.com/hmunye/rio/tree/main/examples).

## License

This project is licensed under the [MIT License].

[MIT License]: https://github.com/hmunye/rio/blob/main/LICENSE
