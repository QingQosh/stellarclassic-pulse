import re

with open('src/handlers.rs', 'r') as f:
    c = f.read()

# For get_events
c = re.sub(
    r'(path = "/v1/events",\s*tag = "events",\s*params\([^)]*\)\s*,\s*responses\()',
    r'path = "/v1/events",\n    tag = "events",\n    params(\n        ("If-None-Match" = Option<String>, Header, description = "Conditional GET: Return 304 if ETag matches")\n    ),\n    responses(\n        (status = 304, description = "Not Modified (ETag matched)"),\n',
    c, flags=re.MULTILINE
)
# Note: get_events already has a lot of params! My regex above overwrote them all. 
