package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"

	"github.com/google/uuid"
	"github.com/tpt-cortex/cortex-daemon/db"
	"github.com/tpt-cortex/cortex-daemon/ipc"
	"github.com/tpt-cortex/cortex-daemon/manifest"
	"github.com/tpt-cortex/cortex-daemon/scheduler"
	"github.com/tpt-cortex/cortex-daemon/tray"
	"github.com/tpt-cortex/cortex-daemon/updater"
)

// Version is stamped at build time via -ldflags "-X main.Version=v1.2.3".
var Version = "dev"

const githubRepo = "tpt-solutions/tpt-cortex"

func main() {
	addr := flag.String("addr", "127.0.0.1:9911", "WebSocket listen address")
	cortexBin := flag.String("cortex-bin", "", "Path to the cortex binary (auto-detected if empty)")
	dbPath := flag.String("db", "", "SQLite database path (default: ~/.cortex/cortex.db)")
	manifestPath := flag.String("manifest", "cortex.manifest.json", "Permission manifest path")
	noTray := flag.Bool("no-tray", false, "Disable system tray icon")
	noUpdate := flag.Bool("no-update-check", false, "Skip startup update check")
	flag.Parse()

	log.Printf("cortex-daemon %s", Version)

	// ── Resolve cortex binary ─────────────────────────────────────────────
	bin, err := resolveCortexBin(*cortexBin)
	if err != nil {
		log.Fatalf("cannot find cortex binary: %v\nSet --cortex-bin or add cortex to PATH", err)
	}
	log.Printf("using cortex binary: %s", bin)

	// ── Load permission manifest ──────────────────────────────────────────
	m, err := manifest.Load(*manifestPath)
	if err != nil {
		log.Fatalf("manifest: %v", err)
	}
	if len(m.Apps) == 0 {
		log.Printf("no cortex.manifest.json found — running in allow-all mode")
	} else {
		log.Printf("manifest loaded from %s (%d app entries)", *manifestPath, len(m.Apps))
	}

	// ── Open SQLite database ──────────────────────────────────────────────
	resolvedDB := resolveDBPath(*dbPath)
	store, err := db.Open(resolvedDB)
	if err != nil {
		log.Fatalf("database: %v", err)
	}
	defer store.Close()
	log.Printf("database: %s", resolvedDB)

	// ── Session token ─────────────────────────────────────────────────────
	token := uuid.New().String()
	log.Printf("session token: %s", token)
	log.Printf("(the PWA receives this automatically on first connect)")

	// ── Background scheduler ──────────────────────────────────────────────
	sched := scheduler.New()
	sched.Start()
	defer sched.Stop()

	// Restore persisted tasks from the previous session.
	// The IPC server wires actual execution closures; here we register
	// lightweight placeholders so IDs are reserved (full wiring happens in
	// server.execute when the scheduler callback fires).
	// For simplicity, the server re-registers tasks when it starts.
	// A future phase can add a RestoreTasks(store, server) helper.

	// ── IPC server ────────────────────────────────────────────────────────
	srv := ipc.NewServer(token, bin, store, m, sched)

	// ── System tray ───────────────────────────────────────────────────────
	// tray.Run() must block on the main OS thread, so we move the server
	// to a goroutine when the tray is enabled.
	if *noTray {
		log.Fatal(srv.ListenAndServe(*addr))
	} else {
		go func() {
			if err := srv.ListenAndServe(*addr); err != nil {
				log.Printf("server error: %v", err)
				os.Exit(1)
			}
		}()
		tray.Run(
			fmt.Sprintf("Cortex Daemon on %s", *addr),
			func() { os.Exit(0) },
		)
	}
}

func resolveDBPath(override string) string {
	if override != "" {
		return override
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return filepath.Join(".", ".cortex", "cortex.db")
	}
	return filepath.Join(home, ".cortex", "cortex.db")
}

func resolveCortexBin(override string) (string, error) {
	if override != "" {
		return override, nil
	}

	if env := os.Getenv("CORTEX_BIN"); env != "" {
		return env, nil
	}

	exe, _ := os.Executable()
	exeDir := filepath.Dir(exe)
	binaryName := "cortex"
	if runtime.GOOS == "windows" {
		binaryName = "cortex.exe"
	}

	candidates := []string{
		filepath.Join(exeDir, "..", "cortex-engine", "target", "debug", binaryName),
		filepath.Join(".", "cortex-engine", "target", "debug", binaryName),
		filepath.Join("cortex-engine", "target", "debug", binaryName),
	}
	for _, c := range candidates {
		if abs, err := filepath.Abs(c); err == nil {
			if _, err := os.Stat(abs); err == nil {
				return abs, nil
			}
		}
	}

	if path, err := exec.LookPath("cortex"); err == nil {
		return path, nil
	}

	return "", fmt.Errorf("cortex binary not found in standard locations or PATH")
}
