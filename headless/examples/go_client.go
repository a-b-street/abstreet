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
	"os"
	"time"
)

const (
	api = "http://localhost:1234/"
)

var (
	mapName         = flag.String("map", "montlake", "map name to simulate")
	hoursToSimulate = flag.Int("hours", 24, "number of hours to simulate")
	comparePct1     = flag.Int64("cmp1", -1, "the baseline percentage for indvidual comparison")
	comparePct2     = flag.Int64("cmp2", -1, "the experimental percentage for indvidual comparison")
)

func main() {
	flag.Parse()
	if *comparePct1 > *comparePct2 {
		fmt.Printf("--cmp1=%v --cmp2=%v invalid, --cmp1 is the baseline\n", *comparePct1, *comparePct2)
		os.Exit(1)
	}

	numSucceededLast := 0
	var results2 results
	for pct := int64(100); pct >= 0; pct -= 10 {
		results, err := run(pct)
		if err != nil {
			fmt.Println("Failure:", err)
			break
		}
		numSucceeded := len(results.successTime)
		if numSucceeded < numSucceededLast {
			fmt.Println("--> less trips succeeded this round, so likely hit gridlock")
			break
		}
		numSucceededLast = numSucceeded

		if *comparePct2 == pct {
			results2 = *results
		}
		if *comparePct1 == pct {
			results1 := results
			fmt.Printf("\nBaseline cancelled %v%%, experimental cancelled %v%%\n", *comparePct1, *comparePct2)
			var faster []float64
			var slower []float64

			for id, experimental_dt := range results2.successTime {
				baseline_dt := results1.successTime[id]
				if baseline_dt == 0.0 {
					// This means the trip didn't finish in hoursToSimulate in the baseline
					//fmt.Printf("Trip %v present in experimental, but not baseline\n", id)
					continue
				}

				if false && baseline_dt != experimental_dt {
					fmt.Printf("  Trip %v: %v baseline, %v experimental\n", id, baseline_dt, experimental_dt)
				}
				if baseline_dt > experimental_dt {
					faster = append(faster, baseline_dt-experimental_dt)
				} else if baseline_dt < experimental_dt {
					slower = append(slower, experimental_dt-baseline_dt)
				}
			}

			fmt.Printf("%v trips faster, average %v\n", len(faster), avg(faster))
			fmt.Printf("%v trips slower, average %v\n\n", len(slower), avg(slower))
		}
	}
}

func run(pct int64) (*results, error) {
	start := time.Now()

	_, err := post("sim/load", LoadSim{
		Scenario:  fmt.Sprintf("data/system/scenarios/%v/weekday.bin", *mapName),
		Modifiers: []ScenarioModifier{{CancelPeople: pct}},
	})
	if err != nil {
		return nil, err
	}

	_, err = get(fmt.Sprintf("sim/goto-time?t=%v:00:00", *hoursToSimulate))
	if err != nil {
		return nil, err
	}

	resp, err := get("data/get-finished-trips")
	if err != nil {
		return nil, err
	}
	var trips []FinishedTrip
	if err := json.Unmarshal([]byte(resp), &trips); err != nil {
		return nil, err
	}

	results := results{}
	results.successTime = make(map[uint64]float64)
	for _, trip := range trips {
		if trip.Mode == "" {
			results.numAborted++
		} else {
			results.successTime[trip.ID] = trip.Duration
		}
	}

	fmt.Printf("%v with %v%% of people cancelled: %v trips aborted, %v trips succeeded. Simulation took %v\n", *mapName, pct, results.numAborted, len(results.successTime), time.Since(start))

	return &results, nil
}

type results struct {
	numAborted int
	// Trip ID to duration
	successTime map[uint64]float64
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

func avg(list []float64) string {
	if len(list) == 0 {
		return "empty"
	}
	sum := 0.0
	for _, x := range list {
		sum += x
	}
	return fmt.Sprintf("%v", sum/float64(len(list)))
}

type LoadSim struct {
	Scenario  string             `json:"scenario"`
	Modifiers []ScenarioModifier `json:"modifiers"`
}

type ScenarioModifier struct {
	CancelPeople int64
}

type FinishedTrip struct {
	ID       uint64  `json:"id"`
	Duration float64 `json:"duration"`
	Mode     string  `json:"mode"`
	Capped   bool    `json:"capped"`
}
