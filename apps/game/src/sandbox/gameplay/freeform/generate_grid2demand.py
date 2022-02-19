"""
Transform a .osm file to grid2demand's input_agent.csv.

This just follows the steps from
https://github.com/asu-trans-ai-lab/grid2demand/blob/main/grid2demand.ipynb.

You may need to first `pip install osm2gmns grid2demand`
"""

import argparse
import grid2demand
import os
import osm2gmns
import tempfile


def main():
    parser = argparse.ArgumentParser()
    # Input and output paths must be absolute
    parser.add_argument('--input_osm', type=str, required=True)
    parser.add_argument('--output_csv', type=str, required=True)
    parser.add_argument('--latitude', type=float, required=True)
    parser.add_argument('--x_blocks', type=int, required=True)
    parser.add_argument('--y_blocks', type=int, required=True)
    args = parser.parse_args()

    # Both libraries write lots of files to the current directory, so isolate them
    with tempfile.TemporaryDirectory() as tmpdir:
        os.chdir(tmpdir)

        net = osm2gmns.getNetFromFile(args.input_osm, POIs=True)
        osm2gmns.connectPOIWithNet(net)
        osm2gmns.generateNodeActivityInfo(net)
        osm2gmns.consolidateComplexIntersections(net)
        # grid2demand.ReadNetworkFiles just assumes the current directory has the
        # network CSV files; there's no way to pass in the network we already have.
        osm2gmns.outputNetToCSV(net)

        grid2demand.ReadNetworkFiles()
        zone = grid2demand.PartitionGrid(
            number_of_x_blocks=args.x_blocks, number_of_y_blocks=args.y_blocks, latitude=args.latitude)
        triprate = grid2demand.GetPoiTripRate()
        nodedemand = grid2demand.GetNodeDemand()
        accessibility = grid2demand.ProduceAccessMatrix(latitude=args.latitude)
        demand = grid2demand.RunGravityModel(
            trip_purpose=1, a=None, b=None, c=None)
        # This writes more csv files, including input_agent.csv
        demand = grid2demand.GenerateAgentBasedDemand()

        os.rename('input_agent.csv', args.output_csv)


if __name__ == '__main__':
    main()
