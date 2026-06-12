-- Add 'responses' variant to the api_format enum for OpenAI Responses API endpoints
ALTER TYPE api_format ADD VALUE IF NOT EXISTS 'responses';
