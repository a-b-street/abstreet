/*
This example runs the same scenario repeatedly, each time cancelling a
different number of trips uniformly at random. The eventual goal is to quantify
how many trips need to be cancelled to substantially speed up remaining ones.

Before running this script, start the API server:

> cargo run --release --bin headless -- --port=1234 data/system/scenarios/montlake/weekday.bin
*/

package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"io/ioutil"
	"net/http"
	"time"
)

const (
	api = "http://localhost:1234/"
)

var (
	mapName         = flag.String("map", "montlake", "map name to simulate")
	hoursToSimulate = flag.Int("hours", 24, "number of hours to simulate")
)

func main() {
	flag.Parse()

	numSucceededLast := 0
	for pct := int64(100); pct >= 0; pct -= 10 {
		numSucceeded, err := run(pct)
		if err != nil {
			fmt.Println("Failure:", err)
			break
		}
		if numSucceeded < numSucceededLast {
			fmt.Println("--> less trips succeeded this round, so likely hit gridlock")
			break
		}
		numSucceededLast = numSucceeded
	}
}

// Returns numSucceeded
func run(pct int64) (int, error) {
	start := time.Now()

	_, err := post("sim/load", SimFlags{
		Load:      fmt.Sprintf("data/system/scenarios/%v/weekday.bin", *mapName),
		Modifiers: []ScenarioModifier{{CancelPeople: pct}},
	})
	if err != nil {
		return 0, err
	}

	_, err = get(fmt.Sprintf("sim/goto-time?t=%v:00:00", *hoursToSimulate))
	if err != nil {
		return 0, err
	}

	resp, err := get("data/get-finished-trips")
	if err != nil {
		return 0, err
	}
	var trips FinishedTrips
	if err := json.Unmarshal([]byte(resp), &trips); err != nil {
		return 0, err
	}

	numAborted := 0
	numSucceeded := 0
	totalDuration := 0.0
	for _, trip := range trips.Trips {
		if trip[2] == nil {
			numAborted++
		} else {
			numSucceeded++
			totalDuration += trip[3].(float64)
		}
	}

	fmt.Printf("%v with %v%% of people cancelled: %v trips aborted, %v trips succeeded totaling %v seconds. Simulation took %v\n", *mapName, pct, numAborted, numSucceeded, totalDuration, time.Since(start))

	return numSucceeded, nil
}

func get(url string) (string, error) {
	resp, err := http.Get(api + url)
	if err != nil {
		return "", err
	}
	body, err := ioutil.ReadAll(resp.Body)
	resp.Body.Close()
	if err != nil {
		return "", err
	}
	return string(body), nil
}

func post(url string, body interface{}) (string, error) {
	encoded, err := json.Marshal(body)
	if err != nil {
		return "", err
	}
	resp, err := http.Post(api+url, "application/json", bytes.NewReader(encoded))
	if err != nil {
		return "", err
	}
	respBody, err := ioutil.ReadAll(resp.Body)
	resp.Body.Close()
	if err != nil {
		return "", err
	}
	return string(respBody), nil
}

type SimFlags struct {
	Load      string             `json:"load"`
	Modifiers []ScenarioModifier `json:"modifiers"`
}

type ScenarioModifier struct {
	CancelPeople int64
}

type FinishedTrips struct {
	// Vec<(Time, TripID, Option<TripMode>, Duration)>
	Trips [][]interface{} `json:"trips"`
}
