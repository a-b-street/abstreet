# A/B Street Map Design

link to code

## Model

The map model is designed for A/B Street's traffic simulation and player
editing, but it likely has other uses.

Objects:

* Road
	* Goes between two Intersections
	* Contains children Lanes in two directions
	* Geometry: a PolyLine representing the yellow line separating the directions of travel
		- This is usually the center of the road, except for one-ways
		  or when a road has more lanes in one direction.
	* Metadata from OSM
* Lane
	* Belongs to a parent Road
	* Has a LaneType: Driving, Parking, Sidewalk, Biking, Bus
		- Buses and bikes can usually use Driving lanes, but Biking and
		  Bus lanes are restricted.
	* Geometry: a PolyLine representing the center of the lane
	* Sidewalks know which Building paths are connected and 


things not represented
	- shared left turn lanes


coord system

![Alt text](https://g.gravizo.com/svg?
  digraph G {
    Road -> Intersection [label="connects two"];
    Road -> Lane [label="contains"];
    Lane -> Building [label="link to"];
    Lane -> BusStop [label="contains"];
    Intersection -> Turn [label="contains"];
    Turn -> Lane [label="connects"];
    Parcel;
    BusRoute -> BusStop [label="connects"];
    Area;
  }
)

### Data format

## Data sources

## Conversion process
