#!/usr/bin/python3
# This example runs the same scenario repeatedly, each time cancelling a
# different number of trips uniformly at random. The eventual goal is to
# quantify how many trips need to be cancelled to substantially speed up
# remaining ones.
#
# Before running this script, start the API server:
#
# > cargo run --release --bin headless -- --port=1234 data/system/scenarios/montlake/weekday.bin
#
# You may need to install https://requests.readthedocs.io
# Keep this script formatted with autopep8 -i

import argparse
import json
import requests
import statistics
import sys
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--api', default='http://localhost:1234')
    parser.add_argument('--map_name', default='montlake')
    parser.add_argument('--hours', type=int, default=24)
    parser.add_argument('--cmp1', type=int, default=None)
    parser.add_argument('--cmp2', type=int, default=None)
    args = parser.parse_args()
    if args.cmp1 and args.cmp2 and args.cmp1 > args.cmp2:
        sys.exit(
            '--cmp1={} --cmp2={} invalid, --cmp1 is the baseline'.format(args.cmp1, args.cmp2))
    print('Simulating {} hours of data/system/scenarios/{}/weekday.bin'.format(args.hours, args.map_name))
    print('')

    num_succeeded_last = 0
    results2 = None
    for pct in range(100, 0, -10):
        start = time.time()
        results = run_sim(args, modifiers=[{'CancelPeople': pct}])
        print('{}% of people cancelled: {:,} trips aborted, {:,} trips succeeded. Simulation took {:.1f}s'.format(
            pct, results.num_aborted, len(results.trip_times), time.time() - start))
        if len(results.trip_times) < num_succeeded_last:
            print('--> less trips succeeded this round, so likely hit gridlock')
            break
        num_succeeded_last = len(results.trip_times)

        if args.cmp2 == pct:
            results2 = results
        if args.cmp1 == pct:
            print('')
            print('Baseline cancelled {}%, experimental cancelled {}%'.format(
                args.cmp1, args.cmp2))
            compare_results(results, results2)
            print('')


# Returns Results
def run_sim(args, modifiers=[]):
    requests.post(args.api + '/sim/load', json={
        'load': 'data/system/scenarios/{}/weekday.bin'.format(args.map_name),
        'modifiers': modifiers,
    })
    requests.get(args.api + '/sim/goto-time',
                 params={'t': '{}:00:00'.format(args.hours)})
    raw_trips = requests.get(
        args.api + '/data/get-finished-trips').json()['trips']

    # Map trip ID to the duration (in seconds) of the trip. Filter out aborted
    # (failed) trips.
    num_aborted = 0
    trip_times = {}
    for (_, trip, mode, duration) in raw_trips:
        if mode is None:
            num_aborted += 1
        else:
            trip_times[trip] = duration

    return Results(num_aborted, trip_times)


class Results:
    def __init__(self, num_aborted, trip_times):
        self.num_aborted = num_aborted
        # Maps trip ID to seconds
        self.trip_times = trip_times


def compare_results(results1, results2):
    faster = []
    slower = []

    for trip, after_dt in results2.trip_times.items():
        before_dt = results1.trip_times.get(trip)
        if not before_dt:
            # The trip didn't finish in time in the baseline run
            continue
        if before_dt:
            if before_dt > after_dt:
                faster.append(before_dt - after_dt)
            elif after_dt > before_dt:
                slower.append(after_dt - before_dt)

    print('{:,} trips faster, average {:.1f}s savings'.format(
        len(faster), statistics.mean(faster)))
    print('{:,} trips slower, average {:.1f}s loss'.format(
        len(slower), statistics.mean(slower)))


if __name__ == '__main__':
    main()
