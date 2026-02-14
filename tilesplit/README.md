# TileSplit

A Rust CLI tool that splits JPEG images into two tiles while preserving Ultra HDR (Gain Map) metadata.

## Overview

TileSplit is designed for splitting panoramic or dual-pane images while maintaining HDR information. When processing Ultra HDR JPEGs, it extracts and correctly maps the Gain Map data to ensure the output tiles remain valid Ultra HDR files viewable on HDR displays.

## Features

- **Ultra HDR Preservation**: Splits images while maintaining HDR gain map metadata
- **Smart Fallback**: Falls back to standard image splitting for non-HDR images
- **Aspect Ratio Support**: Handles 16:10 and 3:2 aspect ratios
- **Dual Extraction Path**: Uses jpegli-rs for primary detection, ultrahdr-rs as fallback

## Installation

```bash
cargo build --release
```

## Usage

```bash
# Basic usage - generates input-left.jpg and input-right.jpg
tilesplit --input photo.jpg

# Specify output paths
tilesplit --input photo.jpg --left-output left.jpg --right-output right.jpg

# Enable debug output
tilesplit --input photo.jpg --debug

# Or via environment variable
TILESPLIT_DEBUG=1 tilesplit --input photo.jpg
```

## Exit Codes

- `0`: Success
- `2`: Usage error
- `3`: Invalid input
- `4`: Invalid crop region
- `10`: Unsupported aspect ratio
- `11`: I/O error

## Dependencies

- [image](https://crates.io/crates/image) - Image processing
- [jpegli-rs](https://crates.io/crates/jpegli-rs) - JPEG encoding/decoding with extras support
- [ultrahdr-rs](https://crates.io/crates/ultrahdr-rs) - Ultra HDR metadata handling
