# mdbook-template

[![build](https://github.com/sgoudham/mdbook-template/actions/workflows/build.yml/badge.svg)](https://github.com/sgoudham/mdbook-template/actions/workflows/build.yml)
[![crate.io](https://img.shields.io/crates/v/mdbook-template)](https://crates.io/crates/mdbook-template)
[![downloads](https://img.shields.io/crates/d/mdbook-template)](https://crates.io/crates/mdbook-template)
[![license](https://img.shields.io/github/license/sgoudham/mdbook-template)](LICENSE)

> A mdbook preprocessor that allows the re-usability of template files with dynamic arguments

## Table of Contents

* [Author Notes](#author-notes)
* [Installation](#installation)
* [About](#about)
* [Format](#format)
    + [Template](#template)
    + [Arguments](#arguments)
    + [Default Values](#default-values)
* [Valid Configurations](#valid-configurations)
    + [Template](#template-config)
    + [Arguments](#arguments-config)
* [Example](#example)
* [GitHub Actions](#github-actions)
* [License](#license)
* [Contributing](#contributing)
* [Acknowledgement](#acknowledgement)

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

**You're good to go :D  
Continue building your mdbook normally!**

```shell
$ mdbook build
```

## About

Given the following directory structure

```text
book.toml
src
├── rust.md
├── go.md
├── friends
│   └── hazel.md
├── images
│   ├── ferris.png
│   └── corro.png
└── SUMMARY.md
```

If we wanted to include the images `ferris.png` and `corro.png` within all the files through a footer, we'd have to copy
the same piece of markdown/code in every file and set a unique path back to the `images/` directory.

This is where `mdbook-template` can help with the introduction of `{{#template ...}`.

Being based on the `{{#include ... }}` feature of mdbook, mdbook-template allows you to use familiar syntax to include
files while passing in arguments to allow for dynamic generation of text.

Please view the given [example](#example) which demonstrates it in action.

## Format

### Template

The format is as follows

```text
        1             2           3
    {{#template     <file>      <args>}}
```

1. The identifier that tells `mdbook-template` that this text should be replaced by a template
2. The `relative path` to the template file
3. Any arguments that should be substituted within the template file. Arguments should be seperated by whitespace and
   should be in the `key=value` format.

### Arguments

Arguments to be replaced within the template files should be wrapped in `[[# ...]]`  
The format is as follows

```text
     1
[[#<name>]]
```

1. The name of the argument

### Default Values

Default values can be set in case some files need dynamic arguments and other don't.  
The format is as follows

```text
      1          2
[[#<name> <default-value>]]
```

1. The name of the argument
2. The value that this argument should have by default

## Valid Configurations

### Template Config

```markdown
{{#template file.txt path=../images author=Goudham}}
```

```markdown
{{#template file.txt path=../images author=Goudham }}
```

```markdown
// Not recommended but valid {{#template file.txt path=../images author=Goudham}}
```

```markdown
// Not recommended but valid {{#template file.txt path=../images author=Goudham }}
```

### Arguments Config

```markdown
\[[#escaped]]
```

```markdown
[[#width]]
```

```markdown
[[#width 200px]]
```

```markdown
// Not recommended but valid
[[   #width   400px   ]]
```

## Example

Given the following directory

```text
book.toml
src
├── rust.md
├── go.md
├── friends
│   └── hazel.md
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
-- Designed By [[#authors]] --
![ferris]([[#path]]/ferris.png)
![corro]([[#path]]/corro.png)
```

`rust.md`

```markdown
# Rust

Some Content...

{{#template templates/footer.md authors=Goudham, Hazel path=images}}
```

`go.md`

```markdown
# Go

Some Content...

{{#template templates/footer.md path=images authors=Goudham, Hazel}}
```

`friends/hazel.md`

```markdown
# Hazel

Some Content...

{{#template ../templates/footer.md path=../images authors=Goudham, Hazel }}
```

After running `mdbook build` with the mdbook-template preprocessor enabled, the files will have dynamic paths to the
images and contain **_identical_** content.

`rust.md`

```markdown
# Rust

Some Content...

-- Designed By Goudham, Hazel --
![ferris](images/ferris.png)
![corro](images/corro.png)
```

`go.md`

```markdown
# Go

Some Content...

-- Designed By Goudham, Hazel --
![ferris](images/ferris.png)
![corro](images/corro.png)
```

`friends/hazel.md`

```markdown
# Hazel

Some Content...

-- Designed By Goudham, Hazel --
![ferris](../images/ferris.png)
![corro](../images/corro.png)
```

Further examples are included within the [examples](/examples) directory which demonstrate a variety of usages.

## GitHub Actions

Include the following within your `.yml` workflow files if you need `mdbook-template` as an executable to build your
book.

```yaml
- name: Install mdbook-template
  run: |
    mkdir mdbook-template
    curl -sSL https://github.com/sgoudham/mdbook-template/releases/latest/download/mdbook-template-x86_64-unknown-linux-gnu.tar.gz | tar -xz --directory=./mdbook-template
    echo `pwd`/mdbook-template >> $GITHUB_PATH
```

The above step will ensure the latest version of mdbook-template is retrieved and built.

## License

[MIT License](LICENSE)

## Contributing

First, thanks for your interest in contributing to this project! Please read the [CONTRIBUTING.md](CONTRIBUTING.md)
before contributing!

## Acknowledgement

This preprocessor is heavily based off the
[`links.rs`](https://github.com/rust-lang/mdBook/blob/master/src/preprocess/links.rs) file within
[mdbook](https://github.com/rust-lang/mdBook) itself. I definitely wouldn't have been able to mock up something like
this without the strong foundations that mdbook already implemented.