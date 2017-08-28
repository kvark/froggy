## Change Log

### v0.4 (2017-08-28)
  - crate now follows almost all points from Rust API Guidelines ([#52](https://github.com/kvark/froggy/pull/52) [#62](https://github.com/kvark/froggy/pull/62))
  - `iter` and `iter_mut` methods now return only alive components ([#55](https://github.com/kvark/froggy/pull/55))
  - Cursor concept has been changed a lot to provide more power ([#58](https://github.com/kvark/froggy/pull/58) [#59](https://github.com/kvark/froggy/pull/59) [#60(https://github.com/kvark/froggy/pull/60))
  - serious bug on 32-bit machines has been fixed ([#64](https://github.com/kvark/froggy/pull/64))

### v0.3 (2017-05-15)
  - removed storage locks ([#32](https://github.com/kvark/froggy/pull/32) [#33](https://github.com/kvark/froggy/pull/33))
  - optimized epoch initialization ([#37](https://github.com/kvark/froggy/pull/37))
  - implemented `FromIterator` and `IntoIterator` ([#39](https://github.com/kvark/froggy/pull/39))
  - got internal benchmarks ([#36](https://github.com/kvark/froggy/pull/36))
  - compacted pointer data ([#40](https://github.com/kvark/froggy/pull/40))

### v0.2 (2017-05-12)
  - fast index operators ([#18](https://github.com/kvark/froggy/pull/18))
  - weak pointers ([#14](https://github.com/kvark/froggy/pull/14))
  - storage iterators ([#12](https://github.com/kvark/froggy/pull/12))
  - cube-ception demo ([#1](https://github.com/kvark/froggy/pull/1))

### v0.1 (2017-02-11)
  - basic component pointers
  - read/write storage locking
  - slice dereferences and index pinning
  - deferred refcount updates
