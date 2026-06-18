// Package updater checks GitHub Releases for a newer version of the daemon and
// optionally downloads the installer/package for the current OS.
//
// Typical usage (non-blocking; runs in the background):
//
//	go updater.CheckAsync("v1.2.3", "tpt-solutions/tpt-cortex", func(info *updater.ReleaseInfo) {
//	    log.Printf("update available: %s → %s  (%s)", info.Current, info.Latest, info.DownloadURL)
//	})
package updater

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"runtime"
	"strings"
	"time"
)

const apiTimeout = 10 * time.Second

// ReleaseInfo is returned by the check when a newer version exists.
type ReleaseInfo struct {
	Current     string
	Latest      string
	ReleaseURL  string // HTML page for the GitHub release
	DownloadURL string // direct download URL for the current OS asset (may be empty)
}

// githubRelease is the minimal subset of the GitHub API response we need.
type githubRelease struct {
	TagName string `json:"tag_name"`
	HTMLURL string `json:"html_url"`
	Assets  []struct {
		Name               string `json:"name"`
		BrowserDownloadURL string `json:"browser_download_url"`
	} `json:"assets"`
}

// CheckAsync runs Check in a goroutine.  cb is called only when an update is
// found; errors are silently ignored (no update available is not an error for
// the user).
func CheckAsync(current, repo string, cb func(*ReleaseInfo)) {
	go func() {
		info, err := Check(current, repo)
		if err != nil || info == nil {
			return
		}
		cb(info)
	}()
}

// Check fetches the latest release from GitHub and returns ReleaseInfo when the
// latest tag is newer than current.  Returns (nil, nil) if already up-to-date.
func Check(current, repo string) (*ReleaseInfo, error) {
	url := fmt.Sprintf("https://api.github.com/repos/%s/releases/latest", repo)
	client := &http.Client{Timeout: apiTimeout}
	req, err := http.NewRequest(http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Accept", "application/vnd.github+json")
	req.Header.Set("User-Agent", "tpt-cortex-daemon")

	resp, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusNotFound {
		// No releases published yet.
		return nil, nil
	}
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("GitHub API returned %d", resp.StatusCode)
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, err
	}

	var rel githubRelease
	if err := json.Unmarshal(body, &rel); err != nil {
		return nil, err
	}

	latest := strings.TrimPrefix(rel.TagName, "v")
	cur := strings.TrimPrefix(current, "v")

	if !isNewer(latest, cur) {
		return nil, nil
	}

	info := &ReleaseInfo{
		Current:    current,
		Latest:     rel.TagName,
		ReleaseURL: rel.HTMLURL,
	}

	// Pick the best asset for the current OS/arch.
	for _, asset := range rel.Assets {
		if matchesCurrentPlatform(asset.Name) {
			info.DownloadURL = asset.BrowserDownloadURL
			break
		}
	}

	return info, nil
}

// isNewer reports whether versionA is strictly greater than versionB.
// Compares major.minor.patch numerically; handles missing patch/minor parts.
func isNewer(a, b string) bool {
	pa := parseSemver(a)
	pb := parseSemver(b)
	for i := 0; i < 3; i++ {
		if pa[i] > pb[i] {
			return true
		}
		if pa[i] < pb[i] {
			return false
		}
	}
	return false
}

func parseSemver(v string) [3]int {
	// Strip any pre-release suffix (e.g. "1.2.3-beta")
	if idx := strings.IndexAny(v, "-+"); idx >= 0 {
		v = v[:idx]
	}
	parts := strings.SplitN(v, ".", 3)
	var out [3]int
	for i, p := range parts {
		if i >= 3 {
			break
		}
		fmt.Sscanf(strings.TrimSpace(p), "%d", &out[i])
	}
	return out
}

// matchesCurrentPlatform returns true if an asset filename looks like it
// targets the current GOOS/GOARCH combination.
//
// Naming convention for release assets (see .github/workflows/release.yml):
//
//	cortex-daemon-windows-amd64.exe
//	cortex-daemon-darwin-amd64
//	cortex-daemon-linux-amd64
//	cortex-android-<version>.apk
func matchesCurrentPlatform(name string) bool {
	name = strings.ToLower(name)
	goos := runtime.GOOS     // "windows", "darwin", "linux", "android"
	goarch := runtime.GOARCH // "amd64", "arm64", "386", ...

	return strings.Contains(name, goos) && strings.Contains(name, goarch)
}
