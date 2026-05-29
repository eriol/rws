# rws

`rws` is an opinionated tool to manage a ROS workspace by registering external
packages, linking them into `src/`, and generating a root `justfile` that
exposes each package as a module. You can also provide a `envrc` and
`dependencies.repos` that will be exposed at the root of the workspace.

The final workspace layout is:

```text
workspace/
  src/
  .rws/
    rws.toml
    justfile.common
    envrc
    dependencies.repos
  justfile
  .envrc
  dependencies.repos
```

Each package can provide its own justfile at `.rws/justfile`:

```text
camera/
  package.xml
  .rws/
    justfile
```

## Install

```
cargo install --branch main --git https://noa.mornie.org/eriol/rws
```

### Shell Completion

After installing `rws`, shell completions can be installed for Bash or Zsh.

For Bash:

```bash
mkdir -p ~/.local/share/bash-completion/completions
rws completion bash > ~/.local/share/bash-completion/completions/rws
```

For Zsh:

```bash
mkdir -p ~/.zfunc
rws completion zsh > ~/.zfunc/_rws
```

Make sure `~/.zfunc` is in your `fpath` for Zsh.

## Usage

```bash
rws init ~/ws/my_robot
cd ~/ws/my_robot
rws update --common-justfile path/to/common.just --envrc path/to/.envrc --dependencies-repos path/to/dependencies.repos
rws package add ~/dev/robot/camera ~/dev/robot/lidar
rws sync
just camera start
just lidar start
```

After `init`, rws commands can run from the workspace root or any
subdirectory. rws finds the workspace by walking up to `.rws/rws.toml`.

To remove managed workspace files later:

```bash
rws update --no-common-justfile
rws update --no-envrc
rws update --no-dependencies-repos
```
