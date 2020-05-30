# Project history

As of June 2020.

tldr: A/B Street has been in active development since June 2018, but the idea has been festering since I was about 16.

<!--ts-->

<!--te-->

## Backstory

I originally wanted to tell a much longer story here of how I came to work on A/B Street, but I'm not sure this is the right time yet. So consider this the quick version.

![Impatience is a virtue](cloud_florida.jpg)

I grew up in Baton Rouge, where driving is effectively the only mode of transport. (I've gone back and made a point of taking long walks to confirm how antagonistically the city is designed towards other modes.) Very early on, I fell in love with a Nintendo 64 game called Banjo Kazooie, which led me to the online fan communities of the early 2000's. I wanted to create games too, so I started learning programming via library books and lots of questions on IRC. Because I never had any confidence in art, I wound up working on roguelikes, which led to a fervent interest in pathfinding algorithms and [collaborative diffusion](http://www.cs.colorado.edu/~ralex/papers/PDF/OOPSLA06antiobjects.pdf). When I started driving in high school, I quickly realized how bad people were at it. I remember being stuck at the intersection of [Florida Blvd and Cloud](https://www.openstreetmap.org/node/1279204989) and first wondering if the pathfinding algorithms could help with traffic. Can you see where this is going?

![Hand-mapping UT Austin](ut_map.png)

I moved to Austin for college. One of the first days of class, I shuffled down the stairs of Gearing Hall past a crackly old speaker apocalyptically announcing the weather forecast (details add color, right?) into a seminar demanding a totally open-ended first assignment to do something interesting. After I left, somebody stopped to ask me for directions, but I didn't know campus well yet. I thought about how Google Maps gave really silly walking directions. So I decided I'd hand-draw a map of campus, showing all of the construction, how to cut through the labryinth that is Welch Hall on hot days, and where to find the 24/7 robot coffee machines, and hack together a routing engine to help people find the shortest path between their classes. The feedback I got on this assignment included something along the lines of, "I was really pretty impressed first that you would be so stupid as to actually try to do this..."

![Approximately Orchestrated Routing and Transportation Analyzer](aorta.gif)

But I did, and that led me to discovering OpenStreetMap, which it turns out was pretty pivotal. (The first version of my campus map was seeded vaguely off an official paper map, but mostly I walked around and invented half-assed surveying methods on the spot.) Next semester, I joined a freshman research stream with somebody who had worked on [AIM](http://www.cs.utexas.edu/~aim/), UT's demonstration that autonomous vehicles wouldn't need traffic lights. Everything came together, and I started a 3 year journey of building [AORTA](https://github.com/dabreegster/aorta/), a traffic simulator for AVs. Guided by the research lab, I explored the really bizarre idea of letting AVs [bid to turn lights green sooner](http://www.cs.utexas.edu/~aim/papers/ITSC13-dcarlino.pdf) and micro-tolling all roads to disincentivize congestion. Both of these mechanisms would be incredibly unfair to people without the spare cash to back up their high value-of-time, but I brushed this off by saying the currency could be based on carpooling, EVs, etc.

![Manhattan took walkability seriously](manhattan.jpg)

It was great to try research in college; I learned I _really_ dislike munging data and compressing my work into 6 pages of conference paper LaTeX. So I moved to Seattle to work in industry instead, on something completely unrelated to transportation. Lots of things began unravelling for me in Seattle, but one of them was biking. In Austin, I had picked up mountain biking, and all but stopped driving; it was an amazing place to explore and commute by bike. Seattle was different. There were many more cyclists around, but the experience felt more stressful, the drivers more aggressive. I had plenty of near-misses. I kept commuting by bike, but the joy of it was gone. I started noticing how many cars were parked on narrow arterials and wondering why that was a fair use of space. I started paying attention to the public discourse around bike infrastructure in Seattle and feeling like the conversation was... chaotic.

Fast forward to late 2017. This is where I'll omit chunks of the story. Lots of things were crumbling at this point. I visited London, my first experience with a city that took public transit seriously. When I returned, lots of latent ideas stopped fermenting and started exploding. I threw together a prototype of A/B Street and started the arduous process at work of open-sourcing it and applying to a program to let me work it on for a few quarters. A few months later, I wound up quitting instead, and began to work on A/B Street in earnest.

## Year 1 (June 2018-2019)

I skimmed through git and summarized roughly what I was working on each month. Milestones are called out.


** dig up old screenshots



UI churn
fix problems in OSM



June 2018: lanes, bldg paths, polyline geometry, protobuf -> serde
July: drawing stop signs and signals, pathfinding and spawning/using parked cars, pedestrians, bikes, lane edits, making intersection policies handle requests from both
August: porting aorta's driving model

Sept: muli-leg trips, buses, lots of UI churn, extracting geom, the first ezgui wizard stuff, randomized scenarios
Oct: a/b test mode (and per-map plugins, and the first proper plugin API), UI churn, drawing routes, forking RNG for edit-invariance, first attempt to use OSM sidewalks, intersection geometry
Nov: clipping / borders, use blockface for parking, time travel, test runner framework, first mention of a DES
Dec: smarter traffic signal policies, bezier curves for turns, tsig editor, use rust 2018 NLL, parking blackholes, starting tutorial, first attempt at merging intersections, right click menus, a top menu?!, the modal menus, tsig diagram
	- modes: view, debug, edit. overlapping keys were the problem
	- colorscheme refactor: use a python script to scrape get_def calls
	- 1000 commits

Jan 2019: intersection geom, careful 1cm resoltuion in geom, the ezgui screencapper, start synthetic map edior
	*** the grand pison->glium change
	- retrospective: not starting with robust geom lib cost lots. the polyline problem
	- retrospective: so much UI churn from lack of design
Feb: fixing sim bugs, map geometry, the weird time-space interval thing, sudden departure to perf
	*** Feb 19: start the new DES model, sheep whiteboard
	- forked sim, manually reimplementing higher-level stuff like roaming for parking, bc no separation of mechanics from controller
	- Feb 27: cutover!
March: poking at CHs, fleshing out DES model (laggy heads!), first attempt to release for windows too, gridlock detector

April 2019: misc polish and fix, start describing features publicly, some kind of splash menu to load diff maps, some refacor of modes that killed off top-menu, the first releases
May: loading_screen, animate peds and bikes, more drawing polish, census tract popdat
	- may 6: make peds travel at a realistic speed. before just to debug faster, they were fast?!
	*** discovered psrc

## Year 2 (June 2019-2020)

June 2019: fast_paths! stackable game states
July: OSM turn restrictions, misc. i think i was in europe.
Aug: ui churn, shitty map-space text, ped crowds, agent color schemes, parking blackholes, some refactor to store Pt2D in raw_data instead of gps, first hackathon
Sept: offstreet parking v1, parked cars per bldg from psrc (ahh before people would use ANY car), implemented texture support for some reason, beefing up synthetic map editor, one set of mapfixes applied to all maps
	- milestone: finally got montlake to run without gridlock, >1 year later?!

Oct: UI churn, basic opportunistic LCing, starting to manually fix parking but only in mapfixes, parking sim fixes (reuse parked cars, less aborted trips), attempting to bring in / manually map sidewalks, some big mapfixes overhaul with IDs, starting challenge modes
Nov: prebaked sims, interactive time-series plots, undo in edit mode, traffic sig editor groups turns
	** yuwen joins, another hackathon
Dec: UI! flexbox, minimap, new time/speed controls, showing trip timelines, cutting over to SVGs, info panels, scrolling?!, preview traffic sig edits, started naming releases sensibly
	- accidentally posted to HN, woops!

Jan 2020: UI churn, but now purposefully! :) the modern tutorial?
Feb: UI and tutorial work. all text pure vector. port to wasm!
March: pandemic started. still UI and tutorial. start mapping person<->trips, population heatmap. working on left-hand driving. massive info panel overhaul, typography overhaul, engage with greenways and start traffic signal mapping effort
April: orestis joins and starts pandemic model, still UI, trip table, refocused to optimize commute challenge, parked cars owned by ppl not bldgs and removing schedule gaps, trip time histogram data viz for challenge results, MAJOR progress fixing gridlock in sim layer
May: proper OSM and no more mapfixes! proper support for more cities. more gridlock work, manually fixing OSM now, differential throughput for routing diversions and first real proposal write-up, make player's map edits long-lasting, dedicated parking mapper, finished fixing hidpi bugs probably, handling multi-step turn restrictions and attempting new tsig editor based on this, polishing UI and rendering, generating random bios for people, and of course writing docs like this to prep for release ;)
