dump-nt out-path:
    cargo run -p pttest dump-noise-tables {{out-path}}

cmp-nt:
    cargo run -p pttest compare-noise-tables