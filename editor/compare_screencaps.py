#!/usr/bin/python2

import os
import subprocess
import sys

def run():
    
    print sorted(os.listdir(sys.argv[1]))
    print sorted(os.listdir(sys.argv[2]))


if __name__ == '__main__':
    run()
