#!/bin/bash
export PATH=$PATH:$HOME/.cargo/bin
cd /mnt/d/XXXX/R2D2/r2d2-ui
cargo check > check_out.txt 2>&1
