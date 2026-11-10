# CHERIoT Compose

## Command Line Arguments

`cheriot-compose` builds CHERIoT firmware from TOML project files. It validates
configuration, shows build commands, compiles firmware and can launch the result
on a configured simulator or board.

Put the command before its options. Use `--help` or `-h` on its own for general
help, or after a command for that command's help:

```sh
cheriot-compose --help
cheriot-compose build --help
cheriot-compose run -h
```

The general help shows the available commands and common options:

```text
Usage: cheriot-compose COMMAND [OPTION]...

Inspect and build CHERIoT firmware from TOML configuration.

Commands:
  analyze-switcher --input FILE
    Check annotated switcher assembly and produce a DOT graph.

  audit --query QUERY
    Build firmware and run an independent audit.

  build
    Build the selected firmware.

  check
    Validate configuration without running compiler tools.

  clean
    Remove files recorded as belonging to the selected build.

  compare-compilation-db
    Compare compiler commands against a reference database.

  configuration
    Show the resolved configuration.

  explain OPERATION-ID
    Show a build operation and the origins of its inputs.

  package
    Build firmware and create its board-specific package.

  plan
    Show the complete clean-build plan.

  run
    Build as needed, then launch the configured runner.

  size [--compartment NAME]
    Report firmware or compartment section sizes.

Options follow the command. Available options and formats vary by command.
Use cheriot-compose COMMAND --help for its complete usage and restrictions.
Compiler probes remain unresolved until a build or an explicit probe.

Common options:
  --board BOARD
    Select a board name or TOML file.

  --build-dir DIR
    Select the root directory for intermediate build files.

  --dry-run
    Describe the work needed without executing it.

  -f, --file FILE
    Select a project or workspace manifest.

  --firmware NAME
    Select a firmware image.

  --format FORMAT
    Select an output format supported by the command.

  --jobs NUMBER
    Limit the number of parallel build jobs.

  --mode debug|release
    Select the build mode.

  --output FILE
    Write the command's text or structured output to a file.

  --output-dir DIR
    Select the destination for final firmware files.

  --overlay FILE
    Apply a project overlay; repeat to apply overlays in order.

  --project NAME
    Select a project from a workspace.

  --quiet
    Hide routine build progress.

  --toolchain-dir DIR
    Select the directory containing LLVM executables.

  --verbose
    Show commands and reasons for rebuilding.

  -h, --help
    Display this help and exit.
```

A command's help lists only the options it accepts, its output formats and any
restrictions. Help works without a project, compiler or simulator.

### Common Arguments

`--file FILE` selects a project or workspace manifest. Within a workspace,
`--project NAME` selects a project; `--firmware NAME` selects an image when a
project defines several. Without `--file` or an environment override, Compose
reads `cheriot-compose.toml` in the working directory. It does not search parent
directories.

`--board BOARD` selects a board name or TOML file. `--mode debug|release`
selects the build mode; release is the default. Repeat `--overlay FILE` to
apply configuration patches in the given order.

`--toolchain-dir DIR` selects the directory containing CHERIoT LLVM
executables. For the installation below, use `"$CHERIOT_TOOLS/bin"`.
Compose does not append `bin` or substitute a host LLVM installation from
`PATH`. The built-in default is `/cheriot-tools/bin`.

Build-related commands accept `--jobs NUMBER` to limit parallel work; the
default is one job. `--verbose` shows commands and rebuild reasons,
`--quiet` hides routine progress, and `--dry-run` describes needed work
without executing it. Verbose and quiet cannot be combined.

`--output-dir DIR` selects the final firmware destination, while
`--build-dir DIR` selects intermediate storage. `--output FILE` instead
writes a command's textual or structured output. For `build` and `package`,
`--output` requires `--format json`; JSON build output cannot be combined
with `--verbose` or `--quiet`.

### Common Commands

After completing the Build section, these commands use the prepared RTOS
workspace and the home-directory compiler installation:

```sh
cd "$CHERIOT_RTOS"

# Validate a project without running the compiler.
cheriot-compose check --project hello-world

# Inspect the commands a complete build would require.
cheriot-compose plan --project hello-world \
  --toolchain-dir "$CHERIOT_TOOLS/bin"

# Build the example for Sail.
cheriot-compose build --project hello-world \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --board sail --mode release --jobs 1

# Build the usual RTOS test selection.
cheriot-compose build --project tests \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --board sail --mode release --jobs 1
```

Use `--file "$CHERIOT_RTOS/cheriot-compose.toml"` to select the same workspace
from another directory. A successful check establishes that the selected
configuration and required inputs are valid; a firmware build checks the
compiler workflow, and a run checks execution on the selected target.

## Build

This README assumes the repository root is $HOME/cheriot-compose, with
Cargo.toml, integrations and templates directly inside it. Place this checkout
there before running the commands.

Use this fresh-install recipe on Debian or Ubuntu with the default
[Rustup](https://rustup.rs/) and Cargo locations. It needs sudo, network access,
CMake 3.20 or newer, Python 3.8 or newer, and LLD 11 or newer. Building LLVM
takes substantial time and disk space.

Skip installation or clone commands for components you already have.
Initialise submodules in an existing RTOS checkout before continuing.
The integration copy replaces matching files in that checkout.

The block installs Compose, builds the CHERIoT compiler, adds the project
manifests to RTOS, and builds hello-world for the Sail board.

```bash
set -euo pipefail

export CHERIOT_COMPOSE="$HOME/cheriot-compose"
export CHERIOT_LLVM="$HOME/cheriot-llvm"
export CHERIOT_RTOS="$HOME/cheriot-rtos"
export CHERIOT_TOOLS="$HOME/cheriot-tools"

# Install host build tools.
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential ca-certificates clang cmake curl git lld python3

# Clone the RTOS and its submodules.
git clone --recurse-submodules \
  https://github.com/CHERIoT-Platform/cheriot-rtos.git "$CHERIOT_RTOS"

# Clone the LLVM sources needed for the low-memory build.
git -c checkout.workers=1 -c index.threads=1 -c pack.threads=1 \
  clone --branch cheriot --depth 1 --filter=blob:none --single-branch --sparse \
  https://github.com/CHERIoT-Platform/llvm-project.git "$CHERIOT_LLVM"

git -C "$CHERIOT_LLVM" \
  -c checkout.workers=1 -c index.threads=1 -c pack.threads=1 \
  sparse-checkout set --cone clang cmake libc libunwind lld llvm third-party

# Install Rust and load its environment.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
  sh -s -- -y --profile minimal
. "$HOME/.cargo/env"

# Build and install Compose.
cd "$CHERIOT_COMPOSE"
cargo install --locked --path . --jobs 1

# Build and install the CHERIoT compiler.
cmake -S "$CHERIOT_LLVM/llvm" \
  -B "$CHERIOT_LLVM/build-lowmem" \
  -G "Unix Makefiles" \
  -DCLANG_ENABLE_CIR=OFF \
  -DCLANG_ENABLE_OBJC_REWRITER=OFF \
  -DCLANG_ENABLE_STATIC_ANALYZER=OFF \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_CXX_COMPILER=/usr/bin/clang++ \
  -DCMAKE_CXX_FLAGS="" \
  -DCMAKE_CXX_FLAGS_RELEASE="-O1 -DNDEBUG" \
  -DCMAKE_C_COMPILER=/usr/bin/clang \
  -DCMAKE_C_FLAGS="" \
  -DCMAKE_C_FLAGS_RELEASE="-O1 -DNDEBUG" \
  -DCMAKE_EXE_LINKER_FLAGS="-fuse-ld=lld -Wl,--threads=1" \
  -DCMAKE_INSTALL_PREFIX="$CHERIOT_TOOLS" \
  -DCMAKE_MODULE_LINKER_FLAGS="-fuse-ld=lld -Wl,--threads=1" \
  -DCMAKE_SHARED_LINKER_FLAGS="-fuse-ld=lld -Wl,--threads=1" \
  -DLLVM_DISTRIBUTION_COMPONENTS="clang;clang-resource-headers;lld;llvm-objdump;llvm-objcopy;llvm-strip" \
  -DLLVM_ENABLE_ASSERTIONS=OFF \
  -DLLVM_ENABLE_LTO=OFF \
  -DLLVM_ENABLE_PROJECTS="clang;lld" \
  -DLLVM_ENABLE_RUNTIMES="" \
  -DLLVM_ENABLE_THREADS=OFF \
  -DLLVM_ENABLE_UNWIND_TABLES=OFF \
  -DLLVM_INCLUDE_BENCHMARKS=OFF \
  -DLLVM_INCLUDE_DOCS=OFF \
  -DLLVM_INCLUDE_EXAMPLES=OFF \
  -DLLVM_INCLUDE_RUNTIMES=OFF \
  -DLLVM_INCLUDE_TESTS=OFF \
  -DLLVM_TARGETS_TO_BUILD=RISCV \
  -DLLVM_USE_LINKER=lld

cmake --build "$CHERIOT_LLVM/build-lowmem" \
  --parallel 1 --target install-distribution

"$CHERIOT_TOOLS/bin/clang" --version

# Add the Compose manifests to the RTOS checkout.
cp -a "$CHERIOT_COMPOSE/integrations/cheriot-rtos/." "$CHERIOT_RTOS/"

# Build hello-world for the Sail board.
cd "$CHERIOT_RTOS"
cheriot-compose build \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project hello-world \
  --board sail \
  --mode release \
  --jobs 1
```

The LLVM sources and build directory are separate from the installed tools.
The build uses one compilation job and one host-linker thread. Disabling LLVM
threading affects the generated tools separately. Keep libc, libunwind and
third-party in the sparse checkout: the build needs their headers.

Cargo uses the Rust toolchain selected by this repository. The installed
application is available through Cargo's bin directory.

The build writes hello_world.elf and hello_world.json under
$CHERIOT_RTOS/examples/01.hello_world. To execute that firmware, install the
Sail runner as described under Runners and the VM below. Compose itself can be installed
without building LLVM by running the Cargo installation command in the
repository root.

## Features

### TOML Projects and Firmware

A project describes its SDK, board, components and firmware images. A service
is a compartment; its threads are declared separately within a firmware.
Libraries and component dependencies provide reusable code.

For example, this manifest can sit beside `hello.cc` in
`$CHERIOT_RTOS/examples/01.hello_world/`, after the Compose integration has
been copied into the RTOS checkout:

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

The `say_hello` entry point must exist in the compartment's source. Adjust
SDK, board and include paths when placing the project elsewhere. Paths are
resolved from the file declaring them, so moving the manifest can change their
meaning.

Keep independent keys alphabetised within each table, with `name` first;
preserve the intended order of array entries.

A workspace lists named project manifests under `[workspace.projects]`.
The RTOS integration supplies projects including `hello-world`,
`producer-consumer` and `tests`. Selecting a workspace project does not
change that project's source directory or default output location.

The format has no schema declaration. Unknown fields, duplicate definitions,
missing component dependencies and invalid references are reported as errors.
Capability checks also identify SDK files required by the selected components;
they do not establish compatibility with every possible SDK revision or board.

### Includes, Overlays and Boards

Use `include` to combine reusable declarations. Included files retain their
own path bases, and include cycles are errors. Use overlays for deliberate
changes to existing configuration rather than duplicating an entire project.

An overlay contains ordered patches. Each patch uses `add`, `replace` or
`remove` and a JSON-pointer path; replacing or removing a missing field is
an error. For example, save this as `debug.toml` in the RTOS workspace:

```toml
patches = [{ op = "add", path = "/mode", value = "debug" }]
```

Apply the overlay and choose a separate destination:

```sh
cd "$CHERIOT_RTOS"
cheriot-compose build --project hello-world \
  --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --overlay debug.toml --output-dir ./debug-dist
```

Use the same overlay and destination when running that build. Multiple
`--overlay` arguments are applied in order. Board files can inherit another
board with `extends` and apply ordered patches; projects can supply
`board_overlays` for board-specific changes.

Named boards come from the selected SDK's `boards/` directory. A board
defines the firmware's memory, devices, clock rates, driver includes and
optional runner or packaging settings. Choose a board that matches the target
on which the firmware will execute.

### Environment and Tool Selection

Command-line choices take precedence over environment settings, which take
precedence over project TOML. The supported overrides include
`CHERIOT_COMPOSE_FILE`, `CHERIOT_COMPOSE_BOARD`,
`CHERIOT_COMPOSE_MODE`, `CHERIOT_COMPOSE_TOOLCHAIN_DIR`,
`CHERIOT_COMPOSE_BUILD_DIR`, `CHERIOT_COMPOSE_OUTPUT_DIR` and
`CHERIOT_COMPOSE_JOBS`.

Relative command-line and direct environment paths start at the invocation
directory. TOML paths start at their declaring file. Explicit TOML
interpolation, such as `toolchain_dir = "${env:CHERIOT_TOOLS}/bin"`,
is supported and reports a missing variable. Interpolation retains the TOML
path base if its result is relative. There is no implicit environment file or
shell expansion.

The LLVM selection order is `--toolchain-dir`,
`CHERIOT_COMPOSE_TOOLCHAIN_DIR`, project `toolchain_dir`, then
`/cheriot-tools/bin`. The selected directory supplies `clang`, `clang++`,
`ld.lld`, `llvm-objdump`, `llvm-objcopy` and `llvm-strip`; execution
checks each tool when an action needs it.

Generic `CC`, `CXX`, `LD`, `OBJDUMP`, `OBJCOPY` and `STRIP`
do not select that suite, although child processes inherit the environment.
Explicit compiler choices for individual components and sources remain
available.

### Outputs and Incremental Builds

By default, final `<firmware>.elf` and `<firmware>.json` files appear beside
the selected project manifest, including when the project is selected through
a workspace. The JSON file is a compartment report. A firmware can disable it
with `compartment_report = false`.

The hello-world project produces `hello_world.elf` and `hello_world.json`
under `$CHERIOT_RTOS/examples/01.hello_world/`. The RTOS test project produces
`test-suite.elf` and `test-suite.json` under `$CHERIOT_RTOS/tests/`.

Intermediate files and incremental state are stored under
`<project>/.cheriot-compose/<board>/cheriot/<mode>/`. Moving only the final
output destination can reuse existing intermediates. Repeated builds reuse
up-to-date work; `--verbose` explains why an operation runs again.
`clean` removes recorded build files while preserving unrelated files.

A plan describes a complete clean build. By contrast, `build --dry-run`
describes the work needed in the current state. Checks and ordinary plans do
not run the compiler. Compiler-dependent probes remain unresolved until a
build or an explicit `--probe-toolchain` request on a supported command.

### Runners and the VM

`run` performs the necessary build and then invokes the selected board's
runner, even when the firmware was already up to date. The runner executable
must be installed and discoverable through its configured path or `PATH`.
A successful build alone does not mean the firmware has been executed.

For Sail, install `cheriot_sim` separately and make it available on `PATH`:

```sh
cheriot-compose run \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project tests --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --board sail --mode release --jobs 1
```

The usual test selection excludes optional cases such as Rust firmware and
bigdata. Sail's runner checks for assertion failures in simulator output,
including when the simulator itself returns success.

For CHERIoT VM, install `cheriot-vm` separately. With its source checkout
at `$HOME/cheriot-vm`, add the driver and Compose board overlays after the
main RTOS integration from the Build section:

```sh
cp -a "$HOME/cheriot-vm/integrations/cheriot-rtos/." "$CHERIOT_RTOS/"
cp -a "$CHERIOT_COMPOSE/integrations/cheriot-vm/." "$CHERIOT_RTOS/"

cheriot-compose run \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project hello-world --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --board cheriot-vm --mode release --jobs 1 \
  --vm-profile "$HOME/cheriot-vm/profiles/cheriot.toml"
```

The board determines firmware memory and device settings. The separate VM
profile determines runtime settings; the two must describe compatible devices
and timing. `--vm-profile` is an argument of `run`, not `build` or
`check`. Additional runner arguments follow `--`.

### Editor Support and Other Tools

A plan can export `compile_commands.json` for editor tools. With the compiler
installed, probe its supported options while generating the database:

```sh
cheriot-compose plan \
  --file "$CHERIOT_RTOS/cheriot-compose.toml" \
  --project hello-world --toolchain-dir "$CHERIOT_TOOLS/bin" \
  --board sail --probe-toolchain --format compilation-database \
  --output "$CHERIOT_RTOS/compile_commands.json"
```

The minimal compiler build does not install `clangd`. Install a compatible
CHERIoT clangd if your editor needs to understand CHERIoT-specific options; an
ordinary host clangd may reject them.

The JSON plan and shell export describe the same underlying build actions.
`compare-compilation-db` accepts `--against FILE` and repeatable
`--map-path OLD=NEW` arguments to compare builds from different source
locations. `--generated FILE` compares an existing database without selecting
a project.

`size` reports section sizes for firmware or a selected compartment.
`package` requires a supported board packaging configuration and its tools;
some workflows need an additional UF2 converter selected by
`toolchain.uf2_converter`. `audit` needs an audit executable, query and any
required modules. `analyze-switcher --input FILE` works with annotated
assembly and does not require a project. Use command-specific help before
these specialized workflows.

### Rust Firmware and Verification

Host Rust builds Compose itself. Firmware written in Rust needs a separate
CHERIoT-capable compiler, selected through `toolchain.rust_compiler` in TOML
or `RUSTC`. `toolchain.cargo` selects the host Cargo executable.

Cargo firmware must specify a package name and a library with
`crate-type = ["staticlib"]`. Supported Cargo flags cover feature selection,
`--locked`, `--offline` and `--frozen`; other flags and workspace layouts
need further work. The ordinary C/C++ hello-world and default RTOS test
selection do not require a firmware Rust compiler.

From the Compose checkout, run the host tests with:

```sh
cd "$CHERIOT_COMPOSE"
cargo test --locked --all-targets
cargo test --locked --all-targets --features host-tool-verification
```

Default tests use product-local fixtures and need neither an external RTOS
checkout nor a CHERIoT compiler. The optional `host-tool-verification` tier
uses a POSIX shell and standard utilities. `rtos-verification` includes
those checks and additionally needs the RTOS sources, reference data and
verification documentation used by that tier; a standalone checkout does not
supply all those assets.

These host tests check the build tool, separately from compiling and executing
RTOS firmware. Recorded host verification used Rust 1.99.1 and the minimum
Rust 1.89.0, and the RTOS test suite has been built and run on Sail. A fresh
LLVM source build remains unverified for these instructions. Some boards,
linker variants, generated-source relationships, component options and physical
hardware remain unverified; the recorded checks do not establish complete
RTOS compatibility.

### Troubleshooting

For a missing compiler, check that `--toolchain-dir` names the executable
directory. For missing SDK inputs, check the project's `sdk` path and
initialize the required RTOS submodules. If project or firmware selection is
ambiguous, add `--project` or `--firmware`. Investigate unexpected rebuilds
with `--verbose` and `explain`. For runner failures, confirm that the runner
is installed and that the selected board and runtime profile agree.

## Licence

The application is MIT licensed; its licence text is in `LICENSE`.
SDK-derived templates retain the notice in `templates/SDK-LICENSE`.

