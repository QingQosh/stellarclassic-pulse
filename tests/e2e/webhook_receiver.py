#!/usr/bin/env python3
"""
webhook_receiver.py — tiny Flask server that records inbound webhook POSTs.

Endpoints
---------
POST /webhook          Accepts any JSON body; stores it in memory.
GET  /received         Returns list of all received payloads (JSON array).
DELETE /received       Clears all stored payloads (used between test cases).
GET  /health           Health-check endpoint.
"""

import json
import threading
from flask import Flask, request, jsonify

app = Flask(__name__)
_lock = threading.Lock()
_received: list[dict] = []


@app.post("/webhook")
def receive_webhook():
    payload = request.get_json(force=True, silent=True) or {}
    headers = dict(request.headers)
    with _lock:
        _received.append({"headers": headers, "payload": payload})
    return jsonify({"status": "ok"}), 200


@app.get("/received")
def get_received():
    with _lock:
        return jsonify(_received), 200


@app.delete("/received")
def clear_received():
    with _lock:
        _received.clear()
    return jsonify({"status": "cleared"}), 200


@app.get("/health")
def health():
    return jsonify({"status": "ok"}), 200


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=9001)
