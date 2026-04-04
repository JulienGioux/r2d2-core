const { Client } = require("@modelcontextprotocol/sdk/client/index.js");
const { StdioClientTransport } = require("@modelcontextprotocol/sdk/client/stdio.js");

async function run() {
    const transport = new StdioClientTransport({
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-github"]
    });
    const client = new Client({ name: "test", version: "1.0.0" });
    await client.connect(transport);
    const tools = await client.listTools();
    console.log(JSON.stringify(tools, null, 2));
    process.exit(0);
}
run();
