// Package scheduler wraps robfig/cron with a registry of named jobs.
// Jobs are plain func() closures — the scheduler has no knowledge of Cortex
// scripts; callers (ipc.Server) provide closures that call the interpreter.
package scheduler

import (
	"fmt"
	"sync"

	"github.com/robfig/cron/v3"
)

// Entry holds metadata about one scheduled job.
type Entry struct {
	CronID   cron.EntryID
	DBID     int64  // persisted task ID from SQLite (0 if ephemeral)
	Schedule string
	Script   string
	Allow    []string
}

// Scheduler wraps robfig/cron and keeps a registry of running entries.
type Scheduler struct {
	c       *cron.Cron
	entries map[cron.EntryID]*Entry
	mu      sync.RWMutex
}

// New creates a Scheduler with second-level cron precision
// (supports 6-field expressions like "*/30 * * * * *").
func New() *Scheduler {
	return &Scheduler{
		c:       cron.New(cron.WithSeconds()),
		entries: make(map[cron.EntryID]*Entry),
	}
}

// Add registers fn to run on schedule and returns the cron entry ID.
// dbID is the SQLite row ID (pass 0 for in-memory-only tasks).
func (s *Scheduler) Add(dbID int64, schedule, script string, allow []string, fn func()) (cron.EntryID, error) {
	id, err := s.c.AddFunc(schedule, fn)
	if err != nil {
		return 0, fmt.Errorf("scheduler: invalid cron expression %q: %w", schedule, err)
	}
	s.mu.Lock()
	s.entries[id] = &Entry{
		CronID:   id,
		DBID:     dbID,
		Schedule: schedule,
		Script:   script,
		Allow:    allow,
	}
	s.mu.Unlock()
	return id, nil
}

// Remove cancels the job and removes its metadata.
func (s *Scheduler) Remove(id cron.EntryID) {
	s.c.Remove(id)
	s.mu.Lock()
	delete(s.entries, id)
	s.mu.Unlock()
}

// List returns a snapshot of all scheduled entries.
func (s *Scheduler) List() []*Entry {
	s.mu.RLock()
	defer s.mu.RUnlock()
	out := make([]*Entry, 0, len(s.entries))
	for _, e := range s.entries {
		out = append(out, e)
	}
	return out
}

// Start begins processing scheduled jobs.
func (s *Scheduler) Start() { s.c.Start() }

// Stop halts the scheduler, waiting for any running jobs to finish.
func (s *Scheduler) Stop() { s.c.Stop() }
