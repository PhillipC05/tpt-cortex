package ipc

// Inbound JSON-RPC 2.0 request from the PWA.
type Request struct {
	JSONRPC string          `json:"jsonrpc"`
	Method  string          `json:"method"`
	Params  ExecuteParams   `json:"params"`
	ID      int             `json:"id"`
	Token   string          `json:"token"`
}

// Parameters for the ExecuteCortex method.
type ExecuteParams struct {
	Script string   `json:"script"`
	Allow  []string `json:"allow"`
}

// Outbound JSON-RPC 2.0 response.
type Response struct {
	JSONRPC string       `json:"jsonrpc"`
	ID      int          `json:"id"`
	Result  *ExecResult  `json:"result,omitempty"`
	Error   *RPCError    `json:"error,omitempty"`
}

type ExecResult struct {
	Logs []string `json:"logs"`
}

type RPCError struct {
	Code    int    `json:"code"`
	Message string `json:"message"`
}

// First message sent to every new client.
type ConnectedMsg struct {
	Type  string `json:"type"`
	Token string `json:"token"`
}

// Response type for the HTTP /execute endpoint.
type HTTPExecuteResponse struct {
	Logs  []string `json:"logs,omitempty"`
	Error string   `json:"error,omitempty"`
}
