# rio

> [!WARNING]
> THIS CRATE IS A WORK IN PROGRESS!

Minimal asynchronous runtime for exploring `async` Rust.

[![MIT Licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/hmunye/rio/blob/main/LICENSE)

## Quick Start

To include `rio` in your project as a dependency:

```bash
cargo add --git https://github.com/hmunye/rio.git
```

Examples using this crate could be found [here](https://github.com/hmunye/rio/tree/main/examples).

## Limitations

This crate is Linux-only due to dependencies on:

- *`epoll(7)`* for efficient single-threaded, non-blocking I/O

## License

This project is licensed under the [MIT License].

[MIT License]: https://github.com/hmunye/rio/blob/main/LICENSE
