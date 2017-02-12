# froggy
[![Build Status](https://travis-ci.org/kvark/froggy.svg)](https://travis-ci.org/kvark/froggy)
[![Crates.io](https://img.shields.io/crates/v/froggy.svg?maxAge=2592000)](https://crates.io/crates/froggy)

Froggy is a prototype for [Component Graph System](https://github.com/kvark/froggy/wiki/Component-Graph-System). Froggy is not an ECS (it could as well be named "finecs" but then it would have "ecs" in the name... yikes)! Give it a try if:
  - you are open to new paradigms and programming models
  - you are tired of being forced to think in terms of ECS
  - you like simple composable things

Check [ecs_bench](https://github.com/lschmierer/ecs_bench) for performance comparisons with actual ECS systems.

## Example

```rust
    let positions = froggy::Storage::new();
    // create entities
    let entities = {
        let mut p = positions.write();
        vec![p.create(1u8), p.create(4u8), p.create(9u8)]
    };
    // update positions
    {
        let mut p = positions.write();
        for e in &entities {
            *p.access(e) += 1;
        }
    }
```
