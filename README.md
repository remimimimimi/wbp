# WBP

[![CI](https://github.com/remimimimimi/wbp/actions/workflows/ci.yml/badge.svg)](https://github.com/remimimimimi/wbp/actions/workflows/ci.yml)
[![docs](https://img.shields.io/github/actions/workflow/status/remimimimimi/wbp/documentation.yml?branch=main&label=docs)](https://remimimimimi.github.io/wbp/wbp/index.html)


WBP (Web Browser Project) is a lightweight, from-scratch web browser engine written in Rust. It parses HTML5 (but supports only HTML4 tags) and CSS2.2, builds a simple DOM tree, computes box-model layouts, and renders content in a native window using winit. Actual specifications supported only loosely, as proper implementation would require much more effort. 

## Goals

- **Learn & Experiment**: We want to explore how browsers work under the hood: parsing, styling, layout, and rendering.
- **Upstream Contributions**: Working on such a big projects like this can show shortcomings of used libraries, which is a great inspiration for contributions. So other goal is to improve rust ecosystem.

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable or nightly)

### Build & Run

```bash
# 1. Clone the repo
git clone https://github.com/remimimimimi/wbp.git
cd wbp

# 2. Build & Run against a sample page. Currently this page is hardcoded to the wbp/test.{html,css}.
cargo run --release
```

## References

- https://limpet.net/mbrubeck/2014/08/08/toy-layout-engine-1.html
- https://web.dev/articles/howbrowserswork

## License

This project is licensed under the AGPL-3.0. By using or contributing to wbp, you agree to keep it free and open for everyone.
