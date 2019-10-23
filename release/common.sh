function common_release {
	OUT=$1;

	rm -rfv $OUT
	mkdir $OUT

	cp docs/INSTRUCTIONS.md $OUT
	mkdir $OUT/data
	cp data/color_scheme.json $OUT/data

	mkdir $OUT/data/maps
	for map in 23rd ballard caphill downtown montlake; do
		cp -v data/maps/$map.bin $OUT/data/maps/
		mkdir -p $OUT/data/scenarios/$map
		cp -v data/scenarios/$map/weekday_typical_traffic_from_psrc.bin $OUT/data/scenarios/$map/
	done

	mkdir $OUT/data/shapes
	cp -v data/shapes/popdat.bin $OUT/data/shapes
}
