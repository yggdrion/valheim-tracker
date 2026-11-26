// fchsync: watch a Valheim characters folder and POST changed characters to the tracker server.
// Read-only on the game folder. Usage:
//
//	fchsync -dir "%USERPROFILE%\AppData\LocalLow\IronGate\Valheim\characters_local" -server http://host:8080 -token SECRET
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strings"
	"time"

	"tracker/fchparse"
)

func main() {
	dir := flag.String("dir", "", "characters folder (characters_local, or Steam cloud .../remote/characters)")
	server := flag.String("server", "http://localhost:8080", "tracker server base URL")
	token := flag.String("token", os.Getenv("TRACKER_TOKEN"), "shared secret (or $TRACKER_TOKEN)")
	every := flag.Duration("interval", 2*time.Second, "how often to look at the files (cheap: one stat per character)")
	hb := flag.Duration("heartbeat", 30*time.Second, "how often to tell the server we are still running")
	once := flag.Bool("once", false, "sync once and exit")
	flag.Parse()
	if *dir == "" {
		log.Fatal("-dir required")
	}
	// ponytail: stat polling instead of fsnotify: no dependency, same result on the Steam cloud folder,
	// and the game itself writes the .fch only on logout / autosave, so sub-second detection gains nothing.
	type fstate struct {
		mod  time.Time
		size int64
		beat time.Time
	}
	seen := map[string]fstate{}
	for {
		files, _ := filepath.Glob(filepath.Join(*dir, "*.fch"))
		for _, f := range files {
			base := strings.TrimSuffix(filepath.Base(f), ".fch")
			if strings.Contains(base, "_backup_") { // game's own backups
				continue
			}
			st, err := os.Stat(f)
			if err != nil {
				continue
			}
			old := seen[f]
			if st.ModTime().Equal(old.mod) && st.Size() == old.size {
				if time.Since(old.beat) >= *hb {
					if !beat(base, *server, *token) { // server forgot us (restart): upload again
						delete(seen, f)
					} else {
						old.beat = time.Now()
						seen[f] = old
					}
				}
				continue
			}
			time.Sleep(700 * time.Millisecond) // let the game finish writing (it writes .fch.new, then renames)
			if st2, err := os.Stat(f); err != nil || !st2.ModTime().Equal(st.ModTime()) || st2.Size() != st.Size() {
				continue // still changing: next poll
			}
			if err := push(f, base, *server, *token); err != nil {
				log.Printf("%s: %v (retry next poll)", base, err)
				continue
			}
			seen[f] = fstate{st.ModTime(), st.Size(), time.Now()}
			log.Printf("%s: uploaded", base)
		}
		if *once {
			return
		}
		time.Sleep(*every)
	}
}

// beat tells the server this client is still running; false means "upload again".
func beat(name, server, token string) bool {
	req, _ := http.NewRequest("POST", server+"/heartbeat?name="+url.QueryEscape(name), nil)
	req.Header.Set("X-Token", token)
	resp, err := (&http.Client{Timeout: 10 * time.Second}).Do(req)
	if err != nil {
		return true
	}
	resp.Body.Close()
	return resp.StatusCode != 404
}

func push(path, name, server, token string) error {
	raw, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	p, err := fchparse.Parse(raw)
	if err != nil {
		return err
	}
	body, _ := json.Marshal(p)
	req, _ := http.NewRequest("POST", server+"/upload?name="+url.QueryEscape(name), bytes.NewReader(body))
	req.Header.Set("X-Token", token)
	resp, err := (&http.Client{Timeout: 15 * time.Second}).Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != 200 {
		return fmt.Errorf("server said %s", resp.Status)
	}
	return nil
}
