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
from abst_helpers import get
from abst_helpers import post
import argparse
import sys
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--api', default='http://localhost:1234')
    parser.add_argument('--map_name', default='montlake')
    parser.add_argument('--hours', type=int, default=24)
    parser.add_argument('--cap_pct', type=int, default=80)
    parser.add_argument('--rounds', type=int, default=10)
    parser.add_argument('--cap_all_roads', type=bool, default=True)
    args = parser.parse_args()
    print('Simulating {} hours of data/system/scenarios/{}/weekday.bin'.format(args.hours, args.map_name))

    baseline = abst_helpers.run_sim(args)
    edits = get(args, '/map/get-edits').json()

    for _ in range(args.rounds):
        print('')
        print('{} roads have a cap'.format(len(edits['commands'])))
        experiment = abst_helpers.run_sim(args, edits=edits)
        baseline.compare(experiment)
        print('{:,} trips changed due to the caps'.format(
            len(experiment.capped_trips)))

        if args.cap_all_roads:
            edits['commands'] = cap_all_roads(args)
            # Individual cap per road; don't merge adjacent roads that happen to have the same cap.
            edits['merge_zones'] = False
        else:
            # Cap the busiest road
            busiest_road, thruput = find_busiest_road(args)
            cmd = get(args, '/map/get-edit-road-command',
                      params={'id': busiest_road}).json()
            cmd['ChangeRoad']['new']['access_restrictions']['cap_vehicles_per_hour'] = int(
                (args.cap_pct / 100.0) * thruput)
            edits['commands'].append(cmd)

    # Write the final edits
    f = open('cap_edits.json', 'w')
    f.write(get(args, '/map/get-edits').text)
    f.close()


# Find the road with the most car traffic in any one hour period
def find_busiest_road(args):
    thruput = get(
        args, '/data/get-road-thruput').json()['counts']
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


# Cap all roads to some percent of their max throughput over any one hour period
def cap_all_roads(args):
    thruput = get(
        args, '/data/get-road-thruput').json()['counts']
    max_per_road = {}
    for r, agent, hr, count in thruput:
        if agent == 'Car':
            if r not in max_per_road or count > max_per_road[r]:
                max_per_road[r] = count
    commands = []
    for r, count in max_per_road.items():
        cap = int((args.cap_pct / 100.0) * count)
        # Don't go too low
        if cap > 10:
            cmd = get(args, '/map/get-edit-road-command',
                      params={'id': r}).json()
            cmd['ChangeRoad']['new']['access_restrictions']['cap_vehicles_per_hour'] = cap
            commands.append(cmd)
    return commands


if __name__ == '__main__':
    main()
