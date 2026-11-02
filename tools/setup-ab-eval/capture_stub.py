"""Capture-only Anthropic API stub for the zero-cost request control (controller ruling C2b).

Runs in a `sab-capture` container on the study's --internal network (no route out). Every
request is appended as one JSON line to /capture/requests.jsonl: method, path, the request
headers minus any key or authorization, and the parsed body. POST /v1/messages gets an HTTP 400
`invalid_request_error` (a 4xx, so the CLI does not retry); every other path gets 404. Nothing
is forwarded anywhere and no model is ever called. Stdlib only.

    python3 capture_stub.py [port]
"""
import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

OUT = "/capture/requests.jsonl"
SECRET_HEADERS = {"x-api-key", "authorization", "proxy-authorization", "cookie"}


class Stub(BaseHTTPRequestHandler):
    def _record(self, body):
        try:
            parsed = json.loads(body) if body else None
        except ValueError:
            parsed = {"unparsed": body.decode("utf-8", "replace")}
        headers = {k: v for k, v in self.headers.items() if k.lower() not in SECRET_HEADERS}
        with open(OUT, "a") as f:
            f.write(json.dumps({"method": self.command, "path": self.path, "headers": headers,
                                "body": parsed}) + "\n")

    def _reply(self, code, payload):
        data = json.dumps(payload).encode()
        self.send_response(code)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        body = self.rfile.read(int(self.headers.get("content-length") or 0))
        self._record(body)
        if self.path.split("?")[0] == "/v1/messages":
            self._reply(400, {"type": "error", "error": {
                "type": "invalid_request_error",
                "message": "sab capture stub: request recorded, no model behind this endpoint"}})
        else:
            self._reply(404, {"type": "error", "error": {"type": "not_found_error",
                                                         "message": "sab capture stub"}})

    def do_GET(self):
        self._record(b"")
        self._reply(404, {"type": "error", "error": {"type": "not_found_error",
                                                     "message": "sab capture stub"}})

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    ThreadingHTTPServer(("0.0.0.0", port), Stub).serve_forever()
