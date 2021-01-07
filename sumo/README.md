# SUMO interoperability

The purpose of this crate is to explore possible interactions between A/B Street
and [SUMO](https://www.eclipse.org/sumo/). Some of the ideas:

- Convert SUMO networks to ABST maps, to make use of SUMO's traffic signal
  heuristics and junction joining
- Convert SUMO demand to ABST scenarios, to leverage all of the existing
  [demand generation](https://sumo.dlr.de/docs/Demand/Introduction_to_demand_modelling_in_SUMO.html)
  techniques
- Prototype a new SUMO frontend by gluing ABST UI code to
  [TraCI](https://sumo.dlr.de/docs/TraCI.html)

## Usage

A quick SUMO primer. To convert an OSM file into a SUMO network:

`netconvert --osm-files data/input/seattle/osm/montlake.osm --output.street-names --keep-edges.components 1 -o montlake.net.xml`

To generate random trips and compute the routes for them:

`/usr/share/sumo/tools/randomTrips.py -n montlake.net.xml -r routes.xml`

To simulate these in SUMO:

`sumo-gui -r routes.xml -n montlake.net.xml`

To convert the network into an ABST map:

`cargo run --bin sumo montlake.net.xml`

To view it in ABST:

`cargo run --bin game -- --dev data/system/sumo/maps/montlake.bin`
