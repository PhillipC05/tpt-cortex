package ipc

import (
	"encoding/json"
	"log"
	"net/http"
	"strings"
	"sync"

	"github.com/gorilla/websocket"
	"github.com/tpt-cortex/cortex-daemon/db"
	"github.com/tpt-cortex/cortex-daemon/interpreter"
	"github.com/tpt-cortex/cortex-daemon/manifest"
	"github.com/tpt-cortex/cortex-daemon/registry"
	"github.com/tpt-cortex/cortex-daemon/scheduler"
)

var upgrader = websocket.Upgrader{
	CheckOrigin: func(r *http.Request) bool {
		origin := r.Header.Get("Origin")
		return origin == "" ||
			strings.HasPrefix(origin, "http://localhost") ||
			strings.HasPrefix(origin, "http://127.0.0.1") ||
			strings.HasPrefix(origin, "https://localhost")
	},
}

// Server is the WebSocket JSON-RPC server.
type Server struct {
	token     string
	cortexBin string
	db        *db.DB
	manifest  *manifest.Manifest
	sched     *scheduler.Scheduler
	mu        sync.Mutex
}

// NewServer creates a server with the given dependencies.
// db, m, and sched may be nil.
func NewServer(token, cortexBin string, d *db.DB, m *manifest.Manifest, sched *scheduler.Scheduler) *Server {
	return &Server{
		token:     token,
		cortexBin: cortexBin,
		db:        d,
		manifest:  m,
		sched:     sched,
	}
}

// ListenAndServe starts the HTTP + WebSocket server on addr.
func (s *Server) ListenAndServe(addr string) error {
	http.HandleFunc("/ws", s.handleWS)
	http.HandleFunc("/execute", s.handleHTTPExecute)
	http.HandleFunc("/", s.handleWS) // backward compat — "/" still upgrades to WS
	log.Printf("cortex-daemon listening on ws://%s", addr)
	return http.ListenAndServe(addr, nil)
}

func (s *Server) handleWS(w http.ResponseWriter, r *http.Request) {
	conn, err := upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("upgrade error: %v", err)
		return
	}
	defer conn.Close()

	origin := r.Header.Get("Origin")
	log.Printf("client connected: %s (origin: %q)", r.RemoteAddr, origin)

	hello, _ := json.Marshal(ConnectedMsg{Type: "connected", Token: s.token})
	if err := conn.WriteMessage(websocket.TextMessage, hello); err != nil {
		return
	}

	for {
		_, msg, err := conn.ReadMessage()
		if err != nil {
			log.Printf("client disconnected: %v", err)
			return
		}

		var req Request
		if err := json.Unmarshal(msg, &req); err != nil {
			s.writeError(conn, 0, -32700, "parse error: "+err.Error())
			continue
		}

		if req.Token != s.token {
			s.writeError(conn, req.ID, -32001, "invalid token")
			continue
		}

		if req.Method != "ExecuteCortex" {
			s.writeError(conn, req.ID, -32601, "unknown method: "+req.Method)
			continue
		}

		logs, execErr := s.execute(req.Params, origin)

		var resp Response
		resp.JSONRPC = "2.0"
		resp.ID = req.ID
		if execErr != nil {
			resp.Error = &RPCError{Code: -32000, Message: execErr.Error()}
		} else {
			resp.Result = &ExecResult{Logs: logs}
		}

		data, _ := json.Marshal(resp)
		conn.WriteMessage(websocket.TextMessage, data)
	}
}

func (s *Server) handleHTTPExecute(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Access-Control-Allow-Origin", "*")
	w.Header().Set("Access-Control-Allow-Headers", "Authorization, Content-Type")
	w.Header().Set("Content-Type", "application/json")

	if r.Method == http.MethodOptions {
		w.WriteHeader(http.StatusOK)
		return
	}

	if r.Method != http.MethodPost {
		w.WriteHeader(http.StatusMethodNotAllowed)
		json.NewEncoder(w).Encode(HTTPExecuteResponse{Error: "method not allowed"})
		return
	}

	authHeader := r.Header.Get("Authorization")
	expectedBearer := "Bearer " + s.token
	if authHeader != expectedBearer {
		w.WriteHeader(http.StatusUnauthorized)
		json.NewEncoder(w).Encode(HTTPExecuteResponse{Error: "unauthorized"})
		return
	}

	var params ExecuteParams
	if err := json.NewDecoder(r.Body).Decode(&params); err != nil {
		w.WriteHeader(http.StatusBadRequest)
		json.NewEncoder(w).Encode(HTTPExecuteResponse{Error: "invalid JSON: " + err.Error()})
		return
	}

	origin := r.Header.Get("Origin")
	logs, err := s.execute(params, origin)
	if err != nil {
		w.WriteHeader(http.StatusInternalServerError)
		json.NewEncoder(w).Encode(HTTPExecuteResponse{Error: err.Error()})
		return
	}

	json.NewEncoder(w).Encode(HTTPExecuteResponse{Logs: logs})
}

func (s *Server) execute(params ExecuteParams, origin string) ([]string, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	reg := registry.NewDefaultRegistry(s.db, s.manifest, origin)

	// Wire the scheduler callback so native.schedule.add works from scripts.
	if s.sched != nil {
		reg.OnScheduleAdd = func(schedule, script string, allow []string) (int64, error) {
			var dbID int64
			if s.db != nil {
				var err error
				dbID, err = s.db.SaveTask(schedule, script, allow)
				if err != nil {
					return 0, err
				}
			}
			_, err := s.sched.Add(dbID, schedule, script, allow, func() {
				bgReg := registry.NewDefaultRegistry(s.db, s.manifest, "scheduler")
				if _, err := interpreter.Execute(script, allow, s.cortexBin, bgReg); err != nil {
					log.Printf("scheduled task error: %v", err)
				}
			})
			return dbID, err
		}
	}

	logs, err := interpreter.Execute(params.Script, params.Allow, s.cortexBin, reg)
	if err != nil {
		return nil, err
	}
	return append(reg.Logs, logs...), nil
}

func (s *Server) writeError(conn *websocket.Conn, id, code int, msg string) {
	resp := Response{
		JSONRPC: "2.0",
		ID:      id,
		Error:   &RPCError{Code: code, Message: msg},
	}
	data, _ := json.Marshal(resp)
	conn.WriteMessage(websocket.TextMessage, data)
}
