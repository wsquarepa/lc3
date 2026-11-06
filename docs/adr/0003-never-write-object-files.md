# Never write object files

The toolchain assembles and converts sources in memory and runs them; it never emits `.obj`, `.sym` or any other output file. `.obj` files produced by `lc3as` or `lc3convert` are still accepted as sources. This removes the `lc3as`/`lc3convert` hazard of leaving a partial object file behind on error, and nothing in the product needs an object file once every format can be run directly.
