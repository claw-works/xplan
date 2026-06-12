# Tool Use Cross-Format Conversion Design

## Overview

xplan needs to transparently convert tool use (function calling) between different provider formats so that:
- A client using OpenAI format can call tools on Anthropic/Bedrock upstreams
- A client using Anthropic format can call tools on OpenAI/Bedrock upstreams

## Format Comparison

### Tool Definition

| | OpenAI | Anthropic | Bedrock |
|---|---|---|---|
| Container | `tools: [...]` | `tools: [...]` | `toolConfig: { tools: [...] }` |
| Item | `{type:"function", function:{name, description, parameters}}` | `{name, description, input_schema}` | `{toolSpec:{name, description, inputSchema:{json: ...}}}` |
| Parameters schema | `parameters: {type:"object", properties:...}` | `input_schema: {type:"object", properties:...}` | `inputSchema: {json: {type:"object", properties:...}}` |

### Tool Call in Response

| | OpenAI | Anthropic | Bedrock |
|---|---|---|---|
| Location | `choices[0].message.tool_calls` | `content[{type:"tool_use",...}]` | `output.message.content[{toolUse:{...}}]` |
| Structure | `[{id, type:"function", function:{name, arguments(string)}}]` | `[{type:"tool_use", id, name, input(object)}]` | `[{toolUse:{toolUseId, name, input(object)}}]` |
| Arguments | JSON string | JSON object | JSON object |

### Tool Result in Request

| | OpenAI | Anthropic | Bedrock |
|---|---|---|---|
| Role | `tool` | `user` (with tool_result content) | `user` (with toolResult content) |
| Structure | `{role:"tool", tool_call_id, content(string)}` | `{role:"user", content:[{type:"tool_result", tool_use_id, content}]}` | `{role:"user", content:[{toolResult:{toolUseId, content:[{text:...}]}}]}` |

## Implementation Plan

### Phase 1: OpenAI ↔ Anthropic Conversion

#### 1.1 Tool Definition Conversion

```
OpenAI → Anthropic:
  {type:"function", function:{name, description, parameters}}
  → {name, description, input_schema: parameters}

Anthropic → OpenAI:
  {name, description, input_schema}
  → {type:"function", function:{name, description, parameters: input_schema}}
```

#### 1.2 Tool Call Response Conversion

```
OpenAI → Anthropic (response):
  choices[0].message.tool_calls[{id, function:{name, arguments}}]
  → content: [{type:"tool_use", id, name, input: parse(arguments)}]

Anthropic → OpenAI (response):
  content[{type:"tool_use", id, name, input}]
  → choices[0].message.tool_calls: [{id, type:"function", function:{name, arguments: stringify(input)}}]
```

#### 1.3 Tool Result Message Conversion

```
OpenAI → Anthropic (in messages):
  {role:"tool", tool_call_id:"x", content:"result"}
  → {role:"user", content:[{type:"tool_result", tool_use_id:"x", content:[{type:"text", text:"result"}]}]}

Anthropic → OpenAI (in messages):
  {role:"user", content:[{type:"tool_result", tool_use_id:"x", content:"result"}]}
  → {role:"tool", tool_call_id:"x", content:"result"}
```

### Phase 2: Bedrock Conversion

#### 2.1 Tool Definition

```
OpenAI → Bedrock:
  tools: [{type:"function", function:{name, description, parameters}}]
  → toolConfig: {tools: [{toolSpec:{name, description, inputSchema:{json: parameters}}}]}

Bedrock → OpenAI:
  reverse of above
```

#### 2.2 Tool Call Response

```
Bedrock → OpenAI:
  output.message.content[{toolUse:{toolUseId, name, input}}]
  → choices[0].message.tool_calls: [{id: toolUseId, type:"function", function:{name, arguments: stringify(input)}}]

Bedrock → Anthropic:
  output.message.content[{toolUse:{toolUseId, name, input}}]
  → content: [{type:"tool_use", id: toolUseId, name, input}]
```

#### 2.3 Tool Result Message

```
OpenAI → Bedrock:
  {role:"tool", tool_call_id:"x", content:"result"}
  → {role:"user", content:[{toolResult:{toolUseId:"x", content:[{text:"result"}]}}]}
```

## File Changes

### New file: `crates/xplan-provider/src/convert.rs`

Central conversion module with pure functions:
- `convert_tools_openai_to_anthropic(tools: &Value) -> Value`
- `convert_tools_anthropic_to_openai(tools: &Value) -> Value`
- `convert_tools_openai_to_bedrock(tools: &Value) -> Value`
- `convert_tools_bedrock_to_openai(tools: &Value) -> Value`
- `convert_tool_calls_openai_to_anthropic(tool_calls: &Value) -> Value`
- `convert_tool_calls_anthropic_to_openai(content: &Value) -> Value`
- `convert_tool_calls_bedrock_to_openai(content: &Value) -> Value`
- `convert_messages_openai_to_anthropic(messages: &[Value]) -> (Option<String>, Vec<Value>)` (extracts system)
- `convert_messages_anthropic_to_openai(messages: &[Value], system: Option<&str>) -> Vec<Value>`
- `convert_messages_openai_to_bedrock(messages: &[Value]) -> (Vec<Value>, Vec<Value>)` (messages, system)

### Modified: `crates/xplan-provider/src/openai.rs`

Before sending to upstream: if request has tools in `extra`, pass them through (already works via flatten).

### Modified: `crates/xplan-provider/src/anthropic.rs`

In `convert_to_anthropic_format`: also handle `extra.tools` → convert to Anthropic tool format.

### Modified: `crates/xplan-provider/src/bedrock.rs`

In `build_body`: also handle tools → convert to Bedrock toolConfig format.

### Modified: `crates/xplan-server/src/proxy/chat.rs`

In `normalize_to_openai_format`: also convert tool_calls from Anthropic/Bedrock format to OpenAI format.

### Modified: `crates/xplan-server/src/proxy/messages.rs`

In `to_anthropic_response`: also convert tool_calls from OpenAI/Bedrock format to Anthropic format.

## Streaming Tool Use

For streaming, tool calls come in chunks:
- OpenAI: `delta.tool_calls[{index, id?, function:{name?, arguments?}}]` (accumulated)
- Anthropic: `content_block_start` with `{type:"tool_use",id,name}` then `input_json_delta` events
- Bedrock: similar to Anthropic

For V1 streaming tool use: pass through as-is for same-format, and for cross-format streaming just accumulate tool call chunks and emit the complete tool_call at the end (simpler than chunk-by-chunk conversion).
