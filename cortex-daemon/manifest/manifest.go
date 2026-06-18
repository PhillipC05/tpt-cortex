// Package manifest loads and evaluates cortex.manifest.json permission rules.
package manifest

import (
	"encoding/json"
	"fmt"
	"os"
)

// Manifest defines allowed native APIs per app origin.
// If the file does not exist the daemon runs in allow-all mode.
type Manifest struct {
	Version string            `json:"version"`
	Apps    map[string]AppDef `json:"apps"`
}

// AppDef lists the native APIs that one app origin may call.
// An empty Allow slice means "allow everything" for that origin.
type AppDef struct {
	Allow []string `json:"allow"`
}

// Load reads cortex.manifest.json from path.
// If the file does not exist, an empty (allow-all) manifest is returned.
func Load(path string) (*Manifest, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return &Manifest{Version: "1", Apps: make(map[string]AppDef)}, nil
		}
		return nil, fmt.Errorf("manifest: read %s: %w", path, err)
	}
	var m Manifest
	if err := json.Unmarshal(data, &m); err != nil {
		return nil, fmt.Errorf("manifest: parse %s: %w", path, err)
	}
	return &m, nil
}

// IsAllowed reports whether origin may call api.
//
// Rules (evaluated in order):
//  1. Empty Apps map (no manifest file) → allow everything.
//  2. Exact origin match → check its Allow list.
//  3. Wildcard "*" origin entry → check its Allow list.
//  4. No matching entry → deny.
//
// Within an Allow list, "*" permits any API.
// An empty Allow list permits all APIs for that origin.
func (m *Manifest) IsAllowed(origin, api string) bool {
	if len(m.Apps) == 0 {
		return true
	}
	for _, key := range []string{origin, "*"} {
		app, ok := m.Apps[key]
		if !ok {
			continue
		}
		if len(app.Allow) == 0 {
			return true
		}
		for _, a := range app.Allow {
			if a == api || a == "*" {
				return true
			}
		}
		return false // origin matched but API not in list
	}
	return false // no matching origin, no wildcard
}
