package ipc

import (
	"encoding/json"
	"testing"
)

func TestRequestParsesFully(t *testing.T) {
	raw := `{
		"jsonrpc": "2.0",
		"method":  "ExecuteCortex",
		"params":  {"script": "task main() -> void {}", "allow": ["native.log", "native.fs.read"]},
		"id":      7,
		"token":   "secret-tok"
	}`
	var req Request
	if err := json.Unmarshal([]byte(raw), &req); err != nil {
		t.Fatalf("unmarshal error: %v", err)
	}
	if req.Method != "ExecuteCortex" {
		t.Errorf("method: want ExecuteCortex, got %q", req.Method)
	}
	if req.ID != 7 {
		t.Errorf("id: want 7, got %d", req.ID)
	}
	if req.Token != "secret-tok" {
		t.Errorf("token: want %q, got %q", "secret-tok", req.Token)
	}
	if req.Params.Script != "task main() -> void {}" {
		t.Errorf("script: got %q", req.Params.Script)
	}
	if len(req.Params.Allow) != 2 || req.Params.Allow[0] != "native.log" {
		t.Errorf("allow: got %v", req.Params.Allow)
	}
}

func TestRequestEmptyAllow(t *testing.T) {
	raw := `{"method":"ExecuteCortex","params":{"script":"x"},"id":1,"token":"t"}`
	var req Request
	if err := json.Unmarshal([]byte(raw), &req); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if req.Params.Allow != nil && len(req.Params.Allow) != 0 {
		t.Errorf("expected nil/empty allow, got %v", req.Params.Allow)
	}
}

func TestSuccessResponseOmitsError(t *testing.T) {
	resp := Response{
		JSONRPC: "2.0",
		ID:      42,
		Result:  &ExecResult{Logs: []string{"line1", "line2"}},
	}
	data, err := json.Marshal(resp)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var m map[string]interface{}
	if err := json.Unmarshal(data, &m); err != nil {
		t.Fatalf("re-unmarshal: %v", err)
	}
	if _, ok := m["error"]; ok {
		t.Error("error field should be omitted when nil")
	}
	result, ok := m["result"].(map[string]interface{})
	if !ok {
		t.Fatalf("result missing or wrong type")
	}
	logs, ok := result["logs"].([]interface{})
	if !ok || len(logs) != 2 {
		t.Errorf("expected 2 logs, got %v", result["logs"])
	}
}

func TestErrorResponseOmitsResult(t *testing.T) {
	resp := Response{
		JSONRPC: "2.0",
		ID:      1,
		Error:   &RPCError{Code: -32001, Message: "invalid token"},
	}
	data, _ := json.Marshal(resp)
	var m map[string]interface{}
	json.Unmarshal(data, &m)

	if _, ok := m["result"]; ok {
		t.Error("result field should be omitted when nil")
	}
	rpcErr, ok := m["error"].(map[string]interface{})
	if !ok {
		t.Fatal("error field missing")
	}
	if int(rpcErr["code"].(float64)) != -32001 {
		t.Errorf("code: want -32001, got %v", rpcErr["code"])
	}
	if rpcErr["message"] != "invalid token" {
		t.Errorf("message: got %v", rpcErr["message"])
	}
}

func TestConnectedMsgSerializes(t *testing.T) {
	msg := ConnectedMsg{Type: "connected", Token: "tok-abc-123"}
	data, err := json.Marshal(msg)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var m map[string]string
	if err := json.Unmarshal(data, &m); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if m["type"] != "connected" {
		t.Errorf("type: want connected, got %q", m["type"])
	}
	if m["token"] != "tok-abc-123" {
		t.Errorf("token: got %q", m["token"])
	}
}

func TestHTTPExecuteResponseLogsOnly(t *testing.T) {
	resp := HTTPExecuteResponse{Logs: []string{"hello", "world"}}
	data, _ := json.Marshal(resp)
	var m map[string]interface{}
	json.Unmarshal(data, &m)

	if _, ok := m["error"]; ok {
		t.Error("error should be omitted when empty string")
	}
	logs := m["logs"].([]interface{})
	if len(logs) != 2 {
		t.Errorf("expected 2 logs, got %d", len(logs))
	}
}

func TestHTTPExecuteResponseErrorOnly(t *testing.T) {
	resp := HTTPExecuteResponse{Error: "something failed"}
	data, _ := json.Marshal(resp)
	var m map[string]interface{}
	json.Unmarshal(data, &m)

	if _, ok := m["logs"]; ok {
		t.Error("logs should be omitted when nil")
	}
	if m["error"] != "something failed" {
		t.Errorf("error: got %v", m["error"])
	}
}

func TestExecuteParamsRoundTrip(t *testing.T) {
	params := ExecuteParams{
		Script: `task run() -> void { native.log("hi"); }`,
		Allow:  []string{"native.log"},
	}
	data, err := json.Marshal(params)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var back ExecuteParams
	if err := json.Unmarshal(data, &back); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if back.Script != params.Script {
		t.Errorf("script mismatch: got %q", back.Script)
	}
	if len(back.Allow) != 1 || back.Allow[0] != "native.log" {
		t.Errorf("allow mismatch: got %v", back.Allow)
	}
}
