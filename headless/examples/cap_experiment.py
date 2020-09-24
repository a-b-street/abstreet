#!/usr/bin/python3
# This example runs a scenario, finds roads with high driver throughput, then
# establishes a per-hour cap.
#
# Before running this script, start the API server:
#
# > cargo run --release --bin headless -- --port=1234 --alerts=silence
#
# You may need to install https://requests.readthedocs.io
# Keep this script formatted with autopep8 -i

import abst_helpers
import argparse
import requests
import sys
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--api', default='http://localhost:1234')
    parser.add_argument('--map_name', default='montlake')
    parser.add_argument('--hours', type=int, default=24)
    parser.add_argument('--cap_pct', type=int, default=80)
    args = parser.parse_args()
    print('Simulating {} hours of data/system/scenarios/{}/weekday.bin'.format(args.hours, args.map_name))
    print('')

    baseline = abst_helpers.run_sim(args)
    busiest_road, thruput = find_busiest_road(args)

    # Cap that road
    edits = requests.get(args.api + '/map/get-edits').json()
    cmd = requests.get(args.api + '/map/get-edit-road-command',
                       params={'id': busiest_road}).json()
    cmd['ChangeRoad']['new']['access_restrictions']['cap_vehicles_per_hour'] = int(
        (args.cap_pct / 100.0) * thruput)
    edits['commands'].append(cmd)

    # See what happened
    print('Rerunning with {}% cap on that road'.format(args.cap_pct))
    print('')
    experiment = abst_helpers.run_sim(args, edits=edits)
    busiest_road, thruput = find_busiest_road(args)
    baseline.compare(experiment)


# Find the road with the most car traffic in any one hour period
def find_busiest_road(args):
    thruput = requests.get(
        args.api + '/data/get-road-thruput').json()['counts']
    max_key = None
    max_value = 0
    for r, agent, hr, count in thruput:
        if agent == 'Car':
            if count > max_value:
                max_key = (r, hr)
                max_value = count
    print('Busiest road is #{}, with {} cars crossing during hour {}'.format(
        max_key[0], max_value, max_key[1]))
    return (max_key[0], max_value)


if __name__ == '__main__':
    main()
