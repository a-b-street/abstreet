# 15-minute Santa

Created by [Dustin Carlino](https://abstreet.org),
[Yuwen Li](https://www.yuwen-li.com/), &
[Michael Kirk](https://michaelkirk.github.io/)

<iframe width="560" height="315" src="https://www.youtube.com/embed/mrIsVMLZ_yc" frameborder="0" allow="autoplay; encrypted-media" allowfullscreen></iframe>

15-minute Santa is a game where you deliver presents across Seattle. You earn
more points delivering to high-density housing, and you need to refuel from
shops, so you'll have to understand where people live in relation to where they
work and shop.

Contact <dabreegster@gmail.com> with any feedback or
[file an issue on Github](https://github.com/dabreegster/abstreet/issues/new).

## Play it

- [Play online](http://abstreet.s3-website.us-east-2.amazonaws.com/dev/santa)
  (slower and no music -- download below if possible)
- [Windows](https://github.com/dabreegster/abstreet/releases/download/v0.2.24/abstreet_windows_v0_2_24.zip)
- [Mac](https://github.com/dabreegster/abstreet/releases/download/v0.2.24/abstreet_mac_v0_2_24.zip)
- [Linux](https://github.com/dabreegster/abstreet/releases/download/v0.2.24/abstreet_linux_v0_2_24.zip)

Unzip, then run `santa.exe` or `santa`. No mobile/tablet support, sorry -- you need a keyboard.

## FAQ

### Why did y'all make this?

We normally work on [A/B Street](https://abstreet.org), a traffic simulation
that lets the general public explore a future prioritizing more sustainable
modes of transportation. All of the recent
[talk](https://crosscut.com/focus/2020/11/seattle-could-become-next-15-minute-city)
about 15-minute cities prompted us to explore how Seattle's zoning causes many
people to live far from where they get groceries. After experimenting with a
[more serious](fifteen_min.md) tool to understand walk-sheds, we decided to
spend a few weeks on something a bit more light-hearted.

### Realism

The map of Seattle and location of shops comes from
[OpenStreetMap](https://www.openstreetmap.org/about). We only consider shops if
they sell food or drinks -- let us know if the map seems to be missing your
favorite restaurant. The number of housing units is based on
[Seattle GIS data](https://data-seattlecitygis.opendata.arcgis.com/datasets/current-land-use-zoning-detail).
Mixed-use buildings with both commercial and residential units aren't
represented. The game lets you upzone any house to place a new store; obviously
this is a vast simplification of how complex a real conversation about changing
zoning codes should be.

We rigorously evaluated the speed and carrying capacity of different cargo bikes
and sleighs on the market to tune the vehicles in the game.

### Modding the game

Native versions only -- sorry, not easy to do on the web.

You can adjust the difficulty of the levels or give yourself all the upzoning
power you want by editing `data/player/santa.json`. You first have to set
`"enable_modding": true`. The format should mostly be self-explanatory; also see
[here](https://github.com/dabreegster/abstreet/blob/be589f7ef4f649bb5a35bfe8de0bc81a9deeb029/santa/src/session.rs#L13)
as a reference. If you break something, just delete the file to start over. If
you come up with a better gameplay progression, please share -- tuning a game is
hard!

### Adding new maps

Missing your slice of Seattle, or want to run somewhere else? If you have a bit
of technical experience,
[follow this guide](https://dabreegster.github.io/abstreet/howto/new_city.html)
and then the above instructions for modding the game. Otherwise, draw the map
boundaries in <http://geojson.io> and
[send it to us](https://github.com/dabreegster/abstreet/issues/new) along with a
time limit, goal, and starting point on the map. If you have a public data
source for the number of housing units per building, please include it!
