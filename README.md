# Fractal Clock

This is a fractal clock written in Rust using [wgpu](https://github.com/gfx-rs/wgpu). The computation of vertex positions
is performed on the GPU using a [compute shader](./src/vertices.wgsl).

I have also written C language bindings for this, so it can be statically linked into other programs, namely, a MacOS
screensaver through Objective-C, which is not published to this repository.

## How to run

First, you need the [rust compiler](https://rustup.rs/), with `cargo` installed. Then run:

    cargo run --bin windowed --features bin --release

The above command is in the [runscript](./run) as `run release`.

A window will appear with the fractal clock. Pressing the up and down arrow keys will increase and decrease the depth of
the fractal.
