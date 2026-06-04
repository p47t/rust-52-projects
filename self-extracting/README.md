# self-extracting

A generic Rust tool that turns **any folder** into a self-extracting
executable — no rebuild required. The same binary acts both as the
**packer** (when run without an embedded payload) and as the
**extractor** (when run with a payload appended to itself).

## How it works

This is the classic *stub + appended archive* trick used by tools like
`makeself`. The on-disk layout of a packed binary is:

```text
[ stub executable ][ tar.gz payload ][ footer (24 bytes) ]
                                       └── magic "SFXR0001" | u64 offset | u64 length
```

- **Pack**: copy the running binary's stub portion into the output
  file, stream a `tar.gz` of the source folder onto the end, then
  append a 24-byte footer recording where the payload starts and how
  long it is. The output gets `+x` on Unix.
- **Extract**: at startup the binary `seek`s its own file (via
  `std::env::current_exe`), reads the last 24 bytes, and if the magic
  matches, slices out `[offset .. offset+length]` and pipes it through
  `GzDecoder` + `tar::Archive`.
- **Re-pack from a packed stub** still works: `stub_size()` strips any
  existing payload before copying, so payloads don't compound.

No `build.rs`, no `include_bytes!` — the payload is decoupled from the
Rust build, so one compiled binary can produce arbitrarily many
self-extracting archives.

## Usage

Build the tool once:

```bash
cargo build --release
alias sfxr=$PWD/target/release/self-extracting
```

Pack any folder into a new self-extracting executable:

```bash
sfxr pack --source ./my-folder --output ./my-folder.run
```

Run the produced binary on any compatible host — no `sfxr` needed:

```bash
./my-folder.run                       # extract to ./extracted
./my-folder.run --dest /opt/app       # extract to a chosen dir
./my-folder.run --dest /opt/app -f    # overwrite existing files
./my-folder.run --info                # offset, sizes, sha256
./my-folder.run --list                # entries inside the archive
```

A plain (unpacked) stub prints a helpful message and exits 2, so it's
obvious when the binary has not been packed yet.

## Cross-platform notes

- **Format portability.** The footer uses little-endian `u64`s and the
  payload is plain `tar.gz`, so a packed binary is portable across
  architectures *as long as the stub itself is*. To produce a Linux
  `.run` from macOS, cross-compile the stub
  (`cargo build --release --target x86_64-unknown-linux-gnu`) and pass
  it via `pack --stub <path>`.
- **Linux (ELF).** Works as-is. Run `strip` on the stub *before*
  packing — `strip` may rewrite the file and would clobber appended
  bytes.
- **macOS (Mach-O).** Works for ad-hoc / unsigned binaries (the
  default `cargo build`). For notarized / hardened-runtime release
  builds, sign the resulting `.run` *after* packing
  (`codesign --sign - <output>` for ad-hoc) — the signature must cover
  the appended bytes.
- **Windows (PE).** Works for unauthenticode-signed binaries. The
  packer auto-appends `.exe` to the output if it has no extension.
  Authenticode signatures cover the whole PE file including the
  payload, so sign *after* packing if needed.
- **Permissions.** File permissions inside the archive are preserved
  by `tar`. The packer marks the output `+x` on Unix and produces a
  `.exe` on Windows.
- **Symlinks.** Resolved to their targets at pack time, so extraction
  doesn't require admin or developer mode on Windows.

## Safety notes

- Extraction sanitizes archive paths: any entry containing `..`, an
  absolute root, or a Windows path prefix is rejected before any file
  is created (basic Zip-Slip defense).
- Existing destination files are **not** overwritten unless `--force`
  is passed.
- The footer carries an 8-byte magic so a random binary that happens
  to be the right length won't be misidentified as a packed stub.

## Try it

```bash
cargo build --release
mkdir -p demo && echo hi > demo/hello.txt
./target/release/self-extracting pack -s demo -o demo.run
./demo.run --list
./demo.run --dest /tmp/demo-out
cat /tmp/demo-out/hello.txt
```
