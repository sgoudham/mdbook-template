# mdbook-template

[![build](https://github.com/sgoudham/mdbook-template/actions/workflows/build.yml/badge.svg)](https://github.com/sgoudham/mdbook-template/actions/workflows/build.yml)
[![crate.io](https://img.shields.io/crates/v/mdbook-template)](https://crates.io/crates/mdbook-template)
[![downloads](https://img.shields.io/crates/d/mdbook-template)](https://crates.io/crates/mdbook-template)
[![license](https://img.shields.io/github/license/sgoudham/mdbook-template)](LICENSE)

> A mdbook preprocessor that allows the re-usability of template files with variable arguments

## Table of Contents

TODO

## Author Notes

I'm still a beginner in terms of my Rust skills, so I'm _definitely... probably_ sure that there are edge cases within
this preprocessor.

## Installation

**Install Through Cargo**

```shell
$ cargo install mdbook-template
```

**Add the following line into your `book.toml`**

```toml
[preprocessor.template]
```

**You're good to go :D Continue building your mdbook normally!**

```shell
$ mdbook build
```

## About

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

Being based on the `{{#include ... }}` feature of mdbook, mdbook-template allows you to use familiar syntax to include
files while passing in arguments to allow for dynamic generation of text.

## Format

The format is as follows

```text
        1             2           3
    {{#template     <file>      <args>}}
```

1. The identifier that this text should be replaced
2. The `relative path` to the template file
3. Any arguments that should be substituted within the template file. Arguments should be seperated by whitespace and
   should be in the `key=value` format.

Arguments to be replaced within the template files should be wrapped in `{}` 

## Example

Given the following directory

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

and the following content

`templates/footer.md`

```markdown
- - - - 
Designed By Goudham
![ferris]({path}/ferris.png)
![ferris]({path}/corro.png)
```

`one.md`
```markdown
# One
Some Content...

{{#template templates/footer.md path=images}}
```

`two.md`
```markdown
# Two
Some Content...

{{#template templates/footer.md path=images}}
```

`two/three.md`
```markdown
# Three
Some Content...

{{#template templates/footer.md path=../images}}
```

After running `mdbook build` with the mdbook-template preprocessor enabled, the files will have dynamic paths to the 
images

`one.md`
```markdown
# One
Some Content...

- - - - 
Designed By Goudham
![ferris](images/ferris.png)
![ferris](images/corro.png)
```

`two.md`
```markdown
# Two
Some Content...

- - - - 
Designed By Goudham
![ferris](images/ferris.png)
![ferris](images/corro.png)
```

`two/three.md`
```markdown
# Three
Some Content...

- - - - 
Designed By Goudham
![ferris](../images/ferris.png)
![ferris](../images/corro.png)
```

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