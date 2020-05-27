# Help map out on-street parking

![parking_mapper](parking_mapper.gif)

This guide assumes you've edited OSM before. Contact <dabreegster@gmail.com> if
you have any trouble. Also give me a heads up when you make some edits, so I can
regenerate the maps!

1.  [Install A/B Street](https://github.com/dabreegster/abstreet/blob/master/docs/INSTRUCTIONS.md)
2.  Choose **Contribute parking data** on the main screen
3.  Change the map if you'd like to focus somewhere in particular
4.  Click a road with unknown parking
5.  Select what kind of on-street parking the road has
6.  Repeat
7.  Click **Generate OsmChange file**
8.  Upload the diff.osc file by adding a layer in JOSM (or send it to me)

Like all edits to OSM, to figure out ground-truth, you can survey in-person or
use
[Bing Streetside](https://wiki.openstreetmap.org/wiki/Bing_Maps#Streetside_imagery).
**Do not use data from Google Maps to edit OSM.**

## FAQ

### Why?

I'm trying to build a realistic traffic simulation of Seattle using OSM data,
then use it to strengthen proposals for
[pedestrianized streets](https://dabreegster.github.io/abstreet/lake_wash/proposal.html),
[improving the bike network](https://www.glwstreets.org/45th-st-bridge-overview),
and
[mitigating the West Seattle bridge closure](https://dabreegster.github.io/abstreet/west_seattle/proposal.html).
A/B Street is only as good as its data, and parking is one of the biggest gaps.
Missing data means unrealistic traffic as vehicles contend for few parking
spots, and roads that look much wider than they are in reality.

### Why put this data in OSM?

Why can't I just grab parking data from
[SDOT's map](http://web6.seattle.gov/SDOT/seattleparkingmap/), using the
[blockface](http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface)
dataset? Well, I'm trying -- when you see a parking lane in the tool, it's
coming from blockface, unless that road in OSM is tagged. But the blockface
dataset is comically wrong in many places -- for example, the Montlake bridge
apparently has unrestricted parking?! King County GIS has confirmed the dataset
isn't meant to be used for this level of detail.

Plus, if the data is in OSM, anybody else can make use of it.

### How does the tool work?

A/B Street attempts to render individual lanes and intersections from OSM data.
This makes it useful to audit the lane tags in OSM, including
[parking:lane](https://wiki.openstreetmap.org/wiki/Key:parking:lane). The tool
tracks your edits and when you generate the OsmChange file, it grabs modified
ways from the OSM API to generate a diff. You can inspect the diff, load it in
JOSM, and upload.

**Your changes won't immediately be reflected in A/B Street.** Let me know when
you've done some amount of mapping, and I'll regenerate the maps from fresh
data.

### Why use this tool?

You don't have to; [this tool](https://zlant.github.io/parking-lanes/) or ID or
JOSM all work. But the UI is clunky for this specific purpose. (Also, if you
find this tool clunky in any way, let me know and I'll fix it.) There's also a
proposed
[StreetComplete quest](https://github.com/westnordost/StreetComplete/issues/771).

### What about parking restrictions?

There are many
[parking:lane](https://wiki.openstreetmap.org/wiki/Key:parking:lane) tags to
indicate restricted parking zones, time restrictions, etc. Feel free to map that
in ID or JOSM, but I'm just looking to make a first pass over a wide area.

### What about off-street parking?

Ideally I'd also like to know how many private parking spots are available to
residents of each building. But I don't know of an OSM schema for mapping this,
or a practical way to collect this data. Let me know if you have ideas.

### What about long roads where parking appears and disappears?

The tool won't help. Use your favorite editor to split the way when the lane
configuration changes. Also feel free to just skip these areas.

### How to coordinate with other mappers?

If somebody wants to set up HOT tasking, that'd be great, but I don't expect so
many people to jump on this.

### I noticed weird roads in the tool

Welcome to my world. ;) If the number of lanes seems wrong, select the road and
check the OSM tags. I'm inferring lanes from that. Feel free to make manual OSM
edits to fix any problems you see. (I'd like to extend this tool to make that
easier; let me know if you have ideas how to do this.)

### I want to map an area, but there's no option for it

To keep the release size small, I'm not including all maps yet. Let me know what
you'd like to see included.

Or if you have a `.osm` file, try the [quick start guide](new_city.md).
