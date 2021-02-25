#!/usr/bin/python3
# This example loads an exported JSON map, finds different buildings, and
# generates a simple travel demand model.
#
# 1) cargo run --bin dump_map data/system/us/seattle/maps/montlake.bin > montlake.json
# 2) ./headless/examples/generate_traffic.py --map=montlake.json --out=traffic.json
# 3) cargo run --bin import_traffic -- --map=data/system/us/seattle/maps/montlake.bin --input=traffic.json
# 4) Use data/system/us/seattle/scenarios/montlake/monday.bin in the game or from the API.
#
# Keep this script formatted with autopep8 -i

import argparse
import json
import random


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--map', type=str, required=True)
    parser.add_argument('--out', type=str, required=True)
    args = parser.parse_args()

    # Load the map and find all buildings
    residential_building_ids = []
    commercial_building_ids = []
    with open(args.map, encoding='utf8') as f:
        map = json.load(f)
        for b in map['buildings']:
            # These categories are inferred from OpenStreetMap tags
            if 'Residential' in b['bldg_type'] or 'ResidentialCommercial' in b['bldg_type']:
                residential_building_ids.append(b['id'])
            if 'Commercial' in b['bldg_type'] or 'ResidentialCommercial' in b['bldg_type']:
                commercial_building_ids.append(b['id'])

    # Randomly generate a few people who take just one trip
    scenario = {
        'scenario_name': 'monday',
        'people': []
    }
    for _ in range(100):
        src = random.choice(residential_building_ids)
        dst = random.choice(commercial_building_ids)
        scenario['people'].append({
            'origin': {
                'TripEndpoint': {
                    'Bldg': src,
                }
            },
            'trips': [{
                'departure': 1.0,
                'destination': {
                    'TripEndpoint': {
                        'Bldg': dst,
                    }
                },
                'mode': 'Bike',
                'purpose': 'Shopping'
            }]
        })

    with open(args.out, 'w') as f:
        f.write(json.dumps(scenario, indent=2))


if __name__ == '__main__':
    main()
