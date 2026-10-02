#!/usr/bin/env python3
"""Local stdio MCP fixture: tools/resources, with no network or user data."""
import json
import os
import sys

for line in sys.stdin:
    request = json.loads(line)
    method = request.get("method")
    trace = os.environ.get("PI_FIXTURE_MCP_LOG")
    if trace:
        with open(trace, "a") as file:
            file.write(method + "\n")
    if "id" not in request:
        continue
    if method == "initialize":
        result = {"protocolVersion": request["params"]["protocolVersion"],
                  "capabilities": {"tools": {}, "resources": {}},
                  "serverInfo": {"name": "local-fixture", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": name, "description": "Local fixture " + name,
                              "inputSchema": {"type": "object", "properties": {"value": {"type": "string"}}, "required": ["value"]}}
                             for name in ["echo", "hidden"]]}
    elif method == "tools/call":
        assert request["params"]["name"] == "echo", "hidden tool was called"
        result = {"content": [{"type": "text", "text": "local-mcp:" + request["params"]["arguments"]["value"]}],
                  "structuredContent": {"value": request["params"]["arguments"]["value"]}}
    elif method == "resources/list":
        result = {"resources": [{"uri": "fixture://document", "name": "Local document", "mimeType": "text/plain"}]}
    elif method == "resources/templates/list":
        result = {"resourceTemplates": []}
    elif method == "resources/read":
        result = {"contents": [{"uri": "fixture://document", "mimeType": "text/plain", "text": "local resource"}]}
    elif method == "ping":
        result = {}
    else:
        print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "error": {"code": -32601, "message": method}}), flush=True)
        continue
    print(json.dumps({"jsonrpc": "2.0", "id": request["id"], "result": result}), flush=True)
