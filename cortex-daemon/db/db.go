// Package db provides a SQLite-backed store for Cortex records and scheduled tasks.
// Uses modernc.org/sqlite (pure Go, no CGO) instead of mattn/go-sqlite3.
package db

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	_ "modernc.org/sqlite"
)

// DB wraps a SQLite connection.
type DB struct {
	conn *sql.DB
}

// ScheduledTask is a persisted background task loaded from the tasks table.
type ScheduledTask struct {
	ID       int64
	Schedule string
	Script   string
	Allow    []string
}

// Open opens (or creates) the SQLite database at path, running migrations.
func Open(path string) (*DB, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, fmt.Errorf("db: mkdir: %w", err)
	}
	conn, err := sql.Open("sqlite", path)
	if err != nil {
		return nil, fmt.Errorf("db: open %s: %w", path, err)
	}
	if err := migrate(conn); err != nil {
		conn.Close()
		return nil, err
	}
	return &DB{conn: conn}, nil
}

func migrate(conn *sql.DB) error {
	_, err := conn.Exec(`
		CREATE TABLE IF NOT EXISTS records (
			id         INTEGER PRIMARY KEY AUTOINCREMENT,
			table_name TEXT    NOT NULL,
			data       TEXT    NOT NULL,
			created_at DATETIME DEFAULT CURRENT_TIMESTAMP
		);
		CREATE TABLE IF NOT EXISTS tasks (
			id         INTEGER PRIMARY KEY AUTOINCREMENT,
			schedule   TEXT NOT NULL,
			script     TEXT NOT NULL,
			allow_list TEXT NOT NULL DEFAULT '[]',
			created_at DATETIME DEFAULT CURRENT_TIMESTAMP
		);
	`)
	if err != nil {
		return fmt.Errorf("db: migrate: %w", err)
	}
	return nil
}

// Append inserts a data record into the named logical table.
func (d *DB) Append(table, data string) error {
	_, err := d.conn.Exec(
		`INSERT INTO records (table_name, data) VALUES (?, ?)`,
		table, data,
	)
	return err
}

// Query returns all records from the named logical table, ordered by insertion.
func (d *DB) Query(table string) ([]string, error) {
	rows, err := d.conn.Query(
		`SELECT data FROM records WHERE table_name = ? ORDER BY id ASC`,
		table,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []string
	for rows.Next() {
		var data string
		if err := rows.Scan(&data); err != nil {
			return nil, err
		}
		out = append(out, data)
	}
	return out, rows.Err()
}

// SaveTask persists a scheduled task and returns its auto-incremented ID.
func (d *DB) SaveTask(schedule, script string, allow []string) (int64, error) {
	raw, _ := json.Marshal(allow)
	res, err := d.conn.Exec(
		`INSERT INTO tasks (schedule, script, allow_list) VALUES (?, ?, ?)`,
		schedule, script, string(raw),
	)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

// LoadTasks returns all persisted scheduled tasks.
func (d *DB) LoadTasks() ([]ScheduledTask, error) {
	rows, err := d.conn.Query(
		`SELECT id, schedule, script, allow_list FROM tasks ORDER BY id ASC`,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var tasks []ScheduledTask
	for rows.Next() {
		var t ScheduledTask
		var allowRaw string
		if err := rows.Scan(&t.ID, &t.Schedule, &t.Script, &allowRaw); err != nil {
			return nil, err
		}
		_ = json.Unmarshal([]byte(allowRaw), &t.Allow)
		tasks = append(tasks, t)
	}
	return tasks, rows.Err()
}

// DeleteTask removes a persisted task by its ID.
func (d *DB) DeleteTask(id int64) error {
	_, err := d.conn.Exec(`DELETE FROM tasks WHERE id = ?`, id)
	return err
}

// Close closes the underlying SQLite connection.
func (d *DB) Close() error {
	return d.conn.Close()
}
