#!/usr/bin/env python3
"""Local OpenAI-shape -> Azure OpenAI embeddings translation proxy.

codescout's EmbedderHttp POSTs {"input": [...], "model": "..."} to
{base}/v1/embeddings with Bearer auth. Azure OpenAI wants
{deployment-url}?api-version=... with an `api-key` header instead. This
tiny stdlib-only server bridges the two so codescout can point
CODESCOUT_EMBEDDER_URL at http://127.0.0.1:PORT.
"""
import json
import os
import re
import sys
import time
import urllib.request
import urllib.error
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

AZURE_ENDPOINT = os.environ["AZURE_OPENAI_ENDPOINT"].rstrip("/")
AZURE_API_KEY = os.environ["AZURE_OPENAI_API_KEY"]
AZURE_DEPLOYMENT = os.environ.get("AZURE_EMBED_DEPLOYMENT", "text-embedding-ada-002")
AZURE_API_VERSION = os.environ.get("AZURE_API_VERSION", "2023-05-15")
PORT = int(os.environ.get("PROXY_PORT", "8091"))

AZURE_URL = f"{AZURE_ENDPOINT}/openai/deployments/{AZURE_DEPLOYMENT}/embeddings?api-version={AZURE_API_VERSION}"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        sys.stderr.write("proxy: " + (fmt % args) + "\n")

    def do_POST(self):
        if not self.path.startswith("/v1/embeddings"):
            self.send_response(404)
            self.end_headers()
            return
        length = int(self.headers.get("Content-Length", 0))
        raw = self.rfile.read(length)
        try:
            payload = json.loads(raw)
        except Exception as e:
            self._error(400, f"bad request json: {e}")
            return
        azure_body = {"input": payload.get("input")}
        req_data = json.dumps(azure_body).encode("utf-8")
        max_retries = 6
        for attempt in range(max_retries):
            req = urllib.request.Request(
                AZURE_URL,
                data=req_data,
                headers={"Content-Type": "application/json", "api-key": AZURE_API_KEY},
                method="POST",
            )
            try:
                with urllib.request.urlopen(req, timeout=60) as resp:
                    body = resp.read()
                    self.send_response(resp.status)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(body)
                    return
            except urllib.error.HTTPError as e:
                body = e.read()
                if e.code == 429 and attempt < max_retries - 1:
                    retry_after = 7.0
                    m = re.search(rb"retry after (\d+(?:\.\d+)?) seconds", body)
                    if m:
                        retry_after = float(m.group(1))
                    sys.stderr.write(
                        f"proxy: 429 rate limited, retry {attempt+1}/{max_retries} after {retry_after}s\n"
                    )
                    time.sleep(retry_after + 0.5)
                    continue
                sys.stderr.write(f"proxy: azure HTTPError {e.code}: input_count={len(payload.get('input') or [])} body={body[:2000]!r}\n")
                self.send_response(e.code)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(body)
                return
            except Exception as e:
                sys.stderr.write(f"proxy: upstream exception: {e!r}\n")
                self._error(502, f"upstream error: {e}")
                return

    def _error(self, code, msg):
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps({"error": msg}).encode("utf-8"))


if __name__ == "__main__":
    server = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    print(f"Azure embeddings proxy listening on 127.0.0.1:{PORT} -> {AZURE_URL}", flush=True)
    server.serve_forever()
