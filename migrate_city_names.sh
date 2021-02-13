#!/bin/bash
# I used this during https://github.com/a-b-street/abstreet/issues/326 to
# rename 37 maps to include country codes. Not keeping around forever, but I
# want the script in version control somewhere.

set -e

for x in gb/allerton_bywater gb/ashton_park gb/aylesbury gb/aylesham gb/bailrigg gb/bath_riverside us/bellevue de/berlin gb/bicester gb/castlemead gb/chapelford gb/clackers_brook gb/culm us/detroit gb/dickens_heath gb/didcot gb/dunton_hills gb/ebbsfleet gb/great_kneighton gb/hampton gb/handforth gb/kidbrooke_village pl/krakow gb/lcid gb/leeds gb/london gb/long_marston gb/micklefield ca/montreal gb/newcastle_great_park us/nyc fr/paris us/providence at/salzburg us/seattle il/tel_aviv pl/warsaw; do
	country="$(echo $x | cut -d'/' -f1)"
	city="$(echo $x | cut -d'/' -f2)"
	echo "Fixing $country / $city"

	# Local data
	#mkdir -p data/input/$country data/system/$country
	#mv data/input/$city data/input/$country/$city
	#mv data/system/$city data/system/$country/$city

	# Local source-of-truth for S3
	#mkdir -p ~/s3_abst_data/dev/data/input/$country ~/s3_abst_data/dev/data/system/$country
	#mv ~/s3_abst_data/dev/data/input/$city ~/s3_abst_data/dev/data/input/$country/$city
	#mv ~/s3_abst_data/dev/data/system/$city ~/s3_abst_data/dev/data/system/$country/$city

	# Remote S3
	#aws s3 mv --recursive s3://abstreet/dev/data/input/$city s3://abstreet/dev/data/input/$country/$city
	#aws s3 mv --recursive s3://abstreet/dev/data/system/$city s3://abstreet/dev/data/system/$country/$city
done
