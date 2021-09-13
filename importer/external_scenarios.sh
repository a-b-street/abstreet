#!/bin/bash
# Scenarios for some cities are based on JSON files produced by external travel
# demand models, such as https://github.com/a-b-street/abstr. This script
# re-imports all of them. When a map's road or intersection IDs change, this
# must be re-run, or the binary scenario will get out-of-sync.

set -e

# Keep this list in sync with release/deploy_actdev.sh
for city in allerton_bywater ashton_park aylesbury aylesham bailrigg bath_riverside bicester castlemead chapelford chapeltown_cohousing clackers_brook cricklewood culm dickens_heath didcot dunton_hills ebbsfleet exeter_red_cow_village great_kneighton halsnead hampton handforth kergilliack kidbrooke_village lcid lockleaze long_marston marsh_barton micklefield newborough_road newcastle_great_park northwick_park poundbury priors_hall taunton_firepool taunton_garden tresham trumpington_meadows tyersal_lane upton water_lane wichelstowe wixams wynyard; do
	./importer/actdev_scenario.sh $city;
done
