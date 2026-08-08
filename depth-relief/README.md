# 3D-Printable Depth Relief

A Rust command-line tool that estimates a depth map from a photo (using a MiDaS ONNX deep learning model) or a stereo pair, and triangulates it into a watertight, solid 3D STL mesh. The output STL is ready to be sliced and 3D-printed as a physical bas-relief of your photo.

This project connects computer vision (depth estimation & stereo block matching) to 3D design and printing.

## Features

- **Single Photo Mode**: Estimates depth from a single image using the MiDaS v2.1 Small model via the OpenCV DNN module. The model is automatically downloaded from the official repository if not present locally.
- **Stereo Pair Mode**: Computes depth (disparity map) from a rectified left and right stereo image pair using OpenCV's Stereo Block Matching (`StereoBM`).
- **Watertight STL Generation**: Triangulates the 2D heightfield into a closed, manifold 3D STL mesh by generating a solid base, side walls, and top surface with correct face normals (proper winding orders).
- **Interactive Proportional Scaling**: Automatically calculates the physical height of the relief to match the aspect ratio of the input image if only the target width is specified.

---

## Requirements

1. **OpenCV 5.x** installed on your system with development headers.
2. **libclang** (required by the Rust `opencv` bindings).
3. **Rust Toolchain** (cargo, rustc).

---

## Build Instructions

To build the project on Windows:

```bash
# Run the build script which sets up paths to the local OpenCV 5 installation
build.bat build --release
```

---

## How to Run

### 1. Single Photo Mode (MiDaS ONNX)

To generate a depth relief from a single photo:

```bash
build.bat run --release -- --mode single --input path/to/photo.jpg --output-stl relief.stl --width 100
```

- If `model-small.onnx` is missing in the project folder, it will be automatically downloaded for you from the official Intel ISL GitHub release page.
- `--width 100` specifies the physical print width in millimeters (100 mm). The height is automatically scaled proportionally.
- You can adjust the relief amplitude with `--relief-height <mm>` (default: 8.0) and the base thickness with `--base-height <mm>` (default: 2.0).

### 2. Stereo Photo Mode (StereoBM)

To generate a depth relief from a left-right stereo image pair:

```bash
build.bat run --release -- --mode stereo --left path/to/left.jpg --right path/to/right.jpg --output-stl relief.stl --width 100
```

  - Optional facial feature enhancement & post-processing arguments:
  - `--detail-weight <0.0..1.0>`: High-pass photo detail blending weight (default: 0.4). Engraves sharp eyes, nostrils, lips, and hair strands from photo shading onto the 3D relief.
  - `--gamma <num>`: Contrast curve exponent (< 1.0 expands facial feature midtones, default: 0.7).
  - `--clahe`: Enable Contrast Limited Adaptive Histogram Equalization for sharp local facial contours.
  - `--relief-height <mm>`: Relief amplitude / max depth displacement in mm (e.g. 14.0 for deep facial reliefs).
  - `--clip-min <0.0..0.49>`: Lower quantile clip bound to eliminate background compression (default: 0.02).
  - `--clip-max <0.51..1.0>`: Upper quantile clip bound to eliminate peak compression (default: 0.98).
  - `--smooth-radius <num>`: Edge-preserving bilateral filter radius (default: 3).

---

## Command Line Reference

```
Usage: depth-relief.exe [OPTIONS]

Options:
  -m, --mode <MODE>                      Mode of operation: single or stereo [default: single] [possible values: single, stereo]
  -i, --input <INPUT>                    Path to input image (required for single image mode)
      --left <LEFT>                      Path to left image (required for stereo mode)
      --right <RIGHT>                    Path to right image (required for stereo mode)
  -M, --model <MODEL>                    Path to the MiDaS ONNX model file [default: model-small.onnx]
  -o, --output-stl <OUTPUT_STL>          Output STL file path [default: relief.stl]
  -d, --output-depth <OUTPUT_DEPTH>      Output depth map image path [default: depth.png]
      --width <WIDTH>                    Physical width of the 3D relief in mm [default: 100.0]
      --height <HEIGHT>                  Physical height of the 3D relief in mm (0.0 calculates it proportionally) [default: 0.0]
  -z, --relief-height <RELIEF_HEIGHT>    Relief amplitude (max depth displacement) in mm [default: 8.0]
  -b, --base-height <BASE_HEIGHT>        Flat base height in mm [default: 2.0]
      --stereo-disparities <DISPARITIES> Number of disparities for stereo matching [default: 64]
      --stereo-block-size <BLOCK_SIZE>   Block size for stereo matching [default: 15]
      --gamma <GAMMA>                    Gamma contrast exponent (< 1.0 expands midtones) [default: 0.7]
      --detail-weight <WEIGHT>           High-pass photo detail weight (0.0 to 1.0) [default: 0.4]
      --clip-min <CLIP_MIN>              Lower quantile clipping bound [default: 0.02]
      --clip-max <CLIP_MAX>              Upper quantile clipping bound [default: 0.98]
      --clahe                            Apply CLAHE for enhanced facial contours
      --smooth-radius <RADIUS>           Edge-preserving bilateral smoothing radius [default: 3]
  -h, --help                             Print help
```

---

## 3D Printing & Slicing Recommendations

1. **Orientation**: Import the STL file into your slicer (e.g., PrusaSlicer, Bambu Studio, Cura). It will sit flat on the build plate (Z = 0).
2. **Infill**: 10% to 15% infill is sufficient since the relief is solid. Use **Gyroid** or **Grid** infill pattern for structural strength.
3. **Layers**:
   - For high-fidelity reliefs, use a smaller layer height (e.g., **0.12 mm** or **0.16 mm**).
   - Consider enabling **Variable Layer Height** in your slicer to print the flat base quickly and print the top relief surface with ultra-fine layers.
4. **Filament**: Grayscale or single-color matte PLA works best. Translucent filament can create a lithophane effect.

---

## OpenSCAD Integration

You can easily import the generated STL into OpenSCAD to add mounting holes, frames, text, or combine it with other mechanical components:

```openscad
// OpenSCAD Script to frame and mount the depth relief
difference() {
    // 1. Outer Frame / Mounting Block
    union() {
        // Base plate frame
        cube([110, 110, 3], center = true);
        
        // Import and position the relief mesh
        // Shift it to sit in the center of the frame
        translate([-50, -50, 1.5]) 
            import("relief.stl");
    }

    // 2. Mounting screw hole (e.g., for wall hanging)
    translate([0, 50, 0])
        cylinder(h = 20, r = 2.5, center = true, $fn = 32);
}
```
