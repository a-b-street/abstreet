#!/usr/bin/python3
# 790e13e9278e54cd7e1f5a2969a00057e33f778c changed the JSON schema. This script
# updates all of the data/ files. Keeping it around as an example for the next
# transition.
#
# The Rust code also implements this transformation (in
# map_model/src/edits/compat.rs), but it's less convenient to run it over all
# the data files, since it operates on entire map edits.

import json
import sys

for path in sys.argv[1:]:
    with open(path) as f:
        data = json.load(f)
        data['plans'] = [{
            'start_time_seconds': 0,
            'stages': data['stages'],
            'offset_seconds': data['offset_seconds'],
        }]
        del data['stages']
        del data['offset_seconds']

        with open(path, 'w') as f:
            f.write(json.dumps(data, indent=2))
            f.close()
