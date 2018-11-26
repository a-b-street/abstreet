#!/usr/bin/python
#
# Put this in your ~/.Xdefaults:
#
# URxvt.perl-ext-common: default,matcher
# URxvt.url-launcher: /home/dabreegster/abstreet/clickable_links.py
# URxvt.matcher.button: 1

import os
import sys

arg = sys.argv[1]
if arg.startswith('http://most/'):
    os.execvp('gedit', ['gedit', arg[len('http://most/'):]])
elif arg.startswith('http://ui/'):
    os.execvp('urxvt', ['urxvt', '-e', 'sh', '-c', 'cd ~/abstreet/editor; cargo run ' + arg[len('http://ui/'):]])
else:
    os.execvp('xdg-open', ['xdg-open', arg])
