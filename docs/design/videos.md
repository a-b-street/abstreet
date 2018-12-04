# Videos

ffmpeg -f x11grab -r 25 -s 1024x768 -i :0.0 -vcodec huffyuv raw.avi

ffmpeg -ss 10.0 -t 5.0 -i raw.avi -f gif screencast.gif
