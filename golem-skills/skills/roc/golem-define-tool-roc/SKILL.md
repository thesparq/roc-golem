---
name: golem-define-tool-roc
description: "Publishing a callable tool from a Roc Golem agent. Use when the user asks to expose a function as a tool, make an agent's capability callable from the CLI or MCP, or bind a tool to an agent type."
---

# Publishing a Tool

> **Status: implemented.**

A tool is a command other callers reach through Golem's tool surface
(`golem:tool/guest`) — the CLI, MCP, and other components. An app declares the
tools it publishes and answers them from one handler.

## Declare and handle

```roc
import pf.Golem exposing [Agent, build, define, withTools, withToolHandler]

tools = [
	{
		name: "analyze_logs",
		description: "Scans log lines for panics and OOM kills.",
		parameters: [],
		readOnly: True,
	},
]

handleTool = |state, call|
	if call.name == "analyze_logs" {
		{
			state,
			result: {
				id: call.id,
				success: True,
				output: "{\"faults\": ${U64.to_str(countFaults(call.arguments))}}",
				stdout: "",
				stderr: "",
			},
		}
	} else {
		{
			state,
			result: {
				id: call.id,
				success: False,
				output: "{\"error\": \"unknown tool ${call.name}\"}",
				stdout: "",
				stderr: "",
			},
		}
	}

agent : Agent(State)
agent =
	define(cfg, |_id| { .. })
		|> withTools(tools)
		|> withToolHandler(handleTool)
		|> build
```

The handler's `call.name` is the tool's **metadata** name (`analyze_logs`). The
surface publishes it under a CLI-shaped name (`analyze-logs`).

## Declare it in the manifest — twice

```yaml
tools:
  analyze-logs:
    component: roc-golem:my-component

agents:
  my-agent:
    tools:
      analyze-logs: {}
```

Both halves are required. A binding with no matching top-level `tools:` entry
fails the deploy with `Discovered tool has no matching top-level declaration`;
some other combinations are rejected by the key pattern
`^[a-z][a-z0-9]*(-[a-z0-9]+)*$`, which is why the key is kebab-case.

## Two things that surprise people

- **A tool call runs in a fresh instance without the agent's state.** A tool
  whose state decodes strictly from `{}` will report a failure rather than invent
  an answer. For a stateful agent, drive the tool through the agent method
  surface, which reaches the same handler with the state:
  `golem agent invoke 'my-agent("id")' analyze_logs '"payload"'`.
- **stdout/stderr are buffered.** They are written after the handler returns, so
  a tool cannot stream output while it works.
