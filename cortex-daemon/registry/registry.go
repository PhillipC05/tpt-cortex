package registry

import "fmt"

// Value is a runtime value produced or consumed by a native call.
type Value struct {
	Kind string // "string" | "i32" | "f64" | "bool" | "void"
	Str  string
	I32  int32
	F64  float64
	Bool bool
}

func StrVal(s string) Value { return Value{Kind: "string", Str: s} }
func I32Val(n int32) Value  { return Value{Kind: "i32", I32: n} }
func BoolVal(b bool) Value  { return Value{Kind: "bool", Bool: b} }
func VoidVal() Value        { return Value{Kind: "void"} }

func (v Value) String() string {
	switch v.Kind {
	case "string":
		return v.Str
	case "i32":
		return fmt.Sprintf("%d", v.I32)
	case "f64":
		return fmt.Sprintf("%g", v.F64)
	case "bool":
		if v.Bool {
			return "true"
		}
		return "false"
	default:
		return ""
	}
}

// NativeRegistry dispatches qualified API names to OS implementations.
type NativeRegistry interface {
	Call(api string, args []Value) (Value, error)
}
