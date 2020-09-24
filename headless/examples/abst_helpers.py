import requests
import statistics


# Returns Results
def run_sim(args, modifiers=[], edits=None):
    requests.post(args.api + '/sim/load', json={
        'load': 'data/system/scenarios/{}/weekday.bin'.format(args.map_name),
        'modifiers': modifiers,
    })
    if edits:
        requests.post(args.api + '/map/set-edits', json=edits)
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

    # self is the baseline, results2 is the experiment
    def compare(self, results2):
        faster = []
        slower = []

        for trip, after_dt in results2.trip_times.items():
            before_dt = self.trip_times.get(trip)
            if not before_dt:
                # The trip didn't finish in time in the baseline run
                continue
            if before_dt:
                if before_dt > after_dt:
                    faster.append(before_dt - after_dt)
                elif after_dt > before_dt:
                    slower.append(after_dt - before_dt)

        print('{:,} trips faster, average {:.1f}s savings'.format(
            len(faster), avg(faster)))
        print('{:,} trips slower, average {:.1f}s loss'.format(
            len(slower), avg(slower)))


def avg(data):
    if data:
        return statistics.mean(data)
    else:
        return 0.0
