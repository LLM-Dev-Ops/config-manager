'use strict';

const assert = require('assert');
const { handler } = require('../index');

// ---------------------------------------------------------------------------
// Minimal mock helpers for Express-style req/res
// ---------------------------------------------------------------------------

function mockReq({ method = 'GET', path = '/', headers = {}, body = undefined } = {}) {
  const normalizedHeaders = { ...headers };
  if (body !== undefined && !Object.keys(normalizedHeaders).some((key) => key.toLowerCase() === 'content-type')) {
    normalizedHeaders['content-type'] = 'application/json';
  }
  return { method, path, headers: normalizedHeaders, body };
}

function mockRes() {
  const res = {
    _status: null,
    _json: null,
    _headers: {},
    _ended: false,
    status(code) { res._status = code; return res; },
    json(obj) { res._json = obj; return res; },
    setHeader(k, v) { res._headers[k] = v; },
    end() { res._ended = true; return res; },
  };
  return res;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    passed++;
    console.log(`  PASS  ${name}`);
  } catch (err) {
    failed++;
    console.log(`  FAIL  ${name}`);
    console.log(`        ${err.message}`);
  }
}

console.log('config-manager-agents Cloud Function tests\n');

// --- CORS ---

test('OPTIONS returns 204 with CORS headers', () => {
  const req = mockReq({ method: 'OPTIONS', path: '/v1/config-manager/validate' });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 204);
  assert.strictEqual(res._ended, true);
  assert.strictEqual(res._headers['Access-Control-Allow-Origin'], '*');
  assert.ok(res._headers['Access-Control-Allow-Methods'].includes('POST'));
});

test('configured CORS allowlist reflects only an exact trusted origin', () => {
  const previous = process.env.ALLOWED_ORIGINS;
  process.env.ALLOWED_ORIGINS = ' https://trusted.example,https://second.example ';
  try {
    const req = mockReq({ method: 'GET', path: '/health', headers: { origin: 'https://second.example' } });
    const res = mockRes();
    handler(req, res);
    assert.strictEqual(res._headers['Access-Control-Allow-Origin'], 'https://second.example');
    assert.strictEqual(res._headers.Vary, 'Origin');
  } finally {
    if (previous === undefined) delete process.env.ALLOWED_ORIGINS;
    else process.env.ALLOWED_ORIGINS = previous;
  }
});

test('configured CORS allowlist rejects an untrusted preflight', () => {
  const previous = process.env.ALLOWED_ORIGINS;
  process.env.ALLOWED_ORIGINS = 'https://trusted.example';
  try {
    const req = mockReq({ method: 'OPTIONS', path: '/v1/config-manager/validate', headers: { origin: 'https://attacker.example' } });
    const res = mockRes();
    handler(req, res);
    assert.strictEqual(res._status, 403);
    assert.strictEqual(res._headers['Access-Control-Allow-Origin'], undefined);
    assert.strictEqual(res._json.error.code, 'CORS_ORIGIN_DENIED');
  } finally {
    if (previous === undefined) delete process.env.ALLOWED_ORIGINS;
    else process.env.ALLOWED_ORIGINS = previous;
  }
});

// --- Health ---

test('GET /health returns healthy status', () => {
  const req = mockReq({ method: 'GET', path: '/health' });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 200);
  assert.strictEqual(res._json.status, 'healthy');
  assert.deepStrictEqual(res._json.agents, ['validate']);
  assert.ok(res._json.execution_metadata);
  assert.ok(res._json.execution_metadata.trace_id);
  assert.strictEqual(res._json.execution_metadata.service, 'config-manager-agents');
  assert.ok(Array.isArray(res._json.layers_executed));
});

test('Health response uses x-correlation-id when provided', () => {
  const req = mockReq({ method: 'GET', path: '/health', headers: { 'x-correlation-id': 'test-trace-123' } });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._json.execution_metadata.trace_id, 'test-trace-123');
});

// --- Validate: success ---

test('POST /v1/config-manager/validate with valid config returns valid=true', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: {
      config: { namespace: 'app/db', key: 'timeout', value: 30 },
    },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 200);
  assert.strictEqual(res._json.success, true);
  assert.strictEqual(res._json.data.valid, true);
  assert.strictEqual(res._json.data.errors.length, 0);
  assert.strictEqual(res._json.data.schema_used, 'llm-config-v1');
  assert.ok(res._json.execution_metadata);
  assert.ok(res._json.layers_executed.some((l) => l.layer === 'CONFIG_MANAGER_VALIDATE'));
});

// --- Validate: missing required field ---

test('POST /v1/config-manager/validate with missing required field returns errors', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: { config: { namespace: 'app/db' } },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 200);
  assert.strictEqual(res._json.data.valid, false);
  assert.ok(res._json.data.errors.some((e) => e.code === 'REQUIRED_FIELD_MISSING'));
});

// --- Validate: pattern mismatch ---

test('POST /v1/config-manager/validate with invalid namespace pattern', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: { config: { namespace: 'INVALID', key: 'k', value: 'v' } },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._json.data.valid, false);
  assert.ok(res._json.data.errors.some((e) => e.code === 'PATTERN_MISMATCH'));
});

// --- Validate: type mismatch ---

test('POST /v1/config-manager/validate with wrong type for namespace', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: { config: { namespace: 123, key: 'k', value: 'v' } },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._json.data.valid, false);
  assert.ok(res._json.data.errors.some((e) => e.code === 'TYPE_MISMATCH'));
});

// --- Validate: strict mode unknown field ---

test('POST /v1/config-manager/validate strict mode warns on unknown fields', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: {
      config: { namespace: 'app/db', key: 'timeout', value: 30, extra: true },
      options: { strict: true },
    },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._json.data.valid, true);
  assert.ok(res._json.data.warnings.some((w) => w.code === 'UNKNOWN_FIELD'));
});

// --- Validate: provider schema ---

test('POST /v1/config-manager/validate with provider-config-v1 schema', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: {
      config: { provider_type: 'vault', endpoint: 'https://vault.example.com' },
      schema: 'provider-config-v1',
    },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._json.data.valid, true);
  assert.strictEqual(res._json.data.schema_used, 'provider-config-v1');
});

// --- Validate: missing config ---

test('POST /v1/config-manager/validate without config returns 400', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: { schema: 'llm-config-v1' },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 400);
  assert.strictEqual(res._json.success, false);
});

test('POST /v1/config-manager/validate rejects non-JSON media types', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    headers: { 'content-type': 'text/plain' },
    body: { config: { namespace: 'app/db', key: 'timeout', value: 30 } },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 415);
  assert.strictEqual(res._json.error.code, 'UNSUPPORTED_MEDIA_TYPE');
});

test('POST /v1/config-manager/validate rejects payloads over 64 KiB', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    headers: { 'content-type': 'application/json', 'content-length': String(65 * 1024) },
    body: { config: { namespace: 'app/db', key: 'value', value: 'small' } },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 413);
  assert.strictEqual(res._json.error.code, 'PAYLOAD_TOO_LARGE');
});

test('POST /v1/config-manager/validate cannot bypass the limit with a false content length', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    headers: { 'content-type': 'application/json', 'content-length': '10' },
    body: { config: { namespace: 'app/db', key: 'value', value: 'x'.repeat(65 * 1024) } },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 413);
  assert.strictEqual(res._json.error.code, 'PAYLOAD_TOO_LARGE');
});

for (const config of ['secret', [], 42]) {
  test(`POST /v1/config-manager/validate rejects non-object config ${JSON.stringify(config)}`, () => {
    const req = mockReq({ method: 'POST', path: '/v1/config-manager/validate', body: { config } });
    const res = mockRes();
    handler(req, res);
    assert.strictEqual(res._status, 400);
    assert.strictEqual(res._json.error.code, 'BAD_REQUEST');
  });
}

test('POST /v1/config-manager/validate rejects non-string schema identifiers', () => {
  const req = mockReq({ method: 'POST', path: '/v1/config-manager/validate', body: { config: {}, schema: {} } });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 400);
  assert.strictEqual(res._json.error.code, 'BAD_REQUEST');
});

test('POST /v1/config-manager/validate rejects non-object options', () => {
  const req = mockReq({ method: 'POST', path: '/v1/config-manager/validate', body: { config: {}, options: 'strict' } });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 400);
  assert.strictEqual(res._json.error.code, 'BAD_REQUEST');
});

test('known route with unsupported method returns 405 and Allow', () => {
  const req = mockReq({ method: 'DELETE', path: '/v1/config-manager/validate' });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 405);
  assert.strictEqual(res._headers.Allow, 'POST');
  assert.strictEqual(res._json.error.code, 'METHOD_NOT_ALLOWED');
});

// --- Validate: unknown schema ---

test('POST /v1/config-manager/validate with unknown schema returns 404', () => {
  const req = mockReq({
    method: 'POST',
    path: '/v1/config-manager/validate',
    body: { config: { a: 1 }, schema: 'nonexistent-v1' },
  });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 404);
});

// Object-prototype property names are not registered schemas. Looking them up
// through the prototype chain used to reach validateAgainstSchema with a
// function/object and throw instead of returning the documented 404 response.
for (const schema of ['toString', 'constructor', '__proto__', 'hasOwnProperty', 'valueOf']) {
  test(`POST /v1/config-manager/validate rejects inherited schema '${schema}'`, () => {
    const req = mockReq({
      method: 'POST',
      path: '/v1/config-manager/validate',
      body: { config: { a: 1 }, schema },
    });
    const res = mockRes();
    handler(req, res);
    assert.strictEqual(res._status, 404);
    assert.strictEqual(res._json.success, false);
    assert.strictEqual(res._json.error.code, 'NOT_FOUND');
    assert.ok(res._json.execution_metadata.trace_id);
  });
}

// --- Schemas endpoint ---

test('GET /v1/config-manager/schemas returns available schemas', () => {
  const req = mockReq({ method: 'GET', path: '/v1/config-manager/schemas' });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 200);
  assert.strictEqual(res._json.success, true);
  assert.ok(res._json.data.length >= 2);
  assert.ok(res._json.data.some((s) => s.id === 'llm-config-v1'));
});

// --- 404 ---

test('Unknown route returns 404 with execution_metadata', () => {
  const req = mockReq({ method: 'GET', path: '/v1/nonexistent' });
  const res = mockRes();
  handler(req, res);
  assert.strictEqual(res._status, 404);
  assert.ok(res._json.execution_metadata);
});

// --- Execution metadata structure ---

test('execution_metadata includes all required fields', () => {
  const req = mockReq({ method: 'GET', path: '/health' });
  const res = mockRes();
  handler(req, res);
  const meta = res._json.execution_metadata;
  assert.ok(meta.trace_id, 'trace_id missing');
  assert.ok(meta.timestamp, 'timestamp missing');
  assert.strictEqual(meta.service, 'config-manager-agents');
  assert.ok(meta.execution_id, 'execution_id missing');
});

test('layers_executed contains AGENT_ROUTING layer', () => {
  const req = mockReq({ method: 'POST', path: '/v1/config-manager/validate', body: { config: { namespace: 'a', key: 'b', value: 'c' } } });
  const res = mockRes();
  handler(req, res);
  assert.ok(res._json.layers_executed.some((l) => l.layer === 'AGENT_ROUTING'));
  for (const layer of res._json.layers_executed) {
    assert.ok(layer.layer, 'layer name missing');
    assert.ok(layer.status, 'layer status missing');
  }
});

// --- Summary ---

console.log(`\n${passed + failed} tests: ${passed} passed, ${failed} failed\n`);
if (failed > 0) process.exit(1);
