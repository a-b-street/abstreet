searchState.loadedDescShard("abstio", 0, "A/B Street organizes data files in a particular way. This …\nA single city is identified using this.\nPlayer-chosen groups of files to opt into downloading\nA single file\nA list of all canonical data files for A/B Street that’…\nA single map is identified using this.\nGenerate paths for different A/B Street files\nmd5sum of the file\nThe name of the city, in filename-friendly form – for …\nCompressed size in bytes\nA two letter lowercase country code, from …\nIdempotent\nDownloads bytes from a URL. This must be called with a …\nDownload a file from a URL. This must be called with a …\nKeyed by path, starting with “data/”\nKeeps file extensions\nPerforms an HTTP GET request and returns the raw response. …\nPerforms an HTTP POST request and returns the response.\nA list of cities to download for running the map importer.\nNormal file IO using the filesystem\nJust list all things from a directory, return sorted by …\nReturns full paths\nLoad all serialized things from a directory, return sorted …\nThe name of the map within the city, in filename-friendly …\nMay be a JSON or binary file. Panics on failure.\nExtract the map and scenario name from a path. Crashes if …\nPrint download progress to STDOUT. Pass this the receiver, …\nMay be a JSON or binary file\nA list of cities to download for using in A/B Street. …\nAn adapter for widgetry::Settings::read_svg to read SVGs …\nUncompressed size in bytes. Because we have some massive …\nReturns path on success\nPlayer-chosen groups of files to opt into downloading\nA single file\nA list of all canonical data files for A/B Street that’…\nFill out all data packs based on the local manifest.\nmd5sum of the file\nCompressed size in bytes\nKeyed by path, starting with “data/”\nRemoves entries from the Manifest to match the DataPacks …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nLook up an entry.\nA list of cities to download for running the map importer.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nBecause there are so many Seattle maps and they get …\nLoad the player’s config for what files to download, or …\nIf an entry’s path is system data, return the city.\nA list of cities to download for using in A/B Street. …\nSaves the player’s config for what files to download.\nUncompressed size in bytes. Because we have some massive …\nA single city is identified using this.\nA single map is identified using this.\nStringify the map name for filenames.\nThe name of the city, in filename-friendly form – for …\nA two letter lowercase country code, from …\nStringify the city name for debug messages. Don’t …\nStringify the map name for debug messages. Don’t …\nReturns the argument unchanged.\nReturns the argument unchanged.\nCreate a MapName from a city and map within that city.\nTransforms a path to a map back to a MapName. Returns <code>None</code> …\nConstructs the path to some city-scoped data/input.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nReturns all city names based on importer config.\nReturns all city names based on the manifest of available …\nReturns all city names available locally.\nReturns all city names either available locally or based …\nReturns all maps from all cities based on the manifest of …\nReturns all maps in a city based on importer config.\nReturns all maps from one city based on the manifest of …\nReturns all maps from one city that’re available locally.\nReturns all maps from one city that’re available either …\nReturns all maps from all cities available locally.\nReturns all maps from all cities either available locally …\nThe name of the map within the city, in filename-friendly …\nCreate a CityName from a country code and city.\nCreate a MapName from a country code, city, and map name.\nParses a CityName from something like “gb/london”; the …\nExtract the map and scenario name from a path. Crashes if …\nReturns the filesystem path to this map.\nConvenient constructor for the main city of the game.\nConvenient constructor for the main city of the game.\nReturns the string to opt into runtime or input files for …\nExpresses the city as a path, like “gb/london”; the …\nShould metric units be used by default for this map? …\nDownloads bytes from a URL. This must be called with a …\nDownload a file from a URL. This must be called with a …\nPrint download progress to STDOUT. Pass this the receiver, …\nPerforms an HTTP GET request and returns the raw response. …\nPerforms an HTTP POST request and returns the response.\nKeeps file extensions\nJust list all things from a directory, return sorted by …\nLoad all serialized things from a directory, return sorted …\nMay be a JSON or binary file. Panics on failure.\nMay be a JSON or binary file\nIdempotent\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nReturns full paths\nAlso hands back a callback that’ll add the final result …\nReturns path on success")