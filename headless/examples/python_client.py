#!/usr/bin/python3
# This example will see how changing one traffic signal affects trip times.
# Before running this script, start the API server:
#
# > cargo run --release --bin headless -- --port=1234

import json
# You may need to install https://requests.readthedocs.io
import requests


api = 'http://localhost:1234'
hours_to_sim = '12:00:00'


def main():
    # Make sure to start the simulation from the beginning
    print('Did you just start the simulation? Time is currently',
          requests.get(api + '/sim/get-time').text)
    print('Reset the simulation:', requests.get(api + '/sim/reset').text)
    print()

    # Run a few hours to get a baseline
    print('Simulating before any edits')
    trips1, delays1, thruput1 = run_experiment()
    print('Baseline: {} finished trips, total of {} seconds'.format(
        len(trips1), sum(trips1.values())))
    print()

    # Find the average position of all active pedestrians
    agents = [x['pos'] for x in requests.get(
        api + '/data/get-agent-positions').json()['agents'] if x['vehicle_type'] is None]
    avg_lon = sum([x['longitude'] for x in agents]) / len(agents)
    avg_lat = sum([x['latitude'] for x in agents]) / len(agents)
    print('Average position of all active pedestrians: {}, {}'.format(avg_lon, avg_lat))
    print()

    # Modify one traffic signal, doubling the duration of its second stage
    print('Modify a traffic signal')
    ts = requests.get(api + '/traffic-signals/get', params={'id': 67}).json()
    ts['stages'][1]['stage_type']['Fixed'] *= 2
    # Also start a new person, just to demonstrate the API
    if False:
        person = {
            'origin': {
                'longitude': -122.3056602,
                'latitude': 47.6458199
            },
            'trips': [
                {
                    'departure': 13 * 3600,
                    'position': {
                        'longitude': -122.3072871,
                        'latitude': 47.6383517
                    },
                    'mode': 'Drive'
                },
                {
                    'departure': 19 * 3600,
                    'position': {
                        'longitude': -122.3056602,
                        'latitude': 47.6458199
                    },
                    'mode': 'Drive'
                }
            ]
        }
        print('Create a new person:', requests.post(
            api + '/sim/new-person', json=person).text)
    # Reset the simulation before applying the edit, since reset also clears edits.
    print('Reset the simulation:', requests.get(api + '/sim/reset').text)
    print('Update a traffic signal:', requests.post(
        api + '/traffic-signals/set', json=ts).text)
    # Sanity check that the edits were applied
    if False:
        print('Current map edits:\n', requests.get(
            api + '/map/get-edits').json())
    print()

    # Repeat the experiment
    print('Simulating after the edits')
    trips2, delays2, thruput2 = run_experiment()
    print('Experiment: {} finished trips, total of {} seconds'.format(
        len(trips2), sum(trips2.values())))
    print()

    # Compare -- did this help or not?
    print('{} more trips finished after the edits (higher is better)'.format(
        len(trips2) - len(trips1)))
    print('Experiment was {} seconds faster, over all trips'.format(
        sum(trips1.values()) - sum(trips2.values())))
    print()

    # Now we'll print some before/after stats per direction of travel through
    # the intersection
    col = '{:<40} {:>20} {:>20} {:>17} {:>17}'
    print(col.format('Direction', 'avg delay before',
                     'avg delay after', 'thruput before', 'thruput after'))
    for k in delays1.keys():
        print(col.format(k, delays1[k], delays2[k], thruput1[k], thruput2[k]))


# Returns (trips, delay, throughput)
def run_experiment():
    print(requests.get(api + '/sim/goto-time',
                       params={'t': hours_to_sim}).text)
    raw_trips = requests.get(api + '/data/get-finished-trips').json()
    raw_delays = requests.get(api + '/traffic-signals/get-delays',
                              params={'id': 67, 't1': '00:00:00', 't2': hours_to_sim}).json()
    raw_thruput = requests.get(
        api + '/traffic-signals/get-cumulative-thruput', params={'id': 67}).json()

    # Map trip ID to the duration (in seconds) of the trip. Filter out
    # cancelled trips.
    trips = {}
    for trip in raw_trips:
        if trip['duration'] is not None:
            trips[trip['id']] = trip['duration']

    # The direction is a dict, but Python can't handle dicts as keys. Stringify
    # the keys, also filtering out crosswalks and empty directions.
    delays = {}
    for k, v in raw_delays['per_direction']:
        k = stringify_direction(k)
        if k and v:
            delays[k] = '{:.1f}'.format(sum(v) / len(v))

    thruput = {}
    for k, v in raw_thruput['per_direction']:
        k = stringify_direction(k)
        if k:
            thruput[k] = v

    return (trips, delays, thruput)


def stringify_direction(direxn):
    if direxn['crosswalk']:
        return None
    return '{} -> {}'.format(stringify_road(direxn['from']), stringify_road(direxn['to']))


def stringify_road(directed_road):
    return 'Road #{} ({})'.format(directed_road['id'], directed_road['dir'])


if __name__ == '__main__':
    main()
