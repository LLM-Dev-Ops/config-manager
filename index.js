'use strict';

const crypto = require('crypto');

// ---------------------------------------------------------------------------
// Validation schemas – ported from agents/config-validation/src/handler/routes.rs
// These are READ-ONLY definitions; the business logic they encode is unchanged.
// ---------------------------------------------------------------------------

const SCHEMAS = {
  'llm-config-v1': {
    id: 'llm-config-v1',
    name: 'LLM Configuration Schema v1',
    version: '1.0.0',
    description: 'Schema for LLM configuration entries',
    fields: [
      { path: 'namespace', field_type: 'string', required: true, pattern: '^[a-z][a-z0-9-/]*$', description: 'Configuration namespace' },
      { path: 'key',       field_type: 'string', required: true, pattern: '^[a-zA-Z][a-zA-Z0-9_-]*$', description: 'Configuration key' },
      { path: 'value',     field_type: 'any',    required: true, pattern: null, description: 'Configuration value' },
      { path: 'environment', field_type: 'string', required: false, pattern: '^(development|staging|production|base)$', description: 'Target environment' },
    ],
  },
  'provider-config-v1': {
    id: 'provider-config-v1',
    name: 'Provider Configuration Schema v1',
    version: '1.0.0',
    description: 'Schema for external provider configurations',
    fields: [
      { path: 'provider_type', field_type: 'string', required: true, pattern: '^(vault|aws|gcp|azure|env)$', description: 'Provider type' },
      { path: 'endpoint',      field_type: 'string', required: false, pattern: '^https?://', description: 'Provider endpoint URL' },
      { path: 'auth',          field_type: 'object', required: false, pattern: null, description: 'Authentication configuration' },
    ],
  },
};

// ---------------------------------------------------------------------------
// Validation helpers – mirrors agents/config-validation/src/handler/routes.rs
// ---------------------------------------------------------------------------

function getJsonPath(obj, path) {
  const parts = path.split('.');
  let current = obj;
  for (const part of parts) {
    if (current == null || typeof current !== 'object') return undefined;
    current = current[part];
  }
  return current;
}

function validateType(value, expected) {
  switch (expected) {
    case 'string':  return typeof value === 'string';
    case 'number':
    case 'integer': return typeof value === 'number';
    case 'boolean': return typeof value === 'boolean';
    case 'array':   return Array.isArray(value);
    case 'object':  return value !== null && typeof value === 'object' && !Array.isArray(value);
    case 'any':     return true;
    case 'null':    return value === null;
    default:        return true;
  }
}

function jsonTypeName(value) {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  return typeof value; // string | number | boolean | object
}

function countFields(value) {
  if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
    return Object.keys(value).length + Object.values(value).reduce((s, v) => s + countFields(v), 0);
  }
  if (Array.isArray(value)) {
    return value.reduce((s, v) => s + countFields(v), 0);
  }
  return 0;
}

function collectTopLevelKeys(obj) {
  if (obj !== null && typeof obj === 'object' && !Array.isArray(obj)) {
    return Object.keys(obj);
  }
  return [];
}

function validateAgainstSchema(config, schema, options) {
  const errors = [];
  const warnings = [];

  for (const field of schema.fields) {
    const value = getJsonPath(config, field.path);

    if (field.required && value === undefined) {
      errors.push({
        path: field.path,
        code: 'REQUIRED_FIELD_MISSING',
        message: `Required field '${field.path}' is missing`,
        expected: field.field_type,
        actual: null,
      });
      continue;
    }

    if (value === undefined) continue;

    if (!validateType(value, field.field_type)) {
      errors.push({
        path: field.path,
        code: 'TYPE_MISMATCH',
        message: `Field '${field.path}' has wrong type`,
        expected: field.field_type,
        actual: jsonTypeName(value),
      });
      continue;
    }

    if (field.pattern && typeof value === 'string') {
      const re = new RegExp(field.pattern);
      if (!re.test(value)) {
        errors.push({
          path: field.path,
          code: 'PATTERN_MISMATCH',
          message: `Field '${field.path}' does not match required pattern`,
          expected: field.pattern,
          actual: value,
        });
      }
    }
  }

  if (options && options.strict) {
    const schemaPaths = new Set(schema.fields.map((f) => f.path));
    for (const key of collectTopLevelKeys(config)) {
      if (!schemaPaths.has(key)) {
        warnings.push({ path: key, code: 'UNKNOWN_FIELD', message: 'Field not defined in schema' });
      }
    }
  }

  return { errors, warnings };
}

// ---------------------------------------------------------------------------
// Execution metadata builder
// ---------------------------------------------------------------------------

function buildExecutionMetadata(req, executionId) {
  return {
    trace_id: (req.headers && req.headers['x-correlation-id']) || crypto.randomUUID(),
    timestamp: new Date().toISOString(),
    service: 'config-manager-agents',
    execution_id: executionId,
  };
}

// ---------------------------------------------------------------------------
// CORS helpers
// ---------------------------------------------------------------------------

const MAX_REQUEST_BYTES = 64 * 1024;

const CORS_HEADERS = {
  'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type, Authorization, X-Correlation-Id, X-Parent-Span-Id',
  'Access-Control-Max-Age': '3600',
};

function getHeader(req, name) {
  const headers = req.headers || {};
  const expected = name.toLowerCase();
  const key = Object.keys(headers).find((candidate) => candidate.toLowerCase() === expected);
  return key ? headers[key] : undefined;
}

function configuredOrigins() {
  const configured = process.env.ALLOWED_ORIGINS || '*';
  return configured.split(',').map((origin) => origin.trim()).filter(Boolean);
}

function setCorsHeaders(req, res) {
  for (const [key, value] of Object.entries(CORS_HEADERS)) {
    res.setHeader(key, value);
  }

  const origins = configuredOrigins();
  const requestOrigin = getHeader(req, 'origin');
  if (origins.includes('*')) {
    res.setHeader('Access-Control-Allow-Origin', '*');
  } else {
    res.setHeader('Vary', 'Origin');
    if (requestOrigin && origins.includes(requestOrigin)) {
      res.setHeader('Access-Control-Allow-Origin', requestOrigin);
    }
  }
}

function jsonError(req, res, executionId, startMs, status, code, message) {
  return res.status(status).json({
    success: false,
    error: { code, message },
    execution_metadata: buildExecutionMetadata(req, executionId),
    layers_executed: [
      { layer: 'AGENT_ROUTING', status: 'failed', duration_ms: Date.now() - startMs },
    ],
  });
}

function requestBodyBytes(req) {
  const measured = [];
  if (Buffer.isBuffer(req.rawBody)) measured.push(req.rawBody.byteLength);

  const contentLength = Number.parseInt(getHeader(req, 'content-length'), 10);
  if (Number.isSafeInteger(contentLength) && contentLength >= 0) measured.push(contentLength);

  if (req.body !== undefined) measured.push(Buffer.byteLength(JSON.stringify(req.body), 'utf8'));
  return measured.length === 0 ? 0 : Math.max(...measured);
}

// ---------------------------------------------------------------------------
// Route handlers
// ---------------------------------------------------------------------------

function handleHealth(_req, res, executionId, startMs) {
  const elapsed = Date.now() - startMs;
  const body = {
    status: 'healthy',
    agents: ['validate'],
    components: {
      validation_engine: true,
      schema_registry: true,
    },
    version: '0.5.0',
    timestamp: new Date().toISOString(),
    execution_metadata: buildExecutionMetadata(_req, executionId),
    layers_executed: [
      { layer: 'AGENT_ROUTING', status: 'completed' },
      { layer: 'HEALTH_CHECK', status: 'completed', duration_ms: elapsed },
    ],
  };
  res.status(200).json(body);
}

function handleValidate(req, res, executionId, startMs) {
  const body = req.body;

  if (!body || typeof body !== 'object' || Array.isArray(body)) {
    const elapsed = Date.now() - startMs;
    return res.status(400).json({
      success: false,
      error: { code: 'BAD_REQUEST', message: 'Request body must be a JSON object' },
      execution_metadata: buildExecutionMetadata(req, executionId),
      layers_executed: [
        { layer: 'AGENT_ROUTING', status: 'completed' },
        { layer: 'CONFIG_MANAGER_VALIDATE', status: 'failed', duration_ms: elapsed },
      ],
    });
  }

  const config = body.config;
  if (config === undefined || config === null) {
    const elapsed = Date.now() - startMs;
    return res.status(400).json({
      success: false,
      error: { code: 'BAD_REQUEST', message: "Missing required field 'config'" },
      execution_metadata: buildExecutionMetadata(req, executionId),
      layers_executed: [
        { layer: 'AGENT_ROUTING', status: 'completed' },
        { layer: 'CONFIG_MANAGER_VALIDATE', status: 'failed', duration_ms: elapsed },
      ],
    });
  }

  if (typeof config !== 'object' || Array.isArray(config)) {
    return jsonError(
      req,
      res,
      executionId,
      startMs,
      400,
      'BAD_REQUEST',
      "Field 'config' must be a JSON object",
    );
  }

  if (body.schema !== undefined && typeof body.schema !== 'string') {
    return jsonError(req, res, executionId, startMs, 400, 'BAD_REQUEST', "Field 'schema' must be a string");
  }

  if (body.options !== undefined && (body.options === null || typeof body.options !== 'object' || Array.isArray(body.options))) {
    return jsonError(req, res, executionId, startMs, 400, 'BAD_REQUEST', "Field 'options' must be a JSON object");
  }

  const schemaId = body.schema || 'llm-config-v1';
  const schema = Object.hasOwn(SCHEMAS, schemaId) ? SCHEMAS[schemaId] : undefined;
  if (!schema) {
    const elapsed = Date.now() - startMs;
    return res.status(404).json({
      success: false,
      error: { code: 'NOT_FOUND', message: `Schema '${schemaId}' not found` },
      execution_metadata: buildExecutionMetadata(req, executionId),
      layers_executed: [
        { layer: 'AGENT_ROUTING', status: 'completed' },
        { layer: 'CONFIG_MANAGER_VALIDATE', status: 'failed', duration_ms: elapsed },
      ],
    });
  }

  const options = body.options || {};
  const { errors, warnings } = validateAgainstSchema(config, schema, options);
  const elapsed = Date.now() - startMs;

  res.status(200).json({
    success: errors.length === 0,
    data: {
      valid: errors.length === 0,
      errors,
      warnings,
      schema_used: schemaId,
      stats: {
        fields_validated: countFields(config),
        rules_applied: schema.fields.length,
        duration_us: elapsed * 1000,
      },
    },
    execution_metadata: buildExecutionMetadata(req, executionId),
    layers_executed: [
      { layer: 'AGENT_ROUTING', status: 'completed' },
      { layer: 'CONFIG_MANAGER_VALIDATE', status: 'completed', duration_ms: elapsed },
    ],
  });
}

function handleSchemas(_req, res, executionId, startMs) {
  const schemas = Object.values(SCHEMAS).map((s) => ({
    id: s.id,
    name: s.name,
    version: s.version,
    description: s.description,
    field_count: s.fields.length,
  }));
  const elapsed = Date.now() - startMs;

  res.status(200).json({
    success: true,
    data: schemas,
    execution_metadata: buildExecutionMetadata(_req, executionId),
    layers_executed: [
      { layer: 'AGENT_ROUTING', status: 'completed' },
      { layer: 'SCHEMA_LIST', status: 'completed', duration_ms: elapsed },
    ],
  });
}

// ---------------------------------------------------------------------------
// Cloud Function entry point
// ---------------------------------------------------------------------------

/**
 * Google Cloud Function HTTP handler for config-manager-agents.
 *
 * Routes:
 *   POST /v1/config-manager/validate  – Config Validation Agent
 *   GET  /health                       – Health / readiness
 *   GET  /v1/config-manager/schemas    – List available schemas
 *
 * @param {import('express').Request} req
 * @param {import('express').Response} res
 */
exports.handler = (req, res) => {
  const startMs = Date.now();
  const executionId = crypto.randomUUID();

  // CORS – handle preflight and set headers on every response
  setCorsHeaders(req, res);
  if (req.method === 'OPTIONS') {
    const origins = configuredOrigins();
    const requestOrigin = getHeader(req, 'origin');
    if (!origins.includes('*') && requestOrigin && !origins.includes(requestOrigin)) {
      return jsonError(req, res, executionId, startMs, 403, 'CORS_ORIGIN_DENIED', 'Origin is not allowed');
    }
    return res.status(204).end();
  }

  // Normalise path: strip trailing slash, lowercase
  const path = (req.path || '/').replace(/\/+$/, '') || '/';

  const routeMethods = {
    '/health': ['GET'],
    '/v1/config-manager/validate': ['POST'],
    '/v1/config-manager/schemas': ['GET'],
  };

  if (Object.hasOwn(routeMethods, path) && !routeMethods[path].includes(req.method)) {
    res.setHeader('Allow', routeMethods[path].join(', '));
    return jsonError(req, res, executionId, startMs, 405, 'METHOD_NOT_ALLOWED', `Method ${req.method} is not allowed for ${path}`);
  }

  if (path === '/v1/config-manager/validate' && req.method === 'POST') {
    const contentType = getHeader(req, 'content-type') || '';
    if (!/^application\/json(?:\s*;|$)/i.test(contentType)) {
      return jsonError(req, res, executionId, startMs, 415, 'UNSUPPORTED_MEDIA_TYPE', 'Content-Type must be application/json');
    }

    let bodyBytes;
    try {
      bodyBytes = requestBodyBytes(req);
    } catch (_error) {
      return jsonError(req, res, executionId, startMs, 400, 'BAD_REQUEST', 'Request body must be valid JSON');
    }
    if (bodyBytes > MAX_REQUEST_BYTES) {
      return jsonError(req, res, executionId, startMs, 413, 'PAYLOAD_TOO_LARGE', `Request body exceeds ${MAX_REQUEST_BYTES} bytes`);
    }
  }

  // ---------- Routing ----------

  if (path === '/health' && req.method === 'GET') {
    return handleHealth(req, res, executionId, startMs);
  }

  if (path === '/v1/config-manager/validate' && req.method === 'POST') {
    return handleValidate(req, res, executionId, startMs);
  }

  if (path === '/v1/config-manager/schemas' && req.method === 'GET') {
    return handleSchemas(req, res, executionId, startMs);
  }

  // ---------- 404 ----------
  const elapsed = Date.now() - startMs;
  res.status(404).json({
    success: false,
    error: { code: 'NOT_FOUND', message: `Route ${req.method} ${path} not found` },
    execution_metadata: buildExecutionMetadata(req, executionId),
    layers_executed: [
      { layer: 'AGENT_ROUTING', status: 'failed', duration_ms: elapsed },
    ],
  });
};
