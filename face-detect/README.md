# face-detect

Face detection using OpenCV's built-in YuNet model (`FaceDetectorYN`). Detects faces in images or webcam feed with bounding boxes, confidence scores, and 5-point facial landmarks (eyes, nose, mouth corners).

## Prerequisites

- **OpenCV 4.7+** (or OpenCV 5.x) installed with development headers
- **libclang** available for the `opencv` crate's build script
- **YuNet ONNX model** — download from [opencv_zoo](https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet):
  ```
  face_detection_yunet_2023mar.onnx
  ```

### Windows (vcpkg)

```powershell
vcpkg install opencv4:x64-windows
# Set environment variables:
$env:OPENCV_LINK_LIBS = "opencv_world4"
$env:OPENCV_LINK_PATHS = "C:\path\to\vcpkg\installed\x64-windows\lib"
$env:OPENCV_INCLUDE_PATHS = "C:\path\to\vcpkg\installed\x64-windows\include"
```

## Usage

```bash
# Detect faces in an image (displays in a window)
cargo run -- --image photo.jpg

# Detect faces and save annotated output
cargo run -- --image photo.jpg --output result.jpg

# Live webcam detection (press Q or ESC to quit)
cargo run

# Custom model path and thresholds
cargo run -- --model path/to/yunet.onnx --score-threshold 0.8
```

## Output format

Each detected face shows:
- **Green bounding box** around the face
- **Confidence percentage** above the box
- **5 coloured landmarks**:
  - 🔵 Right eye (blue)
  - 🔴 Left eye (red)
  - 🟢 Nose tip (green)
  - 🩷 Right mouth corner (pink)
  - 🟡 Left mouth corner (yellow)

## CLI options

| Flag | Description | Default |
|---|---|---|
| `-i, --image <PATH>` | Image file to process | *(webcam)* |
| `-m, --model <PATH>` | YuNet ONNX model file | `face_detection_yunet_2023mar.onnx` |
| `-s, --score-threshold <F32>` | Min confidence to keep a detection | `0.9` |
| `-n, --nms-threshold <F32>` | NMS IoU threshold | `0.3` |
| `-o, --output <PATH>` | Save annotated image (image mode only) | *(display window)* |

## Running Tests

To run the automated benchmarks and unit tests, you must download the necessary test datasets and sample images. You can do this by running the following commands in PowerShell from the project root:

```powershell
# Create the tests directory
New-Item -ItemType Directory -Force -Path tests

# Download the ONNX model (if you haven't already)
Invoke-WebRequest -Uri "https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx" -OutFile "face_detection_yunet_2023mar.onnx"

# Download OpenCV sample test images
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/opencv/opencv/4.x/samples/data/lena.jpg" -OutFile "tests\lena.jpg"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/opencv/opencv/4.x/samples/data/messi5.jpg" -OutFile "tests\messi5.jpg"
Invoke-WebRequest -Uri "https://raw.githubusercontent.com/opencv/opencv/4.x/samples/data/opencv-logo-white.png" -OutFile "tests\no_face.png"

# Download and extract the Labeled Faces in the Wild (LFW) dataset for the hit-rate benchmark (170MB)
# (Using curl for faster download with a progress bar)
curl.exe -L https://ndownloader.figshare.com/files/5976015 -o tests\lfw.tgz
tar -xzf tests\lfw.tgz -C tests\

# Download and extract the Stanford Background dataset for the false-positive benchmark (15MB)
Invoke-WebRequest -Uri "http://dags.stanford.edu/data/iccv09Data.tar.gz" -OutFile "tests\stanford_bg.tar.gz"
tar -xzf tests\stanford_bg.tar.gz -C tests\
```

Then you can run the test suite:
```bash
cargo test
```
