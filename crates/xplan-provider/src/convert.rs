//! Pure format-conversion utilities for tool use across OpenAI, Anthropic, and Bedrock.
//!
//! All functions are stateless and operate on `&Value` → `Value`.  They are
//! intentionally lenient: missing or unexpected fields are skipped gracefully
//! so that non-tool-use requests are not affected.

use serde_json::{json, Value};

// ────────────────────────────────────────────────────────────────────────────
// Tool Definition Conversions
// ────────────────────────────────────────────────────────────────────────────

/// Convert OpenAI tool definitions to Anthropic format.
///
/// OpenAI: `[{type:"function", function:{name, description, parameters}}]`
/// Anthropic: `[{name, description, input_schema}]`
pub fn tools_openai_to_anthropic(tools: &Value) -> Value {
    let arr = match tools.as_array() {
        Some(a) => a,
        None => return json!([]),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|tool| {
            let func = tool.get("function")?;
            let name = func.get("name")?.as_str()?.to_string();
            let mut out = json!({ "name": name });
            if let Some(desc) = func.get("description").and_then(|d| d.as_str()) {
                out["description"] = json!(desc);
            }
            if let Some(params) = func.get("parameters") {
                out["input_schema"] = params.clone();
            } else {
                out["input_schema"] = json!({"type": "object", "properties": {}});
            }
            Some(out)
        })
        .collect();

    json!(converted)
}

/// Convert Anthropic tool definitions to OpenAI format.
///
/// Anthropic: `[{name, description, input_schema}]`
/// OpenAI: `[{type:"function", function:{name, description, parameters}}]`
pub fn tools_anthropic_to_openai(tools: &Value) -> Value {
    let arr = match tools.as_array() {
        Some(a) => a,
        None => return json!([]),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|tool| {
            let name = tool.get("name")?.as_str()?.to_string();
            let mut func = json!({ "name": name });
            if let Some(desc) = tool.get("description").and_then(|d| d.as_str()) {
                func["description"] = json!(desc);
            }
            func["parameters"] = tool
                .get("input_schema")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            Some(json!({ "type": "function", "function": func }))
        })
        .collect();

    json!(converted)
}

/// Convert OpenAI tool definitions to Bedrock `toolConfig` format.
///
/// OpenAI: `[{type:"function", function:{name, description, parameters}}]`
/// Bedrock: `{tools: [{toolSpec:{name, description, inputSchema:{json: parameters}}}]}`
pub fn tools_openai_to_bedrock(tools: &Value) -> Value {
    let arr = match tools.as_array() {
        Some(a) => a,
        None => return json!({"tools": []}),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|tool| {
            let func = tool.get("function")?;
            let name = func.get("name")?.as_str()?.to_string();
            let mut spec = json!({ "name": name });
            if let Some(desc) = func.get("description").and_then(|d| d.as_str()) {
                spec["description"] = json!(desc);
            }
            let schema = func
                .get("parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            spec["inputSchema"] = json!({ "json": schema });
            Some(json!({ "toolSpec": spec }))
        })
        .collect();

    json!({ "tools": converted })
}

/// Convert Bedrock `toolConfig.tools` to OpenAI tool definitions.
///
/// Bedrock: `[{toolSpec:{name, description, inputSchema:{json: ...}}}]`
/// OpenAI: `[{type:"function", function:{name, description, parameters}}]`
pub fn tools_bedrock_to_openai(tool_config: &Value) -> Value {
    let arr = match tool_config.get("tools").and_then(|t| t.as_array()) {
        Some(a) => a,
        None => return json!([]),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|entry| {
            let spec = entry.get("toolSpec")?;
            let name = spec.get("name")?.as_str()?.to_string();
            let mut func = json!({ "name": name });
            if let Some(desc) = spec.get("description").and_then(|d| d.as_str()) {
                func["description"] = json!(desc);
            }
            func["parameters"] = spec
                .get("inputSchema")
                .and_then(|s| s.get("json"))
                .cloned()
                .unwrap_or_else(|| json!({"type": "object", "properties": {}}));
            Some(json!({ "type": "function", "function": func }))
        })
        .collect();

    json!(converted)
}

// ────────────────────────────────────────────────────────────────────────────
// Tool Call Response Conversions
// ────────────────────────────────────────────────────────────────────────────

/// Convert OpenAI `tool_calls` array to Anthropic `tool_use` content blocks.
///
/// OpenAI: `[{id, type:"function", function:{name, arguments:"json_string"}}]`
/// Anthropic: `[{type:"tool_use", id, name, input: parsed_json}]`
pub fn tool_calls_openai_to_anthropic(tool_calls: &Value) -> Value {
    let arr = match tool_calls.as_array() {
        Some(a) => a,
        None => return json!([]),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|tc| {
            let func = tc.get("function")?;
            let id = tc.get("id")?.as_str()?.to_string();
            let name = func.get("name")?.as_str()?.to_string();
            let input = func
                .get("arguments")
                .and_then(|a| a.as_str())
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| json!({}));
            Some(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }))
        })
        .collect();

    json!(converted)
}

/// Convert Anthropic content blocks (may contain `tool_use`) to OpenAI `tool_calls`.
///
/// Anthropic: `[{type:"tool_use", id, name, input:{...}}]`
/// OpenAI: `[{id, type:"function", function:{name, arguments: stringify(input)}}]`
pub fn tool_calls_anthropic_to_openai(content: &Value) -> Value {
    let arr = match content.as_array() {
        Some(a) => a,
        None => return json!([]),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|block| {
            if block.get("type")?.as_str()? != "tool_use" {
                return None;
            }
            let id = block.get("id")?.as_str()?.to_string();
            let name = block.get("name")?.as_str()?.to_string();
            let arguments = block
                .get("input")
                .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()))
                .unwrap_or_else(|| "{}".to_string());
            Some(json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": arguments,
                },
            }))
        })
        .collect();

    json!(converted)
}

/// Convert Bedrock tool-use content blocks to OpenAI `tool_calls`.
///
/// Bedrock: `[{toolUse:{toolUseId, name, input:{...}}}]`
/// OpenAI: `[{id, type:"function", function:{name, arguments: stringify(input)}}]`
pub fn tool_calls_bedrock_to_openai(content: &Value) -> Value {
    let arr = match content.as_array() {
        Some(a) => a,
        None => return json!([]),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|block| {
            let tool_use = block.get("toolUse")?;
            let id = tool_use.get("toolUseId")?.as_str()?.to_string();
            let name = tool_use.get("name")?.as_str()?.to_string();
            let arguments = tool_use
                .get("input")
                .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()))
                .unwrap_or_else(|| "{}".to_string());
            Some(json!({
                "id": id,
                "type": "function",
                "function": {
                    "name": name,
                    "arguments": arguments,
                },
            }))
        })
        .collect();

    json!(converted)
}

/// Convert OpenAI `tool_calls` to Bedrock tool-use content blocks.
///
/// OpenAI: `[{id, type:"function", function:{name, arguments:"json_string"}}]`
/// Bedrock: `[{toolUse:{toolUseId, name, input:{...}}}]`
pub fn tool_calls_openai_to_bedrock(tool_calls: &Value) -> Value {
    let arr = match tool_calls.as_array() {
        Some(a) => a,
        None => return json!([]),
    };

    let converted: Vec<Value> = arr
        .iter()
        .filter_map(|tc| {
            let func = tc.get("function")?;
            let id = tc.get("id")?.as_str()?.to_string();
            let name = func.get("name")?.as_str()?.to_string();
            let input = func
                .get("arguments")
                .and_then(|a| a.as_str())
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or_else(|| json!({}));
            Some(json!({
                "toolUse": {
                    "toolUseId": id,
                    "name": name,
                    "input": input,
                }
            }))
        })
        .collect();

    json!(converted)
}

// ────────────────────────────────────────────────────────────────────────────
// Tool Result Message Conversions
// ────────────────────────────────────────────────────────────────────────────

/// Convert an OpenAI tool message to an Anthropic `tool_result` content block.
///
/// OpenAI: `{role:"tool", tool_call_id:"x", content:"result"}`
/// Anthropic: `{type:"tool_result", tool_use_id:"x", content:[{type:"text", text:"result"}]}`
pub fn tool_result_openai_to_anthropic(msg: &Value) -> Value {
    let tool_use_id = msg
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let text = match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
        None => String::new(),
    };

    json!({
        "type": "tool_result",
        "tool_use_id": tool_use_id,
        "content": [{"type": "text", "text": text}],
    })
}

/// Convert an Anthropic `tool_result` content block to an OpenAI tool message.
///
/// Anthropic: `{type:"tool_result", tool_use_id:"x", content:[...]}`
/// OpenAI: `{role:"tool", tool_call_id:"x", content:"result_text"}`
pub fn tool_result_anthropic_to_openai(block: &Value) -> Value {
    let tool_call_id = block
        .get("tool_use_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let content = match block.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(arr)) => {
            // Collect all text parts
            arr.iter()
                .filter_map(|item| {
                    if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                        item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
        None => String::new(),
    };

    json!({
        "role": "tool",
        "tool_call_id": tool_call_id,
        "content": content,
    })
}

/// Convert an OpenAI tool message to a Bedrock `toolResult` content block.
///
/// OpenAI: `{role:"tool", tool_call_id:"x", content:"result"}`
/// Bedrock: `{toolResult:{toolUseId:"x", content:[{text:"result"}]}}`
pub fn tool_result_openai_to_bedrock(msg: &Value) -> Value {
    let tool_use_id = msg
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let text = match msg.get("content") {
        Some(Value::String(s)) => s.clone(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
        None => String::new(),
    };

    json!({
        "toolResult": {
            "toolUseId": tool_use_id,
            "content": [{"text": text}],
        }
    })
}

/// Convert a Bedrock `toolResult` content block to an OpenAI tool message.
///
/// Bedrock: `{toolResult:{toolUseId:"x", content:[{text:"result"}]}}`
/// OpenAI: `{role:"tool", tool_call_id:"x", content:"result"}`
pub fn tool_result_bedrock_to_openai(block: &Value) -> Value {
    let tr = match block.get("toolResult") {
        Some(v) => v,
        None => return json!({"role": "tool", "tool_call_id": "", "content": ""}),
    };

    let tool_call_id = tr
        .get("toolUseId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let content = match tr.get("content") {
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string()))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(Value::String(s)) => s.clone(),
        Some(other) => serde_json::to_string(other).unwrap_or_default(),
        None => String::new(),
    };

    json!({
        "role": "tool",
        "tool_call_id": tool_call_id,
        "content": content,
    })
}

// ────────────────────────────────────────────────────────────────────────────
// Tool Choice Conversions
// ────────────────────────────────────────────────────────────────────────────

/// Convert an OpenAI `tool_choice` value to Anthropic format.
///
/// OpenAI → Anthropic:
/// - `"auto"` → `{type:"auto"}`
/// - `"none"` → omit (caller should not include tool_choice)
/// - `"required"` → `{type:"any"}`
/// - `{type:"function", function:{name:"x"}}` → `{type:"tool", name:"x"}`
pub fn tool_choice_openai_to_anthropic(tool_choice: &Value) -> Option<Value> {
    match tool_choice {
        Value::String(s) => match s.as_str() {
            "none" => None,
            "required" => Some(json!({"type": "any"})),
            _ => Some(json!({"type": "auto"})), // "auto" and anything else
        },
        Value::Object(_) => {
            // {type:"function", function:{name:"x"}}
            let name = tool_choice
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str());
            if let Some(n) = name {
                Some(json!({"type": "tool", "name": n}))
            } else {
                Some(json!({"type": "auto"}))
            }
        }
        _ => None,
    }
}

/// Convert an OpenAI `tool_choice` value to Bedrock `toolChoice` format.
///
/// OpenAI → Bedrock:
/// - `"auto"` → `{auto:{}}`
/// - `"none"` → omit
/// - `"required"` → `{any:{}}`
/// - `{type:"function", function:{name:"x"}}` → `{tool:{name:"x"}}`
pub fn tool_choice_openai_to_bedrock(tool_choice: &Value) -> Option<Value> {
    match tool_choice {
        Value::String(s) => match s.as_str() {
            "none" => None,
            "required" => Some(json!({"any": {}})),
            _ => Some(json!({"auto": {}})),
        },
        Value::Object(_) => {
            let name = tool_choice
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str());
            if let Some(n) = name {
                Some(json!({"tool": {"name": n}}))
            } else {
                Some(json!({"auto": {}}))
            }
        }
        _ => None,
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Structured Output (response_format) Conversion
// ────────────────────────────────────────────────────────────────────────────

/// Convert an OpenAI `response_format` with `type: "json_schema"` into a forced
/// tool call pattern suitable for Anthropic and Bedrock (neither supports
/// `response_format` natively).
///
/// Returns `Some((tool_definition, tool_choice))` for `json_schema` type;
/// `None` for `json_object` or unknown types (caller should skip conversion).
///
/// Anthropic tool shape: `{name, description, input_schema}`
/// Anthropic tool_choice: `{type: "tool", name: "<name>"}`
pub fn response_format_to_tool(response_format: &Value) -> Option<(Value, Value)> {
    let rf_type = response_format.get("type")?.as_str()?;
    if rf_type != "json_schema" {
        return None;
    }

    let schema_obj = response_format.get("json_schema")?;
    let name = schema_obj
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("structured_output");
    let schema = schema_obj.get("schema")?;

    let tool = json!({
        "name": name,
        "description": "Generate structured output matching the required schema.",
        "input_schema": schema.clone(),
    });
    let tool_choice = json!({"type": "tool", "name": name});

    Some((tool, tool_choice))
}

/// Convert an OpenAI `response_format` with `type: "json_schema"` into a forced
/// tool call pattern suitable for Bedrock `toolConfig`.
///
/// Returns `Some((tool_config, tool_choice))` for `json_schema` type; `None` otherwise.
///
/// Bedrock toolConfig shape: `{tools: [{toolSpec:{name, description, inputSchema:{json: schema}}}]}`
/// Bedrock toolChoice shape: `{tool: {name: "<name>"}}`
pub fn response_format_to_bedrock_tool(response_format: &Value) -> Option<(Value, Value)> {
    let rf_type = response_format.get("type")?.as_str()?;
    if rf_type != "json_schema" {
        return None;
    }

    let schema_obj = response_format.get("json_schema")?;
    let name = schema_obj
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("structured_output");
    let schema = schema_obj.get("schema")?;

    let tool_config = json!({
        "tools": [{
            "toolSpec": {
                "name": name,
                "description": "Generate structured output matching the required schema.",
                "inputSchema": {"json": schema.clone()},
            }
        }],
        "toolChoice": {"tool": {"name": name}},
    });
    let tool_choice = json!({"tool": {"name": name}});

    Some((tool_config, tool_choice))
}

// ────────────────────────────────────────────────────────────────────────────
// Helper: does an Anthropic content array contain tool_use blocks?
// ────────────────────────────────────────────────────────────────────────────

/// Returns `true` if the content array has at least one `tool_use` block.
pub fn has_tool_use_blocks(content: &Value) -> bool {
    content
        .as_array()
        .map(|arr| {
            arr.iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        })
        .unwrap_or(false)
}

/// Returns `true` if the Bedrock output message content has at least one `toolUse` block.
pub fn has_bedrock_tool_use(content: &Value) -> bool {
    content
        .as_array()
        .map(|arr| arr.iter().any(|b| b.get("toolUse").is_some()))
        .unwrap_or(false)
}

// ────────────────────────────────────────────────────────────────────────────
// Per-model parameter modulation
// ────────────────────────────────────────────────────────────────────────────

/// Apply per-model parameter modulation based on `config`.
/// Modifies `body` in place.
///
/// `param_paths` maps logical param names to their JSON path in `body`.
/// Paths are dot-separated, e.g. `"inferenceConfig.maxTokens"`.
///
/// Config fields honoured:
/// - `max_output_tokens` — clamp (or set) the `max_tokens` logical param
/// - `unsupported_params` — remove listed params from `body`
/// - `param_overrides` — `{name: {max?: f64, min?: f64}}` clamp for numeric params
pub fn apply_param_modulation(body: &mut Value, config: &Value, param_paths: &[(&str, &str)]) {
    // 1. Clamp max_tokens by max_output_tokens
    if let Some(max_out) = config.get("max_output_tokens").and_then(|v| v.as_u64()) {
        for (logical, path) in param_paths {
            if *logical == "max_tokens" {
                let current = get_by_path(body, path).and_then(|v| v.as_u64());
                match current {
                    Some(n) if n > max_out => set_by_path(body, path, json!(max_out)),
                    None => set_by_path(body, path, json!(max_out)),
                    _ => {}
                }
                break;
            }
        }
    }

    // 2. Strip unsupported params
    if let Some(unsup) = config.get("unsupported_params").and_then(|v| v.as_array()) {
        let names: Vec<&str> = unsup.iter().filter_map(|u| u.as_str()).collect();
        for name in names {
            for (logical, path) in param_paths {
                if *logical == name {
                    remove_by_path(body, path);
                }
            }
        }
    }

    // 3. Apply param_overrides (min/max clamp)
    if let Some(overrides) = config.get("param_overrides").and_then(|v| v.as_object()) {
        for (name, rules) in overrides {
            for (logical, path) in param_paths {
                if *logical == name.as_str() {
                    if let Some(current) = get_by_path(body, path).and_then(|v| v.as_f64()) {
                        let max = rules.get("max").and_then(|v| v.as_f64());
                        let min = rules.get("min").and_then(|v| v.as_f64());
                        let mut new_val = current;
                        if let Some(m) = max {
                            if new_val > m {
                                new_val = m;
                            }
                        }
                        if let Some(m) = min {
                            if new_val < m {
                                new_val = m;
                            }
                        }
                        if (new_val - current).abs() > f64::EPSILON {
                            set_by_path(body, path, json!(new_val));
                        }
                    }
                }
            }
        }
    }
}

/// Get a value from `v` by a dot-separated path, e.g. `"inferenceConfig.maxTokens"`.
fn get_by_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = v;
    for seg in path.split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

/// Set a value in `v` by a dot-separated path, creating intermediate objects as needed.
fn set_by_path(v: &mut Value, path: &str, new: Value) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }
    let mut cur = v;
    for seg in &parts[..parts.len() - 1] {
        if cur.get(seg).is_none() {
            if let Some(obj) = cur.as_object_mut() {
                obj.insert(seg.to_string(), json!({}));
            }
        }
        if let Some(next) = cur.get_mut(seg) {
            cur = next;
        } else {
            return;
        }
    }
    if let Some(obj) = cur.as_object_mut() {
        obj.insert(parts.last().unwrap().to_string(), new);
    }
}

/// Remove a key from `v` at a dot-separated path.
fn remove_by_path(v: &mut Value, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }
    let mut cur = v;
    for seg in &parts[..parts.len() - 1] {
        match cur.get_mut(seg) {
            Some(next) => cur = next,
            None => return,
        }
    }
    if let Some(obj) = cur.as_object_mut() {
        obj.remove(*parts.last().unwrap());
    }
}
