#!/bin/bash
# xplan API Test Cases
# Usage: Run individual tests by number, e.g. ./tests/test_api.sh 3
# Or run all: ./tests/test_api.sh

API="http://localhost:26011"
KEY="${XPLAN_KEY:-sk-xplan-REPLACE_ME}"
# MODEL="claude-sonnet-4-6"
MODEL="deepseek-v4-flash"
# MODEL="anthropic.claude-fable-5"


# MODEL="glm-5.1"

run_test() {
  echo ""
  echo "============================================"
  echo "  TEST $1: $2"
  echo "============================================"
  echo ""
}

# 1. Health
test_1() {
  run_test 1 "Health Check"
  curl -s $API/health
  echo ""
}

# 2. List Models
test_2() {
  run_test 2 "List Models"
  curl -s $API/v1/models \
    -H "Authorization: Bearer $KEY" | jq .
}

# 3. Non-streaming Chat
test_3() {
  run_test 3 "Non-streaming Chat (OpenAI format)"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [{"role": "user", "content": "who are you?"}],
      "max_tokens": 5000
    }' | jq .
}

# 4. Streaming Chat
test_4() {
  run_test 4 "Streaming Chat (OpenAI format)"
  curl -N -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "stream": true,
      "messages": [{"role": "user", "content": "Count 1 to 5"}],
      "max_tokens": 100
    }'
  echo ""
}

# 5. System Prompt
test_5() {
  run_test 5 "System Prompt"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [
        {"role": "system", "content": "You are a pirate. Always respond in pirate speak."},
        {"role": "user", "content": "How are you today?"}
      ],
      "max_tokens": 1000
    }' | jq .
}

# 6. Tool Use
test_6() {
  run_test 6 "Tool Use (OpenAI format)"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [{"role": "user", "content": "What is the weather in Tokyo?"}],
      "max_tokens": 200,
      "tools": [{
        "type": "function",
        "function": {
          "name": "get_weather",
          "description": "Get current weather for a location",
          "parameters": {
            "type": "object",
            "properties": {
              "location": {"type": "string", "description": "City name"},
              "unit": {"type": "string", "enum": ["celsius", "fahrenheit"]}
            },
            "required": ["location"]
          }
        }
      }],
      "tool_choice": "auto"
    }' | jq .
}

# 7. Tool Use Multi-turn (thinking disabled for compatibility with DeepSeek)
test_7() {
  run_test 7 "Tool Use Multi-turn (with tool result, thinking disabled)"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "thinking": {"type": "disabled"},
      "messages": [
        {"role": "user", "content": "What is the weather in Tokyo?"},
        {"role": "assistant", "content": null, "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"location\":\"Tokyo\",\"unit\":\"celsius\"}"}}]},
        {"role": "tool", "tool_call_id": "call_1", "content": "{\"temperature\": 22, \"condition\": \"sunny\"}"}
      ],
      "max_tokens": 200,
      "tools": [{
        "type": "function",
        "function": {
          "name": "get_weather",
          "description": "Get current weather",
          "parameters": {"type": "object", "properties": {"location": {"type": "string"}, "unit": {"type": "string"}}, "required": ["location"]}
        }
      }]
    }' | jq .
}

# 8. Structured Output
test_8() {
  run_test 8 "Structured Output (response_format)"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [{"role": "user", "content": "Extract info: John Smith, john@example.com, wants Enterprise plan"}],
      "max_tokens": 200,
      "response_format": {
        "type": "json_schema",
        "json_schema": {
          "name": "contact_info",
          "schema": {
            "type": "object",
            "properties": {
              "name": {"type": "string"},
              "email": {"type": "string"},
              "plan": {"type": "string"}
            },
            "required": ["name", "email", "plan"],
            "additionalProperties": false
          }
        }
      }
    }' | jq .
}

# 9. Stop Sequences
test_9() {
  run_test 9 "Stop Sequences"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [{"role": "user", "content": "Count from 1 to 20, one number per line"}],
      "max_tokens": 200,
      "stop": ["10"]
    }' | jq .
}

# 10. Anthropic non-streaming
test_10() {
  run_test 10 "Anthropic /v1/messages (non-streaming)"
  curl -s -X POST $API/v1/messages \
    -H "x-api-key: $KEY" \
    -H "anthropic-version: 2023-06-01" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "max_tokens": 100,
      "messages": [{"role": "user", "content": "Say hello in one word"}]
    }' | jq .
}

# 11. Anthropic streaming
test_11() {
  run_test 11 "Anthropic /v1/messages (streaming)"
  curl -N -s -X POST $API/v1/messages \
    -H "x-api-key: $KEY" \
    -H "anthropic-version: 2023-06-01" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "max_tokens": 100,
      "stream": true,
      "messages": [{"role": "user", "content": "Count 1 to 3"}]
    }'
  echo ""
}

# 12. Anthropic with tools
test_12() {
  run_test 12 "Anthropic /v1/messages with tools"
  curl -s -X POST $API/v1/messages \
    -H "x-api-key: $KEY" \
    -H "anthropic-version: 2023-06-01" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "max_tokens": 200,
      "messages": [{"role": "user", "content": "What is the weather in Paris?"}],
      "tools": [{
        "name": "get_weather",
        "description": "Get weather for a city",
        "input_schema": {
          "type": "object",
          "properties": {"city": {"type": "string"}},
          "required": ["city"]
        }
      }]
    }' | jq .
}

# 13. Anthropic structured output
test_13() {
  run_test 13 "Anthropic /v1/messages with structured output"
  curl -s -X POST $API/v1/messages \
    -H "x-api-key: $KEY" \
    -H "anthropic-version: 2023-06-01" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "max_tokens": 200,
      "messages": [{"role": "user", "content": "Extract: Alice, alice@test.com, Pro plan"}],
      "output_config": {
        "format": {
          "type": "json_schema",
          "schema": {
            "type": "object",
            "properties": {
              "name": {"type": "string"},
              "email": {"type": "string"},
              "plan": {"type": "string"}
            },
            "required": ["name", "email", "plan"],
            "additionalProperties": false
          }
        }
      }
    }' | jq .
}

# 14. Temperature + top_p
test_14() {
  run_test 14 "Temperature + top_p"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [{"role": "user", "content": "Write one creative sentence about the ocean"}],
      "max_tokens": 100,
      "temperature": 0.9,
      "top_p": 0.95
    }' | jq .
}

# 15. Error: Invalid model
test_15() {
  run_test 15 "Error: Invalid model"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{"model": "nonexistent-model", "messages": [{"role": "user", "content": "hi"}]}' | jq .
}

# 16. Error: Missing auth
test_16() {
  run_test 16 "Error: Missing auth"
  curl -s -X POST $API/v1/chat/completions \
    -H "Content-Type: application/json" \
    -d '{"model": "'$MODEL'", "messages": [{"role": "user", "content": "hi"}]}' | jq .
}

# 17. Error: Invalid key
test_17() {
  run_test 17 "Error: Invalid API key"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer sk-xplan-invalidkey123" \
    -H "Content-Type: application/json" \
    -d '{"model": "'$MODEL'", "messages": [{"role": "user", "content": "hi"}]}' | jq .
}

# 18. Multi-turn
test_18() {
  run_test 18 "Multi-turn conversation"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [
        {"role": "user", "content": "My name is Alice"},
        {"role": "assistant", "content": "Hello Alice! Nice to meet you."},
        {"role": "user", "content": "What is my name?"}
      ],
      "max_tokens": 50
    }' | jq .
}

# 19. Tool Use Multi-turn with thinking (DeepSeek/reasoning models)
test_19() {
  run_test 19 "Tool Use Multi-turn with thinking (reasoning_content)"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "messages": [
        {"role": "user", "content": "What is the weather in Tokyo?"},
        {"role": "assistant", "content": null, "reasoning_content": "The user wants to know the weather in Tokyo. I should call the get_weather function.", "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"location\":\"Tokyo\",\"unit\":\"celsius\"}"}}]},
        {"role": "tool", "tool_call_id": "call_1", "content": "{\"temperature\": 22, \"condition\": \"sunny\"}"}
      ],
      "max_tokens": 200,
      "tools": [{
        "type": "function",
        "function": {
          "name": "get_weather",
          "description": "Get current weather",
          "parameters": {"type": "object", "properties": {"location": {"type": "string"}, "unit": {"type": "string"}}, "required": ["location"]}
        }
      }]
    }' | jq .
}

# 20. Tool Use Multi-turn without thinking (non-reasoning models)
test_20() {
  run_test 20 "Tool Use Multi-turn with thinking disabled (DeepSeek non-think)"
  curl -s -X POST $API/v1/chat/completions \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d '{
      "model": "'$MODEL'",
      "thinking": {"type": "disabled"},
      "messages": [
        {"role": "user", "content": "What is the weather in Tokyo?"},
        {"role": "assistant", "content": null, "tool_calls": [{"id": "call_1", "type": "function", "function": {"name": "get_weather", "arguments": "{\"location\":\"Tokyo\",\"unit\":\"celsius\"}"}}]},
        {"role": "tool", "tool_call_id": "call_1", "content": "{\"temperature\": 22, \"condition\": \"sunny\"}"}
      ],
      "max_tokens": 200,
      "tools": [{
        "type": "function",
        "function": {
          "name": "get_weather",
          "description": "Get current weather",
          "parameters": {"type": "object", "properties": {"location": {"type": "string"}, "unit": {"type": "string"}}, "required": ["location"]}
        }
      }]
    }' | jq .
}

# Run specific test or all
if [ -n "$1" ]; then
  test_$1
else
  for i in $(seq 1 20); do
    test_$i
    echo ""
    sleep 1
  done
fi
