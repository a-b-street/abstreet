function common_release {
	OUT=$1;

	rm -rfv $OUT
	mkdir $OUT

	cp docs/INSTRUCTIONS.md $OUT

	mkdir -p $OUT/data
	cp -Rv data/system $OUT/data
	# Not worth blowing up the download size yet
	rm -rfv $OUT/data/system/maps/huge_seattle.bin $OUT/data/system/scenarios/huge_seattle
}
