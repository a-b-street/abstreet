#!/usr/bin/python3
# This example runs the same scenario repeatedly, each time cancelling a
# different number of trips uniformly at random. The eventual goal is to
# quantify how many trips need to be cancelled to substantially speed up
# remaining ones.
#
# Before running this script, start the API server:
#
# > cargo run --release --bin headless -- --port=1234 --alerts=silence
#
# You may need to install https://requests.readthedocs.io
# Keep this script formatted with autopep8 -i

import abst_helpers
import argparse
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
        results = abst_helpers.run_sim(args, modifiers=[{'CancelPeople': pct}])
        print('{}% of people cancelled: {:,} trips cancelled, {:,} trips succeeded. Simulation took {:.1f}s'.format(
            pct, results.num_cancelled, len(results.trip_times), time.time() - start))
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
            results.compare(results, results2)
            print('')


if __name__ == '__main__':
    main()
