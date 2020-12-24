# Year 3 (June 2020-2021)

- June: parking lots, real minimap controls, road labels
  - **June 22**: alpha launch!
    [r/Seattle](https://old.reddit.com/r/Seattle/comments/hdtucd/ab_street_think_you_can_fix_seattles_traffic/),
    [r/SeattleWA](https://old.reddit.com/r/SeattleWA/comments/hdttu8/ab_street_think_you_can_fix_seattles_traffic/),
    [r/UrbanPlanning](https://old.reddit.com/r/urbanplanning/comments/hdylmo/ab_street_a_traffic_simulation_game/),
    [HN](https://news.ycombinator.com/item?id=23605048#23608365),
    [GeekWire](https://www.geekwire.com/2020/want-fix-seattle-traffic-redditor-makes-game-allows-players-tweak-city-streets/),
    [The Stranger](https://www.thestranger.com/slog/2020/06/29/43999454/ab-streets-game-lets-you-create-the-seattle-street-grid-of-your-dreams)
- July: loads of bugfixes, map geometry improvements, UI cleanups,
  access-restricted zones for private neighborhoods and no-through-traffic,
  better traffic generation between home<->work for new maps, complete overhaul
  to bus routes and introduction of light rail, commute pattern explorer,
  importing Krakow and Berlin, smarter lane-changing, walkable shoulders for
  roads without sidewalks
  - [KING 5 Evening](https://www.youtube.com/watch?v=Pk8V-egsUxU) interview
- August: Michael joins, multiple traffic signals can be edited together,
  started a headless JSON API, support for other languages in OSM data, started
  congestion capping, backwards-compatible and more robust map edits, two-way
  cycletracks, more cities imported, slurry of bugfixes and performance
  improvements
  - [Silicon Valley Bike Summit](https://bikesiliconvalley.org/2020/07/poster_dustin-carlino/),
    [Seattle PI](https://www.seattlepi.com/local/transportation/slideshow/solve-Seattles-traffic-problem-in-this-video-game-205839.php)
- September: full support for driving on the left, textured color scheme,
  rendering isometric buildings, editing traffic signal offsets, a big round of
  UI changes, infinite parking mode, trip purpose, alleyways
  - [SeattleMet](https://www.seattlemet.com/news-and-city-life/2020/09/a-new-game-allows-you-to-redesign-seattle-streets)
- October: unit tested turn generation, web version launched with async file
  loading, thought bubbles showing agent goals, slow parts of a trip
  highlighted, more UI overhauls, dedicated OSM viewer mode started, major
  simulation performance optimizations, major progress towards live map edits,
  automatically picking boundaries for arbitrary cities
- November: switched from Dropbox to S3, download new maps in-game, collision
  dataviz UI, day/night color switching, unit testing lane changing behavior,
  starting the 15 min walkshed tool, simplified simulation spawning code,
  recording and replaying traffic around a few intersections, refactoring to
  split out separate tools
- December: lane-changing fixes, blocked-by explorer in debug mode, non-Latin
  font support on web, saving player state on web, census-based scenario
  generation started, 15 minute Santa
