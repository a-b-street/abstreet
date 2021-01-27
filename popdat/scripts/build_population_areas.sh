#!/bin/bash -e

# Outputs a FlatGeoBuf of geometries and their corresponding population,
# suitable for use in the popdat crate.
#
# You'll need to update the `configuration` section to get this to work on your
# machine.
# 
# Note: I've only ever used this for US data, with "Census Blocks" as areas. In
# theory other countries could also massage their data to fit into the combined
# FlatGeoBuf.
#
# ## Input
#
# I wasn't able to find a reasonable solution using the official census.gov
# files or API's. Instead I batch-downloaded all the census blocks and their
# populations from the very nice nhgis.org
#
# Specifically, at the time of writing the steps were:
#   - create an account for nhgis.org and log in
#   - select "Get Data"
#   - Under "Apply Filters": 
#     - Geographic Level: choose "Block", then "submit"
#     - Years: "2010" (hopefully 2020 will be available before long)
#     - Topics: choose "General: Total Population", then "submit"
#   - Under "Select Data":
#     - Select "Total Population"
#   - Click "Continue" towards top right
#   - Click "Continue" again towards top right
#   - Source Tables:
#     - You should see that you've selected "1 source table" 
#       - This corresponds to a CSV file of the populations and GISJOIN id but
#         no geometries 
#     - Click "Geographic Extents" > "Select All" > Submit
#       - This corresponds to shapefiles for every state + DC + Puerto Rico
#         with a GISJOIN attribute to join to the population CSV
#   - Enter a "Description" which is memorable to you like (2010 all US block populations)
#   - Submit and wait for the batch to be prepared
#
# Input files:
# - a csv of populations with a GISJOIN field
# - geometry shapefiles with the area boundaries and a GISJOIN field
# 
# Output:
# An single FGB with a feature for each area which includes its population.

# Configuration

## Inputs

geometry_shapefiles=$(ls source/nhgis_us_block_population/*.shp)
population_csv=source/nhgis_us_block_population/nhgis0004_ds172_2010_block.csv
population_column_name=H7V001

# ## SpatiaLite
#
# Even though we're not doing spatial changes in sqlite, making *any* changes
# to spatial tables, fires db triggers, which require you have the spatialite
# lib linked.
spatialite_bin=/usr/local/Cellar/sqlite/3.34.0/bin/sqlite3 
spatialite_lib=/usr/local/lib/mod_spatialite.dylib

# Main program follows

gpkg_output=generated/population_areas.gpkg

rm -f $gpkg_output

echo "Importing geometries - start $(date)"

for i in $geometry_shapefiles; do
    filename=$(basename -- "$i")
    extension="${filename##*.}"
    layer_name="${filename%.*}"
    if [ ! -f "$gpkg_output" ]; then
		echo "starting with $i"
        # first file - create the consolidated output file

        # I tried selecting only the columns we needed here, but I got a "table
        # not found" error, so instead we filter later when building the FGB
        # ogr2ogr -f "GPKG" -nln areas -t_srs "WGS84" -dialect SQLite -sql "SELECT GISJOIN FROM $layer_name" $gpkg_output $i

        # Note the `nlt` option addresses this warning:
        # > Warning 1: A geometry of type MULTIPOLYGON is inserted into layer areas of geometry type POLYGON, which is not normally allowed by the GeoPackage specification, but the driver will however do it. To create a conformant GeoPackage, if using ogr2ogr, the -nlt option can be used to override the layer geometry type. This warning will no longer be emitted for this combination of layer and feature geometry type.
        ogr2ogr -f "GPKG" -nlt MULTIPOLYGON -nln areas -t_srs "WGS84" $gpkg_output $i
    else
		echo "merging $i"
        # update the output file with new file content
        #ogr2ogr -f "GPKG" -nln areas -t_srs "WGS84" -dialect SQLite -sql "SELECT GISJOIN FROM $layer_name" -update -append $gpkg_output $i
        ogr2ogr -f "GPKG" -nlt MULTIPOLYGON -nln areas -t_srs "WGS84" -update -append $gpkg_output $i
    fi
done

echo "### Indexing geometries - start $(date)"
echo "
PRAGMA journal_mode = off;
CREATE INDEX index_areas_on_gisjoin on areas(GISJOIN)
" | $spatialite_bin $gpkg_output

echo "## Importing and indexing populations data - start $(date)"

echo "
PRAGMA journal_mode = off;
DROP TABLE IF EXISTS populations;
.import --csv $population_csv populations
ALTER TABLE populations RENAME COLUMN $population_column_name TO population;
CREATE INDEX index_populations_on_GISJOIN ON populations(GISJOIN);
" | $spatialite_bin $gpkg_output

echo "## join populations and geometries - start $(date)"

echo "
PRAGMA journal_mode = off;
.load $spatialite_lib
ALTER TABLE areas ADD COLUMN population;
update areas set population=(select population from populations where populations.GISJOIN=areas.GISJOIN);
" | $spatialite_bin $gpkg_output

echo "## outputting fgb - start $(date)"

ogr2ogr generated/population_areas.fgb $gpkg_output -dialect SQLite -sql "SELECT geom, population, GISJOIN FROM areas"

echo "## Done - $(date)"

