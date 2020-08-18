#!/usr/bin/python3

import json
# You may need to install https://requests.readthedocs.io
import requests

api = 'http://localhost:1234'

print('Did you just start the simulation? Time is currently', requests.get(api + '/get-time').text)
print('Is intersection #42 a traffic signal?', requests.get(api + '/get-traffic-signal', params={'id': 42}).text)

# Get the current configuration of one traffic signal
ts = requests.get(api + '/get-traffic-signal', params={'id': 67}).json()
print('Offset of signal #67 is {} seconds'.format(ts['offset']))
print('Phases of signal #67:')
for phase in ts['phases']:
    print('')
    print(json.dumps(phase, indent=2))

# Double the duration of the first phase
print()
print()
ts['phases'][0]['phase_type']['Fixed'] *= 2
print('Update the signal config', requests.post(api + '/set-traffic-signal', json=ts).text)
