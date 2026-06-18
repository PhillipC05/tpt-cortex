package registry

import (
	"strings"
	"testing"

	"github.com/tpt-cortex/cortex-daemon/manifest"
)

// ── Value helpers ─────────────────────────────────────────────────────────────

func TestValueHelpers(t *testing.T) {
	s := StrVal("hello")
	if s.Kind != "string" || s.Str != "hello" {
		t.Errorf("StrVal: got %+v", s)
	}
	if s.String() != "hello" {
		t.Errorf("StrVal.String(): got %q", s.String())
	}

	n := I32Val(42)
	if n.Kind != "i32" || n.I32 != 42 {
		t.Errorf("I32Val: got %+v", n)
	}
	if n.String() != "42" {
		t.Errorf("I32Val.String(): got %q", n.String())
	}

	b := BoolVal(true)
	if b.Kind != "bool" || !b.Bool {
		t.Errorf("BoolVal: got %+v", b)
	}
	if b.String() != "true" {
		t.Errorf("BoolVal.String(): got %q", b.String())
	}

	bf := BoolVal(false)
	if bf.String() != "false" {
		t.Errorf("BoolVal(false).String(): got %q", bf.String())
	}

	v := VoidVal()
	if v.Kind != "void" {
		t.Errorf("VoidVal: got %+v", v)
	}
	if v.String() != "" {
		t.Errorf("VoidVal.String(): want empty, got %q", v.String())
	}
}

// ── native.log ────────────────────────────────────────────────────────────────

func TestNativeLogSingleArg(t *testing.T) {
	reg := NewDefaultRegistry(nil, nil, "http://localhost")
	if _, err := reg.Call("native.log", []Value{StrVal("hello")}); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if len(reg.Logs) != 1 || reg.Logs[0] != "hello" {
		t.Errorf("logs: got %v", reg.Logs)
	}
}

func TestNativeLogMultipleArgs(t *testing.T) {
	reg := NewDefaultRegistry(nil, nil, "")
	reg.Call("native.log", []Value{StrVal("a"), StrVal("b"), I32Val(3)})
	if len(reg.Logs) != 1 || reg.Logs[0] != "a b 3" {
		t.Errorf("expected 'a b 3', got %v", reg.Logs)
	}
}

func TestNativeLogAccumulates(t *testing.T) {
	reg := NewDefaultRegistry(nil, nil, "")
	reg.Call("native.log", []Value{StrVal("first")})
	reg.Call("native.log", []Value{StrVal("second")})
	if len(reg.Logs) != 2 {
		t.Fatalf("expected 2 log entries, got %d", len(reg.Logs))
	}
	if reg.Logs[0] != "first" || reg.Logs[1] != "second" {
		t.Errorf("log order wrong: %v", reg.Logs)
	}
}

// ── Permission manifest enforcement ──────────────────────────────────────────

func TestPermissionAllowed(t *testing.T) {
	m := &manifest.Manifest{
		Apps: map[string]manifest.AppDef{
			"http://localhost": {Allow: []string{"native.log", "native.fs.read"}},
		},
	}
	reg := NewDefaultRegistry(nil, m, "http://localhost")
	if _, err := reg.Call("native.log", []Value{StrVal("ok")}); err != nil {
		t.Errorf("expected allowed, got error: %v", err)
	}
}

func TestPermissionDenied(t *testing.T) {
	m := &manifest.Manifest{
		Apps: map[string]manifest.AppDef{
			"http://localhost": {Allow: []string{"native.log"}},
		},
	}
	reg := NewDefaultRegistry(nil, m, "http://localhost")
	_, err := reg.Call("native.fs.read", []Value{StrVal("/etc/passwd")})
	if err == nil {
		t.Fatal("expected permission denied error")
	}
	if !strings.Contains(err.Error(), "permission denied") {
		t.Errorf("error should mention 'permission denied': %v", err)
	}
}

func TestPermissionDeniedUnknownOrigin(t *testing.T) {
	m := &manifest.Manifest{
		Apps: map[string]manifest.AppDef{
			"http://localhost:5173": {Allow: []string{"native.log"}},
		},
	}
	reg := NewDefaultRegistry(nil, m, "http://evil.example.com")
	_, err := reg.Call("native.log", []Value{StrVal("attack")})
	if err == nil {
		t.Fatal("expected permission denied for unknown origin")
	}
}

func TestNilManifestAllowsAll(t *testing.T) {
	reg := NewDefaultRegistry(nil, nil, "http://any-origin.example.com")
	if _, err := reg.Call("native.log", []Value{StrVal("test")}); err != nil {
		t.Errorf("nil manifest should allow all calls: %v", err)
	}
}

// ── Unknown API ───────────────────────────────────────────────────────────────

func TestUnknownAPIReturnsError(t *testing.T) {
	reg := NewDefaultRegistry(nil, nil, "")
	_, err := reg.Call("native.nonexistent.api", []Value{})
	if err == nil {
		t.Fatal("expected error for unrecognised API")
	}
}

// ── native.fs.read ────────────────────────────────────────────────────────────

func TestNativeFsReadMissingArg(t *testing.T) {
	reg := NewDefaultRegistry(nil, nil, "")
	_, err := reg.Call("native.fs.read", []Value{})
	if err == nil {
		t.Fatal("expected error when path arg is missing")
	}
	if !strings.Contains(err.Error(), "1 argument") {
		t.Errorf("error should mention missing argument: %v", err)
	}
}

func TestNativeFsReadNonexistentFile(t *testing.T) {
	reg := NewDefaultRegistry(nil, nil, "")
	_, err := reg.Call("native.fs.read", []Value{StrVal("/this/file/does/not/exist/cortex-test")})
	if err == nil {
		t.Fatal("expected error for missing file")
	}
}
