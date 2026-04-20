# llm-gguf-parser

A minimal, fast, and educational GGUF model parser written in pure Rust.

## Design Decisions: Why is it implemented this way?

### 1. Memory Mapping (`memmap2`)
Large Language Models (LLMs) range in size from hundreds of megabytes to hundreds of gigabytes. Reading the entire file into memory using `std::fs::read` would cause Out-Of-Memory (OOM) crashes on typical machines. 
Instead, we use **Memory Mapping** (`memmap2` crate) to map the file into the virtual address space of our process:
- **Zero-Copy Reading**: We can slice into the file buffer directly without allocating memory for tensors.
- **On-Demand Loading**: The OS virtual memory manager loads only the disk pages containing the header and metadata. The actual model weights (which make up 99.9% of the file) are never read into RAM.

### 2. Standard Rust Byte Conversions (No Parser Framework)
Instead of using heavy parsing frameworks like `nom` or `serde`, we implement a simple, sequential `Reader` over a byte slice. This provides:
- **Maximum Clarity**: Extremely easy to read and trace how bytes map directly to specifications.
- **Speed & Control**: No compile-time overhead or complex parsing combinators. Bytes are cast to Rust primitives using standard library byte functions (`u32::from_le_bytes`, `u64::from_le_bytes`) which compile directly to hardware-level operations.

### 3. GGUF Version Compatibility
The GGUF specification has evolved over three versions:
- **Version 1**: Designed for 32-bit systems, counts of metadata pairs and tensors are stored as `u32`.
- **Version 2 & 3**: Standardized on 64-bit counts (`u64`) to accommodate massive models with thousands of tensors.

Our parser reads the version number first and adapts its read size dynamically, making it robust against legacy and modern GGUF models.

---

## GGUF File Layout Reference

A GGUF file is serialized in little-endian byte order:

| Section | Size | Description |
|---|---|---|
| **Magic Bytes** | 4 bytes | Always ASCII `'G' 'G' 'U' 'F'` (`0x46554747`) |
| **Version** | 4 bytes | `u32` (1, 2, or 3) |
| **Tensor Count** | 4 or 8 bytes | `u32` (v1) or `u64` (v2/v3) |
| **Metadata KV Count** | 4 or 8 bytes | `u32` (v1) or `u64` (v2/v3) |
| **Metadata Key-Values** | Variable | Sequence of `(string, value_type, value)` |
| **Tensor Metadata** | Variable | Sequence of `(name, dimensions, type, offset)` |
| **Alignment Padding** | Variable | Padding to align the tensor data block boundary |
| **Tensor Data** | Variable | Raw binary tensor weights |

---

## Usage

### Run Unit Tests
```bash
cargo test
```

### Parse a GGUF File
To print the header info and a summary of the metadata and first 5 tensors:
```bash
cargo run -- path/to/model.gguf
```

### View All Tensors
To view details of all tensors, including their shapes and FFI file offsets:
```bash
cargo run -- path/to/model.gguf --tensors
```

### Filter Metadata
Search for specific metadata configuration keys (e.g. context length, tokenizers):
```bash
cargo run -- path/to/model.gguf --query context
```
