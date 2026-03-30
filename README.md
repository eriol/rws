# rws

`rws` manages a ROS workspace by registering external packages, linking them
into `src/`, and generating a root `justfile` that exposes each package as a
module.

The final workspace layout is:

```text
workspace/
  src/
  .rws/
    rws.toml
    justfile.common
  justfile
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
rws init --workspace ~/ws/my_robot --common-justfile path/to/common.just
rws add ~/dev/robot/camera
rws add ~/dev/robot/lidar
rws sync
just camera start
just lidar start
```

The generated root `justfile` imports `.rws/justfile.common` when configured
and adds one module per package:

```just
import '.rws/justfile.common'

mod camera 'src/camera/.rws/justfile'
mod lidar 'src/lidar/.rws/justfile'
```
