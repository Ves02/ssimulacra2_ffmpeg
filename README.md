This is a proof of concept to use ffmpeg for ssimulacra2. 

Why?
1) Support wider range of pixel formats and codecs
2) Allow comparision between 2 file with differing pixel format like YUV, RGB, BGR, GBR, etc...
3) Allow comparision between video and image sequences more seamlessly, you can still compare a single image
4) Codecs like ffv1, exr, etc.. may have unusual pixel format that does not get correctly processed and result in crash or inaccurate score.

Requirement:
Install ffmpeg binary which should contain ffmpeg.exe and ffprobe.exe in a bin folder, add those bin folder to path

Warning:
This is a proof of concept, it's not optimized and it will load a whole file into memory uncompressed and at 12 bytes per pixel, so your memory will fill very fast. Try to limit video or image sequences to just 30 frames or less each, or use test_media folder.

There may be logic error, please submit an issue if you find one.

It will run single thread, so it will be very slow.
Score from ssimulacra2_rs differ from ssimulacra2_ffmpeg. Potentially due to colorspace problem. Need to somehow handle unknown colorspace.

Run the command:

cargo run -r -- test_media/01_crf10.mp4 test_media/01_crf11.mp4

cargo run -r png_seq/%03d.png 01_crf10.mp4

This grab all png file named 001.png, 002.png, 003.png, in png_seq folder. 

Example: hi%04d.png will result in hi0001.png, hi0002.png

