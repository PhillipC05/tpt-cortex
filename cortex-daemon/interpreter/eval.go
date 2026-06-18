// Package interpreter walks the JSON AST produced by `cortex compile --emit=ast`
// and executes it using a Go-native NativeRegistry.
package interpreter

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"strings"

	"github.com/tpt-cortex/cortex-daemon/registry"
)

// Execute compiles and evaluates a Cortex script.
// cortexBin is the path to the `cortex` binary.
// Returns the captured log lines from native.log calls.
func Execute(script string, allow []string, cortexBin string, reg registry.NativeRegistry) ([]string, error) {
	// Write script to a temp file for compilation
	tmp, err := os.CreateTemp("", "cortex-*.ctx")
	if err != nil {
		return nil, fmt.Errorf("failed to create temp file: %w", err)
	}
	defer os.Remove(tmp.Name())
	if _, err := tmp.WriteString(script); err != nil {
		return nil, err
	}
	tmp.Close()

	// Build cortex compile args
	args := []string{"compile", tmp.Name(), "--emit=ast"}
	if len(allow) > 0 {
		args = append(args, "--allow="+strings.Join(allow, ","))
	}

	// Run `cortex compile` to get the JSON AST
	cmd := exec.Command(cortexBin, args...)
	out, err := cmd.Output()
	if err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			return nil, fmt.Errorf("%s", strings.TrimSpace(string(exitErr.Stderr)))
		}
		return nil, fmt.Errorf("cortex compile failed: %w", err)
	}

	// Parse the JSON AST
	var program map[string]interface{}
	if err := json.Unmarshal(out, &program); err != nil {
		return nil, fmt.Errorf("failed to parse AST: %w", err)
	}

	tasks, _ := program["tasks"].([]interface{})
	if len(tasks) == 0 {
		return nil, fmt.Errorf("no tasks found in script")
	}

	task, _ := tasks[0].(map[string]interface{})
	body, _ := task["body"].([]interface{})

	scope := newScope(nil)
	if err := execBlock(body, scope, reg); err != nil {
		return nil, err
	}

	// Surface logs if the registry supports it
	if dr, ok := reg.(interface{ GetLogs() []string }); ok {
		return dr.GetLogs(), nil
	}
	return nil, nil
}

// ── Scope ─────────────────────────────────────────────────────────────────────

type scope struct {
	vars   map[string]registry.Value
	parent *scope
}

func newScope(parent *scope) *scope {
	return &scope{vars: make(map[string]registry.Value), parent: parent}
}

func (s *scope) set(name string, v registry.Value) {
	s.vars[name] = v
}

func (s *scope) get(name string) (registry.Value, bool) {
	if v, ok := s.vars[name]; ok {
		return v, true
	}
	if s.parent != nil {
		return s.parent.get(name)
	}
	return registry.VoidVal(), false
}

// ── Statement execution ───────────────────────────────────────────────────────

// execBlock returns an error; a non-nil returnVal signals a return statement.
func execBlock(stmts []interface{}, sc *scope, reg registry.NativeRegistry) error {
	child := newScope(sc)
	for _, raw := range stmts {
		stmt, _ := raw.(map[string]interface{})
		returned, err := execStmt(stmt, child, reg)
		if err != nil {
			return err
		}
		if returned {
			return nil
		}
	}
	return nil
}

// execStmt returns (returned bool, error). returned=true means a `return` was hit.
func execStmt(stmt map[string]interface{}, sc *scope, reg registry.NativeRegistry) (bool, error) {
	tag, _ := stmt["stmt"].(string)
	switch tag {

	case "Let":
		name, _ := stmt["name"].(map[string]interface{})["name"].(string)
		valRaw, _ := stmt["value"].(map[string]interface{})
		val, err := evalExpr(valRaw, sc, reg)
		if err != nil {
			return false, err
		}
		sc.set(name, val)

	case "Expr":
		exprRaw, _ := stmt["expr"].(map[string]interface{})
		_, err := evalExpr(exprRaw, sc, reg)
		if err != nil {
			return false, err
		}

	case "If":
		condRaw, _ := stmt["condition"].(map[string]interface{})
		cond, err := evalExpr(condRaw, sc, reg)
		if err != nil {
			return false, err
		}
		if cond.Bool {
			thenBlock, _ := stmt["then_block"].([]interface{})
			if err := execBlock(thenBlock, sc, reg); err != nil {
				return false, err
			}
		} else if elseBranch, ok := stmt["else_branch"].(map[string]interface{}); ok {
			kind, _ := elseBranch["kind"].(string)
			switch kind {
			case "Block":
				blkRaw, _ := elseBranch["Block"].([]interface{})
				if err := execBlock(blkRaw, sc, reg); err != nil {
					return false, err
				}
			case "ElseIf":
				nested, _ := elseBranch["ElseIf"].(map[string]interface{})
				if _, err := execStmt(nested, sc, reg); err != nil {
					return false, err
				}
			}
		}

	case "Return":
		// Return value is ignored for void tasks in Phase 3
		return true, nil
	}

	return false, nil
}

// ── Expression evaluation ─────────────────────────────────────────────────────

func evalExpr(expr map[string]interface{}, sc *scope, reg registry.NativeRegistry) (registry.Value, error) {
	tag, _ := expr["expr"].(string)
	switch tag {

	case "Literal":
		kind, _ := expr["kind"].(map[string]interface{})
		litType, _ := kind["type"].(string)
		switch litType {
		case "Str":
			return registry.StrVal(kind["value"].(string)), nil
		case "Int":
			// JSON numbers come as float64
			n := int32(kind["value"].(float64))
			return registry.I32Val(n), nil
		case "Float":
			return registry.Value{Kind: "f64", F64: kind["value"].(float64)}, nil
		case "Bool":
			return registry.BoolVal(kind["value"].(bool)), nil
		}

	case "Ident":
		name, _ := expr["name"].(string)
		if v, ok := sc.get(name); ok {
			return v, nil
		}
		return registry.VoidVal(), fmt.Errorf("undefined variable: %s", name)

	case "NativeCall":
		pathRaw, _ := expr["path"].([]interface{})
		path := make([]string, len(pathRaw))
		for i, p := range pathRaw {
			path[i], _ = p.(string)
		}
		api := "native." + strings.Join(path, ".")

		argsRaw, _ := expr["args"].([]interface{})
		args := make([]registry.Value, len(argsRaw))
		for i, raw := range argsRaw {
			argExpr, _ := raw.(map[string]interface{})
			val, err := evalExpr(argExpr, sc, reg)
			if err != nil {
				return registry.VoidVal(), err
			}
			args[i] = val
		}
		return reg.Call(api, args)

	case "Binary":
		leftExpr, _ := expr["left"].(map[string]interface{})
		rightExpr, _ := expr["right"].(map[string]interface{})
		op, _ := expr["op"].(string)

		left, err := evalExpr(leftExpr, sc, reg)
		if err != nil {
			return registry.VoidVal(), err
		}
		right, err := evalExpr(rightExpr, sc, reg)
		if err != nil {
			return registry.VoidVal(), err
		}
		return evalBinary(op, left, right)

	case "Unary":
		operandExpr, _ := expr["operand"].(map[string]interface{})
		op, _ := expr["op"].(string)
		operand, err := evalExpr(operandExpr, sc, reg)
		if err != nil {
			return registry.VoidVal(), err
		}
		switch op {
		case "Not":
			return registry.BoolVal(!operand.Bool), nil
		case "Neg":
			if operand.Kind == "i32" {
				return registry.I32Val(-operand.I32), nil
			}
			return registry.Value{Kind: "f64", F64: -operand.F64}, nil
		}
	}

	return registry.VoidVal(), nil
}

func evalBinary(op string, l, r registry.Value) (registry.Value, error) {
	switch op {
	case "Add":
		if l.Kind == "string" {
			return registry.StrVal(l.Str + r.Str), nil
		}
		if l.Kind == "i32" {
			return registry.I32Val(l.I32 + r.I32), nil
		}
		return registry.Value{Kind: "f64", F64: l.F64 + r.F64}, nil
	case "Sub":
		if l.Kind == "i32" {
			return registry.I32Val(l.I32 - r.I32), nil
		}
		return registry.Value{Kind: "f64", F64: l.F64 - r.F64}, nil
	case "Mul":
		if l.Kind == "i32" {
			return registry.I32Val(l.I32 * r.I32), nil
		}
		return registry.Value{Kind: "f64", F64: l.F64 * r.F64}, nil
	case "Div":
		if l.Kind == "i32" {
			if r.I32 == 0 {
				return registry.VoidVal(), fmt.Errorf("division by zero")
			}
			return registry.I32Val(l.I32 / r.I32), nil
		}
		return registry.Value{Kind: "f64", F64: l.F64 / r.F64}, nil
	case "Eq":
		return registry.BoolVal(l.String() == r.String()), nil
	case "NotEq":
		return registry.BoolVal(l.String() != r.String()), nil
	case "Lt":
		if l.Kind == "i32" {
			return registry.BoolVal(l.I32 < r.I32), nil
		}
		return registry.BoolVal(l.F64 < r.F64), nil
	case "Gt":
		if l.Kind == "i32" {
			return registry.BoolVal(l.I32 > r.I32), nil
		}
		return registry.BoolVal(l.F64 > r.F64), nil
	case "And":
		return registry.BoolVal(l.Bool && r.Bool), nil
	case "Or":
		return registry.BoolVal(l.Bool || r.Bool), nil
	}
	return registry.VoidVal(), fmt.Errorf("unknown binary op: %s", op)
}
