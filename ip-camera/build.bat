@echo off
set OPENCV_LINK_PATHS=D:\code\opencv\build\install\x64\vc17\lib
set OPENCV_INCLUDE_PATHS=D:\code\opencv\build\install\include
set OPENCV_LINK_LIBS=opencv_calib510,opencv_core510,opencv_dnn510,opencv_features510,opencv_flann510,opencv_geometry510,opencv_highgui510,opencv_imgcodecs510,opencv_imgproc510,opencv_objdetect510,opencv_photo510,opencv_ptcloud510,opencv_stereo510,opencv_stitching510,opencv_video510,opencv_videoio510
set PATH=%PATH%;D:\code\opencv\build\install\x64\vc17\bin
cargo %*
