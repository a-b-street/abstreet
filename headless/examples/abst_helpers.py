import requests
import statistics


def get(args, cmd, **kwargs):
    resp = requests.get(args.api + cmd, **kwargs)
    if resp.status_code != requests.codes.ok:
        raise Exception(resp.text)
    return resp


def post(args, cmd, **kwargs):
    resp = requests.post(args.api + cmd, **kwargs)
    if resp.status_code != requests.codes.ok:
        raise Exception(resp.text)
    return resp


# Returns Results
def run_sim(args, modifiers=[], edits=None):
    post(args, '/sim/load', json={
        'scenario': 'data/system/scenarios/{}/weekday.bin'.format(args.map_name),
        'modifiers': modifiers,
        'edits': edits,
    })
    post(args, '/sim/goto-time',
         params={'t': '{}:00:00'.format(args.hours)})
    raw_trips = get(args, '/data/get-finished-trips').json()

    # Map trip ID to the duration (in seconds) of the trip. Filter out aborted
    # (failed) trips.
    num_aborted = 0
    trip_times = {}
    capped_trips = set()
    for trip in raw_trips:
        if trip['mode'] is None:
            num_aborted += 1
        else:
            trip_times[trip['id']] = trip['duration']
        if trip['capped']:
            capped_trips.add(trip['id'])

    return Results(num_aborted, trip_times, capped_trips)


class Results:
    def __init__(self, num_aborted, trip_times, capped_trips):
        self.num_aborted = num_aborted
        # Maps trip ID to seconds
        self.trip_times = trip_times
        # A set of trip IDs
        self.capped_trips = capped_trips

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
        print('{:,} trips aborted before, {:,} after'.format(
            self.num_aborted, results2.num_aborted))


def avg(data):
    if data:
        return statistics.mean(data)
    else:
        return 0.0
