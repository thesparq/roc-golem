// The Durable Streams protocol's reference server, for the e2e run.
//
// `tooling/e2e.sh` installs the Node package `@durable-streams/server` into a
// scratch directory and copies this file next to that installation, so `node`
// resolves the package from `<scratch>/node_modules`. Storage is in memory:
// the e2e creates the streams it reads, so nothing has to outlive the run.
//
// Usage: node durable_streams_server.mjs [port]   (default 4437)
import { DurableStreamTestServer } from "@durable-streams/server"

const port = Number(process.argv[2] ?? 4437)
const server = new DurableStreamTestServer({ port, host: "127.0.0.1" })

await server.start()
console.log(`durable-streams server listening on 127.0.0.1:${port}`)
