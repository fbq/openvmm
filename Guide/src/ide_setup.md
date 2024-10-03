# Development Environment Setup

**Prerequisites:**

- One of:
  - [Getting started on Windows](./getting_started.md)
  - [Getting started on WSL2](./getting_started_wsl.md).
- One of:
  - [Building OpenVMM](./openvmm/build.md)
  - [Building OpenHCL](./openhcl/build.md)

* * *

This page is for those interested in actively iterating on OpenVMM or OpenHCL.

If you're only interested in building OpenVMM / OpenHCL, you can safely skip
this section.

## Setting up VSCode

These instructions assume you're using [VSCode](https://code.visualstudio.com/).

If you're using a different development environment, we nonetheless suggest
reading through this section, so you can enable similar settings in whatever
editor / IDE you happen to be using.

### [WSL] Connecting to WSL using VSCode

When using Visual Studio Code with WSL, be sure to use the
[WSL extension](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-wsl)
instead of accessing your files using the `\\wsl.localhost` share (the repo
should be cloned in the WSL filesystem, as mentioned in the WSL getting started
guide). This will ensure that all VSCode extensions and features to work properly.

Once the
extension is installed, click the blue arrows in the bottom left corner and
select "Connect to WSL". Then open the folder you cloned the repository into.
More information is available
[here](https://learn.microsoft.com/en-us/windows/wsl/tutorials/wsl-vscode).

### Configuring `rust-analyzer`

[rust-analyzer](https://marketplace.visualstudio.com/items?itemName=matklad.rust-analyzer)
provides IDE-like functionality when writing Rust code (e.g: autocomplete, jump
to definition, refactoring, etc...). It is a massive productivity multiplier
when working with Rust code, and it would be a _very bad idea_ to work in the
OpenVMM repo without having it set up correctly.

Check out the [rust-analyzer manual](https://rust-analyzer.github.io/manual.html)
for a comprehensive overview of rust-analyzer's features.

Once installed, we suggest you specify the following additional configuration
options in the OpenVMM workspace's `.vscode/settings.json` file:

```json
{
    "rust-analyzer.linkedProjects": [
        "Cargo.toml",
    ]
}
```

#### (Strongly Suggested) Avoiding cache invalidation

To avoid unnecessary re-builds or lock-contention in the build directory between
rust-analyzer and manual builds, set the following configuration option to give
rust-analyzer a separate target directory:

```json
{
    "rust-analyzer.cargo.targetDir": true,
}
```

#### (Strongly Suggested) Disable nested imports

When auto-importing deps, rust-analyzer defaults to nesting imports, which isn't
the OpenVMM convention.

This can be changed to one-dep-per-line by specifying the following settings:

```json
{
    "rust-analyzer.imports.granularity.group": "item",
    "rust-analyzer.imports.group.enable": false,
}
```

#### Enabling `clippy`

CI will fail if the code is not clippy-clean.
[Clippy](https://doc.rust-lang.org/stable/clippy/) is a linter that helps catch
common mistakes and improves the quality of our Rust code.

By default, rust-analyzer will use `cargo check` to lint code, but it can be
configured to use `cargo clippy` instead:

```json
{
  "rust-analyzer.check.command": "clippy",
}
```

#### Enabling Format on Save

CI will fail if code is not formatted with `rustfmt`.

You can enable the "format on save" option in VSCode to automatically run
`rustfmt` whenever you save a file:

```json
{
    "[rust]": {
        "editor.formatOnSave": true
    },
}
```

#### Enhanced "Enter"

`rust-analyzer` can override the "Enter" key to make it smarter:

- "Enter" inside triple-slash comments automatically inserts `///`
- "Enter" in the middle or after a trailing space in `//` inserts `//`
- "Enter" inside `//!` doc comments automatically inserts `//!`
- "Enter" after `{` indents contents and closing `}` of single-line block

This action needs to be assigned to shortcut explicitly, which can be done by
adding the following line to `keybindings.json`:

```json
// must be put into keybindings.json, NOT .vscode/settings.json!
{
    "key": "Enter",
    "command": "rust-analyzer.onEnter",
    "when": "editorTextFocus && !suggestWidgetVisible && editorLangId == rust"
}
```

### Running `cargo xtask fmt house-rules` on-save

The OpenVMM project includes a handful of custom "house rule" lints that are
external to `rustfmt`. These are things like checking for the presence of
copyright headers, enforcing single-trailing newlines, etc...

These lints are enfoced using `cargo xtask fmt house-rules`, and can be
automatically fixed by passing the `--fix` flag.

We recommend installing the
[RunOnSave](https://marketplace.visualstudio.com/items?itemName=emeraldwalk.RunOnSave)
extension, and configuring it to run these lints as part of your regular
development flow.

Set the following configuration in your `.vscode/settings.json`

```json
{
    "emeraldwalk.runonsave": {
        "commands": [
            {
                "match": ".*",
                "cmd": "cd ${workspaceFolder}"
            },
            {
                "match": ".*",
                "isAsync": true,
                "cmd": "$(cat ./target/xtask-path) fmt house-rules --fix ${file}"
            }
        ]
    },
}
```

### GitHub Pull Request Integration

As the repo is hosted on GitHub, you might find convenient to use the
[GitHub Pull Request](https://marketplace.visualstudio.com/items?itemName=GitHub.vscode-pull-request-github)
VSCode extension. That allows working through the PR feedback and
issues without leaving the comfort of VSCode.

## Setting up pre-commit and pre-push hooks

It's never fun having CI reject your changes due to some minor formatting issue,
especially when it's super quick to run those formatting checks locally. Running
`cargo xtask fmt` before pushing up your code is quick and easy, and will save
you the annoyance of wrestling with formatting check-in gates!

Of course, it's very easy to forget to run `cargo xtask fmt` after making code
changes, but thankfully, you can set up some [git hooks](https://git-scm.com/book/en/v2/Customizing-Git-Git-Hooks)
that will do this for you automatically!

You can run `cargo xtask install-git-hooks --help` for more details on what
hooks are available and their various configuration options, but for most users,
we suggest the following config:

```bash
cargo xtask install-git-hooks --pre-push --with-fmt=yes
```

And you'll be all set!

> NOTE: If you're worried about time, the `pre-push` hook should only takes ~5
> seconds to run locally. That's far better than sinking ~20+ mins of CI time!