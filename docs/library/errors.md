# `errors` — The crate's error type

**Source code:** [`src/lib.rs`](../../src/lib.rs)

The crate root defines one error type and one constructor. Library functions return `Result<T>`. When a check fails, the `Err` value is a string wrapped in an I/O error, so it can move between threads and print with the `{}` format.

## The error type

*type* `Error`

`Box<dyn std::error::Error + Send + Sync>`. Any error the standard library or a dependency returns can be stored here, and a function can hand it to another thread.

*type* `Result<T>`

`std::result::Result<T, Error>`. This is the `Result` in `use cev_onboarding::Result`. It is a different type from `std::result::Result` until that import is in scope.

`error(message)`

Build an `Error` from anything that converts into a `String`. The message is stored as `std::io::Error` with kind `Other`, then boxed.

```rust
return Err(crate::error("Invalid run id"));
```

The text you pass is the text `main` prints on failure. The helpers in this library use that for a header that does not match, a grid that is too large, a command that is not a real number, and the other checks described on each page.

## See also

- [`protocol`](protocol.md) — header and decode errors
- [`grid`](grid.md) — dimension and storage errors
- [`types`](types.md) — vehicle and observation checks
