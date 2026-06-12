export default function Docs() {
  return (
    <div className="p-8 max-w-4xl space-y-8">
      <div>
        <h2 className="text-xl font-semibold text-white">Configuration Reference</h2>
        <p className="text-sm text-gray-500 mt-0.5">
          Guide to the <code className="text-indigo-400">provider_models.config</code> JSONB field
        </p>
      </div>

      {/* Overview */}
      <Section title="Overview">
        <p>
          Each <strong>Provider Model</strong> row has a <code>config</code> JSONB column that lets
          you fine-tune how xplan interacts with that upstream model. You can set it when creating
          or editing a provider-model mapping on the Routing page.
        </p>
        <p className="mt-2">
          All fields are optional. An empty object <code>{'{}'}</code> is the default and means
          &ldquo;no special behaviour&rdquo;.
        </p>
        <CodeBlock>{`{
  "endpoint_url": "https://...",
  "structured_output": "json_object_only",
  "max_output_tokens": 8192,
  "unsupported_params": ["top_k", "frequency_penalty"],
  "param_overrides": {
    "temperature": { "max": 1.0, "min": 0.0 }
  }
}`}</CodeBlock>
      </Section>

      {/* Endpoint URL */}
      <Section title="endpoint_url">
        <p>
          Override the URL that xplan sends requests to. By default xplan constructs the URL from
          the provider&rsquo;s <code>base_url</code> plus the standard path for the API format
          (e.g. <code>/chat/completions</code> for OpenAI-compatible, <code>/messages</code> for
          Anthropic, <code>/model/&#123;model&#125;/converse</code> for Bedrock).
        </p>
        <p className="mt-2">
          Use this when your provider exposes a custom endpoint, e.g. an Azure OpenAI deployment.
        </p>
        <CodeBlock>{`{
  "endpoint_url": "https://my-resource.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-02-01"
}`}</CodeBlock>
        <Note>
          When <code>endpoint_url</code> is set, <code>base_url</code> is ignored for this model.
        </Note>
      </Section>

      {/* Structured Output Degradation */}
      <Section title="structured_output">
        <p>
          Controls how <code>response_format: json_schema</code> is handled when the upstream
          provider does not natively support the <code>json_schema</code> type.
        </p>
        <p className="mt-2">
          Set to <code>&quot;json_object_only&quot;</code> to degrade: xplan will inject the schema
          as a system prompt instruction and downgrade the response_format to{' '}
          <code>json_object</code>.
        </p>
        <CodeBlock>{`{ "structured_output": "json_object_only" }`}</CodeBlock>
        <p className="mt-2 text-sm text-gray-400">
          Supported by: <code>openai_compatible</code> adapter (e.g. DeepSeek, Together AI).
          Anthropic and Bedrock handle structured output via their own mechanisms.
        </p>
      </Section>

      {/* max_output_tokens */}
      <Section title="max_output_tokens">
        <p>
          Cap the maximum number of output tokens for any request sent to this model. If the
          request includes a <code>max_tokens</code> value that exceeds this limit, it will be
          clamped down. If no <code>max_tokens</code> is specified by the caller, this value is
          used as the default.
        </p>
        <CodeBlock>{`{ "max_output_tokens": 8192 }`}</CodeBlock>
        <p className="mt-2 text-sm text-gray-400">
          Applies to all adapters: OpenAI-compatible, Anthropic, and Bedrock.
        </p>
      </Section>

      {/* unsupported_params */}
      <Section title="unsupported_params">
        <p>
          List parameter names that should be stripped from the request before forwarding to the
          upstream. Useful when a client sends parameters that a particular model does not support,
          which would otherwise cause a 400 error.
        </p>
        <CodeBlock>{`{ "unsupported_params": ["top_k", "frequency_penalty", "presence_penalty"] }`}</CodeBlock>
        <p className="mt-2">Recognised logical parameter names per adapter:</p>
        <table className="mt-2 w-full text-sm border-collapse">
          <thead>
            <tr className="border-b border-gray-700">
              <th className="text-left py-1.5 pr-4 text-gray-400 font-medium">Adapter</th>
              <th className="text-left py-1.5 text-gray-400 font-medium">Supported params</th>
            </tr>
          </thead>
          <tbody className="text-gray-300">
            <tr className="border-b border-gray-800">
              <td className="py-1.5 pr-4 font-mono text-xs text-indigo-400">openai_compatible</td>
              <td className="py-1.5 font-mono text-xs">max_tokens, temperature, top_p, top_k, frequency_penalty, presence_penalty</td>
            </tr>
            <tr className="border-b border-gray-800">
              <td className="py-1.5 pr-4 font-mono text-xs text-indigo-400">anthropic</td>
              <td className="py-1.5 font-mono text-xs">max_tokens, temperature, top_p, top_k</td>
            </tr>
            <tr>
              <td className="py-1.5 pr-4 font-mono text-xs text-indigo-400">bedrock</td>
              <td className="py-1.5 font-mono text-xs">max_tokens, temperature, top_p</td>
            </tr>
          </tbody>
        </table>
      </Section>

      {/* param_overrides */}
      <Section title="param_overrides">
        <p>
          Clamp numeric parameters to a min/max range. If the caller sends a value outside the
          allowed range, it is silently clamped to the nearest boundary. Parameters not present in
          the request are not affected.
        </p>
        <CodeBlock>{`{
  "param_overrides": {
    "temperature": { "max": 1.0, "min": 0.0 },
    "top_p": { "max": 0.95 }
  }
}`}</CodeBlock>
        <p className="mt-2 text-sm text-gray-400">
          Both <code>max</code> and <code>min</code> are optional — you can specify either or both.
        </p>
      </Section>

      {/* Example configs */}
      <Section title="Example Configs">
        <div className="space-y-5">
          <div>
            <h4 className="text-sm font-semibold text-gray-300 mb-1">
              DeepSeek — json_object degradation + strip unsupported params
            </h4>
            <CodeBlock>{`{
  "structured_output": "json_object_only",
  "unsupported_params": ["frequency_penalty", "presence_penalty"],
  "param_overrides": {
    "temperature": { "max": 1.5, "min": 0.0 }
  }
}`}</CodeBlock>
          </div>

          <div>
            <h4 className="text-sm font-semibold text-gray-300 mb-1">
              Azure OpenAI — custom deployment endpoint
            </h4>
            <CodeBlock>{`{
  "endpoint_url": "https://my-resource.openai.azure.com/openai/deployments/gpt-4o/chat/completions?api-version=2024-10-21",
  "max_output_tokens": 16384
}`}</CodeBlock>
          </div>

          <div>
            <h4 className="text-sm font-semibold text-gray-300 mb-1">
              Bedrock — limit tokens and clamp temperature
            </h4>
            <CodeBlock>{`{
  "max_output_tokens": 4096,
  "param_overrides": {
    "temperature": { "max": 1.0, "min": 0.0 },
    "top_p": { "max": 0.999 }
  }
}`}</CodeBlock>
          </div>
        </div>
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="space-y-2">
      <h3 className="text-base font-semibold text-white border-b border-gray-800 pb-2">{title}</h3>
      <div className="text-sm text-gray-300 leading-relaxed">{children}</div>
    </section>
  );
}

function CodeBlock({ children }: { children: string }) {
  return (
    <pre className="mt-2 bg-gray-900 border border-gray-800 rounded-lg p-4 text-xs font-mono text-green-300 overflow-x-auto whitespace-pre">
      {children}
    </pre>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return (
    <div className="mt-2 px-3 py-2 bg-indigo-950/50 border border-indigo-900/50 rounded-lg text-xs text-indigo-300">
      {children}
    </div>
  );
}
