// fchjson FILE.fch  -> parsed achievement data as JSON on stdout (read-only).
package main

import (
	"encoding/json"
	"fmt"
	"os"

	"tracker/fchparse"
)

func main() {
	if len(os.Args) != 2 {
		fmt.Fprintln(os.Stderr, "usage: fchjson FILE.fch")
		os.Exit(2)
	}
	raw, err := os.ReadFile(os.Args[1])
	if err == nil {
		var p *fchparse.Profile
		if p, err = fchparse.Parse(raw); err == nil {
			err = json.NewEncoder(os.Stdout).Encode(p)
		}
	}
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
