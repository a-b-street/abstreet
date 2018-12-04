# Videos

ffmpeg -f x11grab -r 25 -s 1920x1080 -i :0.0 -vcodec huffyuv raw.avi

ffmpeg -ss 10.0 -t 5.0 -i raw.avi -f gif -filter_complex "[0:v] fps=12,scale=1024:-1,split [a][b];[a] palettegen [p];[b][p] paletteuse" screencast.gif
