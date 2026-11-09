# cheriot-compose

`cheriot-compose` builds CHERIoT firmware from TOML projects. It checks
configuration, shows build plans, builds firmware and can run it on a configured
simulator or board.

## Command-Line Reference

### Command-Line Options

#### Common Commands

Run these commands from the root of an RTOS checkout after installing Compose,
copying its integration into the checkout and installing the CHERIoT compiler.
The walkthrough below explains each step. These examples use the compiler at
`~/cheriot-tools/bin`; replace it with your own executable directory if needed.
Compose's built-in default is `/cheriot-tools/bin`. The `run` command also needs
the board's runner installed.

```sh
# Check a project without running the compiler.
cheriot-compose check --toolchain-dir "/cheriot-tools/bin" --project hello-world

# Build an example.
cheriot-compose build --toolchain-dir "/cheriot-tools/bin" --project hello-world

# Build the default RTOS test firmware.
cheriot-compose build --toolchain-dir "/cheriot-tools/bin" --project tests --board sail --mode release --jobs 4

# Inspect the complete build plan.
cheriot-compose plan --toolchain-dir "/cheriot-tools/bin" --project tests --format json

# Build or reuse the example and start its configured runner.
cheriot-compose run --toolchain-dir "/cheriot-tools/bin" --project hello-world
```

#### Command Overview

The command comes first, followed by its options. The `help` and `version`
commands work without a project or compiler. Use `help` with a command name,
or put `--help` after a command, to see its options.

| Command | Purpose |
| --- | --- |
| `analyze-switcher` | Analyze annotated switcher assembly. |
| `audit` | Run an audit after building. |
| `build` | Build the selected firmware. |
| `check` | Validate a project or every project in a workspace. |
| `clean` | Remove files owned by the selected build. |
| `compare-compilation-database` | Compare compiler commands, with optional path mappings. |
| `configuration` | Show the resolved configuration; use `--format json` for JSON. |
| `explain` | Explain an operation in the build plan. |
| `help` | Show help. |
| `package` | Build and create the selected board's package. |
| `plan` | Show the complete clean-build plan in text, JSON, shell or compilation-database format. |
| `run` | Build if needed, then start the board's configured runner. |
| `size` | Report firmware or compartment size. |
| `version` | Show the application version. |

For project commands, `--file` selects a project or workspace, `--project`
selects a project in a workspace, and `--firmware` selects an image when a
project defines more than one. Repeat `--overlay` to apply configuration changes
in order. `--board` and `--mode` select a board and build mode. Build-related
commands accept `--jobs`, `--verbose`, `--quiet` and `--dry-run` as shown in
their command-specific help.

## Source-Build Walkthrough

This Linux/POSIX-shell walkthrough installs CHERIoT LLVM and the Compose host
application, then builds the RTOS test firmware. It needs Git, a working host
C/C++ compiler and build tools, CMake, Ninja, Python 3.8 or later, and Rust 1.89 or later
with Cargo. Python is used by the LLVM build; Compose itself is written in Rust.
A simulator is optional and is needed only to run the firmware.

On Ubuntu or Debian, install the non-Rust host prerequisites with:

```sh
sudo apt update
sudo apt install build-essential ca-certificates cmake curl git ninja-build python3
```

The CHERIoT LLVM `cheriot` branch requires CMake 3.20 or later at the revision
checked for this guide; its build warns that future LLVM versions will require
3.31 or later. Check `cmake --version` and install a suitable newer CMake if
your distribution's package is too old. The branch can change, so check its
build messages if using a newer checkout.

### Choose Directories

Set these shell variables once. They make the commands easier to read; they are
not Compose configuration settings. You can choose different absolute paths,
including existing checkouts. The directories do not have to be siblings.
Clone only repositories that are not already present at your chosen paths.

```sh
export CHERIOT_COMPOSE="$HOME/cheriot-compose"
export CHERIOT_LLVM="$HOME/cheriot-llvm"
export CHERIOT_RTOS="$HOME/cheriot-rtos"
export CHERIOT_TOOLS="$HOME/cheriot-tools"
```

### Build and Install CHERIoT LLVM

An ordinary host LLVM installation cannot compile CHERIoT firmware. For a new
checkout, obtain the CHERIoT branch of the
[CHERIoT LLVM repository](https://github.com/CHERIoT-Platform/llvm-project/tree/cheriot):

```sh
git clone \
  --branch cheriot \
  --depth 1 \
  --single-branch \
  https://github.com/CHERIoT-Platform/llvm-project.git \
  "$CHERIOT_LLVM"
```

For an existing suitable checkout, set `CHERIOT_LLVM` to its location and skip
the clone. Configure and install the compiler, linker and LLVM utilities:

```sh
cmake \
  -S "$CHERIOT_LLVM/llvm" \
  -B "$CHERIOT_LLVM/build" \
  -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$CHERIOT_TOOLS" \
  -DLLVM_ENABLE_PROJECTS="clang;clang-tools-extra;lld" \
  -DLLVM_ENABLE_UNWIND_TABLES=OFF \
  -DLLVM_TARGETS_TO_BUILD=RISCV \
  -DLLVM_DISTRIBUTION_COMPONENTS="clang;clangd;lld;llvm-objdump;llvm-objcopy"

cmake \
  --build "$CHERIOT_LLVM/build" \
  --target install-distribution \
  --parallel 4
```

LLVM is a substantial build. Reduce `--parallel 4` if memory is limited. Check
the installed executables:

```sh
"$CHERIOT_TOOLS/bin/clang" --version
"$CHERIOT_TOOLS/bin/clang++" --version
"$CHERIOT_TOOLS/bin/ld.lld" --version
"$CHERIOT_TOOLS/bin/llvm-objdump" --version
```

`CHERIOT_TOOLS` is the installation prefix. `"$CHERIOT_TOOLS/bin"` is the
executable directory passed to `--toolchain-dir`; Compose does not append `bin`.
The LLVM source and build directories are separate from this installed
directory. Compose's built-in default remains `/cheriot-tools/bin`.

A version check confirms that an executable can run. The firmware build below
checks that the selected tools support the compilation workflow.
Some board-packaging workflows also need `llvm-strip`; the Sail test build does
not. Install that utility as well if a selected packaging plan requires it.

### Prepare the RTOS Sources

For a new checkout, clone [CHERIoT RTOS](https://github.com/CHERIoT-Platform/cheriot-rtos)
and its submodules:

```sh
git clone \
  --recurse-submodules \
  https://github.com/CHERIoT-Platform/cheriot-rtos.git \
  "$CHERIOT_RTOS"
```

For an existing checkout, set `CHERIOT_RTOS` to that directory and initialize
its submodules without replacing or resetting it:

```sh
git -C "$CHERIOT_RTOS" submodule update --init --recursive
```

No particular checkout name, location, branch or revision is required by this
walkthrough. Compose checks the SDK files and capabilities needed by the
selected project. If it reports a missing dependency, provide the reported
requirement in that checkout.

### Install Rust and Compose

Install host Rust 1.89 or later and Cargo if they are not already available.
The following uses the [official rustup installation method](https://rustup.rs/):

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
  sh -s -- --profile minimal
. "$HOME/.cargo/env"
rustc --version
cargo --version
```

For a new Compose checkout, clone this repository:

```sh
git clone \
  https://github.com/adam-3bian/cheri-make.git \
  "$CHERIOT_COMPOSE"
```

For an existing checkout, set `CHERIOT_COMPOSE` to its location and skip the clone.
Build and install the host application from its product directory:

```sh
cd "$CHERIOT_COMPOSE/cheriot-compose"
cargo install --locked --path .
```

Cargo installs `cheriot-compose` in its binary directory, normally
`$HOME/.cargo/bin`. Make sure that directory is on `PATH`; sourcing
`"$HOME/.cargo/env"` as above sets up the usual rustup installation. Host Rust
builds Compose. Optional Rust firmware needs a separate CHERIoT-capable Rust
compiler; the default RTOS test selection below does not need one.

### Install the RTOS Integration

Copy the **contents** of the integration directory into the RTOS root:

```sh
cp -R \
  "$CHERIOT_COMPOSE/cheriot-compose/integrations/cheriot-rtos/." \
  "$CHERIOT_RTOS/"
```

The trailing `/.` copies the contents while preserving their directory layout.
These TOML files configure the workspace, projects, SDK components and boards.
The installed Compose executable supplies its templates and does not require its
source checkout at runtime. The original `xmake.lua` files remain in the RTOS
checkout; Compose reads the accompanying `cheriot-compose.toml` files.

### Check and Build the Test Firmware

The following commands can run from any working directory because `--file`
selects the RTOS workspace explicitly. The workspace project selector is
`tests`. Check its configuration and required SDK inputs without running the
compiler:

```sh
cheriot-compose check \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project tests
```

Build the Sail release firmware:

```sh
cheriot-compose build \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project tests \
  --board sail \
  --mode release \
  --jobs 4
```

This compiles the RTOS components needed by the selected firmware and links
them with the test application. Normal output shows progress and final paths.
For commands and rebuild reasons, repeat the build with `--verbose`:

```sh
cheriot-compose build \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project tests \
  --board sail \
  --mode release \
  --jobs 4 \
  --verbose
```

A successful default build produces firmware and a compartment report:

```text
$CHERIOT_RTOS/tests/test-suite.elf
$CHERIOT_RTOS/tests/test-suite.json
```

The JSON file is the compartment report. Final files go beside the selected
project manifest, even when the workspace manifest was selected from another
working directory. Intermediates and build state are kept at:

```text
$CHERIOT_RTOS/tests/.cheriot-compose/sail/cheriot/release/
```

The shipped `most_tests` selection excludes optional configurations such as
Rust and bigdata. Building this default selection does not build every possible
test configuration. A successful firmware build completes the walkthrough; it
does not execute the tests.

### Optional: Run the Tests

Building does not require a simulator. To run this Sail configuration, install
`cheriot_sim` separately and make it available on `PATH`, then run:

```sh
cheriot-compose run \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project tests \
  --board sail \
  --mode release
```

`run` builds or reuses the firmware and starts the configured runner. The
compiler directory option does not install or find an unavailable simulator.
Runner arguments after `--` are passed as separate arguments. Other boards may
need a different simulator, hardware or board-specific preparation.

## Project Configuration

A small project beside `hello.cc` can use the RTOS SDK and its component
catalogue. Adjust the relative `sdk`, `board` and `include` paths to match your
checkout. A service is a compartment; its thread is declared separately.
Independent keys are alphabetised in each table, with `name` first; array
entries retain their intended order.

```toml
name = "hello-world"
board = "../../sdk/boards/sail.toml"
include = ["../../sdk/cheriot-compose.toml"]
sdk = "../../sdk"

[services.hello]
dependencies = ["freestanding", "debug"]
kind = "compartment"

[services.hello.build]
sources = ["hello.cc"]

[[firmwares]]
name = "hello_world"
libraries = []
services = ["hello"]

[[firmwares.threads]]
name = "hello"
compartment = "hello"
entry = "say_hello"
priority = 1
stack_size = 512
trusted_stack_frames = 1
```

The entry point must exist in the source file. `cheriot-compose.toml` in the
working directory is used by default. `--file` takes precedence over
`CHERIOT_COMPOSE_FILE`, which takes precedence over that default. Compose does
not search parent directories.

## Includes, Overlays and Environment

Includes combine reusable declarations. Their paths, and paths declared inside
them, are relative to the file containing each declaration. Duplicate definitions
and include cycles are errors. Repeat `--overlay` for deliberate patches, in
order; each overlay can add, replace or remove a field. Board files support
inheritance and ordered board patches too.

For example, this excerpt can be saved as an overlay named `debug.toml`:

```toml
patches = [{ op = "add", path = "/mode", value = "debug" }]
```

Apply it from the RTOS workspace. Use the same destination when running it:

```sh
cheriot-compose build --toolchain-dir "/cheriot-tools/bin" --project hello-world --overlay debug.toml --output-dir ./debug-dist
cheriot-compose run --toolchain-dir "/cheriot-tools/bin" --project hello-world --overlay debug.toml --output-dir ./debug-dist
```

The separate output directory keeps the debug files alongside an existing
release build without replacing it.

The LLVM directory is selected by `--toolchain-dir`, then
`CHERIOT_COMPOSE_TOOLCHAIN_DIR`, then `toolchain_dir` in project TOML, and
finally `/cheriot-tools/bin`. This directory contains `clang`, `clang++`,
`ld.lld` and the LLVM utilities directly; Compose does not add another `bin`.
`CC`, `CXX`, `LD`, `OBJDUMP`, `OBJCOPY` and `STRIP` do not select these tools,
although child programs inherit the environment normally. Explicit component or
source compiler selections remain available in configuration.

`--build-dir` / `CHERIOT_COMPOSE_BUILD_DIR` / `build_dir` select intermediate
storage, and `--output-dir` / `CHERIOT_COMPOSE_OUTPUT_DIR` / `output_dir` select
final output. Command-line settings override environment settings, which
override TOML. Relative command-line and environment paths start at the working
directory; TOML paths start at the file declaring them. Explicit interpolation
such as `toolchain_dir = "${env:TOOLS_DIRECTORY}"` retains that TOML path base
and reports a missing variable. No shell expansion or implicit environment file
is used.

For Rust firmware, set `toolchain.rust_compiler` in project TOML or set `RUSTC`.
`toolchain.cargo` selects host Cargo and `toolchain.uf2_converter` selects the
packaging utility when required.

## Output Files and Other Workflows

By default, final `<firmware>.elf` and `<firmware>.json` files appear beside the
selected project manifest, even when it is selected through a workspace. The
JSON is the compartment report; a firmware can explicitly disable it with
`compartment_report = false`. `--output-dir` names the exact final destination.

Intermediate files and incremental build state are kept under
`<project>/.cheriot-compose/<board>/cheriot/<mode>/` by default. `--build-dir`
changes the intermediate root. Changing only the output destination can reuse
intermediates. Clean removes recorded files owned by the build, not unrelated
files in the output directory.

Normal builds show progress and final paths. `--verbose` adds commands and
rebuild reasons; `--quiet` hides routine progress but keeps errors. JSON build
output is available with `--format json`. A plan is a complete clean build;
`build --dry-run` describes work currently needed. Checks and plans do not run
the compiler unless a supported explicit probe is requested.

Plan formats include `json`, `shell` and `compilation-database`. The shell
export shows the actual generation, compiler and linker steps. An audit needs
an audit tool and policy; packaging needs the selected board's packaging tools.
Compilation database comparison accepts `--against` and repeatable `--map-path`
for relocated paths. Switcher analysis accepts assembly input and does not need
a project. See each command's help for its supported options.

For `cheriot-vm`, install `cheriot-vm` and copy both its RTOS driver integration
and the separate Compose VM integration into the RTOS checkout. Select the
`cheriot-vm` board and a compatible VM profile with `--vm-profile`; firmware,
board devices and profile must agree.

## Troubleshooting

- Missing compiler: point `--toolchain-dir` at the directory containing the
  executables. A plan or check can still run without them.
- Missing SDK file or capability: check the project `sdk` path and initialize
  the relevant RTOS submodules if the selected components need them.
- Ambiguous project or firmware: select one of the listed names with `--project`
  or `--firmware`.
- Unexpected rebuild: use `--verbose` for the reason, or `explain` for an
  operation's inputs and origins.
- Runner failure: check that the board's runner is installed and its profile
  matches the firmware and devices.

## Verification and Limitations

From the Compose product directory, `cargo test --locked --all-targets` runs
host-side Compose tests without an RTOS checkout or CHERIoT compiler. Add
`--features host-tool-verification` for tests that use a POSIX shell and
standard utilities. Repository-reference checks use `rtos-verification`, which
includes the host-tool tests and needs the complete repository. These host tests
check the build tool; they are separate from the RTOS firmware tests above.
The recorded host checks use stable Rust 1.99.1 and the minimum Rust 1.89.0.

The RTOS test suite has been built and run on Sail. Some board and linker
variants, generated-source relationships, component options and physical
hardware remain unverified. Cargo firmware needs an explicit package name and
a `[lib]` with `crate-type = ["staticlib"]`. Its supported `cargo_flags` are
feature selection, `--locked`, `--offline` and `--frozen`; other Cargo options
and workspace layouts need further work. These checks do not establish complete
xmake or RTOS compatibility.

## Licence

The application is MIT licensed; see its local [LICENSE](LICENSE). SDK-derived
templates retain their [SDK licence notice](templates/SDK-LICENSE).
