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

## Usage

```bash
rws workspace init --workspace ~/ws/my_robot
rws workspace update --workspace ~/ws/my_robot --common-justfile path/to/common.just --envrc path/to/.envrc
rws package add --workspace ~/ws/my_robot ~/dev/robot/camera
rws package add --workspace ~/ws/my_robot ~/dev/robot/lidar
rws workspace sync --workspace ~/ws/my_robot
just camera start
just lidar start
```

To remove managed workspace files later:

```bash
rws workspace update --workspace ~/ws/my_robot --no-common-justfile
rws workspace update --workspace ~/ws/my_robot --no-envrc
```

The generated root `justfile` imports `.rws/justfile.common` when configured
and adds one module per package:

```just
import '.rws/justfile.common'

mod camera 'src/camera/.rws/justfile'
mod lidar 'src/lidar/.rws/justfile'
```
