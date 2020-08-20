#!/usr/bin/python3
# This example will see how changing one traffic signal affects trip times.
# Before running this script, start the API server:
#
# > cargo run --release --bin headless -- --port=1234 data/system/scenarios/montlake/weekday.bin

import json
# You may need to install https://requests.readthedocs.io
import requests


def main():
    api = 'http://localhost:1234'
    hours_to_sim = '12:00:00'

    # Make sure to start the simulation from the beginning
    print('Did you just start the simulation? Time is currently', requests.get(api + '/sim/get-time').text)
    print('Reset the simulation:', requests.get(api + '/sim/reset').text)
    print()

    # Run a few hours to get a baseline
    print('Simulating before any edits')
    print(requests.get(api + '/sim/goto-time', params={'t': hours_to_sim}).text)
    baseline_trips = process_trips(requests.get(api + '/data/get-finished-trips').json()['trips'])
    baseline_delays = requests.get(api + '/traffic-signals/get-delays', params={'id': 67, 't1': '00:00:00', 't2': hours_to_sim}).json()
    print('Baseline: {} finished trips, total of {} seconds'.format(len(baseline_trips), sum(baseline_trips.values())))
    print()

    # Modify one traffic signal, doubling the duration of its second phase
    ts = requests.get(api + '/traffic-signals/get', params={'id': 67}).json()
    ts['phases'][1]['phase_type']['Fixed'] *= 2
    # Reset the simulation before applying the edit, since reset also clears edits.
    print('Reset the simulation:', requests.get(api + '/sim/reset').text)
    print('Update a traffic signal:', requests.post(api + '/traffic-signals/set', json=ts).text)
    print()

    # Repeat the experiment
    print('Simulating after the edits')
    print(requests.get(api + '/sim/goto-time', params={'t': hours_to_sim}).text)
    experimental_trips = process_trips(requests.get(api + '/data/get-finished-trips').json()['trips'])
    experimental_delays = requests.get(api + '/traffic-signals/get-delays', params={'id': 67, 't1': '00:00:00', 't2': hours_to_sim}).json()
    print('Experiment: {} finished trips, total of {} seconds'.format(len(experimental_trips), sum(experimental_trips.values())))
    print()

    # Compare -- did this help or not?
    print('{} more trips finished after the edits (higher is better)'.format(len(experimental_trips) - len(baseline_trips)))
    print('Experiment was {} seconds faster, over all trips'.format(sum(baseline_trips.values()) - sum(experimental_trips.values())))
    print()

    # How much delay difference was there for each direction of travel?
    print('Average delay per direction of travel at this intersection BEFORE edits:')
    for direction, delays1 in baseline_delays['per_direction']:
        # Skip empty cases and crosswalks
        if delays1 and not direction['crosswalk']:
            print('- {} -> {}: {:.1f} seconds'.format(print_road(direction['from']), print_road(direction['to']), sum(delays1) / len(delays1)))
    # TODO Hard to group these together to compare, because Python can't handle dicts as keys. Stringify it first.
    print('Average delay per direction of travel at this intersection AFTER edits:')
    for direction, delays2 in experimental_delays['per_direction']:
        # Skip empty cases and crosswalks
        if delays2 and not direction['crosswalk']:
            print('- {} -> {}: {:.1f} seconds'.format(print_road(direction['from']), print_road(direction['to']), sum(delays2) / len(delays2)))


# Return a map from trip ID to the duration (in seconds) of the trip. Filter
# out aborted (failed) trips.
def process_trips(trips):
    results = {}
    for (_, trip, mode, duration) in trips:
        if mode is not None:
            results[trip] = duration
    return results


def print_road(directed_road):
    if directed_road['forwards']:
        direxn = 'fwd'
    else:
        direxn = 'back'
    return 'Road #{} ({})'.format(directed_road['id'], direxn)


if __name__ == '__main__':
    main()
