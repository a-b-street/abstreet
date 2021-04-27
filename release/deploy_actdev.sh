#!/bin/bash
# This is like deploy_web.sh, but just creates a directory for https://github.com/cyipt/actdev
#
# IMPORTANT: First run deploy_web.sh to actually build everything. This script
# assumes web/build/dist is up-to-date.

set -x
set -e

mkdir abst_actdev
cp -Rv web/build/dist/abstreet* abst_actdev
# We're not packaging multiple apps, so set the index.html
mv abst_actdev/abstreet.html abst_actdev/index.html

# Copy just what's needed from data
mkdir -p abst_actdev/system/gb
# We could just copy all of system/gb and remove leeds and london, but
# actually, having this list in a script somewhere is kind of convenient.
for dir in allerton_bywater ashton_park aylesbury aylesham bailrigg bath_riverside bicester castlemead chapelford chapeltown_cohousing clackers_brook culm dickens_heath didcot dunton_hills ebbsfleet exeter_red_cow_village great_kneighton halsnead hampton handforth kergilliack kidbrooke_village lcid lockleaze long_marston marsh_barton micklefield newborough_road newcastle_great_park northwick_park poundbury priors_hall taunton_firepool taunton_garden tresham trumpington_meadows tyersal_lane upton water_lane wichelstowe wixams wynyard; do
	cp -Rv data/system/gb/$dir abst_actdev/system/gb
done
cp -Rv data/system/study_areas abst_actdev/system
gzip -v `find abst_actdev/ | grep bin | xargs`

zip -r abst_actdev abst_actdev
rm -rf abst_actdev
echo "Go upload abst_actdev.zip to https://github.com/cyipt/actdev/releases"
