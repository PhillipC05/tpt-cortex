package registry

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strings"
	"time"

	"github.com/gen2brain/beeep"
	"github.com/tpt-cortex/cortex-daemon/db"
	"github.com/tpt-cortex/cortex-daemon/manifest"
)

// DefaultRegistry implements the Phase 3/4 native APIs.
type DefaultRegistry struct {
	Logs []string

	db       *db.DB
	manifest *manifest.Manifest
	origin   string

	// OnScheduleAdd is called by native.schedule.add to register a background task.
	// Set by ipc.Server before execution to avoid a circular package dependency.
	// May be nil if the scheduler is not available.
	OnScheduleAdd func(schedule, script string, allow []string) (int64, error)
}

// NewDefaultRegistry creates a registry.
// d and m may be nil — APIs that need them return descriptive errors when absent.
func NewDefaultRegistry(d *db.DB, m *manifest.Manifest, origin string) *DefaultRegistry {
	return &DefaultRegistry{db: d, manifest: m, origin: origin}
}

func (r *DefaultRegistry) Call(api string, args []Value) (Value, error) {
	// Runtime permission gate against the loaded manifest.
	if r.manifest != nil && !r.manifest.IsAllowed(r.origin, api) {
		return VoidVal(), fmt.Errorf(
			"permission denied: %s not allowed for origin %q (see cortex.manifest.json)", api, r.origin,
		)
	}

	switch api {

	// ── Logging ─────────────────────────────────────────────────────────────

	case "native.log":
		parts := make([]string, len(args))
		for i, a := range args {
			parts[i] = a.String()
		}
		r.Logs = append(r.Logs, strings.Join(parts, " "))
		return VoidVal(), nil

	// ── File system ─────────────────────────────────────────────────────────

	case "native.fs.read":
		if len(args) < 1 {
			return VoidVal(), fmt.Errorf("native.fs.read requires 1 argument (path)")
		}
		data, err := os.ReadFile(args[0].Str)
		if err != nil {
			return VoidVal(), fmt.Errorf("native.fs.read: %w", err)
		}
		return StrVal(string(data)), nil

	// ── Desktop notifications ────────────────────────────────────────────────

	case "native.notify":
		title, body := "TPT Cortex", ""
		switch len(args) {
		case 1:
			body = args[0].String()
		case 2:
			title = args[0].String()
			body = args[1].String()
		default:
			return VoidVal(), fmt.Errorf("native.notify requires 1 or 2 arguments")
		}
		if err := beeep.Notify(title, body, ""); err != nil {
			r.Logs = append(r.Logs, fmt.Sprintf("[notify failed: %v] %s: %s", err, title, body))
		}
		return VoidVal(), nil

	// ── SQLite database ──────────────────────────────────────────────────────

	case "native.db.append":
		if len(args) < 2 {
			return VoidVal(), fmt.Errorf("native.db.append requires 2 arguments (table, data)")
		}
		if r.db == nil {
			return VoidVal(), fmt.Errorf("native.db.append: database not available (start daemon with --db=<path>)")
		}
		if err := r.db.Append(args[0].Str, args[1].Str); err != nil {
			return VoidVal(), fmt.Errorf("native.db.append: %w", err)
		}
		return VoidVal(), nil

	case "native.db.query":
		if len(args) < 1 {
			return VoidVal(), fmt.Errorf("native.db.query requires 1 argument (table)")
		}
		if r.db == nil {
			return VoidVal(), fmt.Errorf("native.db.query: database not available (start daemon with --db=<path>)")
		}
		rows, err := r.db.Query(args[0].Str)
		if err != nil {
			return VoidVal(), fmt.Errorf("native.db.query: %w", err)
		}
		out, _ := json.Marshal(rows)
		return StrVal(string(out)), nil

	// ── HTTP ─────────────────────────────────────────────────────────────────

	case "native.http.post":
		if len(args) < 2 {
			return VoidVal(), fmt.Errorf("native.http.post requires 2 arguments (url, body)")
		}
		resp, err := http.Post( //nolint:gosec // URL comes from the script's manifest-validated allow list
			args[0].Str,
			"application/json",
			strings.NewReader(args[1].Str),
		)
		if err != nil {
			return VoidVal(), fmt.Errorf("native.http.post: %w", err)
		}
		defer resp.Body.Close()
		body, err := io.ReadAll(resp.Body)
		if err != nil {
			return VoidVal(), fmt.Errorf("native.http.post: read body: %w", err)
		}
		return StrVal(string(body)), nil

	// ── Location ─────────────────────────────────────────────────────────────

	// native.location.current() → JSON string {"lat":f64,"lng":f64,"acc":f64,"ts":string}
	// Desktop: simulates Auckland CBD coords with per-second jitter.
	// Android: this case is overridden by the JNI-backed registry.
	case "native.location.current":
		now := time.Now().UTC()
		// Sub-second jitter so consecutive calls produce distinct entries
		jitter := float64(now.UnixMicro()%10000) / 1_000_000.0
		lat := -36.8485 + jitter*0.001
		lng := 174.7633 + jitter*0.001
		entry, _ := json.Marshal(map[string]interface{}{
			"lat": lat,
			"lng": lng,
			"acc": 5.0,
			"ts":  now.Format(time.RFC3339),
		})
		return StrVal(string(entry)), nil

	// ── Background scheduler ─────────────────────────────────────────────────

	// native.schedule.add(cron_expr, script) → task ID string
	// cron_expr supports 6-field second-level syntax: "*/30 * * * * *"
	case "native.schedule.add":
		if len(args) < 2 {
			return VoidVal(), fmt.Errorf("native.schedule.add requires 2 arguments (schedule, script)")
		}
		if r.OnScheduleAdd == nil {
			return VoidVal(), fmt.Errorf("native.schedule.add: scheduler not available")
		}
		id, err := r.OnScheduleAdd(args[0].Str, args[1].Str, nil)
		if err != nil {
			return VoidVal(), fmt.Errorf("native.schedule.add: %w", err)
		}
		return StrVal(fmt.Sprintf("%d", id)), nil

	// ── Remote execution ─────────────────────────────────────────────────────

	// native.cortex.execute_remote(url, token, script[, allow]) → JSON string of logs
	// allow is a comma-separated string; may be empty or omitted.
	case "native.cortex.execute_remote":
		if len(args) < 3 {
			return VoidVal(), fmt.Errorf("native.cortex.execute_remote requires at least 3 args: url, token, script")
		}
		endpoint := strings.TrimRight(args[0].Str, "/")
		token := args[1].Str
		script := args[2].Str
		var allow []string
		if len(args) >= 4 && args[3].Str != "" {
			for _, a := range strings.Split(args[3].Str, ",") {
				allow = append(allow, strings.TrimSpace(a))
			}
		}
		bodyBytes, _ := json.Marshal(map[string]interface{}{"script": script, "allow": allow})
		req, err := http.NewRequestWithContext(context.Background(), http.MethodPost, endpoint+"/execute", bytes.NewReader(bodyBytes))
		if err != nil {
			return VoidVal(), fmt.Errorf("native.cortex.execute_remote: build request: %w", err)
		}
		req.Header.Set("Authorization", "Bearer "+token)
		req.Header.Set("Content-Type", "application/json")
		resp, err := http.DefaultClient.Do(req)
		if err != nil {
			return VoidVal(), fmt.Errorf("native.cortex.execute_remote: %w", err)
		}
		defer resp.Body.Close()
		var result map[string]json.RawMessage
		if jsonErr := json.NewDecoder(resp.Body).Decode(&result); jsonErr != nil {
			return VoidVal(), fmt.Errorf("native.cortex.execute_remote: decode response: %w", jsonErr)
		}
		if errMsg, ok := result["error"]; ok {
			return VoidVal(), fmt.Errorf("remote execution failed: %s", errMsg)
		}
		out, _ := json.Marshal(result["logs"])
		return StrVal(string(out)), nil

	default:
		return VoidVal(), fmt.Errorf("unknown native API: %s", api)
	}
}
