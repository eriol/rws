# rws

`rws` manages a ROS workspace by registering external packages, linking them
into `src/`, and generating a root `justfile` that exposes each package as a
module. You can also provide a `envrc` that will be exposed at the root of the
workspace.

The final workspace layout is:

```text
workspace/
  src/
  .rws/
    rws.toml
    justfile.common
    envrc
  justfile
  .envrc
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

## Usage

```bash
rws init ~/ws/my_robot
cd ~/ws/my_robot
rws update --common-justfile path/to/common.just --envrc path/to/.envrc
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
```

The generated root `justfile` imports `.rws/justfile.common` when configured
and adds one module per package:

```just
import '.rws/justfile.common'

mod camera 'src/camera/.rws/justfile'
mod lidar 'src/lidar/.rws/justfile'
```
