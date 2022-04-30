# mdbook-template

[![build](https://github.com/sgoudham/mdbook-template/actions/workflows/build.yml/badge.svg)](https://github.com/sgoudham/mdbook-template/actions/workflows/build.yml)
[![crate.io](https://img.shields.io/crates/v/mdbook-template)](https://crates.io/crates/mdbook-template)
[![downloads](https://img.shields.io/crates/d/mdbook-template)](https://crates.io/crates/mdbook-template)
[![license](https://img.shields.io/github/license/sgoudham/mdbook-template)](LICENSE)

> A mdbook preprocessor that allows the re-usability of template files with variable arguments

## Table of Contents

TODO

## Author Notes

I'm still a beginner in terms of my Rust skills so I'm _definitely... probably_ sure that there are edge cases within
this preprocessor, and I'm sure that the code could be extra performant.

## Installation

**Install Through Cargo**

```shell
$ cargo install mdbook-template
```

**Add the following line into your `book.toml`**

```toml
[preprocessor.template]
```

**You're good to go! Continue building your mdbook normally!**

```shell
$ mdbook build
```

## About

Being based on the `{{#include ... }}` feature of mdbook, mdbook-template allows you to use familiar syntax to include
files while passing in arguments to allow for dynamic generation of text.

Given the following directory structure

```text
book.toml
src
├── one.md
├── two.md
├── two
│   └── three.md
├── images
│   ├── ferris.png
│   └── corro.png
└── SUMMARY.md
```

If we wanted to include the images `ferris.png` and `corro.png` within all the files through a footer, we'd have to copy
the same piece of markdown/code in every file and set a unique path back to the `images/` directory.

This is where `mdbook-template` can help.

Through the addition of a `footer.md`, you can define a common template that every page will be able to reference with a
relative path back to the images to ensure they are properly displayed.

## Example

```text
book.toml
src
├── one.md
├── two.md
├── two
│   └── three.md
├── images
│   ├── ferris.png
│   └── corro.png
├── templates
│   └── footer.md
└── SUMMARY.md
```

TODO

Further examples are included within the [](/examples) section which demonstrate a variety of usages.

## License

[MIT](LICENSE)

## Contributing

First, thanks for your interest in contributing to this project! Please read the [CONTRIBUTING.md](CONTRIBUTING.md)
before contributing!

## Acknowledgement

This preprocessor is heavily based off the
[`links.rs`](https://github.com/rust-lang/mdBook/blob/master/src/preprocess/links.rs) file within
[mdbook](https://github.com/rust-lang/mdBook) itself. I definitely wouldn't have been able to mock up something like
this without the strong foundations that mdbook already implemented.